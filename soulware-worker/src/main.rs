//! soulware-worker — indirilen KUBRA istemcisi (katkıcı düğüm) v0.1
//! ════════════════════════════════════════════════════════════════════════
//! İnsanlar KUBRA'yı indirir, ÜCRETSİZ kullanır; ve İSTERLERSE (açık izin/consent)
//! boştaki güçlerini (GPU/CPU) ağa verir → koordinatörden iş çeker, KUBRA'yı
//! çalıştırır, sonucu gönderir. Doğrulanınca zincirde ödül (LSC) kazanır.
//!
//! İZİN ŞART: GPU/CPU katkısı YALNIZCA açık onayla. Onay yoksa katkı YAPILMAZ.
//!   Onay: SOULWARE_CONSENT=yes  (uygulamada: "boştaki GPU'mu ağa ver" kutusu)
//!
//! Beyin: yerel soulware-core (KUBRA) /v1/ask ucu (deterministic=greedy → doğrulanabilir).
//! Env: SOULWARE_COORD_URL · SOULWARE_BRAIN_URL · SOULWARE_WORKER_KEY · SOULWARE_POLL_SEC

#[path = "../../soulware-core/src/imza_dosyasi.rs"]
mod imza_dosyasi; // cüzdan anahtarı: FAIL-CLOSED (soulware-core ile ortak kod)
#[path = "../../soulware-coordinator/src/worker_mesaj.rs"]
mod worker_mesaj; // koordinatörün doğruladığı imza mesajı (ortak biçim)

use ed25519_dalek::{Signer, SigningKey};
use lsc_engine::public_key_to_adres;
use serde_json::{json, Value};
use std::time::Duration;

fn ev(k: &str, d: &str) -> String { std::env::var(k).unwrap_or_else(|_| d.to_string()) }

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Cüzdan sahipliği imza alanları (worker_mesaj.rs biçimi, ed25519).
fn imza_alanlari(key: &SigningKey, wallet: &str, nonce: &str) -> serde_json::Map<String, Value> {
    let ts = now_secs();
    let m = worker_mesaj::mesaj(wallet, nonce, ts);
    let mut o = serde_json::Map::new();
    o.insert("ts".into(), json!(ts));
    o.insert("imza".into(), json!(hex::encode(key.sign(m.as_bytes()).to_bytes())));
    o.insert("pubkey".into(), json!(hex::encode(key.verifying_key().to_bytes())));
    o
}

/// Gövdeye imza alanlarını ekle.
fn imzali(mut govde: Value, key: &SigningKey, wallet: &str, nonce: &str) -> Value {
    if let Some(o) = govde.as_object_mut() {
        o.extend(imza_alanlari(key, wallet, nonce));
    }
    govde
}

