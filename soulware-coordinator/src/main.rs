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
use lsc_engine::tx::{ComputeReward, Record};
use lsc_engine::{public_key_to_adres, Vertex};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// LSC ondalık (10^18 wei). tip=16 ComputeReward miktarı wei cinsindendir.
const ONDALIK: u128 = 1_000_000_000_000_000_000;

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
    data_path: String,   // kalıcı durum dosyası (restart'ta kaybolmaz)
    // ── Settlement (tip=16 kontrollü LSC emisyonu) ──
    // GÜVENLİK: settle_key = faucet OWNER anahtarı. MAINNET'te bu anahtar sunucuda
    // TUTULMAZ → auto KAPALI kalır, koordinatör yalnız kuyruk biriktirir; owner
    // offline `soulware-settle` ile boşaltır. DEVNET'te bu anahtar verilip auto
    // açılarak tam otomatik döngü kanıtlanır.
    settle_key_path: Option<String>, // SOULWARE_SETTLE_KEY (yoksa auto imkânsız)
    settle_auto: bool,               // SOULWARE_SETTLE_AUTO=1 → arka plan emisyon döngüsü
    settle_interval: u64,            // SOULWARE_SETTLE_INTERVAL saniye (varsayılan 15)
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
            data_path: ev("SOULWARE_COORD_DATA", "/root/aidag-lsc/.data/soulware-coordinator.json"),
            settle_key_path: std::env::var("SOULWARE_SETTLE_KEY").ok().filter(|s| !s.trim().is_empty()),
            settle_auto: ev("SOULWARE_SETTLE_AUTO", "0") == "1",
            settle_interval: ev("SOULWARE_SETTLE_INTERVAL", "15").parse().unwrap_or(15),
        }
    }
}

