//! Worker cuzdan-sahipligi imza dogrulamasi (ed25519 veya EVM EIP-191).
//! Mesaj bicimi: worker_mesaj.rs.

use crate::worker_mesaj;

/// Imza zaman penceresi (sn).
pub const PENCERE_SN: u64 = 300;

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

/// Cuzdan sahipligini dogrula. Hata metni istemciye doner (gizli bilgi icermez).
pub fn dogrula(wallet: &str, nonce: &str, alan: &ImzaAlanlari, simdi: u64) -> Result<(), String> {
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
            if lsc_engine::public_key_to_adres(&pk32) != adres {
                return Err("pubkey bu cuzdana ait degil".into());
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

    #[test]
    fn eip191_bilinen_vektor() {
        // eth_account / ethers ile ayni: hashMessage("hello")
        assert_eq!(hex::encode(eip191_ozet(b"hello")),
            "50b2c43fd39106bafbba0da34fc430e1f91e3c96ea2acee2bc34119f92b37750");
    }
}
