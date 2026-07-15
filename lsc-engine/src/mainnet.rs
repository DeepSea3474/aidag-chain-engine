//! MAINNET SABITLERI — kalici genesis (guven koku) + kurucu kimligi.
//!
//! ## Neden burada?
//! Genesis, agdaki HER dugumun byte-byte AYNI sekilde uretmesi/tanimasi gereken
//! MUTABAKATIN KOKUDUR. `network_id`, `timestamp`, `payload`, kurucu `public_key`
//! ve `signature` — hepsi genesis id'sinin (blake3 preimage) parcasidir; biri
//! degisirse id degisir ve zincir ayrisir. Bu yuzden bunlar DERLENMIS SABITTIR
//! (env/config DEGIL): kimse yanlislikla degistiremez.
//!
//! ## Kurucu anahtari (KRITIK — geri donulemez)
//! Genesis, KURUCU'nun ed25519 ozel anahtariyla BIR KEZ imzalanir. Ozel anahtar
//! `aidag-kurucu.key` dosyasindadir ve KURUCU'DA KALIR (repoya girmez; .gitignore
//! `*.key`). Bu modul yalnizca PUBLIC verileri (pubkey, imza, id, wire baytlari)
//! tutar — hepsi genesis'le birlikte zaten herkese aciktir. Node calisirken ozel
//! anahtara IHTIYAC YOKTUR (genesis baked-bytes olarak yuklenir); anahtar offline
//! saklanabilir. **Ozel anahtar kaybi = kurucu/hazine kontrolu kaybi.**
//!
//! ## Uretim (tek sefer)
//! `cargo test -p lsc-engine uret_mainnet_genesis -- --ignored --nocapture`
//! anahtari yoksa uretir + kaydeder, genesis'i deterministik hesaplar ve
//! asagidaki sabitlerin degerlerini basar. Ciktiyi bu dosyaya islersin.

use crate::dag::vertex::VertexId;

/// Mainnet ag kimligi. EVM `chain_id` (3474) ile ayni — tek kimlik.
/// Her vertex preimage'inin parcasi; testnet'ten (0xA1DA6) ayri → cross-replay yok.
pub const MAINNET_NETWORK_ID: u32 = 3474;

/// Genesis zamani (Unix saniye). 01.08.2007 00:00:00 UTC.
/// DEGISTIRILEMEZ — genesis id'sinin parcasi. Bitcoin genesis gibi ebedi sabit.
pub const MAINNET_GENESIS_ZAMANI: u64 = 1_185_926_400;

/// Genesis payload (opak). Genesis id'sinin parcasi — degisirse id degisir.
pub const MAINNET_GENESIS_PAYLOAD: &[u8] = b"AIDAG-MAINNET-GENESIS-v1";

/// VESTING BASLANGICI (Unix saniye) — kurucu/destekci/likidite kilit sayaci
/// buradan baslar. **SABIT** olmali: `SystemTime::now()` kullanilirsa her dugum
/// farkli kilit takvimi hesaplar → bakiye/transfer gecerliligi ayrisir (konsensus
/// bolunmesi). Bu yuzden koda pinli. Su an referans: 2026-07-15 00:00:00 UTC.
/// GERCEK MAINNET LAUNCH tarihinde bu deger guncellenip yeniden derlenir.
pub const MAINNET_VESTING_BASLANGIC: u64 = 1_784_073_600;

// === Asagidaki degerler `uret_mainnet_genesis` ciktisiyla doldurulur ===

/// Kurucu (genesis imzalayan) ed25519 public key — 32 bayt hex.
pub const MAINNET_KURUCU_PUBKEY_HEX: &str =
    "cece417af631d437df7adfe7afca45b4745b9958ee446df3062c1c008c2e1c73";

/// Kurucu kanonik adresi — 20 bayt hex (blake3(pubkey)[..20]).
pub const MAINNET_KURUCU_ADRES_HEX: &str = "11c1906e07508e0b83ef4afa042879281e196b9f";

/// Pinli genesis vertex id — 32 bayt hex (guven koku).
pub const MAINNET_GENESIS_ID_HEX: &str =
    "b82345008ae109d842beefa4004a8680fc6f545fefa2c87a6a218de0f1269c39";

/// Genesis'in TAM wire-encoded baytlari (hex). Node acilista bunu decode edip
/// insert_synced eder; ozel anahtara ihtiyac yok. Kurucu pubkey + imza ICINDE.
pub const MAINNET_GENESIS_WIRE_HEX: &str = "01920d0000000000000000000000cdaf46000000001800000000000000cece417af631d437df7adfe7afca45b4745b9958ee446df3062c1c008c2e1c73887b3256f1f2eba7ac83f799584323aba7d5f8f1ab419bea7e3efac82e4ca6fa3b0aadfb43a669d3dd3ac9c32d5dbc819f30b22836cc259ca6fbedc9501beb0741494441472d4d41494e4e45542d47454e455349532d7631";

/// Kurucu adresi — [u8;20]. Sabit hex'ten cozer (panic yok: derleme-zamani sabit,
/// startup'ta bir kez cagirilir; gecersizse acikca DUR).
pub fn kurucu_adres() -> [u8; 20] {
    let b = hex_decode(MAINNET_KURUCU_ADRES_HEX).expect("MAINNET_KURUCU_ADRES_HEX gecersiz hex");
    let mut a = [0u8; 20];
    a.copy_from_slice(&b);
    a
}