// ════════════════════════════ Durum modeli ════════════════════════════
#[derive(Clone, Serialize, Deserialize)]
struct Worker {
    wallet: String,
    reputation: i64,
    earned_lsc: u64,
    jobs_done: u64,
    registered_at: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct WorkResult {
    worker: String,
    answer: String,
    hash: String, // blake3(answer) hex — hızlı eşleşme
    at: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct RewardRec {
    worker: String,
    amount_lsc: u64,
    proof_hash: String,
    chain_ok: bool,
}

/// Bekleyen SETTLEMENT: doğrulanmış kazanç → gerçek LSC emisyonu (tip=16) sırası.
/// `reward_id` node'da çifte-basım kilidi (HashSet<u64>). settled=true → zincirde
/// LSC BASILDI (owner-imzalı). tx=1 "kazanç kanıtı"ndan FARKLI: bu gerçek bakiye.
#[derive(Clone, Serialize, Deserialize)]
struct PendingSettlement {
    reward_id: u64,
    worker: String,     // 0x...40hex
    lsc: u64,           // tam LSC (wei değil; emisyonda 10^18 ile çarpılır)
    job_id: u64,
    created_at: u64,
    settled: bool,      // zincire yazıldı & kabul edildi mi
    settled_at: u64,
    #[serde(default)]
    settled_via: String, // "havuz" (tip=7, ücret-fonlu) | "emisyon" (tip=16, bootstrap)
}

#[derive(Clone, Serialize, Deserialize)]
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
    // Tüketici ücreti (kazan↔harca): bu işi açan taraf havuza ödediği LSC.
    #[serde(default)]
    fee_lsc: u64,
    #[serde(default)]
    payer: Option<String>,
}

struct Coord {
    cfg: Config,
    http: reqwest::Client,
    key: SigningKey,
    key_addr: [u8; 20],
    workers: HashMap<String, Worker>,
    jobs: HashMap<u64, Job>,
    next_job: u64,
    // ── Settlement kuyruğu (tip=16 emisyonu) ──
    settlements: Vec<PendingSettlement>,
    next_reward_id: u64,
    settle_key: Option<SigningKey>,   // faucet owner anahtarı (yalnız devnet/owner makinesi)
    settle_addr: Option<[u8; 20]>,    // owner adresi (emisyon yetkisi kimde)
}

impl Coord {
    /// Durumu diske yaz (restart'ta kaybolmasın). Atomik: önce .tmp, sonra rename.
    fn save(&self) {
        let v = json!({
            "workers": self.workers, "jobs": self.jobs, "next_job": self.next_job,
            "settlements": self.settlements, "next_reward_id": self.next_reward_id,
        });
        let p = &self.cfg.data_path;
        if let Some(dir) = std::path::Path::new(p).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = format!("{p}.tmp");
        if serde_json::to_vec_pretty(&v).ok().and_then(|b| std::fs::write(&tmp, b).ok()).is_some() {
            let _ = std::fs::rename(&tmp, p);
        }
    }
}

/// Diskten yüklenen kalıcı durum.
struct LoadedState {
    workers: HashMap<String, Worker>,
    jobs: HashMap<u64, Job>,
    next_job: u64,
    settlements: Vec<PendingSettlement>,
    next_reward_id: u64,
}

/// Diskten durumu yükle (yoksa boş). Restart sonrası worker/iş/ödül/settlement korunur.
fn load_state(path: &str) -> LoadedState {
    if let Ok(data) = std::fs::read(path) {
        if let Ok(v) = serde_json::from_slice::<Value>(&data) {
            let workers = v.get("workers").cloned().and_then(|x| serde_json::from_value(x).ok()).unwrap_or_default();
            let jobs = v.get("jobs").cloned().and_then(|x| serde_json::from_value(x).ok()).unwrap_or_default();
            let next_job = v.get("next_job").and_then(|x| x.as_u64()).unwrap_or(1);
            let settlements = v.get("settlements").cloned().and_then(|x| serde_json::from_value(x).ok()).unwrap_or_default();
            let next_reward_id = v.get("next_reward_id").and_then(|x| x.as_u64()).unwrap_or(1);
            return LoadedState { workers, jobs, next_job, settlements, next_reward_id };
        }
    }
    LoadedState { workers: HashMap::new(), jobs: HashMap::new(), next_job: 1, settlements: vec![], next_reward_id: 1 }
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

/// "0x...40hex" cüzdanı → [u8;20]. Geçersizse None.
fn cuzdan20(w: &str) -> Option<[u8; 20]> {
    let s = w.trim().trim_start_matches("0x").trim_start_matches("0X");
    let b = hex::decode(s).ok()?;
    if b.len() != 20 { return None; }
    let mut a = [0u8; 20];
    a.copy_from_slice(&b);
    Some(a)
}

/// Bir adresin zincirdeki LSC bakiyesini (wei) çek. Hata → 0.
async fn lsc_bakiye_cek(http: &reqwest::Client, rpc: &str, adres: &[u8; 20]) -> u128 {
    let url = format!("{rpc}/lsc-bakiye/{}", hex::encode(adres));
    if let Ok(resp) = http.get(url).send().await {
        if let Ok(v) = resp.json::<Value>().await {
            if let Some(s) = v.get("lsc_bakiye").and_then(|x| x.as_str()) {
                return s.parse::<u128>().unwrap_or(0);
            }
        }
    }
    0
}

/// Bir adresin bir sonraki beklenen nonce'unu çek (tip=7 için). Hata → 0.
async fn nonce_cek(http: &reqwest::Client, rpc: &str, adres: &[u8; 20]) -> u64 {
    let url = format!("{rpc}/nonce/{}", hex::encode(adres));
    if let Ok(resp) = http.get(url).send().await {
        if let Ok(v) = resp.json::<Value>().await {
            if let Some(n) = v.get("nonce").and_then(|x| x.as_u64()) {
                return n;
            }
        }
    }
    0
}

/// HAVUZDAN ÖDEME: worker'a havuzdaki GERÇEK LSC'yi tip=7 ile TAŞI (emisyon DEĞİL).
/// Havuz anahtarı (koordinatör) imzalar. Sadece var olan LSC'yi taşır — para basmaz.
/// Bu, kazan↔harca döngüsünün "kazan" ödemesidir; kaynağı tüketici ücretleridir.
async fn havuzdan_ode(
    http: &reqwest::Client, rpc: &str, net_id: u32, pool_key: &SigningKey,
    pool_addr: &[u8; 20], worker: [u8; 20], lsc_wei: u128, ts: u64,
) -> (bool, String) {
    let nonce = nonce_cek(http, rpc, pool_addr).await;
    let tips = uclari_cek(http, rpc).await;
    let payload = lsc_engine::tx::LscTransferKaydi::new(worker, lsc_wei, nonce).encode();
    let vertex = match Vertex::new_signed(net_id, tips, payload, ts, pool_key) {
        Ok(v) => v,
        Err(_) => return (false, "vertex üretilemedi".into()),
    };
    let bytes = wire::encode(&vertex);
    match http.post(format!("{rpc}/submit")).json(&json!({ "hex": hex::encode(&bytes) })).send().await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(v) => {
                let ok = v.get("ok").and_then(|o| o.as_bool()).unwrap_or(false);
                let sonuc = v.get("sonuc").and_then(|s| s.as_str()).unwrap_or("").to_string();
                (ok && !sonuc.contains("Rejected"), sonuc)
            }
            Err(_) => (false, "yanıt okunamadı".into()),
        },
        Err(_) => (false, "gönderilemedi".into()),
    }
}

