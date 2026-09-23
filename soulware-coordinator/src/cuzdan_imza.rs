//! Worker cuzdan-sahipligi imza dogrulamasi (ed25519 veya EVM EIP-191).
//! Mesaj bicimi: worker_mesaj.rs.

use crate::worker_mesaj;

/// Imza zaman penceresi (sn).
pub const PENCERE_SN: u64 = 300;

/// Oturum anahtari gecerlilik suresi (sn): 24 saat.
pub const OTURUM_SURE_SN: u64 = 24 * 3600;
/// Bellekteki oturum sayisi bu esigi asinca suresi dolanlar temizlenir.
pub const OTURUM_TEMIZLE_ESIK: usize = 10_000;
/// Bellekteki oturum sayisi ust siniri (suresi dolmamislar dahil) — bellek DoS'una karsi.
pub const OTURUM_UST_SINIR: usize = 200_000;

/// Bir cuzdana bagli gecici ed25519 oturum anahtari (tarayici isci icin).
/// Cuzdan sahibi "oturum:<pubkey hex>" nonce'unu BIR KEZ imzalar; sonraki istekler
/// bu anahtarla ed25519 imzalanir. Yalniz bellekte tutulur (restartta yeniden kayit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Oturum {
    pub pubkey: [u8; 32],
    /// Bu andan (unix sn) itibaren GECERSIZ.
    pub bitis: u64,
    /// Oturumu kuran imzanin ts'i (eski kayit mesajinin yeniden oynatilmasina karsi).
    pub kayit_ts: u64,
}

/// Istekteki imza alanlari.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ImzaAlanlari {
    #[serde(default)]
    pub ts: Option<u64>,
    #[serde(default)]
    pub imza: Option<String>,
    /// Yalniz ed25519 cuzdan: 32 bayt acik anahtar (64 hex).
    #[serde(default)]
    pub pubkey: Option<String>,
}

fn hex_coz(s: &str) -> Option<Vec<u8>> {
    hex::decode(s.trim().trim_start_matches("0x").trim_start_matches("0X")).ok()
}

pub fn cuzdan20(w: &str) -> Option<[u8; 20]> {
    let b = hex_coz(w)?;
    (b.len() == 20).then(|| {
        let mut a = [0u8; 20];
        a.copy_from_slice(&b);
        a
    })
}

/// EIP-191 personal_sign ozeti: keccak256("\x19Ethereum Signed Message:\n" + len + mesaj).
pub fn eip191_ozet(mesaj: &[u8]) -> [u8; 32] {
    use sha3::{Digest, Keccak256};
    let mut h = Keccak256::new();
    h.update(format!("\x19Ethereum Signed Message:\n{}", mesaj.len()).as_bytes());
    h.update(mesaj);
    h.finalize().into()
}

/// EVM imzasindan (65 bayt r|s|v) adres kurtar.
pub fn evm_kurtar(mesaj: &[u8], imza: &[u8]) -> Option<[u8; 20]> {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
    use sha3::{Digest, Keccak256};
    if imza.len() != 65 {
        return None;
    }
    let v = match imza[64] {
        27 | 28 => imza[64] - 27,
        0 | 1 => imza[64],
        _ => return None,
    };
    let sig = Signature::from_slice(&imza[..64]).ok()?;
    let recid = RecoveryId::from_byte(v)?;
    let vk = VerifyingKey::recover_from_prehash(&eip191_ozet(mesaj), &sig, recid).ok()?;
    let nokta = vk.to_encoded_point(false);
    let h = Keccak256::digest(&nokta.as_bytes()[1..]);
    let mut a = [0u8; 20];
    a.copy_from_slice(&h[12..]);
    Some(a)
}

/// Cuzdan sahipligini dogrula (yalniz cuzdanin KENDI anahtari; oturum anahtari kabul EDILMEZ).
/// Hata metni istemciye doner (gizli bilgi icermez).
pub fn dogrula(wallet: &str, nonce: &str, alan: &ImzaAlanlari, simdi: u64) -> Result<(), String> {
    dogrula_oturumlu(wallet, nonce, alan, simdi, None)
}

