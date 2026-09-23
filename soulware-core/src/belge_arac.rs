//! KUBRA araci: BELGE KAYIT TALEBI HAZIRLAMA (arac = "belge-kayit-hazirla").
//!
//! KUBRA IMZALAMAZ ve zincire GONDERMEZ. Yalniz:
//!   - belge hash'ini (tarayicida hesaplanmis, 64 hex) alir — dosya sunucuya gelmez,
//!   - zincir durumunu okur (/belge, /kurum, /tips),
//!   - imzasiz bir kayit talebi (JSON) dondurur.
//!
//! Kayit, kurum personelinin KENDI anahtariyla imzalamasiyla zincire gider
//! (tarayici, cevrimdisi `belge-imzala` veya ileride e-Imza).
//!
//! DURUSTLUK: main'deki kurum kaydi (tip=5) BEYANDIR — kimlik dogrulanmamistir.
//! Talep bunu acikca yazar; "yetkili" iddiasi yalniz zincir dogrularsa yapilir.

use lsc_engine::belge_talep::KayitTalebi;
use lsc_engine::public_key_to_adres;
use serde_json::{json, Value};

pub const ARAC_AD: &str = "belge-kayit-hazirla";
pub const TALEP_TUR: &str = "aidag-belge-kayit-talebi";
pub const TALEP_SURUM: u32 = 1;
pub const KURUM_UYARI: &str = "Kurum kaydı zincirde beyana dayanır; kurum kimliği henüz bağımsız olarak doğrulanmamıştır.";

/// Zincirdeki belge durumu (/belge/:hash).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BelgeDurumu {
    pub kayitli: bool,
    pub kaydeden: Option<String>,
    pub zaman: Option<u64>,
}

/// Imzalayan adresin kurum durumu (/kurum/:adres).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KurumDurumu {
    pub kayitli: bool,
    pub ad: Option<String>,
    pub kategori: Option<String>,
    /// Zincir "dogrulanmis" alani donduruyorsa (M-of-N kurum dogrulamasi). Yoksa None.
    pub dogrulanmis: Option<bool>,
}

pub fn hex32(s: &str) -> Result<[u8; 32], String> {
    let s = s.trim().trim_start_matches("0x");
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("64 haneli hex bekleniyor (32 bayt)".into());
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&hex::decode(s).map_err(|_| "gecersiz hex".to_string())?);
    Ok(a)
}

pub fn belge_durumu_coz(v: &Value) -> BelgeDurumu {
    BelgeDurumu {
        kayitli: v.get("kayitli").and_then(|x| x.as_bool()).unwrap_or(false),
        kaydeden: v.get("kaydeden").and_then(|x| x.as_str()).map(|a| format!("0x{}", a.trim_start_matches("0x").to_lowercase())),
        zaman: v.get("zaman").and_then(|x| x.as_u64()),
    }
}

pub fn kurum_durumu_coz(v: &Value) -> KurumDurumu {
    KurumDurumu {
        kayitli: v.get("kayitli").and_then(|x| x.as_bool()).unwrap_or(false),
        ad: v.get("ad").and_then(|x| x.as_str()).map(String::from),
        kategori: v.get("kategori").and_then(|x| x.as_str()).map(String::from),
        dogrulanmis: v.get("dogrulanmis").and_then(|x| x.as_bool()),
    }
}

