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
//!   POST /job/create       {"prompt","deterministic?","fee_lsc?","payer?"} → {job_id}
//!   POST /job/confirm      {"job_id","odeme_hex"}               → ücretli iş ödemesi (tip=7) bağlanır
//!   POST /worker/register   {"wallet", İMZA}                    → {ok}
//!   GET  /worker/poll/:wallet?ts=&imza=&pubkey=                  → iş | {none:true}
//!   POST /worker/benchmark  {"wallet","cevaplar", İMZA}
//!   POST /worker/submit     {"wallet","job_id","answer", İMZA}  → doğrulama/ödül
//!   GET  /status                                                → özet
//!
//! İMZA = {"ts","imza","pubkey?"}: cüzdan sahipliği (worker_mesaj.rs). İmzasız → 401.

#[path = "../../soulware-core/src/imza_dosyasi.rs"]
mod imza_dosyasi; // havuz anahtarı: FAIL-CLOSED (soulware-core ile ortak kod)
mod worker_mesaj; // worker imza mesajı biçimi (worker ile ortak)
mod cuzdan_imza;  // ed25519 / EVM (EIP-191) cüzdan imzası doğrulama
mod denetim;      // istemci IP, sybil-dirençli oylama, günlük kota, ödeme bağlama

use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::{routing::{get, post}, Json, Router};
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
    // Ücretsiz-tier emisyon bütçesi: ücretsiz işler yalnız emisyonla fonlanır
    // (ücretli havuzu yemez) ve bu tavana kadar. Bootstrap enflasyonunu sınırlar.
    free_budget_lsc: u64,            // SOULWARE_FREE_BUDGET_LSC (0 = ücretsiz tier kapalı)
    gold_path: Option<String>,       // SOULWARE_GOLD_PATH (öz-kıyaslama altın-testleri; yoksa gömülü)
    /// SOULWARE_FREE_DAILY_PER_CLIENT: istemci (IP ve/veya cüzdan) başına günlük
    /// ücretsiz iş sınırı (varsayılan 10; 0 = ücretsiz iş kapalı).
    free_daily_per_client: u32,
}

/// Otomatik settlement YALNIZ açıkça "1" ise açık (varsayılan KAPALI; mainnet güvenli).
fn settle_auto_coz(v: Option<String>) -> bool {
    v.as_deref().map(str::trim) == Some("1")
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
            settle_auto: settle_auto_coz(std::env::var("SOULWARE_SETTLE_AUTO").ok()),
            settle_interval: ev("SOULWARE_SETTLE_INTERVAL", "15").parse().unwrap_or(15),
            free_budget_lsc: ev("SOULWARE_FREE_BUDGET_LSC", "1000").parse().unwrap_or(1000),
            gold_path: std::env::var("SOULWARE_GOLD_PATH").ok().filter(|s| !s.trim().is_empty()),
            free_daily_per_client: ev("SOULWARE_FREE_DAILY_PER_CLIENT", "10").parse().unwrap_or(10),
        }
    }
}

// ════════════════════════════ Öz-kıyaslama (gold benchmark) ════════════════════════════
/// Bilinen-cevaplı altın soru. Worker'ın beyni (KUBRA) yanıtlar; koordinatör
/// doğruluğu ölçer → seviye (tier) atar. Böylece iş, yeteneğine uygun worker'a gider.
#[derive(Clone, Serialize, Deserialize)]
struct GoldQ {
    id: u64,
    soru: String,
    cevap: String, // beklenen cevap (normalize edilip 'içeriyor mu' ile eşlenir)
}

/// Gömülü varsayılan altın-test seti (deterministik, olgusal + aritmetik). Dosya
/// ile (SOULWARE_GOLD_PATH) genişletilebilir/değiştirilebilir.
fn gold_seti(cfg: &Config) -> Vec<GoldQ> {
    if let Some(p) = &cfg.gold_path {
        if let Ok(data) = std::fs::read(p) {
            if let Ok(v) = serde_json::from_slice::<Vec<GoldQ>>(&data) {
                if !v.is_empty() {
                    return v;
                }
            }
        }
    }
    vec![
        GoldQ { id: 1, soru: "2 + 2 kaçtır? Sadece rakam yaz.".into(), cevap: "4".into() },
        GoldQ { id: 2, soru: "5 çarpı 3 kaçtır? Sadece rakam yaz.".into(), cevap: "15".into() },
        GoldQ { id: 3, soru: "Türkiye'nin başkenti neresidir? Tek kelime yaz.".into(), cevap: "ankara".into() },
        GoldQ { id: 4, soru: "Fransa'nın başkenti neresidir? Tek kelime yaz.".into(), cevap: "paris".into() },
    ]
}

/// Metni eşleştirme için normalize et: küçük harf + Türkçe→ascii + yalnız alfanümerik.
fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c { 'ç'=>'c','ğ'=>'g','ı'=>'i','ş'=>'s','ö'=>'o','ü'=>'u', o=>o })
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// gold_score → tier. Yüksek doğruluk = yüksek seviye = zor işlere uygun.
fn skor_tier(score: f64) -> u8 {
    if score >= 0.75 { 3 } else if score >= 0.5 { 2 } else if score > 0.0 { 1 } else { 0 }
}

