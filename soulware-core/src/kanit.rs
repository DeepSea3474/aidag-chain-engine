//! KUBRA zincir kaniti: etkilesim hash'i.
//!
//! Zincire YALNIZ 32 baytlik hash yazilir (tip=1 Record). Soru/cevap metni
//! ve tuz zincire girmez.
//!
//! Uc sema:
//!   - ESKI (tuzsuz): blake3( net_id_le | ts_le | alan1 0x1e alan2 0x1e ... )
//!   - TUZLU (v1):    blake3_keyed(tuz, ayni girdi)
//!     Bu ikisi YALNIZ geriye uyumlu DOGRULAMA icin tutulur; yeni kayit yazilmaz.
//!     ZAYIFLIK: alanlar yalniz 0x1e ile ayrilir, uzunluk oneki yok. Bir alan 0x1e
//!     iceriyorsa ayni bayt dizisi farkli (prompt, answer, model) bolunmesine
//!     karsilik gelir -> dogrulama bu durumda "belirsiz" doner (dogrulandi=false).
//!   - TUZLU (v2):    `blake3_keyed(tuz, "KUBRA-KANIT-v2\0" | L(net_id_le) | L(ts_le) | L(tur) | u64le(alan_sayisi) | L(alan1) | L(alan2) ...)`
//!     L(x) = u64 LE uzunluk | x. Uzunluk oneki sayesinde bolunme TEKTIR (enjeksiyon
//!     yok); alan etiketi v2 girdisini v1/eski girdilerinden ayirir; `tur` sohbet ve
//!     medya kayitlarini birbirinden ayirir. YENI kayitlarin hepsi v2.
//!
//! tuz = 32 bayt CSPRNG (OsRng). Tuz yalniz kullaniciya dondurulur.

use rand::RngCore;

pub const TUZ_LEN: usize = 32;
const AYRAC: u8 = 0x1e;

pub const SEMA_V2: &str = "tuzlu-v2";
pub const SEMA_V1: &str = "tuzlu-v1";
pub const SEMA_ESKI: &str = "eski-tuzsuz";
/// v2 alan (domain) etiketi: v2 girdisi her zaman bununla baslar.
pub const V2_ETIKET: &[u8] = b"KUBRA-KANIT-v2\0";
/// v2 kayit turleri.
pub const TUR_SOHBET: &[u8] = b"sohbet";
pub const TUR_MEDYA: &[u8] = b"medya";

/// 32 bayt kriptografik rastgele tuz (isletim sistemi CSPRNG'si).
pub fn yeni_tuz() -> [u8; TUZ_LEN] {
    let mut t = [0u8; TUZ_LEN];
    rand::rngs::OsRng.fill_bytes(&mut t);
    t
}

/// ESKI/v1 etkilesim hash'i (YALNIZ dogrulama icin). `tuz` yoksa eski (tuzsuz),
/// varsa tuzlu-v1. `alanlar` 0x1e ile ayrilir (eski kodun bayt dizilimiyle birebir).
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

fn uzunluklu(h: &mut blake3::Hasher, b: &[u8]) {
    h.update(&(b.len() as u64).to_le_bytes());
    h.update(b);
}

/// tuzlu-v2 hash'i (uzunluk onekli, alan etiketli). Tum yeni kayitlar bununla yazilir.
pub fn kanit_hash_v2(tuz: &[u8; TUZ_LEN], net_id: u32, ts: u64, tur: &[u8], alanlar: &[&[u8]]) -> [u8; 32] {
    let mut h = blake3::Hasher::new_keyed(tuz);
    h.update(V2_ETIKET);
    uzunluklu(&mut h, &net_id.to_le_bytes());
    uzunluklu(&mut h, &ts.to_le_bytes());
    uzunluklu(&mut h, tur);
    h.update(&(alanlar.len() as u64).to_le_bytes());
    for a in alanlar {
        uzunluklu(&mut h, a);
    }
    *h.finalize().as_bytes()
}