/// Pinli genesis id — [u8;32].
pub fn genesis_id() -> VertexId {
    let b = hex_decode(MAINNET_GENESIS_ID_HEX).expect("MAINNET_GENESIS_ID_HEX gecersiz hex");
    let mut a = [0u8; 32];
    a.copy_from_slice(&b);
    a
}

/// Genesis'in wire baytlari — node acilista decode+ingest eder.
pub fn genesis_wire() -> Vec<u8> {
    hex_decode(MAINNET_GENESIS_WIRE_HEX).expect("MAINNET_GENESIS_WIRE_HEX gecersiz hex")
}

/// Minimal hex decoder (bagimlilik eklemeden). Cift uzunluk + [0-9a-fA-F] bekler.
fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let val = |c: u8| -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    };
    let mut i = 0;
    while i < bytes.len() {
        let hi = val(bytes[i])?;
        let lo = val(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::vertex::Vertex;
    use crate::dag::wire;
    use crate::registry::public_key_to_adres;
    use ed25519_dalek::SigningKey;

    /// Kurucu anahtar dosyasinin yolu (repo koku). Format: [algo_id=1][32 seed].
    fn kurucu_key_path() -> std::path::PathBuf {
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../aidag-kurucu.key"))
    }

    /// TEK SEFERLIK: kurucu anahtarini uret (yoksa) + genesis'i deterministik
    /// hesapla + sabit degerleri bas. Ciktiyi mainnet.rs sabitlerine isle.
    /// `cargo test -p lsc-engine uret_mainnet_genesis -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn uret_mainnet_genesis() {
        use rand::rngs::OsRng;
        use rand::RngCore;

        let path = kurucu_key_path();
        // Anahtari yukle ya da uret+kaydet (idempotent → hep ayni genesis).
        let seed: [u8; 32] = if path.exists() {
            let data = std::fs::read(&path).expect("kurucu.key okunamadi");
            assert!(
                data.len() == 33 && data[0] == 1,
                "kurucu.key format [1][32seed] olmali"
            );
            let mut s = [0u8; 32];
            s.copy_from_slice(&data[1..33]);
            s
        } else {
            let mut s = [0u8; 32];
            OsRng.fill_bytes(&mut s);
            let mut dosya = vec![1u8]; // algo_id = ed25519
            dosya.extend_from_slice(&s);
            std::fs::write(&path, &dosya).expect("kurucu.key yazilamadi");
            eprintln!("[URETILDI] Yeni kurucu anahtari: {:?}", path);
            s
        };

        let key = SigningKey::from_bytes(&seed);
        let pubkey = key.verifying_key().to_bytes();
        let adres = public_key_to_adres(&pubkey);

        // Deterministik genesis: (network_id, [], payload, timestamp, key).
        // ed25519 imzasi RFC8032 belirlenimci → ayni girdi = ayni id, her zaman.
        let genesis = Vertex::new_signed(
            MAINNET_NETWORK_ID,
            vec![],
            MAINNET_GENESIS_PAYLOAD.to_vec(),
            MAINNET_GENESIS_ZAMANI,
            &key,
        )
        .expect("genesis uretilemedi");
        let id = *genesis.id();
        let wire_bytes = wire::encode(&genesis);

        // Tekrar-uretilebilirlik teyidi: decode → ayni id.
        let geri = wire::decode(&wire_bytes).expect("genesis decode");
        assert_eq!(*geri.id(), id, "wire round-trip id uyusmuyor");

        eprintln!("\n================ MAINNET GENESIS (mainnet.rs'e isle) ================");
        eprintln!("MAINNET_KURUCU_PUBKEY_HEX = \"{}\"", hex_encode(&pubkey));
        eprintln!("MAINNET_KURUCU_ADRES_HEX  = \"{}\"", hex_encode(&adres));
        eprintln!("MAINNET_GENESIS_ID_HEX    = \"{}\"", hex_encode(&id));
        eprintln!(
            "MAINNET_GENESIS_WIRE_HEX  = \"{}\"",
            hex_encode(&wire_bytes)
        );
        eprintln!("====================================================================\n");
    }

    /// Sabitler DOLU (uret_mainnet_genesis islenmis) ise: baked genesis gercekten
    /// pinli id'yi ve kurucu adresini uretiyor mu? Placeholder iken atlanir.
    #[test]
    fn baked_genesis_tutarli() {
        if MAINNET_GENESIS_WIRE_HEX.is_empty() {
            eprintln!("baked_genesis_tutarli: sabitler henuz bos — atlandi.");
            return;
        }
        let wire_bytes = genesis_wire();
        let v = wire::decode(&wire_bytes).expect("baked genesis decode");
        v.verify().expect("baked genesis imza/id dogrulanmali");
        assert_eq!(*v.id(), genesis_id(), "baked wire id != MAINNET_GENESIS_ID");
        assert_eq!(
            v.network_id(),
            MAINNET_NETWORK_ID,
            "genesis network_id != 3474"
        );
        assert!(v.parents().is_empty(), "genesis parent'siz olmali");
        assert_eq!(
            v.timestamp(),
            MAINNET_GENESIS_ZAMANI,
            "genesis zamani != sabit"
        );
        assert_eq!(
            public_key_to_adres(v.public_key()),
            kurucu_adres(),
            "genesis imzalayan != kurucu adresi"
        );
    }

    fn hex_encode(b: &[u8]) -> String {
        let mut s = String::with_capacity(b.len() * 2);
        for x in b {
            s.push_str(&format!("{:02x}", x));
        }
        s
    }
}
