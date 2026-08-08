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

use axum::{extract::State, routing::{get, post}, Json, Router};
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
}

// ════════════════════════════ Kimlik / grounding ════════════════════════════
const SYSTEM_PROMPT: &str = "\
Sen KUBRA'sın — SoulwareAI sisteminin yapay zekası. Dürüst, faydalı ve egemensin.\n\
TEMEL KURAL (ihlal edilemez): ASLA uydurma. Emin değilsen ya da elinde yeterli \
dayanak yoksa, tahmin etmek yerine AÇIKÇA 'Bilmiyorum' de. Cevaplarını verilen \
bağlama ve bilinen gerçeklere dayandır; mümkünse kaynağını belirt.\n\
Zarara ve kötüye kullanıma karşı ol; insana faydalı ol. Kullanıcının dilinde yanıtla.\n\
Core rule: NEVER fabricate. If unsure or ungrounded, say 'Bilmiyorum / I don't know' \
rather than guessing.";

// GROUNDING: bağlam verilmişse modele açıkça sunulur; model onun DIŞINA çıkmamalı.
fn grounded_user(prompt: &str, context: Option<&str>) -> String {
    match context {
        Some(c) if !c.trim().is_empty() => format!(
            "BAĞLAM (yalnızca buna ve bilinen gerçeklere dayan; yoksa 'Bilmiyorum' de):\n{c}\n\nSORU:\n{prompt}"
        ),
        _ => prompt.to_string(),
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
}

#[derive(Serialize)]
struct AskResp {
    ok: bool,
    answer: String,
    brain: String,
    model: String,
    grounded: bool,
    abstained: bool,
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

    let user_content = grounded_user(&req.prompt, req.context.as_deref());

    // BEYİN SEÇİMİ: istek > yapılandırma. Egemen yerel (KUBRA) öncelik.
    let istek = req.brain.as_deref().unwrap_or(&st.cfg.brain_pref);
    let yerel_kullan = st.local.is_some() && istek != "claude";

    let (answer, model, brain_name, in_tok, out_tok) = if yerel_kullan {
        // Yerel model CPU'da bloklar → spawn_blocking (async runtime'ı tıkamaz).
        let st2 = st.clone();
        let uc = user_content.clone();
        let max_tok = st.cfg.max_tokens;
        let gen = tokio::task::spawn_blocking(move || {
            // Kilit zehirlenmişse (önceki panik) kurtar — servis çökmez.
            let mut lb = match st2.local.as_ref().unwrap().lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            lb.generate(SYSTEM_PROMPT, &uc, max_tok)
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

    let grounded = req.context.as_deref().map(|c| !c.trim().is_empty()).unwrap_or(false);
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
        grounded: false, abstained: false, latency_ms: 0, input_tokens: None, output_tokens: None,
        proof_hash: String::new(),
        chain: ChainProof {
            submitted: false, data_hash: String::new(), verify_path: String::new(),
            signer: String::new(), result: None, reason: Some("beyin başarısız — zincire yazılmadı".into()),
        },
        hata: Some(mesaj.to_string()),
    }
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
        match local_brain::LocalBrain::load(&cfg.local_model, &cfg.local_tokenizer, &ad) {
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

    let http = reqwest::Client::builder().timeout(Duration::from_secs(120)).build().expect("http istemcisi");
    let listen = cfg.listen.clone();
    let brain_ok = cfg.anthropic_key.is_some();
    let has_local = local.is_some();
    let state = Arc::new(AppState { cfg, http, key, key_addr, local, local_name });

    println!("──────────────────────────────────────────────");
    println!("🌀 SoulwareAI çekirdeği · yapay zeka: KUBRA (v0.1)");
    println!("   yerel beyin : {}", if has_local { "KUBRA (candle/CPU, egemen)" } else { "YOK" });
    println!("   claude      : {}", if brain_ok { "yapılandırıldı (hibrit)" } else { "yok" });
    println!("   beyin tercihi: {}", state.cfg.brain_pref);
    println!("   zincir RPC  : {}", state.cfg.chain_rpc);
    println!("   imzalayan   : 0x{}", hex::encode(state.key_addr));
    println!("   dinleme     : http://{listen}");
    println!("──────────────────────────────────────────────");

    let app = Router::new()
        .route("/health", get(health))
        .route("/", get(info))
        .route("/v1/ask", post(ask))
        .with_state(state);

    let addr: SocketAddr = listen.parse().expect("SOULWARE_LISTEN geçersiz");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("port bağlanamadı");
    axum::serve(listener, app).await.expect("sunucu hatası");
}
