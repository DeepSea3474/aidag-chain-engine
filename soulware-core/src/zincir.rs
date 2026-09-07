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

// ── BELGE DOGRULAMA ARACI ───────────────────────────────────────────────────
// Kullanici bir belge hash'i (64 hex = 32 bayt) verip "bu belge gecerli mi /
// zincirde var mi / degistirilmis mi" diye sorunca, /belge/:hash ucundan
// dogrular. KURUMSAL DEGER: sahte/degistirilmis belge saniyede yakalanir,
// cevap tahrif edilemez zincir kaydina dayanir. Dis-sistem adaptor sablonu.

fn belge_niyeti_mi(sorgu: &str) -> bool {
    let s = sorgu.to_lowercase();
    let anahtarlar = [
        "belge", "dogrula", "doğrula", "gecerli mi", "geçerli mi", "sahte mi",
        "degistirilmis", "değiştirilmiş", "orijinal mi", "zincirde var mi",
        "zincirde var mı", "hash", "belge sorgu", "document", "verify",
    ];
    anahtarlar.iter().any(|a| s.contains(a))
}

/// Sorgudan 64-hex belge hash'i cikar (0x opsiyonel).
fn belge_hash_bul(s: &str) -> Option<String> {
    for kelime in s.split_whitespace() {
        let k = kelime.trim_start_matches("0x")
            .trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
        if k.len() == 64 && k.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(k.to_string());
        }
    }
    None
}

/// Belge dogrulama: hash zincirde kayitli mi? /belge/:hash ucundan.
pub async fn belge_dogrula(http: &reqwest::Client, rpc_url: &str, sorgu: &str) -> Option<String> {
    if !belge_niyeti_mi(sorgu) {
        return None;
    }
    let hash = match belge_hash_bul(sorgu) {
        Some(h) => h,
        None => {
            return Some(
                "Belgeni dogrulamak icin Belge Dogrulama sayfasini kullan: https://aidag-chain.com/belge — oraya dosyani (PDF, resim, Word) yukle, sistem saniyede zincirde kayitli mi, orijinal mi yoksa degistirilmis mi soyler. Belgen tarayicindan cikmaz; yalnizca matematiksel ozeti kontrol edilir. Elinde hazir bir belge hash'i (64 hex) varsa bana dogrudan yazabilirsin, hemen dogrularim.".to_string()
            );
        }
    };
    let url = format!("{}/belge/{}", rpc_url.trim_end_matches('/'), hash);
    let resp = http.get(&url).send().await.ok()?;
    let v: serde_json::Value = resp.json().await.ok()?;

    // /belge/:hash yaniti: kayitli mi + (varsa) zaman/blok bilgisi.
    let kayitli = v.get("kayitli").and_then(|x| x.as_bool())
        .or_else(|| v.get("var").and_then(|x| x.as_bool()))
        .unwrap_or(false);

    if kayitli {
        Some(format!(
            "Belge DOGRULANDI: bu hash zincirde kayitli ({}). Belge orijinal ve degistirilmemis (zincire islenen kayitla birebir eslesiyor). Denetlenebilir, tahrif edilemez.",
            &hash[..16]
        ))
    } else {
        Some(format!(
            "Belge BULUNAMADI: bu hash ({}...) zincirde kayitli DEGIL. Ya hic kaydedilmemis ya da belge degistirilmis (hash tutmuyor). Orijinal belgenin hash'iyle tekrar dene.",
            &hash[..16]
        ))
    }
}
