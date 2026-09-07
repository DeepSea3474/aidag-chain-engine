//! GERCEK TPS benchmark — lsc-engine'in kendi Vertex/wire/tx/registry
//! kodunu DOGRUDAN kullanir. Taklit/sahte imza YOK, gercek ed25519 imzasi.
//!
//! Tasarim:
//!   - ACIK DONGU: sabit hizda gonder, onceki cevabi bekleme
//!   - RING TRANSFER: worker[i] -> worker[(i+1)%N]. Bakiye dairesel dolasir,
//!     tukenmez (tek yonlu drenaj yok).
//!   - Tips (parent) periyodik arka planda yenilenir (1sn) — gercek istemci
//!     davranisi budur, tek seferlik snapshot degil.
//!   - Steady-state (warmup/cooldown haric) OLCULUR, denetim standardi.
//!
//! Kurulum (bir kere): lsc-net/Cargo.toml'a `reqwest` eklenmesi lazim,
//! asagidaki mesajda ayri veriyorum.
//!
//! Calistirma:
//!   cargo build --release --bin tps_bench -p lsc-net
//!   ./target/release/tps_bench http://127.0.0.1:8645

use ed25519_dalek::SigningKey;
use lsc_engine::dag::vertex::{Vertex, VertexId};
use lsc_engine::dag::wire;
use lsc_engine::registry::public_key_to_adres;
use lsc_engine::tx::TransferKaydi;
use rand::rngs::OsRng;
use rand::RngCore;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;

// ===================== AYARLAR =====================
const NUM_WORKERS: usize = 20;
const TARGET_TPS: f64 = 100.0;
const WARMUP_SECS: u64 = 60;
const STEADY_SECS: u64 = 600;
const COOLDOWN_SECS: u64 = 30;
const TIPS_REFRESH_MS: u64 = 1000;
const TRANSFER_AMOUNT: u128 = 1;
const FAUCET_WAIT_TIMEOUT_SECS: u64 = 30;
// =====================================================

struct WorkerRecord {
    ok: u64,
    failed: u64,
    latencies_ms: Vec<f64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let rpc_base = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://127.0.0.1:8645".to_string());
    let client = reqwest::Client::new();

    // --- network_id'yi /status'tan al (TAHMIN YOK, canli sunucudan) ---
    let status: serde_json::Value = client
        .get(format!("{rpc_base}/status"))
        .send()
        .await?
        .json()
        .await?;
    let network_id = status["network_id"].as_u64().expect("network_id yok") as u32;
    println!("[+] network_id = {network_id}");

    // --- baslangic tips ---
    let tips = fetch_tips(&client, &rpc_base).await?;
    println!("[+] baslangic tip sayisi = {}", tips.len());
    let shared_tips: Arc<RwLock<Vec<VertexId>>> = Arc::new(RwLock::new(tips));

    // --- tips'i arka planda periyodik yenile (gercek istemci davranisi) ---
    {
        let client = client.clone();
        let rpc_base = rpc_base.clone();
        let shared_tips = shared_tips.clone();
        tokio::spawn(async move {
            loop {
                if let Ok(t) = fetch_tips(&client, &rpc_base).await {
                    if !t.is_empty() {
                        *shared_tips.write().await = t;
                    }
                }
                sleep(Duration::from_millis(TIPS_REFRESH_MS)).await;
            }
        });
    }

    // --- worker anahtarlari uret ---
    let mut keys: Vec<SigningKey> = Vec::with_capacity(NUM_WORKERS);
    for _ in 0..NUM_WORKERS {
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        keys.push(SigningKey::from_bytes(&seed));
    }
    let addrs: Vec<[u8; 20]> = keys
        .iter()
        .map(|k| public_key_to_adres(&k.verifying_key().to_bytes()))
        .collect();

