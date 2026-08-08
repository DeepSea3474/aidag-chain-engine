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

use ed25519_dalek::SigningKey;
use lsc_engine::public_key_to_adres;
use serde_json::{json, Value};
use std::time::Duration;

fn ev(k: &str, d: &str) -> String { std::env::var(k).unwrap_or_else(|_| d.to_string()) }

fn anahtar_yukle_veya_uret(path: &str) -> SigningKey {
    if let Ok(data) = std::fs::read(path) {
        if data.len() == 33 && data[0] == 1 {
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&data[1..33]);
            return SigningKey::from_bytes(&seed);
        }
    }
    use rand::RngCore;
    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let mut dosya = Vec::with_capacity(33);
    dosya.push(1u8);
    dosya.extend_from_slice(&seed);
    let _ = std::fs::write(path, &dosya);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    SigningKey::from_bytes(&seed)
}

#[tokio::main]
async fn main() {
    let coord = ev("SOULWARE_COORD_URL", "http://127.0.0.1:8647");
    let brain = ev("SOULWARE_BRAIN_URL", "http://127.0.0.1:8646");
    let key_path = ev("SOULWARE_WORKER_KEY", "/root/aidag-lsc/.soulware-worker.key");
    let poll_sec: u64 = ev("SOULWARE_POLL_SEC", "3").parse().unwrap_or(3);
    let consent = ev("SOULWARE_CONSENT", "no").to_lowercase();

    // Cüzdan (AIDAG adresi) = worker kimliği + ödül alıcısı.
    let key = anahtar_yukle_veya_uret(&key_path);
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
    match http.post(format!("{coord}/worker/register")).json(&json!({ "wallet": wallet })).send().await {
        Ok(r) => match r.json::<Value>().await {
            Ok(v) if v.get("ok").and_then(|o| o.as_bool()).unwrap_or(false) => println!("📝 kaydolundu."),
            _ => { eprintln!("kayıt reddedildi"); return; }
        },
        Err(e) => { eprintln!("koordinatöre ulaşılamadı: {e}"); return; }
    }

    // Ana döngü: iş çek → KUBRA çalıştır → gönder.
    loop {
        let is: Option<Value> = match http.get(format!("{coord}/worker/poll/{wallet}")).send().await {
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
            .json(&json!({ "prompt": prompt, "deterministic": det, "brain": "local" }))
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
            .json(&json!({ "wallet": wallet, "job_id": job_id, "answer": cevap }))
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
