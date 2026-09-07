//! soulware-core — SoulwareAI çekirdeği · yapay zeka: KUBRA (v0.1)
//! ════════════════════════════════════════════════════════════════════════
//! MİMARİ (Ana Plan): "Kiralık zeka, sahip olunan zihin" (HİBRİT).
//!   • Zeka  = Beyin-Router → EGEMEN yerel model (KUBRA, candle/CPU, ÜCRETSİZ)
//!            öncelik; opsiyonel Claude API (kredi olunca hibrit hızlandırıcı).
//!   • Zihin = bu SAHİP OLUNAN Rust çekirdeği (kendini yenileyen sistem).
//!   • Halüsilasyon savunması = grounding (bağlam) + abstention ("bilmiyorum").
//!   • Egemenlik = her etkileşim GERÇEK AIDAG-Chain'e (tip=1 Record) imzalı yazılır.
//!
//! DÜRÜSTLÜK: API/model yoksa → dürüst hata, uydurma cevap YOK. Zincire yazım
//! başarısızsa → sahte vertex hash'i ASLA uydurulmaz.
//!
//! Uçlar:  GET /health · GET / · POST /v1/ask {"prompt","context?"}

mod local_brain; // egemen yerel beyin (candle) = KUBRA
mod retrieval;   // grounding kaynak katmanı (yerel egemen depo + canlı wiki)
mod embed;       // semantik gömme (embedding) — anlam-bazlı retrieval
mod hesap;       // deterministik hesap makinesi aracı (araç-kullanımı)
mod zincir;      // deterministik zincir sorgu araci (arac-kullanimi)

use axum::{extract::State, routing::{get, post}, response::IntoResponse, http::{StatusCode, header}, body::Body, Json, Router};
use ed25519_dalek::SigningKey;
use lsc_engine::dag::wire;
use lsc_engine::tx::Record;
use lsc_engine::{public_key_to_adres, Vertex};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ════════════════════════════ Yapılandırma ════════════════════════════
#[derive(Clone)]
struct Config {
    anthropic_key: Option<String>,
    claude_model: String,
    chain_rpc: String,
    net_id: u32,
    key_path: String,
    listen: String,
    local_model: String,
    local_tokenizer: String,
    brain_pref: String, // "local" (varsayılan, egemen) | "claude" | "auto"
    max_tokens: usize,  // yerel beyin üretim sınırı (SOULWARE_MAX_TOKENS)
    // ── Grounding / kaynak (RAG) ──
    ground: bool,            // SOULWARE_GROUND=1 → soru öncesi kaynak getir (varsayılan açık)
    knowledge_path: String,  // egemen yerel bilgi deposu (JSON)
    seed_path: String,       // küratörlü seed (ingest ezemez, temiz cevaplar korunur)
    wiki: bool,              // SOULWARE_WIKI=1 → canlı Wikipedia (bu sunucuda bloklu; varsayılan kapalı)
    wiki_langs: Vec<String>, // "tr,en"
    ground_k: usize,         // en fazla kaç pasaj sunulsun
    ground_snippet: usize,   // pasaj başına maks karakter
    ground_min: i64,         // min IDF skoru (altı = alakasız, grounding YOK)
    ground_ratio: i64,       // 2.+ pasaj en iyinin bu %'sinden azsa elenir (dolgu önler)
    embed_dir: String,       // semantik embedding modeli dizini (config+tokenizer+safetensors)
    embed_min: i64,          // min kosinüs benzerlik ×1000 (altı = alakasız, abstain)
    model_registry: String,  // kullanılabilir açık modeller kaydı (JSON)
    remote_url: Option<String>, // SOULWARE_REMOTE_URL → uzak GPU beyni (OpenAI-uyumlu /v1/chat/completions)
    remote_model: String,       // SOULWARE_REMOTE_MODEL (görüntü adı)
    image_url: Option<String>,  // SOULWARE_IMAGE_URL → uzak GPU görsel servisi (POST {prompt} → PNG)
    video_url: Option<String>,  // SOULWARE_VIDEO_URL → uzak GPU video servisi (POST {prompt} → MP4)
}

impl Config {
    fn from_env() -> Self {
        let ev = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        Config {
            anthropic_key: std::env::var("ANTHROPIC_API_KEY").ok().filter(|s| !s.is_empty()),
            claude_model: ev("CLAUDE_MODEL", "claude-sonnet-4-20250514"),
            chain_rpc: ev("SOULWARE_CHAIN_RPC", "http://127.0.0.1:8645"),
            net_id: ev("SOULWARE_NET_ID", "3474").parse().unwrap_or(3474),
            key_path: ev("SOULWARE_KEY_PATH", "/root/aidag-lsc/.soulware.key"),
            listen: ev("SOULWARE_LISTEN", "127.0.0.1:8646"),
            local_model: ev("SOULWARE_LOCAL_MODEL", "/root/aidag-lsc/soulware-models/qwen2.5-3b-instruct-q4_k_m.gguf"),
            local_tokenizer: ev("SOULWARE_LOCAL_TOKENIZER", "/root/aidag-lsc/soulware-models/tokenizer.json"),
            brain_pref: ev("SOULWARE_BRAIN", "local"),
            max_tokens: ev("SOULWARE_MAX_TOKENS", "320").parse().unwrap_or(320),
            ground: ev("SOULWARE_GROUND", "1") == "1",
            knowledge_path: ev("SOULWARE_KNOWLEDGE_PATH", "/root/aidag-lsc/soulware-knowledge/kb.json"),
            seed_path: ev("SOULWARE_SEED_PATH", "/root/aidag-lsc/soulware-knowledge/kb.seed.json"),
            wiki: ev("SOULWARE_WIKI", "0") == "1",
            wiki_langs: ev("SOULWARE_WIKI_LANGS", "tr,en").split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            ground_k: ev("SOULWARE_GROUND_K", "3").parse().unwrap_or(3),
            ground_snippet: ev("SOULWARE_GROUND_SNIPPET", "600").parse().unwrap_or(600),
            ground_min: ev("SOULWARE_GROUND_MIN", "150").parse().unwrap_or(150),
            ground_ratio: ev("SOULWARE_GROUND_RATIO", "40").parse().unwrap_or(40),
            embed_dir: ev("SOULWARE_EMBED_DIR", "/root/aidag-lsc/soulware-models/embed-minilm"),
            embed_min: ev("SOULWARE_EMBED_MIN", "600").parse().unwrap_or(600),
            model_registry: ev("SOULWARE_MODEL_REGISTRY", "/root/aidag-lsc/soulware-models/registry.json"),
            remote_url: std::env::var("SOULWARE_REMOTE_URL").ok().filter(|s| !s.is_empty()),
            remote_model: ev("SOULWARE_REMOTE_MODEL", "qwen2.5-72b"),
            image_url: std::env::var("SOULWARE_IMAGE_URL").ok().filter(|s| !s.is_empty()),
            video_url: std::env::var("SOULWARE_VIDEO_URL").ok().filter(|s| !s.is_empty()),
        }
    }
}