    // --- FONLAMA: her worker'i faucet'ten fonla, GERCEKTEN bakiyesi
    //     olustugunu /bakiye ile dogrula (varsayim yok) ---
    println!("[+] Worker'lar fonlaniyor (faucet)...");
    for (i, adr) in addrs.iter().enumerate() {
        let adr_hex = hex::encode(adr);
        println!("    worker {i}: adres = {adr_hex}  (fonlaniyor...)");
        let _ = client
            .get(format!("{rpc_base}/faucet/{adr_hex}"))
            .send()
            .await;
        let deadline = Instant::now() + Duration::from_secs(FAUCET_WAIT_TIMEOUT_SECS);
        loop {
            let bak: serde_json::Value = client
                .get(format!("{rpc_base}/bakiye/{adr_hex}"))
                .send()
                .await?
                .json()
                .await?;
            // NOT: JSON alan adi tahmin — /bakiye ciktisini bir kez görüp
            // "bakiye" dogru anahtar mi teyit edelim (asagidaki mesajda soruyorum).
            let bakiye = bak["bakiye"].as_u64().unwrap_or(0);
            if bakiye > 0 {
                println!("    worker {i}: bakiye = {bakiye}");
                break;
            }
            if Instant::now() > deadline {
                eprintln!(
                    "    UYARI: worker {i} icin faucet bakiyesi {FAUCET_WAIT_TIMEOUT_SECS}s icinde onaylanamadi"
                );
                break;
            }
            sleep(Duration::from_millis(500)).await;
        }
    }

    // --- YUK TESTI ---
    let total_secs = WARMUP_SECS + STEADY_SECS + COOLDOWN_SECS;
    let t0 = Instant::now();
    let steady_start = t0 + Duration::from_secs(WARMUP_SECS);
    let steady_end = steady_start + Duration::from_secs(STEADY_SECS);
    let t_end = steady_end + Duration::from_secs(COOLDOWN_SECS);

    println!(
        "\n[+] Yuk testi basliyor: {NUM_WORKERS} worker, hedef {TARGET_TPS} tx/s (acik dongu, RING transfer)"
    );
    println!(
        "[+] Toplam sure: {total_secs}s (warmup {WARMUP_SECS} + steady {STEADY_SECS} + cooldown {COOLDOWN_SECS})\n"
    );

    let records: Arc<Mutex<Vec<WorkerRecord>>> = Arc::new(Mutex::new(
        (0..NUM_WORKERS)
            .map(|_| WorkerRecord {
                ok: 0,
                failed: 0,
                latencies_ms: Vec::new(),
            })
            .collect(),
    ));

    let mut handles = Vec::new();
    for i in 0..NUM_WORKERS {
        let client = client.clone();
        let rpc_base = rpc_base.clone();
        let shared_tips = shared_tips.clone();
        let sk_bytes = keys[i].to_bytes();
        // RING: worker i, worker (i+1)%N'e gonderir -> bakiye dairesel dolasir.
        let alici_adr = addrs[(i + 1) % NUM_WORKERS];
        let records = records.clone();
        let interval = Duration::from_secs_f64(NUM_WORKERS as f64 / TARGET_TPS);

        handles.push(tokio::spawn(async move {
            let sk = SigningKey::from_bytes(&sk_bytes);
            let mut nonce: u64 = 0;
            let mut next_send = Instant::now();
            loop {
                let now = Instant::now();
                if now >= t_end {
                    break;
                }
                if now < next_send {
                    sleep(next_send - now).await;
                }
                next_send += interval;

                let parents = shared_tips.read().await.clone();
                let payload = TransferKaydi::new(alici_adr, TRANSFER_AMOUNT, nonce).encode();
                let now_secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let vtx = match Vertex::new_signed(network_id, parents, payload, now_secs, &sk) {
                    Ok(v) => v,
                    Err(_) => {
                        nonce += 1;
                        continue;
                    }
                };
                let wire_bytes = wire::encode(&vtx);
                let hex_str = hex::encode(&wire_bytes);

                let send_t = Instant::now();
                let ok = match client
                    .post(format!("{rpc_base}/submit"))
                    .json(&serde_json::json!({ "hex": hex_str }))
                    .timeout(Duration::from_secs(10))
                    .send()
                    .await
                {
                    Ok(resp) => resp.status().is_success(),
                    Err(_) => false,
                };
                let latency_ms = send_t.elapsed().as_secs_f64() * 1000.0;
                nonce += 1;

                if send_t >= steady_start && send_t <= steady_end {
                    let mut r = records.lock().unwrap();
                    if ok {
                        r[i].ok += 1;
                    } else {
                        r[i].failed += 1;
                    }
                    r[i].latencies_ms.push(latency_ms);
                }
            }
        }));
    }