// ════════════════════════════ Durum modeli ════════════════════════════
#[derive(Clone, Serialize, Deserialize)]
struct Worker {
    wallet: String,
    reputation: i64,
    earned_lsc: u64,
    jobs_done: u64,
    registered_at: u64,
    // ── Yetenek profili (öz-kıyaslama ile ölçülür) ──
    // Büyük/küçük model karışımı zayıflık yaratmasın: iş, seviyesine uygun worker'a
    // gider. tier 0=ölçülmedi, 1=temel, 2=iyi, 3=güçlü. gold_score=doğruluk (0..1).
    #[serde(default)]
    tier: u8,
    #[serde(default)]
    gold_score: f64,      // altın-test doğruluk oranı (0..1)
    #[serde(default)]
    avg_latency_ms: f64,  // ölçülen ortalama gecikme (yönlendirmede hız için)
    #[serde(default)]
    benchmarked_at: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct WorkResult {
    worker: String,
    answer: String,
    hash: String, // blake3(answer) hex — hızlı eşleşme
    at: u64,
    /// Gönderenin IP'si (biliniyorsa). Aynı IP'den ikinci cüzdan oy sayılmaz (sybil).
    #[serde(default)]
    ip: Option<String>,
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
    // Ücretli işten mi (havuzdan ödenebilir) yoksa ücretsiz işten mi (yalnız emisyon,
    // ücretli havuzu YEMEZ). Doğru ekonomi ayrımı: free-rider paid havuzunu tüketmesin.
    #[serde(default)]
    paid_job: bool,
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
    // Ücretli iş: ödeme HAVUZDA doğrulandı mı (yalnız o zaman dağıtılır). Ücretsiz
    // iş (fee=0): oluşturulurken free bütçeden rezerve edilir, doğrudan paid=true.
    #[serde(default)]
    paid: bool,
    // Gereken minimum worker seviyesi (zorluk). Yüksek doğruluk isteyen iş yalnız
    // yeterli seviyedeki worker'a dağıtılır → küçük model zayıf halka olmaz.
    #[serde(default)]
    min_tier: u8,
    /// Atanan worker → IP (biliniyorsa). Aynı IP'ye aynı iş ikinci kez verilmez.
    #[serde(default)]
    atanan_ip: HashMap<String, String>,
    /// Ücretli işin ödemesi: bağlandığı tip=7 vertex id'si (hex).
    #[serde(default)]
    odeme_vertex: Option<String>,
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
    // ── Ekonomi muhasebesi ──
    fees_committed_wei: u128,          // ödemesi doğrulanmış toplam ücret (high-water-mark)
    free_committed_lsc: u64,           // ücretsiz-tier'e rezerve edilmiş toplam LSC (bütçeye karşı)
    // ── Kötüye kullanım korumaları ──
    ucretsiz_gunluk: HashMap<String, (u64, u32)>, // istemci anahtarı → (gün, ücretsiz iş sayısı)
    kullanilan_odemeler: HashMap<String, u64>,    // vertex id / "payer:nonce" → iş (çifte sayım kilidi)
    // ── Tarayıcı işçi oturum anahtarları (YALNIZ bellekte; restartta yeniden kayıt) ──
    oturumlar: HashMap<String, cuzdan_imza::Oturum>, // cüzdan (küçük harf) → geçerli oturum anahtarı
}

impl Coord {
    /// Durumu diske yaz (restart'ta kaybolmasın). Atomik: önce .tmp, sonra rename.
    fn save(&self) {
        let v = json!({
            "workers": self.workers, "jobs": self.jobs, "next_job": self.next_job,
            "settlements": self.settlements, "next_reward_id": self.next_reward_id,
            "fees_committed_wei": self.fees_committed_wei.to_string(),
            "free_committed_lsc": self.free_committed_lsc,
            "ucretsiz_gunluk": self.ucretsiz_gunluk,
            "kullanilan_odemeler": self.kullanilan_odemeler,
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
    fees_committed_wei: u128,
    free_committed_lsc: u64,
    ucretsiz_gunluk: HashMap<String, (u64, u32)>,
    kullanilan_odemeler: HashMap<String, u64>,
}

/// Diskten durumu yükle (yoksa boş). Restart sonrası worker/iş/ödül/settlement/muhasebe korunur.
fn load_state(path: &str) -> LoadedState {
    if let Ok(data) = std::fs::read(path) {
        if let Ok(v) = serde_json::from_slice::<Value>(&data) {
            let workers = v.get("workers").cloned().and_then(|x| serde_json::from_value(x).ok()).unwrap_or_default();
            let jobs = v.get("jobs").cloned().and_then(|x| serde_json::from_value(x).ok()).unwrap_or_default();
            let next_job = v.get("next_job").and_then(|x| x.as_u64()).unwrap_or(1);
            let settlements = v.get("settlements").cloned().and_then(|x| serde_json::from_value(x).ok()).unwrap_or_default();
            let next_reward_id = v.get("next_reward_id").and_then(|x| x.as_u64()).unwrap_or(1);
            let fees_committed_wei = v.get("fees_committed_wei").and_then(|x| x.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0);
            let free_committed_lsc = v.get("free_committed_lsc").and_then(|x| x.as_u64()).unwrap_or(0);
            let ucretsiz_gunluk = v.get("ucretsiz_gunluk").cloned().and_then(|x| serde_json::from_value(x).ok()).unwrap_or_default();
            let kullanilan_odemeler = v.get("kullanilan_odemeler").cloned().and_then(|x| serde_json::from_value(x).ok()).unwrap_or_default();
            return LoadedState { workers, jobs, next_job, settlements, next_reward_id, fees_committed_wei, free_committed_lsc, ucretsiz_gunluk, kullanilan_odemeler };
        }
    }
    LoadedState { workers: HashMap::new(), jobs: HashMap::new(), next_job: 1, settlements: vec![], next_reward_id: 1, fees_committed_wei: 0, free_committed_lsc: 0,
        ucretsiz_gunluk: HashMap::new(), kullanilan_odemeler: HashMap::new() }
}

type St = Arc<Mutex<Coord>>;

// ════════════════════════════ Zincir: imzalı ödül kaydı ════════════════════════════
// Havuz anahtarı: imza_dosyasi.rs (fail-closed; sessiz üretim kaldırıldı).

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

            // ÜCRETLİ iş → HAVUZDAN öde (ücret-fonlu, enflasyonsuz). ÜCRETSİZ iş →
            // yalnız EMİSYON (bootstrap); ücretli kullanıcıların havuzunu YEMEZ.
            let (ok, via, note) = if s.paid_job && pool_bal >= need_wei {
                let (ok, note) = havuzdan_ode(&http, &rpc, net_id, &pool_key, &pool_addr, w, need_wei, ts).await;
                (ok, "havuz", note)
            } else if let Some(ok_key) = &owner_key {
                // Emisyon: ücretsiz işler + (güvenlik) havuzu geçici yetmeyen ücretli işler.
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
type Yanit = (StatusCode, Json<Value>);

fn yanit(v: Value) -> Yanit {
    (StatusCode::OK, Json(v))
}

fn hata(kod: StatusCode, mesaj: impl Into<String>) -> Yanit {
    (kod, Json(json!({ "ok": false, "hata": mesaj.into() })))
}

/// Cüzdan imzasını doğrula (cüzdanın kendi anahtarı YA DA bu cüzdanın geçerli oturum
/// anahtarı); başarısızsa 401. Oturum yalnız `wallet` anahtarıyla aranır → başka cüzdanın
/// oturum anahtarı bu cüzdan için kabul edilmez.
async fn imza_kontrol(st: &St, wallet: &str, nonce: &str, alan: &cuzdan_imza::ImzaAlanlari) -> Result<(), Yanit> {
    let w = wallet.trim().to_lowercase();
    let oturum = st.lock().await.oturumlar.get(&w).copied();
    cuzdan_imza::dogrula_oturumlu(&w, nonce, alan, now_secs(), oturum.as_ref())
        .map_err(|e| hata(StatusCode::UNAUTHORIZED, e))
}

fn peer_ip(ci: Option<ConnectInfo<SocketAddr>>, h: &HeaderMap) -> Option<String> {
    denetim::istemci_ip(ci.map(|c| c.0), h)
}
#[derive(Deserialize)]
struct CreateJob {
    prompt: String,
    #[serde(default)] deterministic: Option<bool>,
    #[serde(default)] fee_lsc: Option<u64>,   // tüketicinin havuza ödediği ücret (LSC)
    #[serde(default)] payer: Option<String>,  // ücreti ödeyen cüzdan (0x...)
    #[serde(default)] min_tier: Option<u8>,   // gereken min worker seviyesi (zorluk; 0=herkes)
}

async fn job_create(State(st): State<St>, ci: Option<ConnectInfo<SocketAddr>>, h: HeaderMap, Json(req): Json<CreateJob>) -> Json<Value> {
    if req.prompt.trim().is_empty() {
        return Json(json!({ "ok": false, "hata": "prompt boş" }));
    }
    let ip = peer_ip(ci, &h);
    let payer = req.payer.as_deref().map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty());
    if let Some(p) = &payer {
        if cuzdan20(p).is_none() {
            return Json(json!({ "ok": false, "hata": "payer 0x + 40 hex olmalı" }));
        }
    }
    let mut c = st.lock().await;
    let det = req.deterministic.unwrap_or(true);
    let fee = req.fee_lsc.unwrap_or(0);
    // Bir doğrulanmış işin dağıttığı toplam ödül = yedeklilik × ödül/worker.
    let is_maliyeti = (c.cfg.redundancy as u64).saturating_mul(c.cfg.reward_lsc);

    let paid = if fee > 0 {
        // ÜCRETLİ: sürdürülebilirlik kuralı — ücret ≥ iş maliyeti (havuz erimez, büyür).
        if fee < is_maliyeti {
            return Json(json!({ "ok": false,
                "hata": format!("ücret en az {is_maliyeti} LSC olmalı (yedeklilik×ödül); havuzu erirtmez"),
                "min_ucret_lsc": is_maliyeti }));
        }
        // Ödeme belirli bir tip=7 transferine bağlanacak: imzalayan = payer olmalı.
        if payer.is_none() {
            return Json(json!({ "ok": false, "hata": "ücretli iş için payer (ödeyen cüzdan) zorunlu" }));
        }
        false // ödeme /job/confirm ile havuzda doğrulanana kadar dağıtılmaz
    } else {
        // ÜCRETSİZ: yalnız emisyonla fonlanır (ücretli havuzu YEMEZ) + free bütçe tavanı.
        let yeni_taahhut = c.free_committed_lsc.saturating_add(is_maliyeti);
        if c.cfg.free_budget_lsc == 0 {
            return Json(json!({ "ok": false, "hata": "ücretsiz tier kapalı; fee_lsc ile ücretli iş açın" }));
        }
        if yeni_taahhut > c.cfg.free_budget_lsc {
            return Json(json!({ "ok": false,
                "hata": format!("ücretsiz kota doldu ({}/{} LSC). Ücretli kullanın ya da katkı verip kazanın.",
                    c.free_committed_lsc, c.cfg.free_budget_lsc) }));
        }
        // İstemci başına günlük sınır: IP (bilinmiyorsa ortak "bilinmiyor" kovası) + varsa cüzdan.
        let mut anahtarlar = vec![format!("ip:{}", ip.as_deref().unwrap_or("bilinmiyor"))];
        if let Some(p) = &payer { anahtarlar.push(format!("cuzdan:{p}")); }
        let gun = now_secs() / 86_400;
        let sinir = c.cfg.free_daily_per_client;
        if !denetim::gunluk_kota_dene(&mut c.ucretsiz_gunluk, &anahtarlar, gun, sinir) {
            return Json(json!({ "ok": false,
                "hata": format!("günlük ücretsiz iş sınırı doldu (istemci başına {sinir}/gün). Yarın tekrar deneyin ya da ücretli kullanın.") }));
        }
        c.free_committed_lsc = yeni_taahhut; // rezerve et
        true // ücretsiz iş hemen dağıtılabilir (emisyonla ödenir)
    };

    let id = c.next_job;
    c.next_job += 1;
    let min_tier = req.min_tier.unwrap_or(0);
    c.jobs.insert(id, Job {
        id, prompt: req.prompt, deterministic: det, status: "pending".into(),
        assigned: vec![], results: vec![], verified_answer: None, rewards: vec![], created_at: now_secs(),
        fee_lsc: fee, payer, paid, min_tier, atanan_ip: HashMap::new(), odeme_vertex: None,
    });
    c.save();
    Json(json!({ "ok": true, "job_id": id, "deterministic": det, "fee_lsc": fee, "paid": paid, "min_tier": min_tier,
        "not": if fee > 0 {
            "ÜCRETLİ: payer cüzdanıyla havuza tip=7 LSC transferi imzala, imzalı vertex hex'ini POST /job/confirm {job_id, odeme_hex} ile gönder → dağıtılır"
        } else { "ÜCRETSİZ: emisyonla fonlanır (bootstrap), hemen dağıtılır" } }))
}

#[derive(Deserialize)]
struct Reg {
    wallet: String,
    /// Varsa: tarayıcı işçinin geçici ed25519 oturum anahtarı (64 hex). İmza nonce'u
    /// o zaman "kayit" değil "oturum:<oturum_pubkey>" olur (worker_mesaj::oturum_nonce).
    #[serde(default)]
    oturum_pubkey: Option<String>,
    #[serde(flatten)]
    imza: cuzdan_imza::ImzaAlanlari,
}

async fn worker_register(State(st): State<St>, Json(req): Json<Reg>) -> Yanit {
    let w = req.wallet.trim().to_lowercase();
    if cuzdan20(&w).is_none() {
        return hata(StatusCode::BAD_REQUEST, "wallet 0x + 40 hex olmalı");
    }
    // Cüzdan sahipliği: imzasız/yanlış imza → 401 (başkası adına kayıt/sybil zorlaşır).
    // Kayıt (ve oturum kurma) YALNIZ cüzdanın kendi anahtarıyla — oturum anahtarı kabul edilmez.
    let simdi = now_secs();
    let mut c = st.lock().await;
    let oturum = match req.oturum_pubkey.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(pk) => match cuzdan_imza::oturum_kaydet(&mut c.oturumlar, &w, pk, &req.imza, simdi) {
            Ok(o) => Some(o),
            Err(e) => return hata(StatusCode::UNAUTHORIZED, e),
        },
        None => {
            if let Err(e) = cuzdan_imza::dogrula(&w, worker_mesaj::NONCE_KAYIT, &req.imza, simdi) {
                return hata(StatusCode::UNAUTHORIZED, e);
            }
            None
        }
    };
    c.workers.entry(w.clone()).or_insert_with(|| Worker {
        wallet: w.clone(), reputation: 0, earned_lsc: 0, jobs_done: 0, registered_at: now_secs(),
        tier: 0, gold_score: 0.0, avg_latency_ms: 0.0, benchmarked_at: 0,
    });
    c.save();
    yanit(json!({ "ok": true, "wallet": w,
        "oturum_bitis": oturum.map(|o| o.bitis),
        "rıza": "GPU katkısı yalnızca istemci onayıyla",
        "not": "seviye için GET /worker/benchmark/:wallet ile öz-kıyaslamayı yap" }))
}

// ÖZ-KIYASLAMA 1/2: worker altın soruları alır (beyniyle deterministik yanıtlar).
async fn worker_benchmark_al(State(st): State<St>, Path(wallet): Path<String>) -> Json<Value> {
    let w = wallet.trim().to_lowercase();
    let c = st.lock().await;
    if !c.workers.contains_key(&w) {
        return Json(json!({ "ok": false, "hata": "önce kaydol (/worker/register)" }));
    }
    let sorular: Vec<Value> = gold_seti(&c.cfg).into_iter()
        .map(|g| json!({ "id": g.id, "soru": g.soru })).collect();
    Json(json!({ "ok": true, "sorular": sorular,
        "not": "her soruyu beyninle (deterministic) yanıtla, POST /worker/benchmark ile gönder" }))
}

#[derive(Deserialize)]
struct BenchCevap { id: u64, cevap: String, #[serde(default)] ms: u64 }
#[derive(Deserialize)]
struct BenchSubmit {
    wallet: String,
    cevaplar: Vec<BenchCevap>,
    #[serde(flatten)]
    imza: cuzdan_imza::ImzaAlanlari,
}

// ÖZ-KIYASLAMA 2/2: cevapları puanla → gold_score + tier + ortalama gecikme ata.
async fn worker_benchmark_gonder(State(st): State<St>, Json(req): Json<BenchSubmit>) -> Yanit {
    let w = req.wallet.trim().to_lowercase();
    // İmza cevapların kendisine bağlı: başkasının seviyesini değiştiremez.
    let ozet: Vec<(u64, u64, &str)> = req.cevaplar.iter().map(|c| (c.id, c.ms, c.cevap.as_str())).collect();
    if let Err(r) = imza_kontrol(&st, &w, &worker_mesaj::benchmark_nonce(&ozet), &req.imza).await { return r; }
    let mut c = st.lock().await;
    if !c.workers.contains_key(&w) {
        return hata(StatusCode::BAD_REQUEST, "önce kaydol");
    }
    let gold = gold_seti(&c.cfg);
    let toplam = gold.len().max(1);
    let mut dogru = 0usize;
    for g in &gold {
        if let Some(cev) = req.cevaplar.iter().find(|x| x.id == g.id) {
            if normalize(&cev.cevap).contains(&normalize(&g.cevap)) {
                dogru += 1;
            }
        }
    }
    let ort_ms = if req.cevaplar.is_empty() { 0.0 }
        else { req.cevaplar.iter().map(|x| x.ms as f64).sum::<f64>() / req.cevaplar.len() as f64 };
    let score = dogru as f64 / toplam as f64;
    let tier = skor_tier(score);
    let ts = now_secs();
    if let Some(wk) = c.workers.get_mut(&w) {
        wk.gold_score = score;
        wk.tier = tier;
        wk.avg_latency_ms = ort_ms;
        wk.benchmarked_at = ts;
    }
    c.save();
    yanit(json!({ "ok": true, "wallet": w, "dogru": dogru, "toplam": toplam,
        "gold_score": score, "tier": tier, "ort_gecikme_ms": ort_ms,
        "not": "tier ≥ işin min_tier'i ise o iş sana dağıtılır" }))
}

async fn worker_poll(
    State(st): State<St>, Path(wallet): Path<String>, Query(imza): Query<cuzdan_imza::ImzaAlanlari>,
    ci: Option<ConnectInfo<SocketAddr>>, h: HeaderMap,
) -> Yanit {
    let w = wallet.trim().to_lowercase();
    // Başkası adına poll = kurbanın adına iş kapma/slot doldurma → imza şart.
    if let Err(r) = imza_kontrol(&st, &w, worker_mesaj::NONCE_POLL, &imza).await { return r; }
    let ip = peer_ip(ci, &h);
    let mut c = st.lock().await;
    if !c.workers.contains_key(&w) {
        return yanit(json!({ "none": true, "hata": "önce kaydol (/worker/register)" }));
    }
    let max_assign = c.cfg.max_assign;
    let w_tier = c.workers.get(&w).map(|x| x.tier).unwrap_or(0);
    // Bu worker'a atanmamış, hâlâ yayında (pending), kapasitesi dolmamış VE worker'ın
    // seviyesinin yettiği bir iş bul. Zor iş (min_tier) yalnız yeterli worker'a → küçük
    // model zayıf halka olmaz. Yüksek min_tier'li işi önce ver (güçlü worker boşa kalmasın).
    let mut secilen: Option<(u64, String, bool)> = None;
    let mut ids: Vec<u64> = c.jobs.keys().copied().collect();
    // min_tier azalan, sonra id artan: güçlü worker önce zor işi alsın.
    ids.sort_by(|a, b| {
        let ta = c.jobs.get(a).map(|j| j.min_tier).unwrap_or(0);
        let tb = c.jobs.get(b).map(|j| j.min_tier).unwrap_or(0);
        tb.cmp(&ta).then(a.cmp(b))
    });
    for id in ids {
        if let Some(j) = c.jobs.get(&id) {
            // Aynı IP'den ikinci cüzdana aynı iş VERİLMEZ (sybil slot doldurmasın).
            let ayni_ip = ip.as_ref().is_some_and(|i| j.atanan_ip.values().any(|x| x == i));
            let dagitilabilir = j.status == "pending" && j.paid
                && w_tier >= j.min_tier
                && !j.assigned.contains(&w) && j.assigned.len() < max_assign
                && !ayni_ip;
            if dagitilabilir {
                secilen = Some((id, j.prompt.clone(), j.deterministic));
                break;
            }
        }
    }
    match secilen {
        Some((id, prompt, det)) => {
            if let Some(j) = c.jobs.get_mut(&id) {
                j.assigned.push(w.clone());
                if let Some(i) = &ip { j.atanan_ip.insert(w.clone(), i.clone()); }
            }
            c.save();
            yanit(json!({ "job_id": id, "prompt": prompt, "deterministic": det }))
        }
        None => yanit(json!({ "none": true })),
    }
}

#[derive(Deserialize)]
struct Submit {
    wallet: String,
    job_id: u64,
    answer: String,
    #[serde(flatten)]
    imza: cuzdan_imza::ImzaAlanlari,
}

async fn worker_submit(State(st): State<St>, ci: Option<ConnectInfo<SocketAddr>>, h: HeaderMap, Json(req): Json<Submit>) -> Yanit {
    let w = req.wallet.trim().to_lowercase();
    let ts = now_secs();
    // İmza belirli bir iş + belirli bir cevaba bağlı (worker_mesaj::is_nonce).
    if let Err(r) = imza_kontrol(&st, &w, &worker_mesaj::is_nonce(req.job_id, &req.answer), &req.imza).await { return r; }
    let ip = peer_ip(ci, &h);
    let hash = worker_mesaj::cevap_ozeti(&req.answer);

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
            None => return hata(StatusCode::NOT_FOUND, "iş yok"),
        };
        if job.status != "pending" {
            return yanit(json!({ "ok": true, "durum": job.status.clone(), "not": "iş zaten kapandı" }));
        }
        if !job.assigned.contains(&w) {
            return hata(StatusCode::FORBIDDEN, "bu iş sana atanmadı");
        }
        if job.results.iter().any(|r| r.worker == w) {
            return hata(StatusCode::CONFLICT, "zaten gönderdin");
        }
        job.results.push(WorkResult { worker: w.clone(), answer: req.answer.clone(), hash: hash.clone(), at: ts, ip: ip.clone() });

        // Eşleşme sayımı: bir cevap-hash için FARKLI cüzdan + (biliniyorsa) FARKLI IP
        // sayısı ≥ redundancy → DOĞRULANDI. Aynı IP'den ikinci cüzdan oy sayılmaz/ödül almaz.
        let oylar: Vec<denetim::Oy> = job.results.iter()
            .map(|r| denetim::Oy { worker: &r.worker, hash: &r.hash, ip: r.ip.as_deref() }).collect();
        let kazanan = denetim::oylama(&oylar, redundancy);
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
            // ödeme (havuz tip=7 / emisyon tip=16) sıraya girer. reward_id = çifte-basım kilidi.
            // paid_job: ücretli işten mi (havuzdan ödenebilir) yoksa ücretsiz mi (yalnız emisyon).
            let paid_job = c.jobs.get(&job_id).map(|j| j.fee_lsc > 0).unwrap_or(false);
            let reward_id = c.next_reward_id;
            c.next_reward_id += 1;
            c.settlements.push(PendingSettlement {
                reward_id, worker: worker.clone(), lsc: cfg.reward_lsc, job_id,
                created_at: ts, settled: false, settled_at: 0, settled_via: String::new(), paid_job,
            });
            odul_sonuc.push(json!({ "worker": worker, "lsc": cfg.reward_lsc, "chain_ok": chain_ok, "proof": proof, "reward_id": reward_id }));
        }
        { let c = st.lock().await; c.save(); }
        return yanit(json!({
            "ok": true, "durum": "verified",
            "kazananlar": kazananlar.len(), "slashlanan": slashlananlar.len(),
            "oduller": odul_sonuc,
        }));
    }

