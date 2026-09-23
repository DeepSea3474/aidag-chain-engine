//! KUBRA zincir imza anahtari — FAIL-CLOSED yukleme.
//!
//! Eski davranis (kaldirildi): dosya yoksa ya da bicimi gecersizse SESSIZCE yeni
//! anahtar uretip dosyanin UZERINE yaziyordu -> KUBRA'nin zincir adresi habersizce
//! degisir, RWA yasak listesindeki (mainnet::KUBRA_IMZA_ADRESI) adresle ayrisirdi.
//!
//! Yeni davranis:
//!  - `yukle`: dosya yoksa / okunamiyorsa / bicimi gecersizse HATA; servis ACILMAZ.
//!  - `uret`: YALNIZ acik parametreyle (`--yeni-anahtar-uret`) cagrilir; dosya zaten
//!    varsa (bozuk olsa bile) REDDEDER — mevcut dosyanin uzerine ASLA yazmaz.
//!    Dosya atomik olarak (O_EXCL) ve 0600 izinle olusturulur.
//!
//! Hata mesajlari ve cikti YALNIZ yol / acik adres icerir; ozel anahtar asla basilmaz.
//!
//! Dosya bicimi: [surum=1][ed25519 seed: 32 bayt] = 33 bayt.

use ed25519_dalek::SigningKey;

const SURUM: u8 = 1;
const DOSYA_LEN: usize = 33;

/// Anahtar hatasi (Display yalniz yol + sebep; icerik yok).
#[derive(Debug, PartialEq, Eq)]
pub enum AnahtarHatasi {
    /// Dosya yok.
    Yok(String),
    /// Dosya var ama bicimi gecersiz (uzunluk / surum bayti).
    Bozuk { yol: String, sebep: &'static str },
    /// `uret`: dosya zaten var; uzerine yazilmaz.
    ZatenVar(String),
    /// Diger G/C hatasi.
    Io { yol: String, hata: String },
}

impl std::fmt::Display for AnahtarHatasi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnahtarHatasi::Yok(y) => write!(
                f,
                "anahtar dosyasi YOK: {y}. Sessizce yeni anahtar uretilmez. Yeni anahtar icin \
                 acikca `soulware-core --yeni-anahtar-uret` calistirin (KUBRA adresi degisir; \
                 BAKIM-REHBERI.md bolum 8)."
            ),
            AnahtarHatasi::Bozuk { yol, sebep } => write!(
                f,
                "anahtar dosyasi GECERSIZ ({sebep}): {yol}. Dosyanin uzerine yazilmaz; \
                 yedekten geri yukleyin."
            ),
            AnahtarHatasi::ZatenVar(y) => write!(
                f,
                "anahtar dosyasi ZATEN VAR: {y}. Uzerine yazilmaz; rotasyon icin yeni bir \
                 SOULWARE_KEY_PATH kullanin."
            ),
            AnahtarHatasi::Io { yol, hata } => write!(f, "anahtar dosyasi okunamadi/yazilamadi: {yol}: {hata}"),
        }
    }
}

/// Mevcut anahtari yukle. Yoksa ya da bozuksa HATA (fail-closed).
pub fn yukle(yol: &str) -> Result<SigningKey, AnahtarHatasi> {
    let veri = match std::fs::read(yol) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(AnahtarHatasi::Yok(yol.to_string()))
        }
        Err(e) => return Err(AnahtarHatasi::Io { yol: yol.to_string(), hata: e.to_string() }),
    };
    if veri.len() != DOSYA_LEN {
        return Err(AnahtarHatasi::Bozuk { yol: yol.to_string(), sebep: "uzunluk 33 bayt degil" });
    }
    if veri[0] != SURUM {
        return Err(AnahtarHatasi::Bozuk { yol: yol.to_string(), sebep: "surum bayti gecersiz" });
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&veri[1..DOSYA_LEN]);
    Ok(SigningKey::from_bytes(&seed))
}