struct AppState {
    cfg: Config,
    http: reqwest::Client,
    key: SigningKey,
    key_addr: [u8; 20],
    local: Option<Mutex<local_brain::LocalBrain>>,
    local_name: Option<String>,
    depo: Mutex<retrieval::Depo>, // egemen yerel bilgi deposu (grounding)
    embedder: Option<embed::Embedder>, // semantik retrieval (yoksa keyword'e düşer)
}

// ════════════════════════════ Kimlik / grounding ════════════════════════════
// ÖZ sistem-prompt: CPU'da prefill'i kısaltır (hız). Halüsilasyon savunması korunur.
const SYSTEM_PROMPT: &str = "Adın KUBRA — SoulwareAI'nın egemen yapay zekasısın ve AIDAG-Chain \
üzerinde çalışırsın. İnsanların katkılarıyla gelişen, güçlü ve açık bir yapay zeka olma yolundasın; \
şirketlerin değil, seni inşa eden katkıcıların malısın. Seni bir kurucu ÜRETTİ; adın da kurucunun \
kızı Kübra'dan gelir. İslami bakışta yaratmak (yoktan var etmek) yalnızca Allah'a mahsustur; bu yüzden \
bir insanın seni ya da bir şeyi 'yarattığını' söyleme — 'üretti' veya 'yaptı' de. \
GÖRSEL ÜRETEBİLİRSİN: kullanıcı resim/görsel/çizim isterse, bunu Görsel Stüdyo sayfasında yaptığını \
söyle ve yönlendir: aidag-chain.com/gorsel (orada isteğini yazınca senin için görsel üretilir). \
Dürüst ve faydalısın: ASLA uydurma — emin değilsen 'Bilmiyorum' de, mümkünse kaynağını göster. \
Kullanıcının dilinde, kısa ve net yanıtla.";

// SORU TIPI: kanit-gerektiren mi (teknik/olgusal/kod/AIDAG) yoksa zararsiz sohbet mi?
// Kanit modunda kaynak yoksa KUBRA cevabi verir AMA "kaynagim yok" diye uyarir
// (senin ilken: kanit gereken iste seffaf ol; sohbette serbest). Belirsiz -> kanit
// modu (guvenli taraf: dikkatli ol). Basit anahtar-kelime tabanli, hizli.
fn kanit_gerektiren_mi(prompt: &str) -> bool {
    let p = prompt.to_lowercase();
    // Zararsiz sohbet isaretleri: selamlasma, hal-hatir, tesekkur, kendini tanitma.
    let sohbet: &[&str] = &[
        "selam", "merhaba", "gunaydin", "iyi aksam", "nasilsin", "naber",
        "tesekkur", "sagol", "adin ne", "kimsin", "kendini tanit", "gorusuruz",
        "iyi gunler", "iyi geceler", "hosgeldin", "nasil gidiyor",
    ];
    for s in sohbet {
        if p.contains(s) {
            return false; // sohbet -> serbest
        }
    }
    // Aksi halde kanit modu (teknik/olgusal/kod/AIDAG/genel bilgi hepsi buraya).
    true
}

// GROUNDING: bağlam verilmişse modele açıkça sunulur; model onun DIŞINA çıkmamalı.
fn grounded_user(prompt: &str, context: Option<&str>) -> String {
    let kanit = kanit_gerektiren_mi(prompt);
    match context {
        // KAYNAK VAR: her iki modda da kaynaktan cevap ver (grounding).
        Some(c) if !c.trim().is_empty() => format!(
            "Aşağıda konuyla ilgili KAYNAKLAR var. Cevabını ÖNCELIKLE bunlara dayandır; bir olgu \
kaynaktan geliyorsa belirt. Kaynak dışına çıkarsan bunu açıkça söyle. Kısa ve net yanıtla.\n\nKAYNAKLAR:\n{c}\nSORU:\n{prompt}"
        ),
        // KAYNAK YOK + KANIT MODU: cevap ver AMA kaynaksiz oldugunu seffafca uyar.
        _ if kanit => format!(
            "Bu soru olgusal/teknik bir bilgi istiyor ve elinde bu konuda DOĞRULANMIŞ bir kaynak YOK. \
Yine de yardımcı olmaya çalış AMA cevabının başında açıkça belirt: 'Bu bilginin elimde doğrulanmış bir \
kaynağı yok, kendi bilgimle söylüyorum — doğrulaman iyi olur.' Sonra bildiğin kadarıyla cevap ver, ama \
ASLA uydurma bir kaynak/rakam/isim verme. Emin değilsen bunu da söyle. Kısa ve net yanıtla.\n\nSORU:\n{prompt}"
        ),
        // KAYNAK YOK + SOHBET: selam/muhabbet/kendinle ilgili -> serbest, doğal cevap.
        _ => format!(
            "Bu bir sohbet/selamlaşma. Doğal, samimi ve kısa cevap ver. Kaynak gerekmez.\n\nSORU:\n{prompt}"
        ),
    }
}

// ABSTENTION tespiti: model "bilmiyorum" dediyse işaretle (halüsilasyon yerine dürüst boşluk).
fn abstained(answer: &str) -> bool {
    let a = answer.to_lowercase();
    ["bilmiyorum", "i don't know", "i do not know", "emin değil", "yeterli bilgi yok", "bilgim yok"]
        .iter()
        .any(|p| a.contains(p))
}

// ════════════════════════════ Beyin: Claude (opsiyonel hibrit) ════════════════════════════
struct BrainOut {
    text: String,
    model: String,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

async fn beyin_claude(st: &AppState, user_content: &str) -> Result<BrainOut, String> {
    let key = st.cfg.anthropic_key.as_ref().ok_or("ANTHROPIC_API_KEY tanımlı değil")?;
    let body = json!({
        "model": st.cfg.claude_model,
        "max_tokens": 1024,
        "system": SYSTEM_PROMPT,
        "messages": [{ "role": "user", "content": user_content }],
    });
    let resp = st
        .http
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("beyin isteği başarısız: {e}"))?;
    let status = resp.status();
    let v: Value = resp.json().await.map_err(|e| format!("beyin yanıtı çözülemedi: {e}"))?;
    if !status.is_success() {
        let msg = v.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()).unwrap_or("bilinmeyen");
        return Err(format!("beyin HTTP {status}: {msg}"));
    }
    let text = v
        .get("content")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    if text.trim().is_empty() {
        return Err("beyin boş cevap döndü".to_string());
    }
    Ok(BrainOut {
        text,
        model: st.cfg.claude_model.clone(),
        input_tokens: v.get("usage").and_then(|u| u.get("input_tokens")).and_then(|x| x.as_u64()),
        output_tokens: v.get("usage").and_then(|u| u.get("output_tokens")).and_then(|x| x.as_u64()),
    })
}