    yanit(json!({ "ok": true, "durum": "kaydedildi", "not": "doğrulama için daha çok sonuç bekleniyor" }))
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
    let (http, rpc, pool_addr, fees, jobs_paid, fees_committed, free_committed, free_budget) = {
        let c = st.lock().await;
        let fees: u64 = c.jobs.values().filter(|j| j.paid).map(|j| j.fee_lsc).sum();
        let jobs_paid = c.jobs.values().filter(|j| j.fee_lsc > 0 && j.paid).count();
        (c.http.clone(), c.cfg.chain_rpc.clone(), c.key_addr, fees, jobs_paid,
         c.fees_committed_wei, c.free_committed_lsc, c.cfg.free_budget_lsc)
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
        "dogrulanan_ucret_lsc": fees,
        "ucretli_dogrulanan_is": jobs_paid,
        "taahhut_ucret_wei": fees_committed.to_string(),
        "ucretsiz_kullanilan_lsc": free_committed,
        "ucretsiz_butce_lsc": free_budget,
        "odenen_odul_lsc": odenen,
        "not": "kazan↔harca: ücretli iş havuzdan (tip=7, enflasyonsuz); ücretsiz iş emisyondan (bootstrap, tavanlı).",
    }))
}

// Havuza yapılan tip=7 ödemelerin toplamı (wei). Emisyon havuzu etkilemez → hariç.
fn havuz_odemeleri_wei(c: &Coord) -> u128 {
    c.settlements.iter().filter(|s| s.settled && s.settled_via == "havuz")
        .map(|s| (s.lsc as u128).saturating_mul(ONDALIK)).sum()
}