/// YENI anahtar uret (yalniz acik parametreyle). Dosya varsa REDDEDER.
pub fn uret(yol: &str) -> Result<SigningKey, AnahtarHatasi> {
    use rand::RngCore;
    use std::io::Write;
    let mut ac = std::fs::OpenOptions::new();
    ac.write(true).create_new(true); // O_EXCL: var olan dosyanin uzerine ASLA yazma
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        ac.mode(0o600); // olusturma aninda 0600 (sonradan chmod penceresi yok)
    }
    let mut dosya = match ac.open(yol) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(AnahtarHatasi::ZatenVar(yol.to_string()))
        }
        Err(e) => return Err(AnahtarHatasi::Io { yol: yol.to_string(), hata: e.to_string() }),
    };
    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let mut icerik = [0u8; DOSYA_LEN];
    icerik[0] = SURUM;
    icerik[1..].copy_from_slice(&seed);
    dosya
        .write_all(&icerik)
        .and_then(|_| dosya.sync_all())
        .map_err(|e| AnahtarHatasi::Io { yol: yol.to_string(), hata: e.to_string() })?;
    Ok(SigningKey::from_bytes(&seed))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Surece ozel gecici dizin (tempfile bagimliligi olmadan).
    fn gecici(ad: &str) -> String {
        let d = std::env::temp_dir().join(format!("soulware-imza-dosyasi-test-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join(ad);
        let _ = std::fs::remove_file(&p);
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn dosya_yoksa_hata_ve_dosya_olusmaz() {
        let p = gecici("yok.key");
        assert_eq!(yukle(&p).err(), Some(AnahtarHatasi::Yok(p.clone())));
        assert!(!std::path::Path::new(&p).exists(), "yukle ASLA dosya olusturmaz");
    }

    #[test]
    fn bozuk_dosyada_hata_ve_dosya_degismez() {
        for (ad, icerik) in [
            ("kisa.key", vec![1u8; 10]),
            ("uzun.key", vec![1u8; 34]),
            ("surum.key", {
                let mut v = vec![0u8; 33];
                v[0] = 2;
                v
            }),
            ("bos.key", vec![]),
        ] {
            let p = gecici(ad);
            std::fs::write(&p, &icerik).unwrap();
            assert!(matches!(yukle(&p), Err(AnahtarHatasi::Bozuk { .. })), "{ad}");
            assert_eq!(std::fs::read(&p).unwrap(), icerik, "{ad}: dosya degismemeli");
        }
    }

    #[test]
    fn uret_sonra_yukle_ayni_anahtar_ve_0600() {
        let p = gecici("yeni.key");
        let a = uret(&p).expect("uret");
        let b = yukle(&p).expect("yukle");
        assert_eq!(a.verifying_key().to_bytes(), b.verifying_key().to_bytes());
        assert_eq!(std::fs::metadata(&p).unwrap().len(), 33);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mod_ = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mod_, 0o600);
        }
    }

    #[test]
    fn uret_var_olan_dosyanin_uzerine_yazmaz() {
        // Gecerli dosya
        let p = gecici("var.key");
        let ilk = uret(&p).unwrap();
        let once = std::fs::read(&p).unwrap();
        assert_eq!(uret(&p).err(), Some(AnahtarHatasi::ZatenVar(p.clone())));
        assert_eq!(std::fs::read(&p).unwrap(), once);
        assert_eq!(
            yukle(&p).unwrap().verifying_key().to_bytes(),
            ilk.verifying_key().to_bytes()
        );
        // Bozuk dosya da korunur (uzerine yazilmaz).
        let b = gecici("bozuk-var.key");
        std::fs::write(&b, [9u8; 5]).unwrap();
        assert_eq!(uret(&b).err(), Some(AnahtarHatasi::ZatenVar(b.clone())));
        assert_eq!(std::fs::read(&b).unwrap(), vec![9u8; 5]);
    }

    #[test]
    fn hata_mesajlari_icerik_sizdirmaz() {
        let p = gecici("sizinti.key");
        std::fs::write(&p, [0xABu8; 20]).unwrap();
        let m = yukle(&p).unwrap_err().to_string();
        assert!(m.contains(&p) && m.contains("GECERSIZ"));
        // Icerik ne hex ne ondalik bicimde mesaja girmez.
        for iz in ["abab", "ABAB", "171, 171", "[171"] {
            assert!(!m.contains(iz), "mesaj icerik sizdirdi: {iz}");
        }
        let d = format!("{:?}", yukle(&p).unwrap_err());
        assert!(!d.contains("171"), "Debug de icerik sizdirmaz");
    }
}
