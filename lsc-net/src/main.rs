//! LSC ag dugumu - calistirilabilir binary (Asama 1 testi).
//! Kullanim:
//!   cargo run --bin lsc-node -- <listen_addr> [dial_addr]

use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args: Vec<String> = env::args().collect();

    let listen_addr = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/ip4/0.0.0.0/tcp/0".to_string());

    // KONUM-BAGIMSIZ arguman cozumleme (mDNS otomatik kesif sayesinde dial_addr
    // ARTIK OPSIYONEL). arg[1] = listen_addr (zorunlu). Kalanlar:
    //   - "/ip4/..." ile baslayan -> manuel dial_addr (opsiyonel; mDNS varsa gereksiz)
    //   - "produce" / "listen" -> mod kelimesi (konumdan bagimsiz)
    // UC mod:
    //   mod kelimesi YOK   -> ANA URETICI: genesis URETIR + vertex URETIR
    //   "produce"          -> IKINCI URETICI: genesis URETMEZ (aga bagli, ceker)
    //                         + vertex URETIR (paralel DAG)
    //   "listen"           -> DINLEYICI: genesis URETMEZ + vertex URETMEZ
    // Tek genesis ilkesi: agda BIR genesis (ana uretici). Ikinci uretici ayni
    // genesis'in ustune paralel vertex ekler (iki genesis = bolunmus ag).
    let rest: Vec<&str> = args.iter().skip(2).map(|s| s.as_str()).collect();
    let dial_addr = rest.iter().copied().find(|a| a.starts_with("/ip4/"));
    let mode = rest
        .iter()
        .copied()
        .find(|a| *a == "produce" || *a == "listen");
    let is_producer = mode != Some("listen"); // vertex uretir mi (ana VEYA produce)
    let produce_genesis = mode.is_none(); // SADECE ana uretici genesis uretir
                                          // Uretici her 5 saniyede bir yeni vertex uretir; dinleyici uretmez.
                                          // GERCEKCILIK: otomatik sentetik vertex uretimi KAPATILDI (eski: her 5sn 'belge-N').
                                          // Vertex artik SADECE gercek islem (gercek transfer/faucet/eslestirme/belge) ile olusur.
    let _ = is_producer;
    let produce_interval: Option<std::time::Duration> = None;

    // KALICILIK: data dosyasi yolu, listen portuna gore otomatik (cakisma olmasin).
    // Ayni port -> ayni dosya -> restart'ta veri hatirlanir.
    // Explicit data dosyasi: ".log" ile biten arguman varsa onu kullan
    // (konum-bagimsiz); yoksa porttan turet.
    let explicit_data = rest.iter().copied().find(|a| a.ends_with(".log"));
    let data_file = if let Some(explicit) = explicit_data {
        Some(std::path::PathBuf::from(explicit))
    } else {
        // listen_addr ornegi: /ip4/127.0.0.1/tcp/40001 -> port "40001"
        let port = listen_addr.rsplit('/').next().unwrap_or("0");
        Some(std::path::PathBuf::from(format!("aidag-data-{port}.log")))
    };
    if let Some(ref df) = data_file {
        tracing::info!("Kalici veri dosyasi: {df:?}");
    }

    // KALICI KIMLIK: imzalama anahtari dosyasi, porta gore (cakisma olmasin).
    // Ayni port -> ayni anahtar -> restart'ta AYNI kimlik.
    let port = listen_addr.rsplit('/').next().unwrap_or("0");
    let key_file = Some(std::path::PathBuf::from(format!("aidag-key-{port}.bin")));
    if let Some(ref kf) = key_file {
        tracing::info!("Kalici kimlik anahtari: {kf:?}");
    }

    lsc_net::run_node(
        &listen_addr,
        dial_addr,
        produce_genesis,
        produce_interval,
        data_file,
        key_file,
    )
    .await
}