#[derive(Deserialize)]
struct Confirm {
    job_id: u64,
    /// Payer'ın imzaladığı tip=7 LSC transfer vertex'i (wire hex). Ödeme bu İŞLEME bağlanır.
    #[serde(default)]
    odeme_hex: Option<String>,
}

// Ücretli işin ödemesini DOĞRULA ve BELİRLİ bir tip=7 transferine BAĞLA:
//  1) odeme_hex: imzalı vertex, doğru ağ, alıcı = havuz, miktar ≥ ücret, imzalayan = işin payer'ı;
//  2) aynı vertex id / (payer, nonce) başka bir işe sayılmışsa RED (çifte sayım yok);
//  3) gönderim öncesi payer nonce'u == transfer nonce'u; vertex'i koordinatör gönderir
//     ve zincir "Integrated" der; gönderim sonrası nonce ilerlemiş olmalı (transfer İŞLENDİ);
//  5) ek güvence: havuzdaki taahhüt edilmemiş bakiye ücreti karşılamalı.
async fn job_confirm(State(st): State<St>, Json(req): Json<Confirm>) -> Yanit {
    let (http, rpc, net_id, pool_addr, fee_wei, payer, already) = {
        let c = st.lock().await;
        let job = match c.jobs.get(&req.job_id) {
            Some(j) => j,
            None => return hata(StatusCode::NOT_FOUND, "iş yok"),
        };
        if job.fee_lsc == 0 {
            return hata(StatusCode::BAD_REQUEST, "ücretsiz iş, ödeme gerekmez");
        }
        (c.http.clone(), c.cfg.chain_rpc.clone(), c.cfg.net_id, c.key_addr,
         (job.fee_lsc as u128).saturating_mul(ONDALIK), job.payer.clone(), job.paid)
    };
    if already {
        return yanit(json!({ "ok": true, "job_id": req.job_id, "paid": true, "not": "zaten doğrulanmış" }));
    }
    let Some(payer) = payer else { return hata(StatusCode::BAD_REQUEST, "işin payer adresi yok") };
    let Some(odeme_hex) = req.odeme_hex.as_deref().filter(|s| !s.trim().is_empty()) else {
        return hata(StatusCode::BAD_REQUEST, "odeme_hex zorunlu: payer'ın havuza imzaladığı tip=7 transfer vertex'i");
    };
    let odeme = match denetim::odeme_coz(odeme_hex, net_id, &pool_addr, &payer, fee_wei) {
        Ok(o) => o,
        Err(e) => return hata(StatusCode::BAD_REQUEST, e),
    };
    let kilit_anahtarlari = [odeme.vertex_id.clone(), odeme.nonce_anahtari()];
    {
        let c = st.lock().await;
        if let Some(j) = kilit_anahtarlari.iter().find_map(|k| c.kullanilan_odemeler.get(k)) {
            return hata(StatusCode::CONFLICT, format!("bu ödeme zaten iş #{j} için sayıldı"));
        }
    }
    // Transferin GERÇEKTEN bu gönderimle işlendiğini kanıtla:
    //  (a) gönderim ÖNCESİ payer nonce'u == transfer nonce'u (sıradaki geçerli işlem),
    //  (b) vertex'i KOORDİNATÖR gönderir ve zincir "Integrated" der (Duplicate = daha
    //      önce başka yoldan gönderilmiş → sonucu bu gönderimle kanıtlanamaz → RED),
    //  (c) gönderim SONRASI payer nonce'u transfer nonce'unu geçmiş (yalnız BAŞARILI
    //      tip=7 nonce ilerletir: bakiye yetersiz/yanlış nonce → ilerlemez).
    let payer20 = match cuzdan20(&payer) { Some(a) => a, None => return hata(StatusCode::BAD_REQUEST, "payer geçersiz") };
    let onceki_nonce = nonce_cek(&http, &rpc, &payer20).await;
    if onceki_nonce != odeme.nonce {
        return hata(StatusCode::CONFLICT, format!(
            "transfer nonce'u ({}) payer'ın sıradaki nonce'u ({onceki_nonce}) değil; GET /nonce ile yeniden imzalayın", odeme.nonce));
    }
    let temiz_hex = odeme_hex.trim().trim_start_matches("0x").to_string();
    let sonuc = match http.post(format!("{rpc}/submit")).json(&json!({ "hex": temiz_hex })).send().await {
        Ok(r) => r.json::<Value>().await.ok()
            .and_then(|v| v.get("sonuc").and_then(|s| s.as_str()).map(String::from))
            .unwrap_or_default(),
        Err(e) => return hata(StatusCode::BAD_GATEWAY, format!("zincire ulaşılamıyor: {e}")),
    };
    if !sonuc.starts_with("Integrated") {
        return hata(StatusCode::BAD_REQUEST, format!(
            "ödeme vertex'i bu gönderimle işlenmedi ({sonuc}). İmzalı transferi zincire kendiniz göndermeyin; yalnız /job/confirm'e verin"));
    }
    let sonraki_nonce = nonce_cek(&http, &rpc, &payer20).await;
    if sonraki_nonce <= odeme.nonce {
        return hata(StatusCode::CONFLICT, format!(
            "ödeme zincirde uygulanmadı (payer nonce {sonraki_nonce}); bakiye yetersiz olabilir"));
    }
    let pool_live = lsc_bakiye_cek(&http, &rpc, &pool_addr).await;
    // Nihai kararı kilit altında ver (fresh fees_committed + çifte sayım kilidi — yarış güvenli).
    let mut c = st.lock().await;
    if let Some(j) = kilit_anahtarlari.iter().find_map(|k| c.kullanilan_odemeler.get(k)) {
        return hata(StatusCode::CONFLICT, format!("bu ödeme zaten iş #{j} için sayıldı"));
    }
    let total_in = pool_live.saturating_add(havuz_odemeleri_wei(&c));
    let available = total_in.saturating_sub(c.fees_committed_wei);
    if available < fee_wei {
        return (StatusCode::CONFLICT, Json(json!({ "ok": false, "job_id": req.job_id, "paid": false,
            "hata": "ödeme havuzda görünmüyor",
            "gereken_wei": fee_wei.to_string(), "musait_wei": available.to_string() })));
    }
    c.fees_committed_wei = c.fees_committed_wei.saturating_add(fee_wei);
    for k in kilit_anahtarlari { c.kullanilan_odemeler.insert(k, req.job_id); }
    if let Some(j) = c.jobs.get_mut(&req.job_id) {
        j.paid = true;
        j.odeme_vertex = Some(odeme.vertex_id.clone());
    }
    c.save();
    yanit(json!({ "ok": true, "job_id": req.job_id, "paid": true, "odeme_vertex": odeme.vertex_id,
        "not": "ödeme bu işe bağlandı → iş dağıtılabilir" }))
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "servis": "soulware-coordinator", "surum": "0.1.0" }))
}

