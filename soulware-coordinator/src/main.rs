//! soulware-coordinator — SoulwareAI hesaplama & ödül koordinatörü (v0.1)
//! ════════════════════════════════════════════════════════════════════════
//! Faz 0'ın kalbi: katkıcı worker'lara iş dağıtır, YEDEKLİ DOĞRULAMA yapar
//! (aynı iş ≥2 worker'a → çıktılar birebir eşleşirse doğrulanır), ve doğrulanan
//! işi GERÇEK AIDAG-Chain'e imzalı ÖDÜL KAYDI (tip=1 Record) olarak yazar.
//!
//! DÜRÜST: Ödül kaydı = zincirde doğrulanabilir KAZANÇ KANITI. Gerçek LSC bakiye
//! ödemesi (settlement) ayrı, owner-onaylı bir adımdır — burada sahte ödeme YOK.
//!
//! İZİN/RIZA: Worker (indirilen KUBRA istemcisi) ağa YALNIZCA kullanıcının açık
//! izniyle katılır (istemci tarafı consent kutusu). Koordinatör kaydolan worker'ı
//! rızalı kabul eder.
//!
//! Uçlar:
//!   POST /job/create       {"prompt","deterministic?"}         → {job_id}
//!   POST /worker/register   {"wallet"}                          → {ok}
//!   GET  /worker/poll/:wallet                                   → iş | {none:true}
//!   POST /worker/submit     {"wallet","job_id","answer"}        → doğrulama/ödül
//!   GET  /status                                                → özet

use axum::{extract::{Path, State}, routing::{get, post}, Json, Router};
use ed25519_dalek::SigningKey;
use lsc_engine::dag::wire;
use lsc_engine::tx::Record;
use lsc_engine::{public_key_to_adres, Vertex};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

// ════════════════════════════ Yapılandırma ════════════════════════════
#[derive(Clone)]
struct Config {
    chain_rpc: String,
    net_id: u32,
    key_path: String,
    listen: String,
    reward_lsc: u64,     // doğrulanan iş başına ödül (LSC, tam sayı)
    redundancy: usize,   // eşleşme için gereken worker sayısı (varsayılan 2)
    max_assign: usize,   // bir işe en fazla kaç worker (tiebreak için, varsayılan 3)
}
impl Config {
    fn from_env() -> Self {
        let ev = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        Config {
            chain_rpc: ev("SOULWARE_CHAIN_RPC", "http://127.0.0.1:8645"),
            net_id: ev("SOULWARE_NET_ID", "3474").parse().unwrap_or(3474),
            key_path: ev("SOULWARE_COORD_KEY", "/root/aidag-lsc/.soulware-coordinator.key"),
            listen: ev("SOULWARE_COORD_LISTEN", "127.0.0.1:8647"),
            reward_lsc: ev("SOULWARE_REWARD_LSC", "1").parse().unwrap_or(1),
            redundancy: ev("SOULWARE_REDUNDANCY", "2").parse().unwrap_or(2),
            max_assign: ev("SOULWARE_MAX_ASSIGN", "3").parse().unwrap_or(3),
        }
    }
}

// ════════════════════════════ Durum modeli ════════════════════════════
#[derive(Clone, Serialize)]
struct Worker {
    wallet: String,
    reputation: i64,
    earned_lsc: u64,
    jobs_done: u64,
    registered_at: u64,
}

#[derive(Clone, Serialize)]
struct WorkResult {
    worker: String,
    answer: String,
    hash: String, // blake3(answer) hex — hızlı eşleşme
    at: u64,
}

#[derive(Clone, Serialize)]
struct RewardRec {
    worker: String,
    amount_lsc: u64,
    proof_hash: String,
    chain_ok: bool,
}

#[derive(Clone, Serialize)]
struct Job {
    id: u64,
    prompt: String,
    deterministic: bool,
    status: String, // "pending" | "verified" | "disputed"
    assigned: Vec<String>,
    results: Vec<WorkResult>,
    verified_answer: Option<String>,
    rewards: Vec<RewardRec>,
    created_at: u64,
}

struct Coord {
    cfg: Config,
    http: reqwest::Client,
    key: SigningKey,
    key_addr: [u8; 20],
    workers: HashMap<String, Worker>,
    jobs: HashMap<u64, Job>,
    next_job: u64,
}