/// SETTLEMENT: doğrulanmış kazancı GERÇEK LSC emisyonuna çevir (tip=16 ComputeReward).
/// Owner (faucet) anahtarıyla imzalar; node emisyon tavanı + çifte-basım kilidini
/// UYGULAR. Döner: (zincir kabul etti mi, kısa not). Sahte başarı YOK.
async fn settle_zincire(
    http: &reqwest::Client, rpc: &str, net_id: u32, settle_key: &SigningKey,
    worker: [u8; 20], lsc_tam: u64, reward_id: u64, ts: u64,
) -> (bool, String) {
    let lsc_wei = match (lsc_tam as u128).checked_mul(ONDALIK) {
        Some(v) => v,
        None => return (false, "lsc taştı".into()),
    };
    let tips = uclari_cek(http, rpc).await;
    let payload = ComputeReward::new(worker, lsc_wei, reward_id).encode();
    let vertex = match Vertex::new_signed(net_id, tips, payload, ts, settle_key) {
        Ok(v) => v,
        Err(_) => return (false, "vertex üretilemedi".into()),
    };
    let bytes = wire::encode(&vertex);
    match http.post(format!("{rpc}/submit")).json(&json!({ "hex": hex::encode(&bytes) })).send().await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(v) => {
                let ok = v.get("ok").and_then(|o| o.as_bool()).unwrap_or(false);
                let sonuc = v.get("sonuc").and_then(|s| s.as_str()).unwrap_or("").to_string();
                (ok && !sonuc.contains("Rejected"), sonuc)
            }
            Err(_) => (false, "yanıt okunamadı".into()),
        },
        Err(_) => (false, "gönderilemedi".into()),
    }
}

/// Arka plan SETTLEMENT döngüsü (kazan↔harca'nın "kazan" ödemesi).
/// Her bekleyen kazanç için ÖNCE HAVUZDAN öder (tip=7, ücret-fonlu, para basmaz);
/// havuz yetersizse VE owner anahtarı varsa EMİSYON (tip=16, sınırlı bootstrap).
/// YALNIZCA SOULWARE_SETTLE_AUTO=1 iken çalışır. Mainnet varsayılanı KAPALI.
async fn settle_loop(st: St) {
    let interval = { st.lock().await.cfg.settle_interval.max(2) };
    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        // Bekleyenleri + imza malzemesini kilit altında kopyala; await kilit dışında.
        let (bekleyen, net_id, rpc, pool_key, pool_addr, owner_key) = {
            let c = st.lock().await;
            let bek: Vec<PendingSettlement> = c.settlements.iter().filter(|s| !s.settled).cloned().collect();
            (bek, c.cfg.net_id, c.cfg.chain_rpc.clone(), c.key.clone(), c.key_addr, c.settle_key.clone())
        };
        if bekleyen.is_empty() { continue; }
        let http = { st.lock().await.http.clone() };
        for s in bekleyen {
            let w = match cuzdan20(&s.worker) { Some(w) => w, None => continue };
            let ts = now_secs();
            let need_wei = (s.lsc as u128).saturating_mul(ONDALIK);
            let pool_bal = lsc_bakiye_cek(&http, &rpc, &pool_addr).await;

            let (ok, via, note) = if pool_bal >= need_wei {
                // 1) HAVUZDAN öde (ücret-fonlu dolaşım — enflasyon yok).
                let (ok, note) = havuzdan_ode(&http, &rpc, net_id, &pool_key, &pool_addr, w, need_wei, ts).await;
                (ok, "havuz", note)
            } else if let Some(ok_key) = &owner_key {
                // 2) Havuz yetersiz → EMİSYON (bootstrap; tavanlı, owner-imzalı).
                let (ok, note) = settle_zincire(&http, &rpc, net_id, ok_key, w, s.lsc, s.reward_id, ts).await;
                (ok, "emisyon", note)
            } else {
                (false, "-", format!("havuz yetersiz ({pool_bal} wei) ve owner anahtarı yok"))
            };

            if ok {
                let mut c = st.lock().await;
                if let Some(p) = c.settlements.iter_mut().find(|p| p.reward_id == s.reward_id) {
                    p.settled = true;
                    p.settled_at = ts;
                    p.settled_via = via.to_string();
                }
                c.save();
                println!("💠 settlement OK: reward_id={} worker={} lsc={} via={}", s.reward_id, s.worker, s.lsc, via);
            } else {
                eprintln!("⚠ settlement bekliyor: reward_id={} → {}", s.reward_id, note);
            }
        }
    }
}