/// `dogrula` + ed25519 dalinda bu cuzdanin gecerli oturum anahtari da kabul edilir.
/// `oturum` CAGIRAN tarafindan `wallet` icin aranmis olmalidir (baska cuzdanin oturumu verilmez).
pub fn dogrula_oturumlu(
    wallet: &str, nonce: &str, alan: &ImzaAlanlari, simdi: u64, oturum: Option<&Oturum>,
) -> Result<(), String> {
    let adres = cuzdan20(wallet).ok_or("wallet 0x + 40 hex olmali")?;
    let ts = alan.ts.ok_or("imza gerekli: ts, imza (ve ed25519 icin pubkey) alanlari eksik")?;
    let imza_hex = alan.imza.as_deref().filter(|s| !s.trim().is_empty())
        .ok_or("imza gerekli: imza alani eksik")?;
    if simdi.abs_diff(ts) > PENCERE_SN {
        return Err(format!("imza zamani (ts) gecersiz: +-{PENCERE_SN} sn pencere disinda"));
    }
    let imza = hex_coz(imza_hex).ok_or("imza gecersiz hex")?;
    let mesaj = worker_mesaj::mesaj(wallet, nonce, ts);
    match alan.pubkey.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(pk_hex) => {
            // ed25519 cuzdan
            let pk = hex_coz(pk_hex).filter(|b| b.len() == 32).ok_or("pubkey 32 bayt (64 hex) olmali")?;
            let mut pk32 = [0u8; 32];
            pk32.copy_from_slice(&pk);
            let sahip = lsc_engine::public_key_to_adres(&pk32) == adres;
            if !sahip {
                match oturum {
                    Some(o) if o.pubkey == pk32 && simdi < o.bitis => {}
                    Some(o) if o.pubkey == pk32 => {
                        return Err("oturum suresi doldu: cuzdanla yeniden oturum imzala (/worker/register)".into());
                    }
                    _ => return Err("pubkey bu cuzdana ait degil".into()),
                }
            }
            let vk = ed25519_dalek::VerifyingKey::from_bytes(&pk32).map_err(|_| "pubkey gecersiz")?;
            let sig: [u8; 64] = imza.as_slice().try_into().map_err(|_| "ed25519 imza 64 bayt olmali")?;
            vk.verify_strict(mesaj.as_bytes(), &ed25519_dalek::Signature::from_bytes(&sig))
                .map_err(|_| "imza dogrulanamadi".to_string())
        }
        None => {
            // EVM cuzdan (EIP-191 personal_sign)
            match evm_kurtar(mesaj.as_bytes(), &imza) {
                Some(a) if a == adres => Ok(()),
                Some(_) => Err("imza bu cuzdana ait degil".into()),
                None => Err("EVM imzasi gecersiz (65 bayt r|s|v bekleniyor)".into()),
            }
        }
    }
}

/// Oturum kaydi: `wallet` sahibi (EVM EIP-191 ya da ed25519 cuzdan anahtari — oturum
/// anahtari DEGIL) "oturum:<oturum_pubkey hex>" nonce'unu imzalamis olmali.
/// Basarida oturumu `oturumlar`a yazar (eskisini gecersiz kilar) ve dondurur.
pub fn oturum_kaydet(
    oturumlar: &mut std::collections::HashMap<String, Oturum>,
    wallet: &str, oturum_pubkey_hex: &str, alan: &ImzaAlanlari, simdi: u64,
) -> Result<Oturum, String> {
    let w = wallet.trim().to_lowercase();
    let pk = hex_coz(oturum_pubkey_hex).filter(|b| b.len() == 32)
        .ok_or("oturum_pubkey 32 bayt (64 hex) olmali")?;
    let mut pk32 = [0u8; 32];
    pk32.copy_from_slice(&pk);
    ed25519_dalek::VerifyingKey::from_bytes(&pk32).map_err(|_| "oturum_pubkey gecersiz")?;
    // Oturumu yalniz cuzdanin KENDISI kurabilir (oturum anahtari kendini yenileyemez).
    dogrula(&w, &worker_mesaj::oturum_nonce(&pk32), alan, simdi)?;
    let ts = alan.ts.unwrap_or(0);
    if let Some(eski) = oturumlar.get(&w) {
        if ts < eski.kayit_ts {
            return Err("daha yeni bir oturum zaten kayitli (eski oturum imzasi yeniden kullanilamaz)".into());
        }
    }
    if oturumlar.len() >= OTURUM_TEMIZLE_ESIK && !oturumlar.contains_key(&w) {
        oturumlar.retain(|_, o| simdi < o.bitis);
        if oturumlar.len() >= OTURUM_UST_SINIR {
            return Err("oturum kapasitesi dolu, sonra tekrar dene".into());
        }
    }
    let o = Oturum { pubkey: pk32, bitis: simdi + OTURUM_SURE_SN, kayit_ts: ts };
    oturumlar.insert(w, o);
    Ok(o)
}