type St = Arc<Mutex<Coord>>;

// ════════════════════════════ Zincir: imzalı ödül kaydı ════════════════════════════
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
    let mut out: Vec<[u8; 32]> = Vec::new();
    if let Ok(resp) = http.get(format!("{rpc}/tips")).send().await {
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

/// Ödül kazanç kanıtını tip=1 Record olarak GERÇEK zincire yaz. Sahte hash YOK.
/// Döner: (chain_ok, proof_hash_hex).
async fn odul_zincire(
    http: &reqwest::Client, rpc: &str, net_id: u32, key: &SigningKey,
    coord_addr: &[u8; 20], worker_wallet: &str, job_id: u64, amount: u64, ts: u64,
) -> (bool, String) {
    // Kanonik kazanç dizesi → blake3 → 32 bayt
    let canon = format!(
        "soulware-reward|coord=0x{}|worker={}|job={}|lsc={}|ts={}",
        hex::encode(coord_addr), worker_wallet, job_id, amount, ts
    );
    let data_hash: [u8; 32] = *blake3::hash(canon.as_bytes()).as_bytes();
    let proof = hex::encode(data_hash);

    let tips = uclari_cek(http, rpc).await;
    let payload = Record::new(data_hash).encode();
    let vertex = match Vertex::new_signed(net_id, tips, payload, ts, key) {
        Ok(v) => v,
        Err(_) => return (false, proof),
    };
    let bytes = wire::encode(&vertex);
    match http.post(format!("{rpc}/submit")).json(&json!({ "hex": hex::encode(&bytes) })).send().await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(v) => {
                let ok = v.get("ok").and_then(|o| o.as_bool()).unwrap_or(false);
                let sonuc = v.get("sonuc").and_then(|s| s.as_str()).unwrap_or("");
                (ok && !sonuc.contains("Rejected"), proof)
            }
            Err(_) => (false, proof),
        },
        Err(_) => (false, proof),
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ════════════════════════════ Uçlar ════════════════════════════
#[derive(Deserialize)]
struct CreateJob { prompt: String, #[serde(default)] deterministic: Option<bool> }

async fn job_create(State(st): State<St>, Json(req): Json<CreateJob>) -> Json<Value> {
    if req.prompt.trim().is_empty() {
        return Json(json!({ "ok": false, "hata": "prompt boş" }));
    }
    let mut c = st.lock().await;
    let id = c.next_job;
    c.next_job += 1;
    // Doğrulama için varsayılan deterministic=true (yedekli çıktılar eşleşsin).
    let det = req.deterministic.unwrap_or(true);
    c.jobs.insert(id, Job {
        id, prompt: req.prompt, deterministic: det, status: "pending".into(),
        assigned: vec![], results: vec![], verified_answer: None, rewards: vec![], created_at: now_secs(),
    });
    Json(json!({ "ok": true, "job_id": id, "deterministic": det }))
}

#[derive(Deserialize)]
struct Reg { wallet: String }

async fn worker_register(State(st): State<St>, Json(req): Json<Reg>) -> Json<Value> {
    let w = req.wallet.trim().to_lowercase();
    if w.is_empty() {
        return Json(json!({ "ok": false, "hata": "wallet boş" }));
    }
    let mut c = st.lock().await;
    c.workers.entry(w.clone()).or_insert_with(|| Worker {
        wallet: w.clone(), reputation: 0, earned_lsc: 0, jobs_done: 0, registered_at: now_secs(),
    });
    Json(json!({ "ok": true, "wallet": w, "rıza": "GPU katkısı yalnızca istemci onayıyla" }))
}

async fn worker_poll(State(st): State<St>, Path(wallet): Path<String>) -> Json<Value> {
    let w = wallet.trim().to_lowercase();
    let mut c = st.lock().await;
    if !c.workers.contains_key(&w) {
        return Json(json!({ "none": true, "hata": "önce kaydol (/worker/register)" }));
    }
    let max_assign = c.cfg.max_assign;
    // Bu worker'a atanmamış, hâlâ yayında (pending) ve kapasitesi dolmamış bir iş bul.
    let mut secilen: Option<(u64, String, bool)> = None;
    let mut ids: Vec<u64> = c.jobs.keys().copied().collect();
    ids.sort();
    for id in ids {
        if let Some(j) = c.jobs.get(&id) {
            if j.status == "pending" && !j.assigned.contains(&w) && j.assigned.len() < max_assign {
                secilen = Some((id, j.prompt.clone(), j.deterministic));
                break;
            }
        }
    }
    match secilen {
        Some((id, prompt, det)) => {
            if let Some(j) = c.jobs.get_mut(&id) {
                j.assigned.push(w.clone());
            }
            Json(json!({ "job_id": id, "prompt": prompt, "deterministic": det }))
        }
        None => Json(json!({ "none": true })),
    }
}

#[derive(Deserialize)]
struct Submit { wallet: String, job_id: u64, answer: String }

async fn worker_submit(State(st): State<St>, Json(req): Json<Submit>) -> Json<Value> {
    let w = req.wallet.trim().to_lowercase();
    let ts = now_secs();
    let hash = hex::encode(blake3::hash(req.answer.trim().as_bytes()).as_bytes());

    // 1) Sonucu kaydet + doğrulama/karar (kilit altında, await YOK).
    //    verdict = Some((job_id, kazananlar, slashlananlar)) doğrulandıysa.
    let (verdict, cfg, coord_addr): (Option<(u64, Vec<String>, Vec<String>)>, Config, [u8; 20]) = {
        let mut c = st.lock().await;
        let cfg = c.cfg.clone();
        let coord_addr = c.key_addr;
        let redundancy = cfg.redundancy;
        let max_assign = cfg.max_assign;
        let job = match c.jobs.get_mut(&req.job_id) {
            Some(j) => j,
            None => return Json(json!({ "ok": false, "hata": "iş yok" })),
        };
        if job.status != "pending" {
            return Json(json!({ "ok": true, "durum": job.status.clone(), "not": "iş zaten kapandı" }));
        }
        if !job.assigned.contains(&w) {
            return Json(json!({ "ok": false, "hata": "bu iş sana atanmadı" }));
        }
        if job.results.iter().any(|r| r.worker == w) {
            return Json(json!({ "ok": false, "hata": "zaten gönderdin" }));
        }
        job.results.push(WorkResult { worker: w.clone(), answer: req.answer.clone(), hash: hash.clone(), at: ts });

        // Eşleşme sayımı: bir cevap-hash ≥ redundancy kez → DOĞRULANDI (o cevap doğru kabul).
        let mut sayac: HashMap<String, Vec<String>> = HashMap::new();
        for r in &job.results {
            sayac.entry(r.hash.clone()).or_default().push(r.worker.clone());
        }
        let kazanan = sayac.iter().find(|(_, ws)| ws.len() >= redundancy).map(|(h, ws)| (h.clone(), ws.clone()));
        let maxed = job.assigned.len() >= max_assign && job.results.len() >= job.assigned.len();

        if let Some((khash, kazananlar)) = kazanan {
            let ans = job.results.iter().find(|r| r.hash == khash).map(|r| r.answer.clone()).unwrap_or_default();
            // SLASH: kazanan gruptan FARKLI cevap verenler = yanlış/sahtekâr → cezalandırılır.
            let slashlananlar: Vec<String> = job.results.iter().filter(|r| r.hash != khash).map(|r| r.worker.clone()).collect();
            job.status = "verified".into();
            job.verified_answer = Some(ans);
            (Some((req.job_id, kazananlar, slashlananlar)), cfg, coord_addr)
        } else if maxed {
            // Kapasite doldu, çoğunluk eşleşmesi yok → tartışmalı (ödül yok).
            job.status = "disputed".into();
            (None, cfg, coord_addr)
        } else {
            (None, cfg, coord_addr) // daha çok sonuç bekleniyor
        }
    };

    // 2) Karar varsa: önce SLASH (kilit altı, await yok), sonra ÖDÜL (zincir, await).
    if let Some((job_id, kazananlar, slashlananlar)) = verdict {
        // SLASH: itibar düşür (sahtekâra caydırıcı). Gerçek stake yakımı ileride.
        {
            let mut c = st.lock().await;
            for l in &slashlananlar {
                if let Some(wk) = c.workers.get_mut(l) {
                    wk.reputation -= 2;
                }
            }
        }
        let key = { let c = st.lock().await; c.key.clone() };
        let http = { let c = st.lock().await; c.http.clone() };
        let mut odul_sonuc = Vec::new();
        for worker in &kazananlar {
            let (chain_ok, proof) = odul_zincire(
                &http, &cfg.chain_rpc, cfg.net_id, &key, &coord_addr, worker, job_id, cfg.reward_lsc, ts,
            ).await;
            let mut c = st.lock().await;
            if let Some(wk) = c.workers.get_mut(worker) {
                wk.earned_lsc += cfg.reward_lsc;
                wk.jobs_done += 1;
                wk.reputation += 1;
            }
            if let Some(j) = c.jobs.get_mut(&job_id) {
                j.rewards.push(RewardRec { worker: worker.clone(), amount_lsc: cfg.reward_lsc, proof_hash: proof.clone(), chain_ok });
            }
            odul_sonuc.push(json!({ "worker": worker, "lsc": cfg.reward_lsc, "chain_ok": chain_ok, "proof": proof }));
        }
        return Json(json!({
            "ok": true, "durum": "verified",
            "kazananlar": kazananlar.len(), "slashlanan": slashlananlar.len(),
            "oduller": odul_sonuc,
        }));
    }

    Json(json!({ "ok": true, "durum": "kaydedildi", "not": "doğrulama için daha çok sonuç bekleniyor" }))
}

async fn status(State(st): State<St>) -> Json<Value> {
    let c = st.lock().await;
    let mut workers: Vec<&Worker> = c.workers.values().collect();
    workers.sort_by(|a, b| b.earned_lsc.cmp(&a.earned_lsc));
    let jobs: Vec<&Job> = {
        let mut v: Vec<&Job> = c.jobs.values().collect();
        v.sort_by(|a, b| b.id.cmp(&a.id));
        v.into_iter().take(20).collect()
    };
    let toplam_odul: u64 = c.workers.values().map(|w| w.earned_lsc).sum();
    Json(json!({
        "ok": true,
        "koordinator": format!("0x{}", hex::encode(c.key_addr)),
        "worker_sayisi": c.workers.len(),
        "is_sayisi": c.jobs.len(),
        "toplam_dagitilan_lsc": toplam_odul,
        "yedeklilik": c.cfg.redundancy,
        "odul_lsc": c.cfg.reward_lsc,
        "workers": workers,
        "son_isler": jobs,
        "not": "earned_lsc = zincirde kanıtlı KAZANÇ; gerçek bakiye ödemesi ayrı owner-onaylı adım.",
    }))
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "servis": "soulware-coordinator", "surum": "0.1.0" }))
}