    // ilerleme yazdirici
    loop {
        sleep(Duration::from_secs(15)).await;
        let elapsed = t0.elapsed().as_secs();
        if elapsed >= total_secs {
            break;
        }
        let phase = if elapsed < WARMUP_SECS {
            "WARMUP"
        } else if elapsed < WARMUP_SECS + STEADY_SECS {
            "STEADY"
        } else {
            "COOLDOWN"
        };
        let total_ok: u64 = records.lock().unwrap().iter().map(|r| r.ok).sum();
        println!("    t={elapsed:>4}s [{phase}] steady-state basarili so far = {total_ok}");
    }

    for h in handles {
        let _ = h.await;
    }

    // ===================== RAPOR =====================
    let recs = records.lock().unwrap();
    let total_ok: u64 = recs.iter().map(|r| r.ok).sum();
    let total_failed: u64 = recs.iter().map(|r| r.failed).sum();
    let mut all_lat: Vec<f64> = recs.iter().flat_map(|r| r.latencies_ms.clone()).collect();
    all_lat.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let sustained_tps = total_ok as f64 / STEADY_SECS as f64;

    println!("\n============================================================");
    println!("SONUC — steady-state (warmup/cooldown haric), GERCEK imzali islemler");
    println!("============================================================");
    println!("Basarili (ingest edildi, HTTP 200):  {total_ok}");
    println!("Basarisiz:                            {total_failed}");
    println!("SURDURULEBILIR TPS:                   {sustained_tps:.1}");
    if !all_lat.is_empty() {
        let p50 = all_lat[all_lat.len() * 50 / 100];
        let p95 = all_lat[all_lat.len() * 95 / 100];
        let p99 = all_lat[(all_lat.len() * 99 / 100).min(all_lat.len() - 1)];
        println!("Gecikme p50 / p95 / p99:              {p50:.0}ms / {p95:.0}ms / {p99:.0}ms");
    }
    println!("============================================================");
    println!("\nNOT: Bu sayi HTTP 200 (DAG'a basariyla ingest) oranidir.");
    println!("Bakiye/nonce durumuna gore bazi islemler DAG'a girip de state");
    println!("degisikligi yapmamis olabilir (sessizce reddediliyor, node.rs");
    println!("tasarimi geregi). Kesin dogrulama icin testten SONRA birkac");
    println!("worker adresinin /bakiye'sini kontrol edelim.");

    Ok(())
}

async fn fetch_tips(
    client: &reqwest::Client,
    rpc_base: &str,
) -> Result<Vec<VertexId>, Box<dyn std::error::Error + Send + Sync>> {
    let v: serde_json::Value = client
        .get(format!("{rpc_base}/tips"))
        .send()
        .await?
        .json()
        .await?;
    let arr = v["tips"].as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    for t in arr {
        if let Some(s) = t.as_str() {
            if let Ok(bytes) = hex::decode(s) {
                if bytes.len() == 32 {
                    let mut id = [0u8; 32];
                    id.copy_from_slice(&bytes);
                    out.push(id);
                }
            }
        }
    }
    Ok(out)
}






