/// Imzasiz talebi kur (SAF: ag yok, anahtar yok). `imzalayan` = personelin
/// acik anahtari + kurum durumu (verilmediyse talep "imzalayan-bekleniyor").
pub fn talep_kur(
    net_id: u32,
    belge_hash: [u8; 32],
    tips: Vec<[u8; 32]>,
    ts: u64,
    zincir: &BelgeDurumu,
    imzalayan: Option<([u8; 32], &KurumDurumu)>,
) -> Result<Value, String> {
    let t = KayitTalebi::yeni(net_id, belge_hash, tips, ts).map_err(|e| e.to_string())?;
    let durum = if zincir.kayitli {
        "zaten-kayitli"
    } else {
        match &imzalayan {
            None => "imzalayan-bekleniyor",
            Some((_, k)) if !k.kayitli => "imzalayan-kurum-degil",
            Some(_) => "imza-bekliyor",
        }
    };
    let (imz, imzalanacak) = match imzalayan {
        Some((pk, k)) => {
            let adres = format!("0x{}", hex::encode(public_key_to_adres(&pk)));
            let id = (durum == "imza-bekliyor").then(|| hex::encode(t.imzalanacak_id(&pk)));
            (json!({
                "pubkey": hex::encode(pk),
                "adres": adres,
                "kurum": { "kayitli": k.kayitli, "ad": k.ad, "kategori": k.kategori, "dogrulanmis": k.dogrulanmis },
            }), id)
        }
        None => (Value::Null, None),
    };
    let sonraki = match durum {
        "zaten-kayitli" => "Bu belge zaten zincirde kayıtlı; yeni kayıt gerekmez. Doğrulama/çıktı için /belge/<hash> sayfasını kullanın.",
        "imzalayan-bekleniyor" => "Kurum personeli kendi anahtarını seçip talebi imzalamalı (tarayıcı veya çevrimdışı belge-imzala aracı).",
        "imzalayan-kurum-degil" => "Bu anahtarın adresi zincirde kurum olarak kayıtlı değil; kayıt yapılamaz.",
        _ => "Kurum personeli talebi kendi anahtarıyla imzalayıp zincire göndermeli. KUBRA imzalamaz.",
    };
    Ok(json!({
        "tur": TALEP_TUR,
        "surum": TALEP_SURUM,
        "arac": ARAC_AD,
        "network_id": net_id,
        "belge_hash": hex::encode(belge_hash),
        "payload_hex": hex::encode(t.payload()),
        "parents": t.parents.iter().map(hex::encode).collect::<Vec<_>>(),
        "ts": ts,
        "imzalayan": imz,
        // Bilgi amacli (bu ts ile). Imzalayan id'yi KENDISI hesaplamali; ts'yi
        // imza anina guncellerse id degisir.
        "imzalanacak_id": imzalanacak,
        "zincir": { "kayitli": zincir.kayitli, "kaydeden": zincir.kaydeden, "zaman": zincir.zaman },
        "durum": durum,
        "kubra_imzalamaz": true,
        "kurum_uyari": KURUM_UYARI,
        "sonraki_adim": sonraki,
    }))
}