/// v2 sohbet kaydi (/v1/ask, /v1/ask-stream; beyin veya arac):
/// alanlar = prompt, answer, model, context (istemci baglami; yoksa bos).
pub fn sohbet_hash_v2(tuz: &[u8; TUZ_LEN], net_id: u32, ts: u64, prompt: &str, answer: &str, model: &str, context: &str) -> [u8; 32] {
    kanit_hash_v2(tuz, net_id, ts, TUR_SOHBET,
        &[prompt.as_bytes(), answer.as_bytes(), model.as_bytes(), context.as_bytes()])
}

/// v2 medya kaydi (/v1/image, /v1/video): alanlar = prompt, wallet (yoksa bos), icerik.
pub fn medya_hash_v2(tuz: &[u8; TUZ_LEN], net_id: u32, ts: u64, prompt: &str, wallet: Option<&str>, icerik: &[u8]) -> [u8; 32] {
    kanit_hash_v2(tuz, net_id, ts, TUR_MEDYA,
        &[prompt.as_bytes(), wallet.unwrap_or("").as_bytes(), icerik])
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
    /// Yanittaki `model`. Tuzlu (v1/v2) kayitta ZORUNLU. Eski kayitta: /v1/ask ve
    /// arac cevaplari icin verilir, eski /v1/ask-stream model cevabi icin verilmez.
    #[serde(default)]
    pub model: Option<String>,
    /// Yanittaki `salt` (64 hex). Yoksa eski (tuzsuz) sema kullanilir.
    #[serde(default)]
    pub salt: Option<String>,
    /// Istege bagli: elindeki proof_hash; verilirse hesaplananla karsilastirilir.
    #[serde(default)]
    pub proof_hash: Option<String>,
    /// Istege bagli sema: "tuzlu-v2" | "tuzlu-v1" | "eski-tuzsuz".
    /// Verilmezse: salt varsa once v2, sonra v1; salt yoksa eski-tuzsuz.
    #[serde(default)]
    pub sema: Option<String>,
    /// /v1/ask'a gonderilen istemci baglami (v2'de hash'e girer; yoksa bos).
    #[serde(default)]
    pub context: Option<String>,
}

/// Dogrulama adayi: hangi sema, hangi hash, (v1/eski icin) belirsizlik sebebi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aday {
    pub sema: &'static str,
    pub hash: [u8; 32],
    /// Some(sebep) -> bu sema bu girdiler icin guvenle dogrulanamaz (dogrulandi=false).
    pub belirsiz: Option<String>,
}

pub const BELIRSIZ_SEBEP: &str = "alanlardan biri 0x1e (alan ayraci) iceriyor; v1/eski semada alan sinirlari \
belirsiz oldugundan ayni hash farkli (prompt, answer, model) bolunmesine karsilik gelebilir. Bu kayit guvenle \
dogrulanamaz (tuzlu-v2 bu sorunu uzunluk onekiyle cozer).";