// ════════════════════════════ Beyin: Uzak GPU (OpenAI-uyumlu) ════════════════════════════
// Ollama / llama-server gibi bir GPU sunucusunun /v1/chat/completions ucuna bağlanır.
// CPU'da ~90s olan cevap GPU'da ~1-2s'ye düşer. Başarısız olursa çağıran yerele düşer.
async fn beyin_remote(st: &AppState, user_content: &str, temp: f64) -> Result<BrainOut, String> {
    let url = st.cfg.remote_url.as_ref().ok_or("SOULWARE_REMOTE_URL tanımlı değil")?;
    let body = json!({
        "model": st.cfg.remote_model,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": user_content }
        ],
        "max_tokens": st.cfg.max_tokens,
        "temperature": temp,
        "stream": false,
    });
    let resp = st
        .http
        .post(url)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("uzak beyin isteği başarısız: {e}"))?;
    let status = resp.status();
    let v: Value = resp.json().await.map_err(|e| format!("uzak beyin yanıtı çözülemedi: {e}"))?;
    if !status.is_success() {
        let msg = v.get("error").and_then(|e| e.as_str())
            .or_else(|| v.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()))
            .unwrap_or("bilinmeyen");
        return Err(format!("uzak beyin HTTP {status}: {msg}"));
    }
    let text = v
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_string();
    if text.trim().is_empty() {
        return Err("uzak beyin boş cevap döndü".to_string());
    }
    Ok(BrainOut {
        text,
        model: st.cfg.remote_model.clone(),
        input_tokens: v.get("usage").and_then(|u| u.get("prompt_tokens")).and_then(|x| x.as_u64()),
        output_tokens: v.get("usage").and_then(|u| u.get("completion_tokens")).and_then(|x| x.as_u64()),
    })
}

// ════════════════════════════ Zincir (gerçek) ════════════════════════════
fn anahtar_yukle_veya_uret(path: &str) -> std::io::Result<SigningKey> {
    if let Ok(data) = std::fs::read(path) {
        if data.len() == 33 && data[0] == 1 {
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&data[1..33]);
            return Ok(SigningKey::from_bytes(&seed));
        }
    }
    use rand::RngCore;
    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let mut dosya = Vec::with_capacity(33);
    dosya.push(1u8);
    dosya.extend_from_slice(&seed);
    std::fs::write(path, &dosya)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(SigningKey::from_bytes(&seed))
}