/// Sohbet cevabi: talep ozeti + KANIT IZI (hangi arac, hangi hash, zincir durumu).
pub fn sohbet_metni(talep: &Value) -> String {
    let hash = talep["belge_hash"].as_str().unwrap_or("");
    let zincir = if talep["zincir"]["kayitli"].as_bool() == Some(true) {
        format!("kayıtlı (kaydeden {})", talep["zincir"]["kaydeden"].as_str().unwrap_or("?"))
    } else {
        "kayıtlı değil".to_string()
    };
    let durum = talep["durum"].as_str().unwrap_or("");
    let govde = if durum == "zaten-kayitli" {
        "Bu belge zaten AIDAG-Chain'de kayıtlı; yeni kayıt gerekmez. Doğrulamak veya QR'lı çıktı almak için:".to_string()
    } else {
        "Kayıt talebini hazırladım. Ben İMZALAMADIM ve zincire göndermedim: kayıt, yetkili kurum personelinin \
kendi anahtarıyla onaylamasıyla zincire gider. Belge dosyası bana gelmedi; yalnız özeti (hash) kullanıldı. Onay için:".to_string()
    };
    format!(
        "{govde} https://aidag-chain.com/belge/{hash}\n\n\
Kanıt izi — araç: {ARAC_AD} · hash: {hash} · zincir durumu: {zincir} · talep durumu: {durum}\n\
Not: {KURUM_UYARI}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    const H: &str = "0dcce43d9a705bcd6f3b3a8a1b2c3d4e5f60718293a4b5c6d7e8f90112233445";
    fn kayitsiz() -> BelgeDurumu { BelgeDurumu { kayitli: false, kaydeden: None, zaman: None } }
    fn kurum(kayitli: bool) -> KurumDurumu {
        KurumDurumu { kayitli, ad: kayitli.then(|| "Ornek Tapu Mudurlugu".into()), kategori: kayitli.then(|| "devlet".into()), dogrulanmis: None }
    }

    #[test]
    fn hash_girdisi_siki() {
        assert!(hex32(H).is_ok());
        assert!(hex32(&format!("0x{H}")).is_ok());
        assert!(hex32(&H[..63]).is_err());
        assert!(hex32(&format!("{H}00")).is_err());
        assert!(hex32(&H.replace('0', "g")).is_err());
        assert!(hex32("").is_err());
    }

    #[test]
    fn talep_imzasiz_ve_kanonik() {
        let t = talep_kur(3474, hex32(H).unwrap(), vec![[5u8; 32], [1u8; 32]], 100, &kayitsiz(), None).unwrap();
        assert_eq!(t["arac"], ARAC_AD);
        assert_eq!(t["kubra_imzalamaz"], true);
        assert_eq!(t["payload_hex"], format!("01{H}"));
        assert_eq!(t["parents"][0], hex::encode([1u8; 32]));
        assert_eq!(t["durum"], "imzalayan-bekleniyor");
        assert!(t["imzalanacak_id"].is_null());
        // talepte hicbir imza/gizli anahtar alani yok
        let s = t.to_string();
        for yasak in ["signature", "imza\"", "secret", "seed", "gizli"] {
            assert!(!s.contains(yasak), "{yasak}");
        }
    }

    #[test]
    fn durumlar() {
        let pk = SigningKey::from_bytes(&[3; 32]).verifying_key().to_bytes();
        let h = hex32(H).unwrap();
        let t = talep_kur(3474, h, vec![], 1, &kayitsiz(), Some((pk, &kurum(false)))).unwrap();
        assert_eq!(t["durum"], "imzalayan-kurum-degil");
        assert!(t["imzalanacak_id"].is_null());
        let t = talep_kur(3474, h, vec![], 1, &kayitsiz(), Some((pk, &kurum(true)))).unwrap();
        assert_eq!(t["durum"], "imza-bekliyor");
        assert_eq!(t["imzalayan"]["adres"], format!("0x{}", hex::encode(public_key_to_adres(&pk))));
        let kayitli = BelgeDurumu { kayitli: true, kaydeden: Some("0xab".into()), zaman: Some(9) };
        let t = talep_kur(3474, h, vec![], 1, &kayitli, Some((pk, &kurum(true)))).unwrap();
        assert_eq!(t["durum"], "zaten-kayitli");
        assert!(t["imzalanacak_id"].is_null());
    }

    #[test]
    fn imzalanacak_id_personel_imzasiyla_gecerli_vertex_olur() {
        let k = SigningKey::from_bytes(&[3; 32]);
        let pk = k.verifying_key().to_bytes();
        let h = hex32(H).unwrap();
        let t = talep_kur(3474, h, vec![[1u8; 32]], 77, &kayitsiz(), Some((pk, &kurum(true)))).unwrap();
        let id = hex32(t["imzalanacak_id"].as_str().unwrap()).unwrap();
        let sig = k.sign(&id).to_bytes();
        let kt = KayitTalebi::yeni(3474, h, vec![[1u8; 32]], 77).unwrap();
        let v = kt.imzayla_birlestir(pk, sig).unwrap();
        assert_eq!(v.public_key(), &pk);
    }

    #[test]
    fn sohbet_kanit_izi() {
        let t = talep_kur(3474, hex32(H).unwrap(), vec![], 1, &kayitsiz(), None).unwrap();
        let m = sohbet_metni(&t);
        assert!(m.contains("araç: belge-kayit-hazirla"));
        assert!(m.contains(&format!("hash: {H}")));
        assert!(m.contains("zincir durumu: kayıtlı değil"));
        assert!(m.contains("İMZALAMADIM"));
        assert!(m.contains(KURUM_UYARI));
    }
}