/// Dogrulama icin aday hash'leri hesapla (ag yok, saf).
pub fn dogrulama_adaylari(net_id: u32, r: &DogrulaIstek) -> Result<Vec<Aday>, String> {
    let salt = r.salt.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let sema = r.sema.as_deref().map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty());
    if let Some(s) = sema.as_deref() {
        if ![SEMA_V2, SEMA_V1, SEMA_ESKI].contains(&s) {
            return Err(format!("bilinmeyen sema: {s} (gecerli: {SEMA_V2}, {SEMA_V1}, {SEMA_ESKI})"));
        }
    }
    let ctx = r.context.as_deref().unwrap_or("");
    // v1/eski: alan icinde ayrac varsa bolunme belirsiz.
    let ayrac_var = [Some(r.prompt.as_str()), Some(r.answer.as_str()), r.model.as_deref()]
        .iter()
        .flatten()
        .any(|a| a.as_bytes().contains(&AYRAC));
    let belirsiz = || ayrac_var.then(|| BELIRSIZ_SEBEP.to_string());

    let semalar: Vec<&'static str> = match (salt, sema.as_deref()) {
        (Some(_), None) => vec![SEMA_V2, SEMA_V1],
        (Some(_), Some(s)) if s == SEMA_V2 => vec![SEMA_V2],
        (Some(_), Some(s)) if s == SEMA_V1 => vec![SEMA_V1],
        (Some(_), Some(_)) => return Err("eski-tuzsuz semada salt verilmez".into()),
        (None, None) => vec![SEMA_ESKI],
        (None, Some(s)) if s == SEMA_ESKI => vec![SEMA_ESKI],
        (None, Some(_)) => return Err("tuzlu sema icin salt zorunlu".into()),
    };
    let tuz = match salt {
        Some(s) => Some(tuz_coz(s)?),
        None => None,
    };
    let mut out = Vec::new();
    for s in semalar {
        let aday = match s {
            SEMA_V2 => {
                let model = r.model.as_deref().ok_or("tuzlu kayit icin model zorunlu")?;
                let t = tuz.as_ref().ok_or("salt zorunlu")?;
                Aday { sema: SEMA_V2, hash: sohbet_hash_v2(t, net_id, r.ts, &r.prompt, &r.answer, model, ctx), belirsiz: None }
            }
            SEMA_V1 => {
                let model = r.model.as_deref().ok_or("tuzlu kayit icin model zorunlu")?;
                let alanlar: [&[u8]; 3] = [r.prompt.as_bytes(), r.answer.as_bytes(), model.as_bytes()];
                Aday { sema: SEMA_V1, hash: kanit_hash(net_id, r.ts, &alanlar, tuz.as_ref()), belirsiz: belirsiz() }
            }
            _ => {
                let h = match r.model.as_deref() {
                    Some(m) => kanit_hash(net_id, r.ts, &[r.prompt.as_bytes(), r.answer.as_bytes(), m.as_bytes()], None),
                    None => kanit_hash(net_id, r.ts, &[r.prompt.as_bytes(), r.answer.as_bytes()], None),
                };
                Aday { sema: SEMA_ESKI, hash: h, belirsiz: belirsiz() }
            }
        };
        out.push(aday);
    }
    Ok(out)
}

/// Zincirdeki /belge/<hash> durumu (dogrulama karari icin).
#[derive(Debug, Clone, Default)]
pub struct ZincirBilgi {
    pub kayitli: bool,
    /// Kaydeden adres (kucuk harf, 0x'siz).
    pub kaydeden: Option<String>,
    pub zaman: Option<serde_json::Value>,
}

impl ZincirBilgi {
    pub fn coz(v: &serde_json::Value) -> Self {
        ZincirBilgi {
            kayitli: v.get("kayitli").and_then(|x| x.as_bool()).unwrap_or(false),
            kaydeden: v.get("kaydeden").and_then(|x| x.as_str()).map(|a| a.trim_start_matches("0x").to_lowercase()),
            zaman: v.get("zaman").cloned(),
        }
    }
}