// ════════════════════════════ Uçlar ════════════════════════════
#[derive(Deserialize)]
struct CreateJob {
    prompt: String,
    #[serde(default)] deterministic: Option<bool>,
    #[serde(default)] fee_lsc: Option<u64>,   // tüketicinin havuza ödediği ücret (LSC)
    #[serde(default)] payer: Option<String>,  // ücreti ödeyen cüzdan (0x...)
}

async fn job_create(State(st): State<St>, Json(req): Json<CreateJob>) -> Json<Value> {
    if req.prompt.trim().is_empty() {
        return Json(json!({ "ok": false, "hata": "prompt boş" }));
    }
    let mut c = st.lock().await;
    let id = c.next_job;
    c.next_job += 1;
    // Doğrulama için varsayılan deterministic=true (yedekli çıktılar eşleşsin).
    let det = req.deterministic.unwrap_or(true);
    let fee = req.fee_lsc.unwrap_or(0);
    c.jobs.insert(id, Job {
        id, prompt: req.prompt, deterministic: det, status: "pending".into(),
        assigned: vec![], results: vec![], verified_answer: None, rewards: vec![], created_at: now_secs(),
        fee_lsc: fee, payer: req.payer.map(|p| p.trim().to_lowercase()),
    });
    c.save();
    Json(json!({ "ok": true, "job_id": id, "deterministic": det, "fee_lsc": fee,
        "not": if fee > 0 { "ücreti havuza öde: soulware-pay ile tip=7 → havuz adresi" } else { "ücretsiz (bootstrap emisyonu)" } }))
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
    c.save();
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
            c.save();
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

        let verdict = if let Some((khash, kazananlar)) = kazanan {
            let ans = job.results.iter().find(|r| r.hash == khash).map(|r| r.answer.clone()).unwrap_or_default();
            // SLASH: kazanan gruptan FARKLI cevap verenler = yanlış/sahtekâr → cezalandırılır.
            let slashlananlar: Vec<String> = job.results.iter().filter(|r| r.hash != khash).map(|r| r.worker.clone()).collect();
            job.status = "verified".into();
            job.verified_answer = Some(ans);
            Some((req.job_id, kazananlar, slashlananlar))
        } else if maxed {
            // Kapasite doldu, çoğunluk eşleşmesi yok → tartışmalı (ödül yok).
            job.status = "disputed".into();
            None
        } else {
            None // daha çok sonuç bekleniyor
        };
        c.save();
        (verdict, cfg, coord_addr)
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
            // SETTLEMENT kuyruğuna ekle: kazanç kanıtı (tip=1) yazıldı → şimdi gerçek
            // LSC emisyonu (tip=16) sıraya girer. reward_id = çifte-basım kilidi.
            let reward_id = c.next_reward_id;
            c.next_reward_id += 1;
            c.settlements.push(PendingSettlement {
                reward_id, worker: worker.clone(), lsc: cfg.reward_lsc, job_id,
                created_at: ts, settled: false, settled_at: 0, settled_via: String::new(),
            });
            odul_sonuc.push(json!({ "worker": worker, "lsc": cfg.reward_lsc, "chain_ok": chain_ok, "proof": proof, "reward_id": reward_id }));
        }
        { let c = st.lock().await; c.save(); }
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