#[tokio::main]
async fn main() {
    let cfg = Config::from_env();
    let key = anahtar_yukle_veya_uret(&cfg.key_path).expect("koordinatör anahtarı");
    let key_addr = public_key_to_adres(&key.verifying_key().to_bytes());
    let http = reqwest::Client::builder().timeout(Duration::from_secs(30)).build().expect("http");
    let listen = cfg.listen.clone();

    println!("──────────────────────────────────────────────");
    println!("🛰  SoulwareAI Koordinatör (v0.1)");
    println!("   koordinatör : 0x{}", hex::encode(key_addr));
    println!("   zincir RPC  : {}", cfg.chain_rpc);
    println!("   yedeklilik  : {} · ödül/iş: {} LSC", cfg.redundancy, cfg.reward_lsc);
    println!("   dinleme     : http://{listen}");
    println!("──────────────────────────────────────────────");

    let st: St = Arc::new(Mutex::new(Coord {
        cfg, http, key, key_addr, workers: HashMap::new(), jobs: HashMap::new(), next_job: 1,
    }));

    let app = Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/job/create", post(job_create))
        .route("/worker/register", post(worker_register))
        .route("/worker/poll/:wallet", get(worker_poll))
        .route("/worker/submit", post(worker_submit))
        .with_state(st);

    let addr: SocketAddr = listen.parse().expect("SOULWARE_COORD_LISTEN geçersiz");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("port");
    axum::serve(listener, app).await.expect("sunucu");
}
