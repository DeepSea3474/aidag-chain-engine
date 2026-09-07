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