/// Dogrulama karari (saf). `kubra_hex` = KUBRA imza adresi (40 hex, 0x'siz).
/// Aday secimi: proof_hash verildiyse onunla eslesen; yoksa zincirde kayitli ilk
/// aday; o da yoksa ilk aday. Belirsiz aday ASLA dogrulandi=true vermez.
pub fn dogrulama_karari(sonuclar: &[(Aday, ZincirBilgi)], kubra_hex: &str, proof: Option<&str>, context_verildi: bool) -> serde_json::Value {
    use serde_json::json;
    let proof = proof.map(|p| p.trim().trim_start_matches("0x").to_ascii_lowercase());
    let sec = proof
        .as_deref()
        .and_then(|p| sonuclar.iter().find(|(a, _)| hex::encode(a.hash) == p))
        .or_else(|| sonuclar.iter().find(|(_, z)| z.kayitli))
        .or_else(|| sonuclar.first());
    let Some((aday, z)) = sec else {
        return json!({ "ok": false, "hata": "aday yok" });
    };
    let hash_hex = hex::encode(aday.hash);
    let proof_eslesir = proof.as_deref().map(|p| p == hash_hex);
    let kubra_imzali = z.kaydeden.as_deref().map(|a| a == kubra_hex);
    let belirsiz = aday.belirsiz.is_some();
    let dogrulandi = z.kayitli && kubra_imzali == Some(true) && proof_eslesir != Some(false) && !belirsiz;
    let mut v = json!({
        "ok": true,
        "proof_hash": hash_hex,
        "sema": aday.sema,
        "denenen_semalar": sonuclar.iter().map(|(a, _)| a.sema).collect::<Vec<_>>(),
        "proof_eslesir": proof_eslesir,
        "zincirde": z.kayitli,
        "kaydeden": z.kaydeden.as_ref().map(|a| format!("0x{a}")),
        "kubra_imzali": kubra_imzali,
        "zaman": z.zaman.clone(),
        "belirsiz": belirsiz,
        "dogrulandi": dogrulandi,
    });
    if let Some(s) = &aday.belirsiz {
        v["sebep"] = json!(s);
    }
    if context_verildi && aday.sema != SEMA_V2 {
        v["uyari"] = json!("bu semada (v1/eski) istemci baglami (context) hash'e girmez; baglam dogrulanmadi");
    }
    v
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
            model: model.map(Into::into), salt, proof_hash: None, sema: None, context: None,
        }
    }
    fn tek(net: u32, r: &DogrulaIstek, sema: &str) -> Aday {
        dogrulama_adaylari(net, r).unwrap().into_iter().find(|a| a.sema == sema).expect(sema)
    }

    #[test]
    fn dogrulama_eski_kayitlari_kabul_eder() {
        // tuzsuz /v1/ask (modelli) ve eski /v1/ask-stream (modelsiz)
        let a = dogrulama_adaylari(NET, &istek(7, Some("qwen"), None)).unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].sema, SEMA_ESKI);
        assert_eq!(a[0].hash, eski_ask(NET, 7, "AIDAG nedir?", "Bir DAG L1.", "qwen"));
        assert!(a[0].belirsiz.is_none());
        let a = dogrulama_adaylari(NET, &istek(7, None, None)).unwrap();
        assert_eq!(a[0].hash, eski_stream(NET, 7, "AIDAG nedir?", "Bir DAG L1."));
        // bos salt = tuzsuz (eski istemciler "salt":"" gonderebilir)
        let b = dogrulama_adaylari(NET, &istek(7, None, Some("  ".into()))).unwrap();
        assert_eq!(b, a);
    }

    #[test]
    fn dogrulama_tuzu_kabul_eder_v2_once_v1_sonra() {
        let t = yeni_tuz();
        let v1 = kanit_hash(NET, 7, &[b"AIDAG nedir?", b"Bir DAG L1.", b"qwen"], Some(&t));
        let v2 = sohbet_hash_v2(&t, NET, 7, "AIDAG nedir?", "Bir DAG L1.", "qwen", "");
        assert_ne!(v1, v2);
        for s in [hex::encode(t), format!("0x{}", hex::encode(t)), hex::encode(t).to_uppercase()] {
            let a = dogrulama_adaylari(NET, &istek(7, Some("qwen"), Some(s))).unwrap();
            assert_eq!(a.iter().map(|x| (x.sema, x.hash)).collect::<Vec<_>>(), vec![(SEMA_V2, v2), (SEMA_V1, v1)]);
        }
        // sema zorlama
        let mut r = istek(7, Some("qwen"), Some(hex::encode(t)));
        r.sema = Some("tuzlu-v1".into());
        assert_eq!(dogrulama_adaylari(NET, &r).unwrap().iter().map(|x| x.sema).collect::<Vec<_>>(), vec![SEMA_V1]);
        r.sema = Some("TUZLU-V2".into());
        assert_eq!(dogrulama_adaylari(NET, &r).unwrap().iter().map(|x| x.sema).collect::<Vec<_>>(), vec![SEMA_V2]);
        // yanlis tuz -> farkli hash
        let a = dogrulama_adaylari(NET, &istek(7, Some("qwen"), Some(hex::encode(yeni_tuz())))).unwrap();
        assert!(a.iter().all(|x| x.hash != v1 && x.hash != v2));
        // tuzlu kayit tuzsuz dogrulanamaz
        let a = dogrulama_adaylari(NET, &istek(7, Some("qwen"), None)).unwrap();
        assert!(a.iter().all(|x| x.hash != v1 && x.hash != v2));
    }

    #[test]
    fn dogrulama_gecersiz_girdiyi_reddeder() {
        assert!(dogrulama_adaylari(NET, &istek(7, Some("m"), Some("zz".into()))).is_err());
        assert!(dogrulama_adaylari(NET, &istek(7, Some("m"), Some("ab".repeat(31)))).is_err());
        assert!(dogrulama_adaylari(NET, &istek(7, Some("m"), Some("ab".repeat(33)))).is_err());
        // tuzlu kayitta model zorunlu
        assert!(dogrulama_adaylari(NET, &istek(7, None, Some("ab".repeat(32)))).is_err());
        // bilinmeyen sema / tutarsiz sema-salt
        let mut r = istek(7, Some("m"), Some("ab".repeat(32)));
        r.sema = Some("v3".into());
        assert!(dogrulama_adaylari(NET, &r).is_err());
        r.sema = Some(SEMA_ESKI.into());
        assert!(dogrulama_adaylari(NET, &r).is_err());
        let mut r = istek(7, Some("m"), None);
        r.sema = Some(SEMA_V2.into());
        assert!(dogrulama_adaylari(NET, &r).is_err());
    }

    // ── BULGU 1: alan-ayraci enjeksiyonu ──

    /// SALDIRI (v1): mesru kayit prompt="P\x1eQ", answer="A". Saldirgan ayni baytlari
    /// prompt="P", answer="Q\x1eA" diye boler -> v1 hash'i BIREBIR ayni (eski acik).
    #[test]
    fn v1_ayrac_enjeksiyonu_hash_ayni_ama_belirsiz_doner() {
        let t = [3u8; 32];
        let mesru = kanit_hash(NET, 9, &[b"P\x1eQ", b"A", b"m"], Some(&t));
        let saldiri = kanit_hash(NET, 9, &[b"P", b"Q\x1eA", b"m"], Some(&t));
        assert_eq!(mesru, saldiri, "v1 zayifligi (referans): ayni hash");
        // Dogrulama: saldirganin bolunmesi v1 adayi olarak AYNI hash'i uretir ama belirsiz.
        let r = DogrulaIstek {
            ts: 9, prompt: "P".into(), answer: "Q\u{1e}A".into(), model: Some("m".into()),
            salt: Some(hex::encode(t)), proof_hash: None, sema: None, context: None,
        };
        let v1 = tek(NET, &r, SEMA_V1);
        assert_eq!(v1.hash, mesru);
        assert!(v1.belirsiz.is_some());
        // Zincirde KUBRA imzali kayitli olsa bile dogrulandi=false
        let kubra = "11".repeat(20);
        let z = ZincirBilgi { kayitli: true, kaydeden: Some(kubra.clone()), zaman: None };
        let adaylar = dogrulama_adaylari(NET, &r).unwrap();
        let sonuc: Vec<_> = adaylar.into_iter().map(|a| {
            let zz = if a.hash == mesru { z.clone() } else { ZincirBilgi::default() };
            (a, zz)
        }).collect();
        let k = dogrulama_karari(&sonuc, &kubra, None, false);
        assert_eq!(k["dogrulandi"], false, "{k}");
        assert_eq!(k["belirsiz"], true);
        assert_eq!(k["sema"], SEMA_V1);
        assert_eq!(k["zincirde"], true);
        assert!(k["sebep"].as_str().unwrap().contains("0x1e"));
        // ESKI (tuzsuz) semada da ayni kural; model alaninda ayrac da yakalanir.
        let mut r2 = r;
        r2.salt = None;
        r2.prompt = "P".into(); r2.answer = "A".into(); r2.model = Some("m\u{1e}x".into());
        assert!(tek(NET, &r2, SEMA_ESKI).belirsiz.is_some());
        // eski-akis (modelsiz) <-> eski-ask (modelli) gecisi: answer'da ayrac -> belirsiz
        let r3 = DogrulaIstek { ts: 9, prompt: "P".into(), answer: "A\u{1e}m".into(), model: None,
            salt: None, proof_hash: None, sema: None, context: None };
        let a3 = tek(NET, &r3, SEMA_ESKI);
        assert_eq!(a3.hash, kanit_hash(NET, 9, &[b"P", b"A", b"m"], None));
        assert!(a3.belirsiz.is_some());
    }

    /// v2: ayni bolunme saldirisi FARKLI hash uretir (enjeksiyon yok).
    #[test]
    fn v2_bolunme_tektir() {
        let t = [3u8; 32];
        let h = |p: &str, a: &str, m: &str, c: &str| sohbet_hash_v2(&t, NET, 9, p, a, m, c);
        let mesru = h("P\u{1e}Q", "A", "m", "");
        for (p, a, m, c) in [
            ("P", "Q\u{1e}A", "m", ""), ("P\u{1e}Q\u{1e}A", "", "m", ""), ("P\u{1e}Q", "A\u{1e}m", "", ""),
            ("P\u{1e}Q", "A", "", "m"), ("P\u{1e}Q", "", "Am", ""),
        ] {
            assert_ne!(mesru, h(p, a, m, c), "{p:?}|{a:?}|{m:?}|{c:?}");
        }
        // uzunluk oneki: "ab"+"c" != "a"+"bc" (ayrac olmadan da)
        assert_ne!(h("ab", "c", "m", ""), h("a", "bc", "m", ""));
        // tur ayrimi: sohbet kaydi medya kaydi olarak okunamaz
        assert_ne!(
            kanit_hash_v2(&t, NET, 9, TUR_SOHBET, &[b"p", b"w", b"x"]),
            kanit_hash_v2(&t, NET, 9, TUR_MEDYA, &[b"p", b"w", b"x"])
        );
        // alan sayisi hash'e girer
        assert_ne!(
            kanit_hash_v2(&t, NET, 9, TUR_SOHBET, &[b"p", b""]),
            kanit_hash_v2(&t, NET, 9, TUR_SOHBET, &[b"p"])
        );
        // net_id / ts / tuz hash'e girer
        assert_ne!(mesru, sohbet_hash_v2(&t, NET + 1, 9, "P\u{1e}Q", "A", "m", ""));
        assert_ne!(mesru, sohbet_hash_v2(&t, NET, 10, "P\u{1e}Q", "A", "m", ""));
        assert_ne!(mesru, sohbet_hash_v2(&[4u8; 32], NET, 9, "P\u{1e}Q", "A", "m", ""));
        // v2 ile v1 ayni tuzla cakismaz
        assert_ne!(mesru, kanit_hash(NET, 9, &[b"P\x1eQ", b"A", b"m"], Some(&t)));
        // v2 adayi ayraca ragmen belirsiz DEGIL (bolunme tek)
        let r = DogrulaIstek { ts: 9, prompt: "P\u{1e}Q".into(), answer: "A".into(), model: Some("m".into()),
            salt: Some(hex::encode(t)), proof_hash: None, sema: None, context: None };
        let v2 = tek(NET, &r, SEMA_V2);
        assert_eq!(v2.hash, mesru);
        assert!(v2.belirsiz.is_none());
        let kubra = "22".repeat(20);
        let sonuc = vec![
            (v2, ZincirBilgi { kayitli: true, kaydeden: Some(kubra.clone()), zaman: None }),
            (tek(NET, &r, SEMA_V1), ZincirBilgi::default()),
        ];
        let k = dogrulama_karari(&sonuc, &kubra, Some(&hex::encode(mesru)), false);
        assert_eq!(k["dogrulandi"], true, "{k}");
        assert_eq!(k["sema"], SEMA_V2);
    }

    #[test]
    fn v2_sabit_vektor() {
        // Python `blake3` ile bagimsiz hesaplandi (KANIT.md'deki formul).
        let t = [7u8; 32];
        assert_eq!(hex::encode(sohbet_hash_v2(&t, NET, 1_758_650_000, "AIDAG nedir?",
            "AIDAG Chain bir DAG L1 zinciridir.", "qwen2.5-7b", "")),
            "dc13da1e1a0729d490a6182aa6de888f777f2e2ebf68f4264e010e0f5bc80e1b");
        assert_eq!(hex::encode(medya_hash_v2(&t, NET, 1_758_650_000, "kedi", Some("0xabc"), &[0, 1, 0x1e, 255])),
            "c2aaf126a2552877e7913ca6340533d59831f0f0d87b60ec9479a7037e890167");
    }

    // ── BULGU 2: istemci baglami v2 hash'ine girer ──
    #[test]
    fn v2_istemci_baglami_hasha_girer() {
        let t = [5u8; 32];
        let ile = sohbet_hash_v2(&t, NET, 1, "p", "a", "m", "BAGLAM");
        assert_ne!(ile, sohbet_hash_v2(&t, NET, 1, "p", "a", "m", ""));
        assert_ne!(ile, sohbet_hash_v2(&t, NET, 1, "p", "a", "m", "BAGLAM2"));
        let mut r = DogrulaIstek { ts: 1, prompt: "p".into(), answer: "a".into(), model: Some("m".into()),
            salt: Some(hex::encode(t)), proof_hash: None, sema: None, context: Some("BAGLAM".into()) };
        assert_eq!(tek(NET, &r, SEMA_V2).hash, ile);
        r.context = None;
        assert_ne!(tek(NET, &r, SEMA_V2).hash, ile, "baglamsiz dogrulama baglamli kaydi TUTMAZ");
    }

    #[test]
    fn karar_kubra_disi_imza_ve_proof_uyusmazligi() {
        let t = [1u8; 32];
        let r = DogrulaIstek { ts: 1, prompt: "p".into(), answer: "a".into(), model: Some("m".into()),
            salt: Some(hex::encode(t)), proof_hash: None, sema: None, context: None };
        let adaylar = dogrulama_adaylari(NET, &r).unwrap();
        let kubra = "aa".repeat(20);
        let z = |k: &str| ZincirBilgi { kayitli: true, kaydeden: Some(k.to_string()), zaman: None };
        let s: Vec<_> = adaylar.iter().cloned().map(|a| (a, z(&"bb".repeat(20)))).collect();
        assert_eq!(dogrulama_karari(&s, &kubra, None, false)["dogrulandi"], false, "baska imzaci");
        let s: Vec<_> = adaylar.iter().cloned().map(|a| (a, z(&kubra))).collect();
        assert_eq!(dogrulama_karari(&s, &kubra, None, false)["dogrulandi"], true);
        assert_eq!(dogrulama_karari(&s, &kubra, Some(&"00".repeat(32)), false)["dogrulandi"], false);
        // v1 kaydinda context verildi -> uyari
        let s1: Vec<_> = adaylar.iter().filter(|a| a.sema == SEMA_V1).cloned().map(|a| (a, z(&kubra))).collect();
        assert!(dogrulama_karari(&s1, &kubra, None, true).get("uyari").is_some());
    }
}