#[tokio::main]
async fn main() {
    let coord = ev("SOULWARE_COORD_URL", "http://127.0.0.1:8647");
    let brain = ev("SOULWARE_BRAIN_URL", "http://127.0.0.1:8646");
    let key_path = ev("SOULWARE_WORKER_KEY", "/root/aidag-lsc/.soulware-worker.key");
    let poll_sec: u64 = ev("SOULWARE_POLL_SEC", "3").parse().unwrap_or(3);
    let consent = ev("SOULWARE_CONSENT", "no").to_lowercase();

    // Cüzdan (AIDAG adresi) = worker kimliği + ödül alıcısı. FAIL-CLOSED: dosya yoksa
    // ya da bozuksa BAŞLAMAZ (ödül adresi habersizce değişmesin); ilk kurulumda
    // `soulware-worker --yeni-anahtar-uret` (dosya 0600, var olanın üzerine yazmaz).
    let argumanlar: Vec<String> = std::env::args().skip(1).collect();
    match argumanlar.as_slice() {
        [] => {}
        [a] if a == "--yeni-anahtar-uret" => match imza_dosyasi::uret(&key_path) {
            Ok(k) => {
                println!("YENI CUZDAN ANAHTARI URETILDI: {key_path}\n   cuzdan: 0x{}",
                    hex::encode(public_key_to_adres(&k.verifying_key().to_bytes())));
                return;
            }
            Err(e) => { eprintln!("HATA: {e}"); std::process::exit(2); }
        },
        _ => {
            eprintln!("HATA: bilinmeyen arguman: {argumanlar:?}. Gecerli: (yok) | --yeni-anahtar-uret");
            std::process::exit(2);
        }
    }
    let key = match imza_dosyasi::yukle(&key_path) {
        Ok(k) => k,
        Err(e) => { eprintln!("HATA: {e}"); std::process::exit(2); }
    };
    let wallet = format!("0x{}", hex::encode(public_key_to_adres(&key.verifying_key().to_bytes())));

    println!("──────────────────────────────────────────────");
    println!("💻 SoulwareAI Worker (KUBRA istemcisi) v0.1");
    println!("   cüzdan (ödül): {wallet}");
    println!("   koordinatör  : {coord}");
    println!("   beyin (KUBRA): {brain}");
    println!("──────────────────────────────────────────────");

    // RIZA KAPISI: açık onay yoksa katkı YAPMA (GPU/CPU kullanılmaz).
    if consent != "yes" && consent != "evet" && consent != "true" {
        println!("⛔ Katkı için AÇIK İZİN gerekli. Boştaki gücünü ağa vermek istiyorsan:");
        println!("   SOULWARE_CONSENT=yes ile başlat (uygulamada: 'boştaki GPU'mu ağa ver' kutusu).");
        println!("   İzin olmadan hiçbir kaynak kullanılmaz. KUBRA'yı kullanmak yine ÜCRETSİZ.");
        return;
    }
    println!("✅ İzin verildi — boştaki güç ağa katkı sağlayacak (istediğin an durdurabilirsin).");

    let http = reqwest::Client::builder().timeout(Duration::from_secs(180)).build().expect("http");

    // Kaydol.
    match http.post(format!("{coord}/worker/register"))
        .json(&imzali(json!({ "wallet": wallet }), &key, &wallet, worker_mesaj::NONCE_KAYIT)).send().await {
        Ok(r) => match r.json::<Value>().await {
            Ok(v) if v.get("ok").and_then(|o| o.as_bool()).unwrap_or(false) => println!("📝 kaydolundu."),
            _ => { eprintln!("kayıt reddedildi"); return; }
        },
        Err(e) => { eprintln!("koordinatöre ulaşılamadı: {e}"); return; }
    }

    // ── ÖZ-KIYASLAMA: seviye (tier) belirle → iş yeteneğe göre dağıtılsın ──
    // Worker altın soruları kendi beyniyle (KUBRA) yanıtlar; koordinatör puanlar.
    // Büyük-GPU worker yüksek tier alır → zor işleri o alır (küçük model zayıf halka olmaz).
    if ev("SOULWARE_BENCHMARK", "yes") != "no" {
        if let Ok(r) = http.get(format!("{coord}/worker/benchmark/{wallet}")).send().await {
            if let Ok(v) = r.json::<Value>().await {
                if let Some(sorular) = v.get("sorular").and_then(|s| s.as_array()) {
                    println!("🎓 öz-kıyaslama: {} altın soru yanıtlanıyor...", sorular.len());
                    let mut cevaplar = Vec::new();
                    for q in sorular {
                        let (Some(id), Some(soru)) = (q.get("id").and_then(|x| x.as_u64()), q.get("soru").and_then(|x| x.as_str())) else { continue };
                        let t0 = std::time::Instant::now();
                        let cevap = match http.post(format!("{brain}/v1/ask"))
                            .json(&json!({ "prompt": soru, "deterministic": true }))
                            .send().await {
                            Ok(r) => r.json::<Value>().await.ok()
                                .and_then(|v| v.get("answer").and_then(|a| a.as_str()).map(|s| s.to_string()))
                                .unwrap_or_default(),
                            Err(_) => String::new(),
                        };
                        cevaplar.push(json!({ "id": id, "cevap": cevap, "ms": t0.elapsed().as_millis() as u64 }));
                    }
                    let ozet: Vec<(u64, u64, &str)> = cevaplar.iter().filter_map(|c| Some((
                        c.get("id")?.as_u64()?, c.get("ms")?.as_u64()?, c.get("cevap")?.as_str()?))).collect();
                    let nonce = worker_mesaj::benchmark_nonce(&ozet);
                    if let Ok(r) = http.post(format!("{coord}/worker/benchmark"))
                        .json(&imzali(json!({ "wallet": wallet, "cevaplar": cevaplar }), &key, &wallet, &nonce)).send().await {
                        if let Ok(v) = r.json::<Value>().await {
                            println!("🎓 seviye belirlendi: tier={} (doğru {}/{} · {}ms ort.)",
                                v.get("tier").and_then(|x| x.as_u64()).unwrap_or(0),
                                v.get("dogru").and_then(|x| x.as_u64()).unwrap_or(0),
                                v.get("toplam").and_then(|x| x.as_u64()).unwrap_or(0),
                                v.get("ort_gecikme_ms").and_then(|x| x.as_f64()).unwrap_or(0.0) as u64);
                        }
                    }
                }
            }
        }
    }

    // Ana döngü: iş çek → KUBRA çalıştır → gönder.
    loop {
        let poll_imza: Vec<(String, String)> = imza_alanlari(&key, &wallet, worker_mesaj::NONCE_POLL)
            .into_iter().map(|(k, v)| (k, v.as_str().map(String::from).unwrap_or_else(|| v.to_string()))).collect();
        let is: Option<Value> = match http.get(format!("{coord}/worker/poll/{wallet}")).query(&poll_imza).send().await {
            Ok(r) => r.json::<Value>().await.ok(),
            Err(_) => None,
        };
        let (job_id, prompt, det) = match is {
            Some(v) if v.get("none").and_then(|n| n.as_bool()).unwrap_or(false) => {
                tokio::time::sleep(Duration::from_secs(poll_sec)).await;
                continue;
            }
            Some(v) => {
                let id = v.get("job_id").and_then(|x| x.as_u64());
                let p = v.get("prompt").and_then(|x| x.as_str()).map(|s| s.to_string());
                let d = v.get("deterministic").and_then(|x| x.as_bool()).unwrap_or(true);
                match (id, p) { (Some(id), Some(p)) => (id, p, d), _ => { tokio::time::sleep(Duration::from_secs(poll_sec)).await; continue; } }
            }
            None => { tokio::time::sleep(Duration::from_secs(poll_sec)).await; continue; }
        };

        println!("⚙  iş #{job_id} alındı → KUBRA çalıştırılıyor...");
        // KUBRA'yı çağır (deterministic → doğrulanabilir birebir çıktı).
        let cevap = match http.post(format!("{brain}/v1/ask"))
            .json(&json!({ "prompt": prompt, "deterministic": det }))
            .send().await {
            Ok(r) => match r.json::<Value>().await {
                Ok(v) if v.get("ok").and_then(|o| o.as_bool()).unwrap_or(false) =>
                    v.get("answer").and_then(|a| a.as_str()).unwrap_or("").to_string(),
                Ok(v) => { eprintln!("beyin hata: {}", v.get("hata").and_then(|h| h.as_str()).unwrap_or("?")); continue; }
                Err(e) => { eprintln!("beyin yanıtı: {e}"); continue; }
            },
            Err(e) => { eprintln!("beyin isteği: {e}"); continue; }
        };
        if cevap.trim().is_empty() { eprintln!("boş cevap, atlanıyor"); continue; }

        // Sonucu gönder.
        match http.post(format!("{coord}/worker/submit"))
            .json(&imzali(json!({ "wallet": wallet, "job_id": job_id, "answer": cevap }), &key, &wallet,
                &worker_mesaj::is_nonce(job_id, &cevap)))
            .send().await {
            Ok(r) => if let Ok(v) = r.json::<Value>().await {
                let durum = v.get("durum").and_then(|d| d.as_str()).unwrap_or("?");
                if let Some(od) = v.get("oduller") {
                    println!("💰 iş #{job_id}: {durum} — ödül: {od}");
                } else {
                    println!("📤 iş #{job_id}: {durum}");
                }
            },
            Err(e) => eprintln!("gönderim hata: {e}"),
        }
    }
}