#[cfg(test)]
pub mod test_yardim {
    use super::*;
    use ed25519_dalek::Signer;

    pub fn ed_cuzdan(seed: u8) -> (ed25519_dalek::SigningKey, String) {
        let sk = ed25519_dalek::SigningKey::from_bytes(&[seed; 32]);
        let w = format!("0x{}", hex::encode(lsc_engine::public_key_to_adres(&sk.verifying_key().to_bytes())));
        (sk, w)
    }
    pub fn ed_imzala(sk: &ed25519_dalek::SigningKey, wallet: &str, nonce: &str, ts: u64) -> ImzaAlanlari {
        let m = worker_mesaj::mesaj(wallet, nonce, ts);
        ImzaAlanlari {
            ts: Some(ts),
            imza: Some(hex::encode(sk.sign(m.as_bytes()).to_bytes())),
            pubkey: Some(hex::encode(sk.verifying_key().to_bytes())),
        }
    }
    pub fn evm_cuzdan(seed: u8) -> (k256::ecdsa::SigningKey, String) {
        use sha3::{Digest, Keccak256};
        let sk = k256::ecdsa::SigningKey::from_slice(&[seed; 32]).unwrap();
        let p = sk.verifying_key().to_encoded_point(false);
        let h = Keccak256::digest(&p.as_bytes()[1..]);
        (sk, format!("0x{}", hex::encode(&h[12..])))
    }
    pub fn evm_imzala(sk: &k256::ecdsa::SigningKey, wallet: &str, nonce: &str, ts: u64) -> ImzaAlanlari {
        let m = worker_mesaj::mesaj(wallet, nonce, ts);
        let (sig, rec) = sk.sign_prehash_recoverable(&eip191_ozet(m.as_bytes())).unwrap();
        let mut b = sig.to_bytes().to_vec();
        b.push(27 + rec.to_byte());
        ImzaAlanlari { ts: Some(ts), imza: Some(hex::encode(b)), pubkey: None }
    }
    /// Oturum anahtariyla (pubkey = oturum anahtari) `wallet` adina imza.
    pub fn oturum_imzala(osk: &ed25519_dalek::SigningKey, wallet: &str, nonce: &str, ts: u64) -> ImzaAlanlari {
        let mut a = ed_imzala(osk, wallet, nonce, ts);
        a.pubkey = Some(hex::encode(osk.verifying_key().to_bytes()));
        a
    }
    pub fn pk_hex(osk: &ed25519_dalek::SigningKey) -> String {
        hex::encode(osk.verifying_key().to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::test_yardim::*;
    use super::*;

    const T: u64 = 1_758_650_000;

    #[test]
    fn ed25519_imza_kabul_ve_red() {
        let (sk, w) = ed_cuzdan(1);
        let a = ed_imzala(&sk, &w, "kayit", T);
        assert_eq!(dogrula(&w, "kayit", &a, T + 10), Ok(()));
        assert_eq!(dogrula(&w.to_uppercase().replace("0X", "0x"), "kayit", &a, T), Ok(()), "buyuk harf cuzdan");
        // imzasiz
        assert!(dogrula(&w, "kayit", &ImzaAlanlari::default(), T).is_err());
        // baska nonce (ornegin kayit imzasi submit icin kullanilamaz)
        assert!(dogrula(&w, "is:1:ab", &a, T).is_err());
        // pencere disi (replay)
        assert!(dogrula(&w, "kayit", &a, T + PENCERE_SN + 1).is_err());
        assert!(dogrula(&w, "kayit", &a, T - PENCERE_SN - 1).is_err());
        // baska cuzdan adina (pubkey baskasinin)
        let (_, w2) = ed_cuzdan(2);
        assert_eq!(dogrula(&w2, "kayit", &a, T), Err("pubkey bu cuzdana ait degil".into()));
        // baskasinin anahtariyla imzalanmis ama pubkey'i kurbaninki
        let (sk3, _) = ed_cuzdan(3);
        let mut sahte = ed_imzala(&sk3, &w, "kayit", T);
        sahte.pubkey = a.pubkey.clone();
        assert_eq!(dogrula(&w, "kayit", &sahte, T), Err("imza dogrulanamadi".into()));
    }

    #[test]
    fn evm_personal_sign_kabul_ve_red() {
        let (sk, w) = evm_cuzdan(7);
        let a = evm_imzala(&sk, &w, "poll", T);
        assert_eq!(dogrula(&w, "poll", &a, T), Ok(()));
        // v = 0/1 bicimi de kabul
        let mut a01 = a.clone();
        let mut b = hex::decode(a01.imza.as_deref().unwrap()).unwrap();
        b[64] -= 27;
        a01.imza = Some(format!("0x{}", hex::encode(&b)));
        assert_eq!(dogrula(&w, "poll", &a01, T), Ok(()));
        // yanlis nonce / baska cuzdan / bozuk imza
        assert!(dogrula(&w, "kayit", &a, T).is_err());
        let (_, w2) = evm_cuzdan(8);
        assert_eq!(dogrula(&w2, "poll", &a, T), Err("imza bu cuzdana ait degil".into()));
        let mut bozuk = a.clone();
        bozuk.imza = Some("00".repeat(65));
        assert!(dogrula(&w, "poll", &bozuk, T).is_err());
        let mut kisa = a;
        kisa.imza = Some("11".repeat(64));
        assert!(dogrula(&w, "poll", &kisa, T).is_err());
    }

    // ── OTURUM ANAHTARI: saldiri (baskasinin cuzdani adina is/odul toplama) hala engelli ──
    use std::collections::HashMap;

    fn oturum_kur(evm_seed: u8, oturum_seed: u8) -> (k256::ecdsa::SigningKey, String, ed25519_dalek::SigningKey, HashMap<String, Oturum>) {
        let (esk, w) = evm_cuzdan(evm_seed);
        let osk = ed25519_dalek::SigningKey::from_bytes(&[oturum_seed; 32]);
        let mut m = HashMap::new();
        let a = evm_imzala(&esk, &w, &worker_mesaj::oturum_nonce(&osk.verifying_key().to_bytes()), T);
        let o = oturum_kaydet(&mut m, &w, &pk_hex(&osk), &a, T).expect("oturum kaydi");
        assert_eq!(o.bitis, T + OTURUM_SURE_SN);
        (esk, w, osk, m)
    }

    #[test]
    fn oturum_eip191_kayit_ve_kullanim() {
        let (_, w, osk, m) = oturum_kur(7, 40);
        let o = m.get(&w).copied();
        // poll / submit / benchmark oturum anahtariyla gecer (buyuk harf cuzdan da)
        for nonce in ["poll", &worker_mesaj::is_nonce(5, "cevap"), &worker_mesaj::benchmark_nonce(&[(1, 2, "4")])] {
            let a = oturum_imzala(&osk, &w, nonce, T + 60);
            assert_eq!(dogrula_oturumlu(&w, nonce, &a, T + 60, o.as_ref()), Ok(()), "{nonce}");
        }
        // oturum anahtari DUZ dogrula()'da (oturumsuz) kabul EDILMEZ
        let a = oturum_imzala(&osk, &w, "poll", T);
        assert_eq!(dogrula(&w, "poll", &a, T), Err("pubkey bu cuzdana ait degil".into()));
        // submit imzasi belirli cevaba bagli: baska cevap icin kullanilamaz
        let a = oturum_imzala(&osk, &w, &worker_mesaj::is_nonce(5, "cevap"), T);
        assert!(dogrula_oturumlu(&w, &worker_mesaj::is_nonce(5, "baska"), &a, T, o.as_ref()).is_err());
        // pencere disi ts yine reddedilir
        let a = oturum_imzala(&osk, &w, "poll", T);
        assert!(dogrula_oturumlu(&w, "poll", &a, T + PENCERE_SN + 1, o.as_ref()).is_err());
    }

    #[test]
    fn oturum_baska_cuzdan_icin_kullanilamaz() {
        let (_, w, osk, m) = oturum_kur(7, 40);
        let (_, kurban) = evm_cuzdan(8);
        // kurbanin oturumu yok: saldirgan kendi oturum anahtariyla kurban adina imzalar
        let a = oturum_imzala(&osk, &kurban, "poll", T);
        assert_eq!(dogrula_oturumlu(&kurban, "poll", &a, T, m.get(&kurban)), Err("pubkey bu cuzdana ait degil".into()));
        // cagiran yanlislikla BASKA cuzdanin oturumunu verse bile: pubkey eslesir ama... bu
        // yalniz cagiran hatasinda olur; koordinator oturumu HER ZAMAN wallet ile arar (main.rs).
        // Kurbanin KENDI oturumu varken saldirganin oturum anahtari yine reddedilir:
        let (_, kurban2, _osk2, m2) = oturum_kur(9, 41);
        let a = oturum_imzala(&osk, &kurban2, "poll", T);
        assert_eq!(dogrula_oturumlu(&kurban2, "poll", &a, T, m2.get(&kurban2)), Err("pubkey bu cuzdana ait degil".into()));
        // saldirganin kendi cuzdani icin gecerli (kontrol)
        let a = oturum_imzala(&osk, &w, "poll", T);
        assert_eq!(dogrula_oturumlu(&w, "poll", &a, T, m.get(&w)), Ok(()));
    }

    #[test]
    fn oturum_suresi_dolunca_red() {
        let (_, w, osk, m) = oturum_kur(7, 40);
        let o = m.get(&w).copied();
        let son = T + OTURUM_SURE_SN - 1;
        assert_eq!(dogrula_oturumlu(&w, "poll", &oturum_imzala(&osk, &w, "poll", son), son, o.as_ref()), Ok(()));
        let bitti = T + OTURUM_SURE_SN;
        let r = dogrula_oturumlu(&w, "poll", &oturum_imzala(&osk, &w, "poll", bitti), bitti, o.as_ref());
        assert!(r.as_ref().unwrap_err().contains("oturum suresi doldu"), "{r:?}");
    }

    #[test]
    fn oturum_yanlis_cuzdan_imzasiyla_kurulamaz() {
        let (_, kurban) = evm_cuzdan(8);
        let (saldirgan_sk, _) = evm_cuzdan(9);
        let osk = ed25519_dalek::SigningKey::from_bytes(&[42; 32]);
        let nonce = worker_mesaj::oturum_nonce(&osk.verifying_key().to_bytes());
        let mut m = HashMap::new();
        // saldirgan kendi EVM anahtariyla kurban cuzdani icin oturum imzalar
        let a = evm_imzala(&saldirgan_sk, &kurban, &nonce, T);
        assert_eq!(oturum_kaydet(&mut m, &kurban, &pk_hex(&osk), &a, T), Err("imza bu cuzdana ait degil".into()));
        // saldirgan ed25519 cuzdaniyla (pubkey kendi) kurban icin
        let (esk, _) = ed_cuzdan(5);
        let a = ed_imzala(&esk, &kurban, &nonce, T);
        assert_eq!(oturum_kaydet(&mut m, &kurban, &pk_hex(&osk), &a, T), Err("pubkey bu cuzdana ait degil".into()));
        // imzasiz
        assert!(oturum_kaydet(&mut m, &kurban, &pk_hex(&osk), &ImzaAlanlari::default(), T).is_err());
        // kurbanin "kayit"/"poll" imzasi oturum kurmaz (nonce baglama)
        let (ksk, kw) = evm_cuzdan(10);
        for n in ["kayit", "poll"] {
            let a = evm_imzala(&ksk, &kw, n, T);
            assert!(oturum_kaydet(&mut m, &kw, &pk_hex(&osk), &a, T).is_err(), "{n}");
        }
        // kurbanin BASKA bir oturum anahtari icin imzasi, saldirganin anahtarina tasinamaz
        let kurban_osk = ed25519_dalek::SigningKey::from_bytes(&[43; 32]);
        let a = evm_imzala(&ksk, &kw, &worker_mesaj::oturum_nonce(&kurban_osk.verifying_key().to_bytes()), T);
        assert!(oturum_kaydet(&mut m, &kw, &pk_hex(&osk), &a, T).is_err());
        // bozuk oturum_pubkey
        assert!(oturum_kaydet(&mut m, &kw, "abcd", &a, T).is_err());
        assert!(m.is_empty(), "hicbir oturum kurulmamali");
    }

    #[test]
    fn oturum_anahtari_kendini_yenileyemez_ve_yenisi_eskiyi_iptal_eder() {
        let (esk, w, osk, mut m) = oturum_kur(7, 40);
        // oturum anahtari (cuzdan anahtari degil) yeni oturum kuramaz / suresini uzatamaz
        let yeni = ed25519_dalek::SigningKey::from_bytes(&[50; 32]);
        let a = oturum_imzala(&osk, &w, &worker_mesaj::oturum_nonce(&yeni.verifying_key().to_bytes()), T + 10);
        assert!(oturum_kaydet(&mut m, &w, &pk_hex(&yeni), &a, T + 10).is_err());
        let a = oturum_imzala(&osk, &w, &worker_mesaj::oturum_nonce(&osk.verifying_key().to_bytes()), T + 10);
        assert!(oturum_kaydet(&mut m, &w, &pk_hex(&osk), &a, T + 10).is_err());
        assert_eq!(m[&w].pubkey, osk.verifying_key().to_bytes());
        // cuzdan sahibi yeni oturum kurar -> eski anahtar gecersiz
        let a = evm_imzala(&esk, &w, &worker_mesaj::oturum_nonce(&yeni.verifying_key().to_bytes()), T + 20);
        oturum_kaydet(&mut m, &w, &pk_hex(&yeni), &a, T + 20).unwrap();
        let o = m.get(&w);
        assert_eq!(dogrula_oturumlu(&w, "poll", &oturum_imzala(&osk, &w, "poll", T + 30), T + 30, o), Err("pubkey bu cuzdana ait degil".into()));
        assert_eq!(dogrula_oturumlu(&w, "poll", &oturum_imzala(&yeni, &w, "poll", T + 30), T + 30, o), Ok(()));
        // ESKI oturum kayit mesajinin (pencere icinde) yeniden oynatilmasi eski anahtari geri getiremez
        let eski = evm_imzala(&esk, &w, &worker_mesaj::oturum_nonce(&osk.verifying_key().to_bytes()), T);
        assert!(oturum_kaydet(&mut m, &w, &pk_hex(&osk), &eski, T + 30).is_err());
        assert_eq!(m[&w].pubkey, yeni.verifying_key().to_bytes());
    }

    #[test]
    fn oturum_ed25519_yerel_cuzdan_ve_oturum_adresi() {
        // yerel ed25519 cuzdan eskisi gibi (oturum gerekmeden) calisir
        let (sk, w) = ed_cuzdan(1);
        assert_eq!(dogrula_oturumlu(&w, "poll", &ed_imzala(&sk, &w, "poll", T), T, None), Ok(()));
        // tarayicinin "yerel cuzdan" modu: adres = blake3(oturum_pk)[..20] -> dogrudan gecer
        let osk = ed25519_dalek::SigningKey::from_bytes(&[40; 32]);
        let yerel = format!("0x{}", hex::encode(lsc_engine::public_key_to_adres(&osk.verifying_key().to_bytes())));
        assert_eq!(dogrula_oturumlu(&yerel, "poll", &oturum_imzala(&osk, &yerel, "poll", T), T, None), Ok(()));
        // ed25519 cuzdan da kendi anahtariyla oturum kurabilir
        let mut m = HashMap::new();
        let a = ed_imzala(&sk, &w, &worker_mesaj::oturum_nonce(&osk.verifying_key().to_bytes()), T);
        oturum_kaydet(&mut m, &w, &pk_hex(&osk), &a, T).unwrap();
        assert_eq!(dogrula_oturumlu(&w, "poll", &oturum_imzala(&osk, &w, "poll", T), T, m.get(&w)), Ok(()));
        // imzasiz her durumda red
        assert!(dogrula_oturumlu(&w, "poll", &ImzaAlanlari::default(), T, m.get(&w)).is_err());
        let mut sadece_pk = ImzaAlanlari::default();
        sadece_pk.ts = Some(T);
        sadece_pk.pubkey = Some(pk_hex(&osk));
        assert!(dogrula_oturumlu(&w, "poll", &sadece_pk, T, m.get(&w)).is_err());
    }

    #[test]
    fn oturum_mesaj_bicimi_tarayici_ile_ayni() {
        // katil.html: "AIDAG-WORKER|<wallet>|oturum:<pk hex>|<ts>"
        let pk = [0xABu8; 32];
        assert_eq!(worker_mesaj::mesaj("0xAA00000000000000000000000000000000000001", &worker_mesaj::oturum_nonce(&pk), 9),
            format!("AIDAG-WORKER|0xaa00000000000000000000000000000000000001|oturum:{}|9", "ab".repeat(32)));
    }

    #[test]
    fn eip191_bilinen_vektor() {
        // eth_account / ethers ile ayni: hashMessage("hello")
        assert_eq!(hex::encode(eip191_ozet(b"hello")),
            "50b2c43fd39106bafbba0da34fc430e1f91e3c96ea2acee2bc34119f92b37750");
    }
}
