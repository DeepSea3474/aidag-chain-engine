//! KUBRA zincir kaniti: etkilesim hash'i.
//!
//! Zincire YALNIZ 32 baytlik hash yazilir (tip=1 Record). Soru/cevap metni
//! ve tuz zincire girmez.
//!
//! Iki sema:
//!   - ESKI (tuzsuz): blake3( net_id_le | ts_le | alan1 0x1e alan2 0x1e ... )
//!     Bugune kadar yazilan kayitlar. Dogrulamasi AYNEN korunur.
//!   - TUZLU (v1):    blake3_keyed(tuz, ayni girdi)
//!     tuz = 32 bayt CSPRNG (OsRng). Tuz yalniz kullaniciya dondurulur.
//!     blake3 keyed modu ayri bir alan (domain) oldugundan tuzlu ve tuzsuz
//!     hash'ler birbirine karismaz.
//!
//! Neden tuz: metni tahmin edilebilen etkilesimler (kisa soru + sabit arac
//! cevabi) tuzsuz hash'ten tahmin-dene yoluyla bulunabilir. Tuzu bilmeyen
//! biri, zincirdeki hash'ten icerigi dogrulayamaz/tahmin edemez.

use rand::RngCore;

pub const TUZ_LEN: usize = 32;
const AYRAC: u8 = 0x1e;

/// 32 bayt kriptografik rastgele tuz (isletim sistemi CSPRNG'si).
pub fn yeni_tuz() -> [u8; TUZ_LEN] {
    let mut t = [0u8; TUZ_LEN];
    rand::rngs::OsRng.fill_bytes(&mut t);
    t
}

/// Etkilesim hash'i. `tuz` yoksa ESKI (tuzsuz) sema, varsa TUZLU sema.
/// `alanlar` 0x1e ile ayrilir (eski kodun bayt dizilimiyle birebir ayni).
pub fn kanit_hash(net_id: u32, ts: u64, alanlar: &[&[u8]], tuz: Option<&[u8; TUZ_LEN]>) -> [u8; 32] {
    let mut h = match tuz {
        Some(t) => blake3::Hasher::new_keyed(t),
        None => blake3::Hasher::new(),
    };
    h.update(&net_id.to_le_bytes());
    h.update(&ts.to_le_bytes());
    for (i, a) in alanlar.iter().enumerate() {
        if i > 0 {
            h.update(&[AYRAC]);
        }
        h.update(a);
    }
    *h.finalize().as_bytes()
}

/// Hex tuzu coz (0x opsiyonel). Tam 32 bayt olmali.
pub fn tuz_coz(s: &str) -> Result<[u8; TUZ_LEN], String> {
    let b = hex::decode(s.trim().trim_start_matches("0x"))
        .map_err(|_| "salt gecersiz hex".to_string())?;
    if b.len() != TUZ_LEN {
        return Err(format!("salt {TUZ_LEN} bayt (64 hex) olmali, {} bayt geldi", b.len()));
    }
    let mut t = [0u8; TUZ_LEN];
    t.copy_from_slice(&b);
    Ok(t)
}

/// POST /v1/verify govdesi (metin etkilesimleri: /v1/ask, /v1/ask-stream).
#[derive(Debug, serde::Deserialize)]
pub struct DogrulaIstek {
    /// Yanittaki `ts` (unix saniye).
    pub ts: u64,
    pub prompt: String,
    pub answer: String,
    /// Yanittaki `model`. Tuzlu kayitta ZORUNLU. Eski kayitta: /v1/ask ve arac
    /// cevaplari icin verilir, eski /v1/ask-stream model cevabi icin verilmez.
    #[serde(default)]
    pub model: Option<String>,
    /// Yanittaki `salt` (64 hex). Yoksa eski (tuzsuz) sema kullanilir.
    #[serde(default)]
    pub salt: Option<String>,
    /// Istege bagli: elindeki proof_hash; verilirse hesaplananla karsilastirilir.
    #[serde(default)]
    pub proof_hash: Option<String>,
}

