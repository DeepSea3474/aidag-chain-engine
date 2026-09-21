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
    system_prompt: &str,
    user_content: &str,
    temp: f64,
    max_tokens: usize,
    tx: &tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
) -> Result<String, String> {
    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_content }
        ],
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
    let mut buf = String::new();
    let mut byte_stream = resp.bytes_stream();

    while let Some(chunk) = byte_stream.next().await {
        let bytes = chunk.map_err(|e| format!("stream okuma hatasi: {e}"))?;
        buf.push_str(&String::from_utf8_lossy(&bytes));

        // SSE satirlari "data: {...}\n\n" formatinda gelir; satir satir isle.
        while let Some(nl) = buf.find('\n') {
            let line = buf[..nl].trim().to_string();
            buf.drain(..=nl);
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