// Tüketici: bir işin (kendi sorusunun) doğrulanmış sonucunu sorgular.
async fn job_get(State(st): State<St>, Path(id): Path<u64>) -> Json<Value> {
    let c = st.lock().await;
    match c.jobs.get(&id) {
        Some(j) => Json(json!({ "ok": true, "job": j })),
        None => Json(json!({ "ok": false, "hata": "iş yok" })),
    }
}

// Settlement kuyruğu: bekleyen (henüz LSC basılmamış) kazançlar.
async fn settlement_pending(State(st): State<St>) -> Json<Value> {
    let c = st.lock().await;
    let bekleyen: Vec<&PendingSettlement> = c.settlements.iter().filter(|s| !s.settled).collect();
    Json(json!({
        "ok": true,
        "auto": c.cfg.settle_auto,
        "bekleyen_sayisi": bekleyen.len(),
        "bekleyen": bekleyen,
        "not": "auto=false ise owner offline `soulware-settle` ile bu kuyruğu boşaltır.",
    }))
}

// Settlement özeti: kaç tanesi zincire basıldı, havuz vs emisyon dağılımı.
async fn settlement_status(State(st): State<St>) -> Json<Value> {
    let c = st.lock().await;
    let toplam = c.settlements.len();
    let basildi: Vec<&PendingSettlement> = c.settlements.iter().filter(|s| s.settled).collect();
    let odenen_lsc: u64 = basildi.iter().map(|s| s.lsc).sum();
    let havuzdan: u64 = basildi.iter().filter(|s| s.settled_via == "havuz").map(|s| s.lsc).sum();
    let emisyondan: u64 = basildi.iter().filter(|s| s.settled_via == "emisyon").map(|s| s.lsc).sum();
    Json(json!({
        "ok": true,
        "auto": c.cfg.settle_auto,
        "emisyon_fallback_owner": c.settle_addr.map(|a| format!("0x{}", hex::encode(a))),
        "toplam_settlement": toplam,
        "basildi": basildi.len(),
        "bekleyen": toplam - basildi.len(),
        "odenen_lsc": odenen_lsc,
        "havuzdan_lsc": havuzdan,
        "emisyondan_lsc": emisyondan,
        "not": "havuzdan = ücret-fonlu dolaşım (enflasyonsuz); emisyondan = bootstrap (tavanlı tip=16).",
    }))
}