/// Dogrulama icin hash'i yeniden hesapla. Donus: (hash, tuzlu_mu).
pub fn dogrulama_hash(net_id: u32, r: &DogrulaIstek) -> Result<([u8; 32], bool), String> {
    match r.salt.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => {
            let tuz = tuz_coz(s)?;
            let model = r.model.as_deref().ok_or("tuzlu kayit icin model zorunlu")?;
            let alanlar: [&[u8]; 3] = [r.prompt.as_bytes(), r.answer.as_bytes(), model.as_bytes()];
            Ok((kanit_hash(net_id, r.ts, &alanlar, Some(&tuz)), true))
        }
        None => {
            let h = match r.model.as_deref() {
                Some(m) => kanit_hash(net_id, r.ts, &[r.prompt.as_bytes(), r.answer.as_bytes(), m.as_bytes()], None),
                None => kanit_hash(net_id, r.ts, &[r.prompt.as_bytes(), r.answer.as_bytes()], None),
            };
            Ok((h, false))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsc_engine::tx::Record;

    const NET: u32 = 3474;

    // ── ESKI KODUN BIREBIR KOPYALARI (main fdfe6d6, soulware-core/src/main.rs) ──
    // Geriye uyumlulugun referansi: yeni kanit_hash(None) bunlarla AYNI olmali.
    fn eski_ask(net: u32, ts: u64, prompt: &str, answer: &str, model: &str) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        h.update(&net.to_le_bytes());
        h.update(&ts.to_le_bytes());
        h.update(prompt.as_bytes());
        h.update(&[0x1e]);
        h.update(answer.as_bytes());
        h.update(&[0x1e]);
        h.update(model.as_bytes());
        *h.finalize().as_bytes()
    }
    fn eski_stream(net: u32, ts: u64, prompt: &str, metin: &str) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        h.update(&net.to_le_bytes()); h.update(&ts.to_le_bytes());
        h.update(prompt.as_bytes()); h.update(&[0x1e]);
        h.update(metin.as_bytes());
        *h.finalize().as_bytes()
    }
    fn eski_medya(net: u32, ts: u64, prompt: &str, wallet: Option<&str>, bytes: &[u8]) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        h.update(&net.to_le_bytes());
        h.update(&ts.to_le_bytes());
        h.update(prompt.as_bytes());
        h.update(&[0x1e]);
        if let Some(w) = wallet { h.update(w.as_bytes()); h.update(&[0x1e]); }
        h.update(bytes);
        *h.finalize().as_bytes()
    }

    fn ornekler() -> Vec<(u64, &'static str, &'static str, &'static str)> {
        vec![
            (1_758_650_000, "AIDAG nedir?", "AIDAG Chain bir DAG L1 zinciridir.", "qwen2.5-7b"),
            (0, "", "", ""),
            (u64::MAX, "çğıöşü İĞŞ 😀", "yanıt\u{1e}ayraçlı", "ag-durumu"),
            (1_758_650_123, "7 çarpı 8", "56", "hesap-makinesi"),
        ]
    }

    #[test]
    fn eski_sema_birebir_korunur() {
        for (ts, p, a, m) in ornekler() {
            // /v1/ask ve arac_kanit: prompt|cevap|model
            assert_eq!(kanit_hash(NET, ts, &[p.as_bytes(), a.as_bytes(), m.as_bytes()], None), eski_ask(NET, ts, p, a, m));
            // /v1/ask-stream model cevabi: prompt|metin
            assert_eq!(kanit_hash(NET, ts, &[p.as_bytes(), a.as_bytes()], None), eski_stream(NET, ts, p, a));
            // gorsel/video: prompt|[cuzdan]|bayt
            let b = [0u8, 1, 0x1e, 255];
            assert_eq!(kanit_hash(NET, ts, &[p.as_bytes(), &b], None), eski_medya(NET, ts, p, None, &b));
            assert_eq!(kanit_hash(NET, ts, &[p.as_bytes(), b"0xabc", &b], None), eski_medya(NET, ts, p, Some("0xabc"), &b));
        }
    }

    #[test]
    fn eski_sema_sabit_vektor() {
        // Uygulamadan bagimsiz sabit: blake3 kutuphanesi/kod degisse bile eski
        // kayitlarin hash'i degismemeli. Deger Python `blake3` paketiyle
        // (Rust kodundan bagimsiz) hesaplandi.
        let h = eski_ask(NET, 1_758_650_000, "AIDAG nedir?", "AIDAG Chain bir DAG L1 zinciridir.", "qwen2.5-7b");
        assert_eq!(hex::encode(h), SABIT_ESKI);
        assert_eq!(
            hex::encode(kanit_hash(NET, 1_758_650_000,
                &[b"AIDAG nedir?", "AIDAG Chain bir DAG L1 zinciridir.".as_bytes(), b"qwen2.5-7b"], None)),
            SABIT_ESKI
        );
    }
    const SABIT_ESKI: &str = "45f56b3afbc22499c78f951094dfdb4b07c76868f71f0ed3910ce74cd3c9d654";

    #[test]
    fn tuzlu_sema_sabit_vektor() {
        // Dis dogrulayicilar icin referans: Python `blake3.blake3(girdi, key=tuz)`
        // ile ayni sonucu verir (tuz = 32 x 0x07).
        let h = kanit_hash(NET, 1_758_650_000,
            &[b"AIDAG nedir?", "AIDAG Chain bir DAG L1 zinciridir.".as_bytes(), b"qwen2.5-7b"], Some(&[7u8; 32]));
        assert_eq!(hex::encode(h), "bfe6123c22fabe0d7458f1694f3bb0091f50de14cd7953fade0d2b8a1f8ac192");
    }

    #[test]
    fn tuzlu_hash_eskiden_ve_birbirinden_farkli() {
        let al: [&[u8]; 3] = [b"soru", b"cevap", b"model"];
        let eski = kanit_hash(NET, 5, &al, None);
        let t1 = yeni_tuz();
        let t2 = yeni_tuz();
        let h1 = kanit_hash(NET, 5, &al, Some(&t1));
        assert_ne!(h1, eski);
        assert_ne!(h1, kanit_hash(NET, 5, &al, Some(&t2)));
        // ayni tuz -> ayni hash (deterministik, dogrulanabilir)
        assert_eq!(h1, kanit_hash(NET, 5, &al, Some(&t1)));
        // sifir tuz bile eski semayla cakismaz (keyed = ayri alan)
        assert_ne!(kanit_hash(NET, 5, &al, Some(&[0u8; 32])), eski);
        // alanlarin her biri hash'e girer
        assert_ne!(h1, kanit_hash(NET, 6, &al, Some(&t1)));
        assert_ne!(h1, kanit_hash(NET + 1, 5, &al, Some(&t1)));
        assert_ne!(h1, kanit_hash(NET, 5, &[b"soru", b"cevap!", b"model"], Some(&t1)));
    }

    #[test]
    fn tuz_rastgele_ve_32_bayt() {
        let mut gorulen = std::collections::HashSet::new();
        for _ in 0..1000 {
            let t = yeni_tuz();
            assert_eq!(t.len(), 32);
            assert_ne!(t, [0u8; 32]);
            assert!(gorulen.insert(t), "tuz tekrarlandi");
        }
    }

    #[test]
    fn tuz_zincir_payloadina_girmez() {
        let t = yeni_tuz();
        let h = kanit_hash(NET, 9, &[b"p", b"a", b"m"], Some(&t));
        let payload = Record::new(h).encode();
        // payload = [tip=1][hash:32], 33 bayt; tuz icin alan yok
        assert_eq!(payload.len(), 33);
        assert_eq!(payload[0], 1);
        assert_eq!(&payload[1..], &h);
        assert!(!payload.windows(32).any(|w| w == t), "tuz payload'da bulunmamali");
        // ve hash'ten geri cozulebilir degil: Record yalniz hash'i tasir
        assert_eq!(Record::decode(&payload).unwrap().data_hash, h);
    }

    fn istek(ts: u64, model: Option<&str>, salt: Option<String>) -> DogrulaIstek {
        DogrulaIstek {
            ts, prompt: "AIDAG nedir?".into(), answer: "Bir DAG L1.".into(),
            model: model.map(Into::into), salt, proof_hash: None,
        }
    }

    #[test]
    fn dogrulama_eski_kayitlari_kabul_eder() {
        // tuzsuz /v1/ask (modelli) ve eski /v1/ask-stream (modelsiz)
        let (h, tuzlu) = dogrulama_hash(NET, &istek(7, Some("qwen"), None)).unwrap();
        assert!(!tuzlu);
        assert_eq!(h, eski_ask(NET, 7, "AIDAG nedir?", "Bir DAG L1.", "qwen"));
        let (h, _) = dogrulama_hash(NET, &istek(7, None, None)).unwrap();
        assert_eq!(h, eski_stream(NET, 7, "AIDAG nedir?", "Bir DAG L1."));
        // bos salt = tuzsuz (eski istemciler "salt":"" gonderebilir)
        let (h2, tuzlu) = dogrulama_hash(NET, &istek(7, None, Some("  ".into()))).unwrap();
        assert!(!tuzlu);
        assert_eq!(h2, h);
    }

    #[test]
    fn dogrulama_tuzu_kabul_eder() {
        let t = yeni_tuz();
        let beklenen = kanit_hash(NET, 7, &[b"AIDAG nedir?", b"Bir DAG L1.", b"qwen"], Some(&t));
        for s in [hex::encode(t), format!("0x{}", hex::encode(t)), hex::encode(t).to_uppercase()] {
            let (h, tuzlu) = dogrulama_hash(NET, &istek(7, Some("qwen"), Some(s))).unwrap();
            assert!(tuzlu);
            assert_eq!(h, beklenen);
        }
        // yanlis tuz -> farkli hash (eslesmez)
        let (h, _) = dogrulama_hash(NET, &istek(7, Some("qwen"), Some(hex::encode(yeni_tuz())))).unwrap();
        assert_ne!(h, beklenen);
        // tuzlu kayit tuzsuz dogrulanamaz
        let (h, _) = dogrulama_hash(NET, &istek(7, Some("qwen"), None)).unwrap();
        assert_ne!(h, beklenen);
    }

    #[test]
    fn dogrulama_gecersiz_tuzu_reddeder() {
        assert!(dogrulama_hash(NET, &istek(7, Some("m"), Some("zz".into()))).is_err());
        assert!(dogrulama_hash(NET, &istek(7, Some("m"), Some("ab".repeat(31)))).is_err());
        assert!(dogrulama_hash(NET, &istek(7, Some("m"), Some("ab".repeat(33)))).is_err());
        // tuzlu kayitta model zorunlu
        assert!(dogrulama_hash(NET, &istek(7, None, Some("ab".repeat(32)))).is_err());
    }
}