#[tokio::main]
async fn main() {
    let cfg = Config::from_env();
    // HAVUZ ANAHTARI (FAIL-CLOSED): yoksa/bozuksa koordinatör AÇILMAZ; yeni anahtar yalnız
    // `--yeni-anahtar-uret` ile (havuz adresi değişir → eski havuz bakiyesi erişilemez olur).
    let argumanlar: Vec<String> = std::env::args().skip(1).collect();
    match argumanlar.as_slice() {
        [] => {}
        [a] if a == "--yeni-anahtar-uret" => match imza_dosyasi::uret(&cfg.key_path) {
            Ok(k) => {
                println!("YENI HAVUZ ANAHTARI URETILDI: {}\n   havuz adresi: 0x{}\nServis BASLATILMADI.",
                    cfg.key_path, hex::encode(public_key_to_adres(&k.verifying_key().to_bytes())));
                return;
            }
            Err(e) => { eprintln!("HATA: {e}"); std::process::exit(2); }
        },
        _ => {
            eprintln!("HATA: bilinmeyen arguman: {argumanlar:?}. Gecerli: (yok) | --yeni-anahtar-uret");
            std::process::exit(2);
        }
    }
    let key = match imza_dosyasi::yukle(&cfg.key_path) {
        Ok(k) => k,
        Err(e) => { eprintln!("HATA: {e}"); std::process::exit(2); }
    };
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
    println!("   ekonomi     : min ücret {} LSC (yedeklilik×ödül) · ücretsiz bütçe {} LSC",
        (cfg.redundancy as u64) * cfg.reward_lsc, cfg.free_budget_lsc);
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
        fees_committed_wei: ls.fees_committed_wei, free_committed_lsc: ls.free_committed_lsc,
        ucretsiz_gunluk: ls.ucretsiz_gunluk, kullanilan_odemeler: ls.kullanilan_odemeler,
        oturumlar: HashMap::new(),
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
        .route("/worker/benchmark/:wallet", get(worker_benchmark_al))
        .route("/worker/benchmark", post(worker_benchmark_gonder))
        .route("/worker/poll/:wallet", get(worker_poll))
        .route("/worker/submit", post(worker_submit))
        .route("/job/confirm", post(job_confirm))
        .route("/settlement/pending", get(settlement_pending))
        .route("/settlement/status", get(settlement_status))
        .route("/pool/status", get(pool_status))
        // CORS: tarayıcı katkı sayfası (sıfır-kurulum) koordinatöre ulaşabilsin.
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(st);

    let addr: SocketAddr = listen.parse().expect("SOULWARE_COORD_LISTEN geçersiz");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("port");
    // ConnectInfo: istemci IP'si (sybil/kota); yerel vekil arkasında X-Real-IP/XFF.
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.expect("sunucu");
}