// Havuz durumu: kazan↔harca döngüsünün kalbi. Zincirdeki gerçek havuz bakiyesi +
// toplanan ücretler + ödenen ödüller.
async fn pool_status(State(st): State<St>) -> Json<Value> {
    let (http, rpc, pool_addr, fees, jobs_paid) = {
        let c = st.lock().await;
        let fees: u64 = c.jobs.values().map(|j| j.fee_lsc).sum();
        let jobs_paid = c.jobs.values().filter(|j| j.fee_lsc > 0).count();
        (c.http.clone(), c.cfg.chain_rpc.clone(), c.key_addr, fees, jobs_paid)
    };
    let bal_wei = lsc_bakiye_cek(&http, &rpc, &pool_addr).await;
    let odenen: u64 = {
        let c = st.lock().await;
        c.settlements.iter().filter(|s| s.settled).map(|s| s.lsc).sum()
    };
    Json(json!({
        "ok": true,
        "havuz_adresi": format!("0x{}", hex::encode(pool_addr)),
        "havuz_bakiye_wei": bal_wei.to_string(),
        "havuz_bakiye_lsc": (bal_wei / ONDALIK).to_string(),
        "toplanan_ucret_lsc": fees,
        "ucretli_is_sayisi": jobs_paid,
        "odenen_odul_lsc": odenen,
        "not": "kazan↔harca: tüketici ücreti havuza (tip=7) → worker havuzdan ödenir (tip=7). Kapalı döngü.",
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

    // SETTLEMENT owner anahtarı (varsa): tip=16 emisyon yetkisi. Anahtar dosyası
    // [algo=1][32 seed] biçiminde. YOKSA auto emisyon devre dışı (mainnet güvenli).
    let (settle_key, settle_addr) = match &cfg.settle_key_path {
        Some(p) => match std::fs::read(p) {
            Ok(d) if d.len() == 33 && d[0] == 1 => {
                let mut seed = [0u8; 32];
                seed.copy_from_slice(&d[1..33]);
                let sk = SigningKey::from_bytes(&seed);
                let addr = public_key_to_adres(&sk.verifying_key().to_bytes());
                (Some(sk), Some(addr))
            }
            _ => {
                eprintln!("⚠ SOULWARE_SETTLE_KEY okunamadı/format hatalı ([1][32 seed] olmalı) → auto emisyon KAPALI");
                (None, None)
            }
        },
        None => (None, None),
    };
    // Otomatik settlement TEK anahtarı: SOULWARE_SETTLE_AUTO. Havuz ödemesi (tip=7)
    // koordinatörün kendi anahtarıyla yapılır (owner gerekmez); emisyon (tip=16)
    // fallback'i için owner anahtarı opsiyoneldir.
    let auto_aktif = cfg.settle_auto;

    println!("──────────────────────────────────────────────");
    println!("🛰  SoulwareAI Koordinatör (v0.1)");
    println!("   koordinatör : 0x{} (= ödül havuzu adresi)", hex::encode(key_addr));
    println!("   zincir RPC  : {}", cfg.chain_rpc);
    println!("   yedeklilik  : {} · ödül/iş: {} LSC", cfg.redundancy, cfg.reward_lsc);
    println!("   dinleme     : http://{listen}");
    if auto_aktif {
        let emis = match &settle_addr {
            Some(a) => format!("emisyon fallback owner=0x{}", hex::encode(a)),
            None => "emisyon fallback YOK (yalnız havuz-fonlu)".to_string(),
        };
        println!("   settlement  : OTOMATİK ✅ önce HAVUZ (tip=7, ücret-fonlu), sonra {emis}; her {}s", cfg.settle_interval);
    } else {
        println!("   settlement  : KUYRUK modu (SOULWARE_SETTLE_AUTO=1 değil) → owner offline araçla basar [mainnet güvenli]");
    }
    println!("──────────────────────────────────────────────");

    // KALICI durum: restart'ta worker/iş/ödül/itibar/settlement korunur.
    let ls = load_state(&cfg.data_path);
    if !ls.workers.is_empty() || !ls.jobs.is_empty() || !ls.settlements.is_empty() {
        let bekleyen = ls.settlements.iter().filter(|s| !s.settled).count();
        println!("   💾 durum yüklendi: {} worker · {} iş · {} settlement ({} bekleyen) · next_job={}",
            ls.workers.len(), ls.jobs.len(), ls.settlements.len(), bekleyen, ls.next_job);
    }
    let st: St = Arc::new(Mutex::new(Coord {
        cfg, http, key, key_addr,
        workers: ls.workers, jobs: ls.jobs, next_job: ls.next_job,
        settlements: ls.settlements, next_reward_id: ls.next_reward_id,
        settle_key, settle_addr,
    }));

    // Arka plan settlement döngüsü (yalnız auto aktifse gerçek iş yapar).
    if auto_aktif {
        let st_loop = st.clone();
        tokio::spawn(async move { settle_loop(st_loop).await; });
    }

    let app = Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/job/create", post(job_create))
        .route("/job/:id", get(job_get))
        .route("/worker/register", post(worker_register))
        .route("/worker/poll/:wallet", get(worker_poll))
        .route("/worker/submit", post(worker_submit))
        .route("/settlement/pending", get(settlement_pending))
        .route("/settlement/status", get(settlement_status))
        .route("/pool/status", get(pool_status))
        .with_state(st);

    let addr: SocketAddr = listen.parse().expect("SOULWARE_COORD_LISTEN geçersiz");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("port");
    axum::serve(listener, app).await.expect("sunucu");
}