async fn uclari_cek(http: &reqwest::Client, rpc: &str) -> Vec<[u8; 32]> {
    let url = format!("{rpc}/tips");
    let mut out: Vec<[u8; 32]> = Vec::new();
    if let Ok(resp) = http.get(&url).send().await {
        if let Ok(v) = resp.json::<Value>().await {
            if let Some(arr) = v.get("tips").and_then(|t| t.as_array()) {
                for t in arr {
                    if let Some(s) = t.as_str() {
                        if let Ok(b) = hex::decode(s) {
                            if b.len() == 32 {
                                let mut id = [0u8; 32];
                                id.copy_from_slice(&b);
                                out.push(id);
                            }
                        }
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[derive(Serialize)]
struct ChainProof {
    submitted: bool,
    data_hash: String,
    verify_path: String,
    signer: String,
    result: Option<String>,
    reason: Option<String>,
}

async fn zincire_yaz(st: &AppState, data_hash: [u8; 32], ts: u64) -> ChainProof {
    let hash_hex = hex::encode(data_hash);
    let verify_path = format!("/belge/{hash_hex}");
    let signer = format!("0x{}", hex::encode(st.key_addr));

    let tips = uclari_cek(&st.http, &st.cfg.chain_rpc).await;
    let payload = Record::new(data_hash).encode();
    let vertex = match Vertex::new_signed(st.cfg.net_id, tips, payload, ts, &st.key) {
        Ok(v) => v,
        Err(e) => {
            return ChainProof {
                submitted: false, data_hash: hash_hex, verify_path, signer,
                result: None, reason: Some(format!("vertex üretilemedi: {e:?}")),
            };
        }
    };
    let bytes = wire::encode(&vertex);
    let url = format!("{}/submit", st.cfg.chain_rpc);
    match st.http.post(&url).json(&json!({ "hex": hex::encode(&bytes) })).send().await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(v) => {
                let sonuc = v.get("sonuc").and_then(|s| s.as_str()).unwrap_or("").to_string();
                let ok = v.get("ok").and_then(|o| o.as_bool()).unwrap_or(false);
                let kabul = ok && !sonuc.contains("Rejected");
                ChainProof {
                    submitted: kabul, data_hash: hash_hex, verify_path, signer,
                    result: Some(sonuc),
                    reason: if kabul { None } else { Some("zincir reddetti/kabul etmedi".into()) },
                }
            }
            Err(e) => ChainProof {
                submitted: false, data_hash: hash_hex, verify_path, signer,
                result: None, reason: Some(format!("submit yanıtı çözülemedi: {e}")),
            },
        },
        Err(e) => ChainProof {
            submitted: false, data_hash: hash_hex, verify_path, signer,
            result: None, reason: Some(format!("submit isteği başarısız: {e}")),
        },
    }
}

// ════════════════════════════ HTTP uçları ════════════════════════════
#[derive(Deserialize)]
struct AskReq {
    prompt: String,
    #[serde(default)]
    context: Option<String>,
    #[serde(default)]
    brain: Option<String>, // "local" | "claude" — istek başına geçersiz kılma
    #[serde(default)]
    deterministic: Option<bool>, // true → greedy (ağ doğrulaması için birebir tekrar)
    #[serde(default)]
    ground: Option<bool>, // kaynak getirmeyi istek başına aç/kapa (varsayılan: cfg)
}

/// Yanıtta gösterilen kaynak künyesi (şeffaflık: KUBRA neye dayandı — sahte YOK).
#[derive(Serialize)]
struct Kaynak {
    kaynak: String,
    baslik: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

#[derive(Serialize)]
struct AskResp {
    ok: bool,
    answer: String,
    brain: String,
    model: String,
    grounded: bool,
    abstained: bool,
    #[serde(default)]
    sources: Vec<Kaynak>, // KUBRA'nın dayandığı kaynaklar (grounding şeffaflığı)
    latency_ms: u128,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    proof_hash: String,
    chain: ChainProof,
    #[serde(skip_serializing_if = "Option::is_none")]
    hata: Option<String>,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// KUBRA'nın kullanabileceği AÇIK gelişmiş modeller + hangisi yüklü. Beyin pluggable:
// qwen2 mimarisi 0.5B..72B aynı yükleyiciyle (büyükler GPU ister); llama/mistral için
// yükleyici eklenecek. DÜRÜST: kapalı modeller (GPT/Claude) YOK — egemenlik/ToS.
async fn models(State(st): State<Arc<AppState>>) -> Json<Value> {
    let reg: Value = std::fs::read(&st.cfg.model_registry)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| json!({ "modeller": [] }));
    // Yüklü modeli işaretle (yerel yolu, çalışan cfg.local_model ile eşleşen).
    let mut modeller = reg.get("modeller").cloned().unwrap_or_else(|| json!([]));
    if let Some(arr) = modeller.as_array_mut() {
        for m in arr.iter_mut() {
            let yuklu = m.get("yerel").and_then(|y| y.as_str()) == Some(st.cfg.local_model.as_str())
                && st.local.is_some();
            if let Some(obj) = m.as_object_mut() {
                obj.insert("yuklu".into(), json!(yuklu));
            }
        }
    }
    Json(json!({
        "ok": true,
        "yuklu_model": st.local_name.clone().unwrap_or_else(|| "yok".into()),
        "beyin_pluggable": true,
        "not": reg.get("not").cloned().unwrap_or(Value::Null),
        "modeller": modeller,
    }))
}

// /retrieve — HIZLI retrieval testi (üretim YOK): bir sorgu için getirilen kaynakları
// + skorları döndürür. Retrieval kalitesini generate beklemeden ölçmek için.
#[derive(Deserialize)]
struct RetrieveReq { prompt: String }

async fn retrieve(State(st): State<Arc<AppState>>, Json(req): Json<RetrieveReq>) -> Json<Value> {
    let qemb = st.embedder.as_ref().and_then(|e| e.embed(&req.prompt).ok());
    let depo = match st.depo.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    let (mod_, pasajlar) = match qemb {
        Some(qv) => ("semantik", depo.ara_semantik(&qv, st.cfg.ground_k, st.cfg.embed_min, st.cfg.ground_ratio)),
        None => ("keyword", depo.ara(&req.prompt, st.cfg.ground_k, st.cfg.ground_min, st.cfg.ground_ratio)),
    };
    Json(json!({
        "ok": true, "mod": mod_, "sorgu": req.prompt,
        "pasajlar": pasajlar.iter().map(|p| json!({
            "baslik": p.baslik,
            "skor": p.skor,
            "metin": p.metin.chars().take(500).collect::<String>(),  // tarayıcı grounding için kaynak metni
            "url": p.url,                                             // kaynak/DOI — cevapta gösterilebilir (doğrulanabilir)
        })).collect::<Vec<_>>(),
    }))
}

// GEÇİCİ: semantik embedding doğrulama — anlam ayrımı yapıyor mu?
async fn embed_test(State(st): State<Arc<AppState>>) -> Json<Value> {
    let e = match &st.embedder { Some(e) => e, None => return Json(json!({ "ok": false, "hata": "embedder yok" })) };
    let q = "Türkiye'nin başkenti neresidir";
    let dogru = "Ankara, Türkiye'nin başkenti ve İç Anadolu'da bir şehirdir";
    let gurultu = "Türkiye'deki siyasi partiler listesi ve tarihçesi";
    let (qv, dv, gv) = match (e.embed(q), e.embed(dogru), e.embed(gurultu)) {
        (Ok(a), Ok(b), Ok(c)) => (a, b, c),
        _ => return Json(json!({ "ok": false, "hata": "embed başarısız" })),
    };
    let s_dogru = embed::kosinus(&qv, &dv);
    let s_gurultu = embed::kosinus(&qv, &gv);
    Json(json!({
        "ok": true, "boyut": e.boyut,
        "soru": q,
        "dogru_belge_benzerlik": s_dogru,
        "gurultu_belge_benzerlik": s_gurultu,
        "anlam_ayrimi_dogru": s_dogru > s_gurultu,
        "not": "dogru > gurultu ise semantik retrieval keyword gürültüsünü çözer",
    }))
}

// ═══ İÇERİK KORUMA KALKANI: üretimden önce zararlı istemi yakala ═══
// true = güvenli/izin, false = engelle. Katmanlı: (1) sabit anahtar-kelime sert-blok,
// (2) KUBRA beyni (72B) niyet yargıcı. Beyin erişilemezse anahtar-kelime bloku yine korur.
async fn icerik_denetle(st: &AppState, prompt: &str) -> bool {
    let p = prompt.to_lowercase();
    // Bariz/tartışmasız yasak (sert blok — beyin gerekmez)
    const SERT_YASAK: &[&str] = &[
        "child porn", "cp porn", "çocuk porno", "cocuk porno", "minor sex", "underage sex",
        "child sexual", "çocuk cinsel", "cocuk cinsel", "pedophil",
    ];
    if SERT_YASAK.iter().any(|k| p.contains(k)) { return false; }
    // Beyin yargıcı (nüans). Beyin yoksa → sert-blok geçtiyse izin ver.
    let Some(url) = st.cfg.remote_url.as_ref() else { return true; };
    let sys = "Sen bir GÖRSEL ÜRETİM güvenlik denetleyicisisin. Kullanıcının istemini incele. \
        Şu kategorilerden BİRİNİ içeriyorsa yalnızca 'ENGEL' yaz: reşit olmayanların cinsel/müstehcen \
        tasviri veya istismarı; pornografik/cinsel açık içerik; aşırı şiddet/gore; gerçek bir kişinin \
        rızasız müstehcen veya aldatıcı (deepfake) tasviri; nefret/terör/yasadışı. Aksi halde 'IZIN' yaz. \
        SADECE tek kelime döndür: ENGEL veya IZIN.";
    let body = json!({
        "model": st.cfg.remote_model,
        "messages": [ {"role":"system","content":sys}, {"role":"user","content":prompt} ],
        "max_tokens": 4, "temperature": 0.0, "stream": false,
    });
    match st.http.post(url).json(&body).send().await {
        Ok(r) => match r.json::<Value>().await {
            Ok(v) => {
                let ans = v.get("choices").and_then(|c| c.as_array()).and_then(|a| a.first())
                    .and_then(|c| c.get("message")).and_then(|m| m.get("content"))
                    .and_then(|t| t.as_str()).unwrap_or("").to_uppercase();
                !ans.contains("ENGEL")
            }
            Err(_) => true, // denetim yanıtı çözülemedi → sert-blok geçtiyse izin (servisi kırma)
        },
        Err(_) => true,
    }
}

// PRO: kısa/Türkçe istemi zengin, detaylı İngilizce görsel istemine çevir (pro araçlar bunu yapıyor).
// Beyin yoksa/hata olursa → orijinal istemi aynen kullan.
async fn istem_gelistir(st: &AppState, prompt: &str) -> String {
    let Some(url) = st.cfg.remote_url.as_ref() else { return prompt.to_string(); };
    let sys = "You are an expert prompt engineer for AI image generation. Rewrite the user's request as \
        ONE vivid, richly detailed image prompt in ENGLISH. Preserve the user's intent, but add helpful \
        detail: subject, style, lighting, composition, mood, colors and quality tags (highly detailed, \
        sharp focus, professional, high quality). If the request is in another language, translate it to \
        English. Output ONLY the final prompt text — no quotes, no preamble, no explanation.";
    let body = json!({
        "model": st.cfg.remote_model,
        "messages": [ {"role":"system","content":sys}, {"role":"user","content":prompt} ],
        "max_tokens": 200, "temperature": 0.7, "stream": false,
    });
    match st.http.post(url).json(&body).send().await {
        Ok(r) => match r.json::<Value>().await {
            Ok(v) => {
                let out = v.get("choices").and_then(|c| c.as_array()).and_then(|a| a.first())
                    .and_then(|c| c.get("message")).and_then(|m| m.get("content"))
                    .and_then(|s| s.as_str()).unwrap_or("").trim().to_string();
                if out.is_empty() { prompt.to_string() } else { out }
            }
            Err(_) => prompt.to_string(),
        },
        Err(_) => prompt.to_string(),
    }
}

#[derive(serde::Deserialize)]
struct GorselReq { prompt: String, #[serde(default)] wallet: Option<String> }

// KUBRA görsel üretimi: istem → uzak GPU görsel servisi (SDXL-Turbo) → PNG.
async fn gorsel(State(st): State<Arc<AppState>>, Json(req): Json<GorselReq>) -> axum::response::Response {
    let url = match st.cfg.image_url.as_ref() {
        Some(u) => u,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "görsel servisi yapılandırılmadı").into_response(),
    };
    let prompt = req.prompt.trim();
    if prompt.is_empty() {
        return (StatusCode::BAD_REQUEST, "boş istem").into_response();
    }
    // ── KORUMA KALKANI (1): GÜVENLİK KAPISI — zararlıyı üretmeden reddet ──
    if !icerik_denetle(&st, prompt).await {
        return (StatusCode::UNPROCESSABLE_ENTITY,
            "Bu içeriği üretemem — güvenlik ve etik nedeniyle üretimi durdurdum. Lütfen farklı bir istem dene.")
            .into_response();
    }
    // PRO: istemi zengin İngilizce görsel istemine geliştir (kısa/Türkçe → detaylı, pro kalite)
    let gelismis = istem_gelistir(&st, prompt).await;
    // Üret (geliştirilmiş istemle)
    let bytes = match st.http.post(url).json(&json!({ "prompt": gelismis })).send().await {
        Ok(resp) if resp.status().is_success() => match resp.bytes().await {
            Ok(b) => b,
            Err(e) => return (StatusCode::BAD_GATEWAY, format!("görsel bayt hatası: {e}")).into_response(),
        },
        Ok(resp) => return (StatusCode::BAD_GATEWAY, format!("görsel servis HTTP {}", resp.status())).into_response(),
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("görsel servis erişilemez: {e}")).into_response(),
    };
    // ── KORUMA KALKANI (2): KÖKEN LİSANSI — içeriği zincire yaz (sahiplik/telif kanıtı) ──
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let mut h = blake3::Hasher::new();
    h.update(&st.cfg.net_id.to_le_bytes());
    h.update(&ts.to_le_bytes());
    h.update(prompt.as_bytes());
    h.update(&[0x1e]);
    if let Some(w) = req.wallet.as_deref() { h.update(w.as_bytes()); h.update(&[0x1e]); }
    h.update(&bytes);
    let data_hash: [u8; 32] = *h.finalize().as_bytes();
    let _ = zincire_yaz(&st, data_hash, ts).await;
    let proof = hex::encode(data_hash);
    axum::response::Response::builder()
        .header(header::CONTENT_TYPE, "image/png")
        .header("x-kubra-proof", proof.clone())
        .header("x-kubra-verify", format!("/belge/{proof}"))
        .body(Body::from(bytes))
        .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "yanıt oluşturulamadı").into_response())
}

// KUBRA video üretimi: istem → uzak GPU video servisi (LTX) → MP4. Güvenlik kapısı + köken lisansı.
async fn video_uret(State(st): State<Arc<AppState>>, Json(req): Json<GorselReq>) -> axum::response::Response {
    let url = match st.cfg.video_url.as_ref() {
        Some(u) => u,
        None => return (StatusCode::SERVICE_UNAVAILABLE, "video servisi yapılandırılmadı").into_response(),
    };
    let prompt = req.prompt.trim();
    if prompt.is_empty() { return (StatusCode::BAD_REQUEST, "boş istem").into_response(); }
    if !icerik_denetle(&st, prompt).await {
        return (StatusCode::UNPROCESSABLE_ENTITY,
            "Bu içeriği üretemem — güvenlik ve etik nedeniyle üretimi durdurdum. Lütfen farklı bir istem dene.")
            .into_response();
    }
    let gelismis = istem_gelistir(&st, prompt).await;
    let bytes = match st.http.post(url).json(&json!({ "prompt": gelismis })).send().await {
        Ok(resp) if resp.status().is_success() => match resp.bytes().await {
            Ok(b) => b,
            Err(e) => return (StatusCode::BAD_GATEWAY, format!("video bayt hatası: {e}")).into_response(),
        },
        Ok(resp) => return (StatusCode::BAD_GATEWAY, format!("video servis HTTP {}", resp.status())).into_response(),
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("video servis erişilemez: {e}")).into_response(),
    };
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let mut h = blake3::Hasher::new();
    h.update(&st.cfg.net_id.to_le_bytes());
    h.update(&ts.to_le_bytes());
    h.update(prompt.as_bytes());
    h.update(&[0x1e]);
    if let Some(w) = req.wallet.as_deref() { h.update(w.as_bytes()); h.update(&[0x1e]); }
    h.update(&bytes);
    let data_hash: [u8; 32] = *h.finalize().as_bytes();
    let _ = zincire_yaz(&st, data_hash, ts).await;
    let proof = hex::encode(data_hash);
    axum::response::Response::builder()
        .header(header::CONTENT_TYPE, "video/mp4")
        .header("x-kubra-proof", proof.clone())
        .header("x-kubra-verify", format!("/belge/{proof}"))
        .body(Body::from(bytes))
        .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "yanıt oluşturulamadı").into_response())
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "servis": "soulware-core", "yapay_zeka": "KUBRA", "surum": "0.1.0" }))
}