#[cfg(test)]
mod tests {
    use super::settle_auto_coz;

    // ── Uç (handler) düzeyi: oturum anahtarı ile başkası adına iş/ödül toplanamaz (401) ──
    mod oturum_uclari {
        use super::super::*;
        use crate::cuzdan_imza::test_yardim::*;

        fn test_st(ad: &str) -> St {
            let mut cfg = Config::from_env();
            let d = std::env::temp_dir().join(format!("swc-oturum-test-{ad}-{}", std::process::id()));
            cfg.data_path = d.join("coord.json").to_string_lossy().into_owned();
            cfg.free_daily_per_client = 100;
            cfg.free_budget_lsc = 1000;
            cfg.redundancy = 2;
            cfg.max_assign = 3;
            let key = SigningKey::from_bytes(&[3u8; 32]);
            let key_addr = public_key_to_adres(&key.verifying_key().to_bytes());
            Arc::new(Mutex::new(Coord {
                cfg, http: reqwest::Client::new(), key, key_addr,
                workers: HashMap::new(), jobs: HashMap::new(), next_job: 1,
                settlements: vec![], next_reward_id: 1, settle_key: None, settle_addr: None,
                fees_committed_wei: 0, free_committed_lsc: 0,
                ucretsiz_gunluk: HashMap::new(), kullanilan_odemeler: HashMap::new(),
                oturumlar: HashMap::new(),
            }))
        }
        async fn kayit(st: &St, wallet: &str, oturum_pubkey: Option<String>, imza: cuzdan_imza::ImzaAlanlari) -> Yanit {
            worker_register(State(st.clone()), Json(Reg { wallet: wallet.into(), oturum_pubkey, imza })).await
        }
        async fn poll(st: &St, wallet: &str, imza: cuzdan_imza::ImzaAlanlari) -> Yanit {
            worker_poll(State(st.clone()), Path(wallet.into()), Query(imza), None, HeaderMap::new()).await
        }
        async fn submit(st: &St, wallet: &str, job_id: u64, answer: &str, imza: cuzdan_imza::ImzaAlanlari) -> Yanit {
            worker_submit(State(st.clone()), None, HeaderMap::new(),
                Json(Submit { wallet: wallet.into(), job_id, answer: answer.into(), imza })).await
        }
        async fn is_ac(st: &St) -> u64 {
            let r = job_create(State(st.clone()), None, HeaderMap::new(), Json(CreateJob {
                prompt: "2+2?".into(), deterministic: Some(true), fee_lsc: None, payer: None, min_tier: None,
            })).await;
            r.0["job_id"].as_u64().expect("is acilmali")
        }

