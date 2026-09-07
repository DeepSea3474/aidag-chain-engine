//! zincir - deterministik zincir sorgu araci
use serde_json::json;

fn niyet_cikar(sorgu: &str) -> Option<(&'static str, serde_json::Value, String)> {
    let s = sorgu.to_lowercase();
    if s.contains("bakiye") || s.contains("balance") {
        if let Some(adr) = adres_bul(sorgu) {
            return Some(("eth_getBalance", json!([adr, "latest"]),
                format!("{} adresinin bakiyesi", adr)));
        }
    }
    if s.contains("blok") || s.contains("block") || s.contains("yukseklik") {
        if s.contains("kac") || s.contains("son") || s.contains("number") || s.contains("numara") {
            return Some(("eth_blockNumber", json!([]), "guncel blok yuksekligi".to_string()));
        }
    }
    None
}

fn adres_bul(s: &str) -> Option<String> {
    for kelime in s.split_whitespace() {
        let k = kelime.trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
        if k.len() == 42 && k.starts_with("0x") && k[2..].chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(k.to_string());
        }
    }
    None
}

fn hex_to_dec_str(hex: &str) -> String {
    let h = hex.trim_start_matches("0x");
    match u128::from_str_radix(h, 16) {
        Ok(v) => v.to_string(),
        Err(_) => hex.to_string(),
    }
}

pub async fn sorgula(http: &reqwest::Client, rpc_url: &str, sorgu: &str) -> Option<String> {
    let (method, params, aciklama) = niyet_cikar(sorgu)?;
    let istek = json!({"jsonrpc":"2.0","method":method,"params":params,"id":1});
    let resp = http.post(rpc_url).json(&istek).send().await.ok()?;
    let v: serde_json::Value = resp.json().await.ok()?;
    let sonuc = v.get("result")?.as_str()?;
    let okunur = if sonuc.starts_with("0x") { hex_to_dec_str(sonuc) } else { sonuc.to_string() };
    Some(format!("{}: {}", aciklama, okunur))
}

// ── AG DURUMU ARACI ─────────────────────────────────────────────────────────
// Canli zincirin /status ucundan anlik ag sagligini ceker. Ayni zamanda
// GENEL DIS-SISTEM ADAPTORU sablonudur: bir HTTP GET -> JSON parse -> insan
// okunur ozet. Kurumsal entegrasyonlar (stok, IK, dokuman sistemi) ayni
// kalipla eklenir: url degistir, alanlari esle, formatla.

fn ag_niyeti_mi(sorgu: &str) -> bool {
    let s = sorgu.to_lowercase();
    let anahtarlar = [
        "ag durumu", "ağ durumu", "ag saglik", "ağ sağlık", "network durum",
        "kac dugum", "kaç düğüm", "kac node", "kaç node", "dugum sayisi",
        "tps", "ag nasil", "ağ nasıl", "zincir durum", "network status",
        "kac vertex", "kaç vertex", "tip sayisi", "orphan",
    ];
    anahtarlar.iter().any(|a| s.contains(a))
}

/// /status'tan ag durumu ceker, insan-okunur ozet doner. RPC yaninda status
/// ucu (8645) ayni sunucuda; buradaki base_url RPC ile ayni ana makinedir.
pub async fn ag_durumu(http: &reqwest::Client, rpc_url: &str, sorgu: &str) -> Option<String> {
    if !ag_niyeti_mi(sorgu) {
        return None;
    }
    // RPC url'inden status url tureti: .../  -> ayni host, /status yolu.
    // rpc_url ornegi: http://127.0.0.1:8645  (JSON-RPC ayni portta /status sunar)
    let status_url = format!("{}/status", rpc_url.trim_end_matches('/'));
    let resp = http.get(&status_url).send().await.ok()?;
    let v: serde_json::Value = resp.json().await.ok()?;

    let vertex = v.get("vertex_count").and_then(|x| x.as_u64()).unwrap_or(0);
    let tip = v.get("tip_count").and_then(|x| x.as_u64()).unwrap_or(0);
    let orphan = v.get("orphan_count").and_then(|x| x.as_u64()).unwrap_or(0);
    let net = v.get("network_id").and_then(|x| x.as_u64()).unwrap_or(0);
    let staker = v.get("staker_count").and_then(|x| x.as_u64()).unwrap_or(0);

    // Basit saglik yorumu (deterministik, uydurma yok).
    let saglik = if orphan == 0 && tip >= 1 {
        "saglikli (tek tip, kopuk blok yok)"
    } else if orphan > 0 {
        "dikkat: kopuk (orphan) blok var"
    } else {
        "tip yok (baslangic/bekleme)"
    };

    Some(format!(
        "AIDAG ag durumu (canli, network {}): {} blok (vertex), {} aktif tip, {} orphan, {} staker. Durum: {}.",
        net, vertex, tip, orphan, staker, saglik
    ))
}