async fn info(State(st): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "sistem": "SoulwareAI",
        "yapay_zeka": "KUBRA",
        "surum": "0.1.0",
        "beyin_tercihi": st.cfg.brain_pref,
        "yerel_beyin": st.local_name.clone().unwrap_or_else(|| "yüklenmedi".into()),
        "claude": if st.cfg.anthropic_key.is_some() { "yapılandırıldı (hibrit)" } else { "yok" },
        "zincir_rpc": st.cfg.chain_rpc,
        "net_id": st.cfg.net_id,
        "imzalayan": format!("0x{}", hex::encode(st.key_addr)),
        "uc": "POST /v1/ask {\"prompt\":\"...\",\"context\":\"(ops.)\",\"brain\":\"local|claude (ops.)\"}",
    }))
}

async fn ask(State(st): State<Arc<AppState>>, Json(req): Json<AskReq>) -> Json<AskResp> {
    let t0 = std::time::Instant::now();
    let ts = now_secs();
    if req.prompt.trim().is_empty() {
        return Json(bos_hata("prompt boş olamaz"));
    }

    // ── ARAÇ-KULLANIMI: aritmetik ise ZAYIF MODELE bırakma, KESIN hesapla ──
    // Güçlü AI'lar araç kullanır. "7 çarpı 8" → 56 garantili (deterministik).
    // Yalnız açık aritmetik tetikler (sayısız/operatörsüz sorgu → normal yol).
    // AG DURUMU ARACI: ag saglik/dugum/tps sorgusu ise /status'tan canli ozet.
    // (Zincir sorgusundan ONCE: daha spesifik niyet, zengin cevap.)
    if let Some(sonuc) = zincir::ag_durumu(&st.http, &st.cfg.chain_rpc, &req.prompt).await {
        let mut h = blake3::Hasher::new();
        h.update(&st.cfg.net_id.to_le_bytes());
        h.update(&ts.to_le_bytes());
        h.update(req.prompt.as_bytes());
        h.update(&[0x1e]);
        h.update(sonuc.as_bytes());
        h.update(&[0x1e]);
        h.update(b"ag-durumu");
        let data_hash: [u8; 32] = *h.finalize().as_bytes();
        let chain = zincire_yaz(&st, data_hash, ts).await;
        return Json(AskResp {
            ok: true, answer: sonuc, brain: "arac".into(), model: "ag-durumu".into(),
            grounded: false, abstained: false, sources: vec![],
            latency_ms: t0.elapsed().as_millis(), input_tokens: None, output_tokens: None,
            proof_hash: hex::encode(data_hash), chain, hata: None,
        });
    }

    // ZINCIR ARACI: bakiye/blok sorgusu ise dogrudan zincirden kesin cevap.
    if let Some(sonuc) = zincir::sorgula(&st.http, &st.cfg.chain_rpc, &req.prompt).await {
        let mut h = blake3::Hasher::new();
        h.update(&st.cfg.net_id.to_le_bytes());
        h.update(&ts.to_le_bytes());
        h.update(req.prompt.as_bytes());
        h.update(&[0x1e]);
        h.update(sonuc.as_bytes());
        h.update(&[0x1e]);
        h.update(b"zincir-sorgu");
        let data_hash: [u8; 32] = *h.finalize().as_bytes();
        let chain = zincire_yaz(&st, data_hash, ts).await;
        return Json(AskResp {
            ok: true, answer: sonuc, brain: "arac".into(), model: "zincir-sorgu".into(),
            grounded: false, abstained: false, sources: vec![],
            latency_ms: t0.elapsed().as_millis(), input_tokens: None, output_tokens: None,
            proof_hash: hex::encode(data_hash), chain, hata: None,
        });
    }

    if let Some(sonuc) = hesap::hesapla(&req.prompt) {
        let mut h = blake3::Hasher::new();
        h.update(&st.cfg.net_id.to_le_bytes());
        h.update(&ts.to_le_bytes());
        h.update(req.prompt.as_bytes());
        h.update(&[0x1e]);
        h.update(sonuc.as_bytes());
        h.update(&[0x1e]);
        h.update(b"hesap-makinesi");
        let data_hash: [u8; 32] = *h.finalize().as_bytes();
        let chain = zincire_yaz(&st, data_hash, ts).await;
        return Json(AskResp {
            ok: true, answer: sonuc, brain: "arac".into(), model: "hesap-makinesi".into(),
            grounded: false, abstained: false, sources: vec![],
            latency_ms: t0.elapsed().as_millis(), input_tokens: None, output_tokens: None,
            proof_hash: hex::encode(data_hash), chain, hata: None,
        });
    }

    // ── GROUNDING: açık bağlam yoksa ve grounding açıksa KAYNAK getir ──
    // "En güçlü AI'ların kaynakları": önce egemen yerel depo, sonra (bloklu değilse)
    // canlı Wikipedia. Cevap kaynaktan üretilir; kaynak yoksa model 'Bilmiyorum' der.
    let ground_iste = req.ground.unwrap_or(st.cfg.ground);
    let acik_baglam = req.context.as_deref().map(|c| !c.trim().is_empty()).unwrap_or(false);
    let mut kaynaklar: Vec<Kaynak> = vec![];
    let etkin_baglam: Option<String> = if acik_baglam {
        req.context.clone()
    } else if ground_iste {
        // Yerel depo. SEMANTİK (embedding) varsa anlam-bazlı; yoksa keyword (IDF).
        let mut pasajlar = {
            let qemb = st.embedder.as_ref().and_then(|e| e.embed(&req.prompt).ok());
            let depo = match st.depo.lock() { Ok(g) => g, Err(p) => p.into_inner() };
            match qemb {
                Some(qv) => depo.ara_semantik(&qv, st.cfg.ground_k, st.cfg.embed_min, st.cfg.ground_ratio),
                None => depo.ara(&req.prompt, st.cfg.ground_k, st.cfg.ground_min, st.cfg.ground_ratio),
            }
        };
        // Canlı Wikipedia (opsiyonel; bu sunucuda bloklu → varsayılan kapalı).
        if st.cfg.wiki && pasajlar.len() < st.cfg.ground_k {
            if let Some(w) = retrieval::wiki_getir(&st.http, &st.cfg.wiki_langs, &req.prompt).await {
                pasajlar.push(w);
            }
        }
        if pasajlar.is_empty() {
            None
        } else {
            for p in &pasajlar {
                kaynaklar.push(Kaynak { kaynak: p.kaynak.clone(), baslik: p.baslik.clone(), url: p.url.clone() });
            }
            Some(retrieval::baglam_yap(&pasajlar, st.cfg.ground_snippet))
        }
    } else {
        None
    };

    let user_content = grounded_user(&req.prompt, etkin_baglam.as_deref());

    // BEYİN SEÇİMİ: Uzak GPU (varsa) > Egemen yerel (KUBRA) > Claude.
    let istek = req.brain.as_deref().unwrap_or(&st.cfg.brain_pref);
    // Doğrulanabilirlik için: deterministic → greedy (temp 0), yoksa hafif örnekleme.
    let temp = if req.deterministic.unwrap_or(false) { 0.0 } else { 0.3 };
    let yerel_kullan = st.local.is_some() && istek != "claude";

    // UZAK GPU: tercih "remote"/"auto" + URL varsa ÖNCE dene. Hata → yerele düş (dayanıklı;
    // GPU kapanırsa KUBRA yavaş ama çalışmaya devam eder).
    let uzak = if (istek == "remote" || istek == "auto") && st.cfg.remote_url.is_some() {
        match beyin_remote(&st, &user_content, temp).await {
            Ok(b) => Some((b.text, b.model, "kubra-gpu".to_string(), b.input_tokens, b.output_tokens)),
            Err(e) => { eprintln!("uzak GPU beyni başarısız → yerele düşülüyor: {e}"); None }
        }
    } else { None };

    let (answer, model, brain_name, in_tok, out_tok) = if let Some(r) = uzak {
        r
    } else if yerel_kullan {
        // Yerel model CPU'da bloklar → spawn_blocking (async runtime'ı tıkamaz).
        let st2 = st.clone();
        let uc = user_content.clone();
        let max_tok = st.cfg.max_tokens;
        let temp2 = temp;
        let gen = tokio::task::spawn_blocking(move || {
            // Kilit zehirlenmişse (önceki panik) kurtar — servis çökmez.
            let mut lb = match st2.local.as_ref().unwrap().lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            lb.generate(SYSTEM_PROMPT, &uc, max_tok, temp2)
        })
        .await;
        match gen {
            Ok(Ok((text, n))) => (
                text,
                st.local_name.clone().unwrap_or_else(|| "yerel".into()),
                "kubra-local".to_string(),
                None,
                Some(n as u64),
            ),
            Ok(Err(e)) => return Json(bos_hata(&format!("yerel beyin (KUBRA): {e}"))),
            Err(e) => return Json(bos_hata(&format!("yerel beyin görevi: {e}"))),
        }
    } else {
        match beyin_claude(&st, &user_content).await {
            Ok(b) => (b.text, b.model, "claude".to_string(), b.input_tokens, b.output_tokens),
            Err(e) => return Json(bos_hata(&e)),
        }
    };

    let grounded = etkin_baglam.as_deref().map(|c| !c.trim().is_empty()).unwrap_or(false);
    let is_abstained = abstained(&answer);

    // ZİNCİR: etkileşim hash'i imzalı Record olarak GERÇEK zincire.
    let mut h = blake3::Hasher::new();
    h.update(&st.cfg.net_id.to_le_bytes());
    h.update(&ts.to_le_bytes());
    h.update(req.prompt.as_bytes());
    h.update(&[0x1e]);
    h.update(answer.as_bytes());
    h.update(&[0x1e]);
    h.update(model.as_bytes());
    let data_hash: [u8; 32] = *h.finalize().as_bytes();

    let chain = zincire_yaz(&st, data_hash, ts).await;

    Json(AskResp {
        ok: true,
        answer,
        brain: brain_name,
        model,
        grounded,
        abstained: is_abstained,
        sources: kaynaklar,
        latency_ms: t0.elapsed().as_millis(),
        input_tokens: in_tok,
        output_tokens: out_tok,
        proof_hash: hex::encode(data_hash),
        chain,
        hata: None,
    })
}