        #[tokio::test]
        async fn oturum_uclari_saldiri_engelli() {
            let st = test_st("a");
            let t = now_secs();
            // A: MetaMask (EVM) cuzdan + tarayici oturum anahtari
            let (a_sk, a_w) = evm_cuzdan(21);
            let osk = ed25519_dalek::SigningKey::from_bytes(&[61; 32]);
            let o_nonce = worker_mesaj::oturum_nonce(&osk.verifying_key().to_bytes());
            // Saldirgan A adina oturum kurmaya calisir (kendi EVM anahtariyla) -> 401
            let (sald_sk, _) = evm_cuzdan(22);
            let r = kayit(&st, &a_w, Some(pk_hex(&osk)), evm_imzala(&sald_sk, &a_w, &o_nonce, t)).await;
            assert_eq!(r.0, StatusCode::UNAUTHORIZED, "{:?}", r.1 .0);
            assert!(st.lock().await.oturumlar.is_empty());
            // imzasiz kayit -> 401
            let r = kayit(&st, &a_w, Some(pk_hex(&osk)), Default::default()).await;
            assert_eq!(r.0, StatusCode::UNAUTHORIZED);
            // A'nin gercek EIP-191 oturum imzasi -> 200 + oturum_bitis
            let r = kayit(&st, &a_w, Some(pk_hex(&osk)), evm_imzala(&a_sk, &a_w, &o_nonce, t)).await;
            assert_eq!(r.0, StatusCode::OK, "{:?}", r.1 .0);
            let bitis = r.1 .0["oturum_bitis"].as_u64().expect("oturum_bitis");
            assert!(bitis >= t + cuzdan_imza::OTURUM_SURE_SN && bitis <= t + cuzdan_imza::OTURUM_SURE_SN + 5, "{bitis}");

            // B (kurban): kendi EVM cuzdaniyla normal kayit
            let (b_sk, b_w) = evm_cuzdan(23);
            assert_eq!(kayit(&st, &b_w, None, evm_imzala(&b_sk, &b_w, "kayit", t)).await.0, StatusCode::OK);
            // oturum anahtari "kayit" icin kabul edilmez (kayit yalniz cuzdan anahtariyla)
            assert_eq!(kayit(&st, &b_w, None, oturum_imzala(&osk, &b_w, "kayit", t)).await.0, StatusCode::UNAUTHORIZED);

            let is1 = is_ac(&st).await;
            // A'nin oturum anahtariyla B adina poll -> 401 (kurbanin slotunu kapamaz)
            assert_eq!(poll(&st, &b_w, oturum_imzala(&osk, &b_w, "poll", t)).await.0, StatusCode::UNAUTHORIZED);
            // imzasiz poll -> 401
            assert_eq!(poll(&st, &a_w, Default::default()).await.0, StatusCode::UNAUTHORIZED);
            // A oturumla poll -> is
            let r = poll(&st, &a_w, oturum_imzala(&osk, &a_w, "poll", t)).await;
            assert_eq!(r.0, StatusCode::OK);
            assert_eq!(r.1 .0["job_id"].as_u64(), Some(is1));
            // B kendi imzasiyla poll -> ayni is B'ye de atanir
            let r = poll(&st, &b_w, evm_imzala(&b_sk, &b_w, "poll", t)).await;
            assert_eq!(r.1 .0["job_id"].as_u64(), Some(is1));
            // A'nin oturumuyla B adina submit -> 401 (B'nin oyunu/odulunu yonlendiremez)
            let n = worker_mesaj::is_nonce(is1, "4");
            assert_eq!(submit(&st, &b_w, is1, "4", oturum_imzala(&osk, &b_w, &n, t)).await.0, StatusCode::UNAUTHORIZED);
            // A oturumla kendi submit'i -> 200, sonuc A adina
            let r = submit(&st, &a_w, is1, "4", oturum_imzala(&osk, &a_w, &n, t)).await;
            assert_eq!(r.0, StatusCode::OK, "{:?}", r.1 .0);
            {
                let c = st.lock().await;
                let j = &c.jobs[&is1];
                assert_eq!(j.results.len(), 1);
                assert_eq!(j.results[0].worker, a_w.to_lowercase());
            }

            // Suresi dolmus oturum -> 401
            st.lock().await.oturumlar.get_mut(&a_w.to_lowercase()).unwrap().bitis = t.saturating_sub(1);
            let r = poll(&st, &a_w, oturum_imzala(&osk, &a_w, "poll", t)).await;
            assert_eq!(r.0, StatusCode::UNAUTHORIZED);
            assert!(r.1 .0["hata"].as_str().unwrap().contains("oturum suresi doldu"));

            // ed25519 yerel cuzdan eskisi gibi dogrudan calisir
            let (e_sk, e_w) = ed_cuzdan(24);
            assert_eq!(kayit(&st, &e_w, None, ed_imzala(&e_sk, &e_w, "kayit", t)).await.0, StatusCode::OK);
            assert_eq!(poll(&st, &e_w, ed_imzala(&e_sk, &e_w, "poll", t)).await.0, StatusCode::OK);
            let yol = st.lock().await.cfg.data_path.clone();
            let _ = std::fs::remove_dir_all(std::path::Path::new(&yol).parent().unwrap());
        }
    }

    // Bulgu 8e: otomatik settlement varsayılan KAPALI; yalnız açıkça "1".
    #[test]
    fn oto_settlement_varsayilan_kapali() {
        assert!(!settle_auto_coz(None));
        for v in ["", "0", "true", "yes", "evet", "on", "11"] {
            assert!(!settle_auto_coz(Some(v.into())), "{v}");
        }
        assert!(settle_auto_coz(Some("1".into())));
        assert!(settle_auto_coz(Some(" 1 ".into())));
    }
}
