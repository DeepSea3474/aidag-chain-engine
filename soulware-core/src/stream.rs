//! SSE streaming: KUBRA cevabini harf harf (token token) akitir.
//! Beyin (llama-server, OpenAI-uyumlu) stream:true ile cagirilir, gelen
//! delta'lar SSE event olarak tarayiciya akar. Cevap bitince cagiran taraf
//! zincire yazar ve proof event'i gonderir.
use axum::response::sse::Event;
use futures::StreamExt;
use std::convert::Infallible;

/// Beyne stream:true ile baglanir, token'lari kanal uzerinden yollar.
/// Donus: uretilen tam metin (zincire yazim icin).
pub async fn beyin_stream(
    http: &reqwest::Client,
    remote_url: &str,
    model: &str,
    messages: serde_json::Value, // sistem + örnek turlar + kullanıcı (main::mesajlar)
    temp: f64,
    max_tokens: usize,
    tx: &tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
) -> Result<String, String> {
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temp,
        "stream": true,
        "stop": ["\nuser", "user\n", "\nUser", "<|im_end|>", "<|im_start|>", "\nSORU:"],
    });

    let resp = http.post(remote_url)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("stream istegi basarisiz: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("beyin HTTP {}", resp.status()));
    }

    let mut tam_metin = String::new();
    // HAM BAYT tamponu: ağ parçası çok baytlı bir harfi (ş, ğ, ı, ü) ikiye bölebilir.
    // Her parçayı ayrı UTF-8 çözmek o harfi U+FFFD'ye bozar (tarayıcıya ve zincire
    // yazılan hash'e bozuk metin gider). Yalnız TAM satırlar çözülür ('\n' = 0x0A
    // hiçbir çok baytlı UTF-8 dizisinin içinde geçmez → satır sınırı güvenli).
    let mut buf: Vec<u8> = Vec::new();
    let mut byte_stream = resp.bytes_stream();

    while let Some(chunk) = byte_stream.next().await {
        let bytes = chunk.map_err(|e| format!("stream okuma hatasi: {e}"))?;
        buf.extend_from_slice(&bytes);

        // SSE satirlari "data: {...}\n\n" formatinda gelir; satir satir isle.
        for line in tam_satirlar(&mut buf) {
            if let Some(json_str) = line.strip_prefix("data: ") {
                if json_str.trim() == "[DONE]" { continue; }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(delta) = v.get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|t| t.as_str())
                    {
                        if !delta.is_empty() {
                            tam_metin.push_str(delta);
                            // Token'i tarayiciya akit
                            let _ = tx.send(Ok(Event::default().event("token").data(delta))).await;
                        }
                    }
                }
            }
        }
    }
    Ok(tam_metin)
}

/// Tampondaki TAM satırları (\n ile biten) çıkarıp UTF-8 çözer; yarım satır
/// (ve içindeki yarım harf) bir sonraki ağ parçasını beklemek üzere tamponda kalır.
fn tam_satirlar(buf: &mut Vec<u8>) -> Vec<String> {
    let mut out = Vec::new();
    while let Some(nl) = buf.iter().position(|&b| b == b'\n') {
        let satir: Vec<u8> = buf.drain(..=nl).collect();
        out.push(String::from_utf8_lossy(&satir).trim().to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::tam_satirlar;

    #[test]
    fn parcalara_bolunen_turkce_harf_bozulmaz() {
        let tam = "data: {\"t\":\"şğıü\"}\n".as_bytes();
        // "ş" (0xC5 0x9F) tam ortasından bölünür.
        let kes = tam.iter().position(|&b| b == 0xC5).unwrap() + 1;
        let mut buf = Vec::new();
        buf.extend_from_slice(&tam[..kes]);
        assert!(tam_satirlar(&mut buf).is_empty(), "yarım satır beklemede kalmalı");
        buf.extend_from_slice(&tam[kes..]);
        let satirlar = tam_satirlar(&mut buf);
        assert_eq!(satirlar, vec!["data: {\"t\":\"şğıü\"}".to_string()]);
        assert!(!satirlar[0].contains('\u{FFFD}'));
        assert!(buf.is_empty());
    }
}