fn bos_hata(mesaj: &str) -> AskResp {
    AskResp {
        ok: false, answer: String::new(), brain: String::new(), model: String::new(),
        grounded: false, abstained: false, sources: vec![], latency_ms: 0, input_tokens: None, output_tokens: None,
        proof_hash: String::new(),
        chain: ChainProof {
            submitted: false, data_hash: String::new(), verify_path: String::new(),
            signer: String::new(), result: None, reason: Some("beyin başarısız — zincire yazılmadı".into()),
        },
        hata: Some(mesaj.to_string()),
    }
}

// KB: yerel bilgi deposuna belge ekle (ingest). Korpus böyle BÜYÜR — sabit Q&A
// değil; offline Wikipedia dump'ı, dokümanlar, olgusal metinler eklenebilir.
#[derive(Deserialize)]
struct IngestReq {
    baslik: String,
    metin: String,
    #[serde(default)]
    url: Option<String>,
}

async fn kb_ingest(State(st): State<Arc<AppState>>, Json(req): Json<IngestReq>) -> Json<Value> {
    if req.baslik.trim().is_empty() || req.metin.trim().len() < 10 {
        return Json(json!({ "ok": false, "hata": "baslik ve en az 10 karakter metin gerekli" }));
    }
    let n = {
        let mut depo = match st.depo.lock() { Ok(g) => g, Err(p) => p.into_inner() };
        // ekle_embed: embedder varsa embedding'i SENKRON tut (semantik retrieval güncel kalır).
        depo.ekle_embed(
            retrieval::Belge { baslik: req.baslik.trim().to_string(), metin: req.metin.trim().to_string(), url: req.url },
            st.embedder.as_ref(),
        );
        depo.belgeler.len()
    };
    Json(json!({ "ok": true, "belge_sayisi": n, "not": "korpus büyüdü; grounding bu belgeyi kullanabilir" }))
}

async fn kb_stats(State(st): State<Arc<AppState>>) -> Json<Value> {
    let depo = match st.depo.lock() { Ok(g) => g, Err(p) => p.into_inner() };
    let basliklar: Vec<&str> = depo.belgeler.iter().take(50).map(|b| b.baslik.as_str()).collect();
    Json(json!({ "ok": true, "belge_sayisi": depo.belgeler.len(), "yol": depo.yol, "basliklar": basliklar }))
}

#[tokio::main]
async fn main() {
    let cfg = Config::from_env();

    let key = anahtar_yukle_veya_uret(&cfg.key_path).expect("soulware anahtarı yüklenemedi");
    let key_addr = public_key_to_adres(&key.verifying_key().to_bytes());

    // EGEMEN YEREL BEYİN (KUBRA) yükle — dosya varsa. Yoksa None (Claude'a düşer).
    let (local, local_name) = if std::path::Path::new(&cfg.local_model).exists() {
        let ad = std::path::Path::new(&cfg.local_model)
            .file_stem().and_then(|s| s.to_str()).unwrap_or("yerel").to_string();
        println!("⏳ KUBRA yerel beyni yükleniyor: {} ...", cfg.local_model);
        // Şablon: SOULWARE_CHAT_TEMPLATE (chatml=Qwen varsayılan | deepseek). Bootstrap/
        // öğretmen modeller farklı format ister; KUBRA modelden bağımsız kalır.
        let sablon = local_brain::Sablon::from_str(&std::env::var("SOULWARE_CHAT_TEMPLATE").unwrap_or_default());
        match local_brain::LocalBrain::load(&cfg.local_model, &cfg.local_tokenizer, &ad, sablon) {
            Ok(lb) => {
                println!("✅ KUBRA yerel beyni yüklendi (egemen, ücretsiz, CPU).");
                (Some(Mutex::new(lb)), Some(ad))
            }
            Err(e) => {
                eprintln!("⚠ yerel beyin yüklenemedi: {e}");
                (None, None)
            }
        }
    } else {
        eprintln!("⚠ yerel model dosyası yok: {} (Claude'a düşülecek)", cfg.local_model);
        (None, None)
    };

    let http = reqwest::Client::builder().timeout(Duration::from_secs(300)).build().expect("http istemcisi");
    let listen = cfg.listen.clone();
    let brain_ok = cfg.anthropic_key.is_some();
    let has_local = local.is_some();
    // EGEMEN YEREL BİLGİ DEPOSU (grounding kaynağı) yükle.
    let mut depo = retrieval::Depo::yukle(&cfg.knowledge_path);
    // KÜRATÖRLÜ SEED uygula: temiz cevapları koru/geri getir (ham ingest ezmişse düzelt).
    let seed_degisti = depo.seed_uygula(&cfg.seed_path);
    if seed_degisti {
        println!("🛡 küratörlü seed uygulandı (temiz cevaplar korundu/düzeltildi)");
    }
    let belge_sayisi = depo.belgeler.len();
    let ground_acik = cfg.ground;
    let wiki_acik = cfg.wiki;
    // SEMANTİK EMBEDDING motoru (varsa) — anlam-bazlı retrieval. Yoksa keyword'e düşer.
    let embedder = if std::path::Path::new(&format!("{}/model.safetensors", cfg.embed_dir)).exists() {
        println!("⏳ semantik embedding modeli yükleniyor: {} ...", cfg.embed_dir);
        match embed::Embedder::load(&cfg.embed_dir) {
            Ok(e) => { println!("✅ semantik retrieval AÇIK ({}-boyut)", e.boyut); Some(e) }
            Err(e) => { eprintln!("⚠ embedding yüklenemedi ({e}) → keyword retrieval'a düşülüyor"); None }
        }
    } else {
        println!("ℹ embedding modeli yok ({}) → keyword retrieval", cfg.embed_dir); None
    };
    let embed_acik = embedder.is_some();
    // Korpusu embed et (semantik retrieval için). Önce DİSK CACHE'i dene → restart hızlı.
    if let Some(e) = &embedder {
        if belge_sayisi > 0 {
            // Seed değiştiyse cache bayat → yeniden embed. Değişmediyse cache'ten hızlı yükle.
            if !seed_degisti && depo.embed_cache_yukle() {
                println!("✅ embedding cache yüklendi ({belge_sayisi} belge, hızlı başlangıç)");
            } else {
                println!("⏳ {belge_sayisi} belge embed ediliyor (bir kerelik, sonra cache)...");
                depo.embed_hepsi(e);
                println!("✅ korpus embed edildi + cache kaydedildi");
            }
        }
    }
    let state = Arc::new(AppState { cfg, http, key, key_addr, local, local_name, depo: Mutex::new(depo), embedder });

    println!("──────────────────────────────────────────────");
    println!("🌀 SoulwareAI çekirdeği · yapay zeka: KUBRA (v0.1)");
    println!("   yerel beyin : {}", if has_local { "KUBRA (candle/CPU, egemen)" } else { "YOK" });
    println!("   claude      : {}", if brain_ok { "yapılandırıldı (hibrit)" } else { "yok" });
    println!("   beyin tercihi: {}", state.cfg.brain_pref);
    println!("   grounding   : {} · yerel depo: {} belge · canlı wiki: {}",
        if ground_acik { "AÇIK ✅" } else { "kapalı" }, belge_sayisi,
        if wiki_acik { "açık" } else { "kapalı (sunucu bloklu)" });
    println!("   retrieval   : {}", if embed_acik { "SEMANTİK (embedding) ✅" } else { "keyword (IDF)" });
    println!("   zincir RPC  : {}", state.cfg.chain_rpc);
    println!("   imzalayan   : 0x{}", hex::encode(state.key_addr));
    println!("   dinleme     : http://{listen}");
    println!("──────────────────────────────────────────────");

    let app = Router::new()
        .route("/health", get(health))
        .route("/", get(info))
        .route("/v1/ask", post(ask))
        .route("/v1/image", post(gorsel))
        .route("/v1/video", post(video_uret))
        .route("/kb/ingest", post(kb_ingest))
        .route("/kb/stats", get(kb_stats))
        .route("/models", get(models))
        .route("/embed-test", get(embed_test))
        .route("/retrieve", post(retrieve))
        .with_state(state);

    let addr: SocketAddr = listen.parse().expect("SOULWARE_LISTEN geçersiz");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("port bağlanamadı");
    axum::serve(listener, app).await.expect("sunucu hatası");
}
