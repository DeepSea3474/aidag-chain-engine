//! RPC katmani — zincir durumunu HTTP/JSON uzerinden disariya acar.
//!
//! Bu katman lsc-engine konsensusune DOKUNMAZ; NodeState'i SADECE OKUR
//! (Arc<RwLock<NodeState>> uzerinden paylasimli erisim). Gelistiriciler
//! zincire disaridan baglanip durum sorgulayabilsin diye ilk arayuz.
//!
//! Endpoint'ler (Asama 1 - okuma):
//!   GET /health        -> { "ok": true }            (dugum ayakta mi)
//!   GET /status        -> zincir durumu (vertex, token, stake, peer...)

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{mpsc::UnboundedSender, RwLock};

/// RPC sunucusunun paylastigi durum: NodeState'e okuma erisimi.
#[derive(Clone)]
pub struct RpcState {
    pub node: Arc<RwLock<lsc_engine::NodeState>>,
    /// /submit ile gelen+ingest edilen vertex'i AGA yayinlamak icin ag dongusune kanal.
    pub submit_tx: UnboundedSender<Vec<u8>>,
    /// Faucet vertex'ini (tip=6) imzalamak icin node imza anahtari (owner).
    pub signing_key: ed25519_dalek::SigningKey,
}

/// GET /health — dugum ayakta mi (en basit canlilik kontrolu).
async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}

/// GET /status — zincirin anlik durumu (okuma; konsensusu degistirmez).
async fn status(State(st): State<RpcState>) -> Json<Value> {
    let node = st.node.read().await;
    let genesis = node
        .genesis_id()
        .map(|id| hex::encode(&id[..8]))
        .unwrap_or_else(|| "yok".to_string());
    Json(json!({
        "network_id": node.network_id(),
        "vertex_count": node.vertex_count(),
        "orphan_count": node.orphan_count(),
        "token_count": node.token_sayisi(),
        "toplam_stake": node.toplam_stake(),
        "staker_count": node.staker_sayisi(),
        "genesis": genesis,
        "tip_count": node.tips().len(),
    }))
}

/// GET /tokens — kayitli kanonik token listesi (adres + sembol). KALKAN vitrini.
async fn tokens(State(st): State<RpcState>) -> Json<Value> {
    let node = st.node.read().await;
    let liste: Vec<Value> = node
        .tum_tokenlar()
        .iter()
        .map(|(adres, sembol)| {
            // sembol: sifir-dolgulu 8 bayt -> okunabilir string.
            let sym = String::from_utf8_lossy(sembol)
                .trim_end_matches('\0')
                .to_string();
            json!({
                "adres": hex::encode(adres),
                "sembol": sym,
            })
        })
        .collect();
    Json(json!({ "count": liste.len(), "tokens": liste }))
}

/// GET /avm-kontratlar — deploy edilmis AVM kontrat adresleri (canli deploy kaniti).
async fn avm_kontratlar(State(st): State<RpcState>) -> Json<Value> {
    let node = st.node.read().await;
    let liste: Vec<String> = node
        .avm_kontrat_adresleri()
        .iter()
        .map(hex::encode)
        .collect();
    Json(json!({ "count": liste.len(), "kontratlar": liste }))
}

/// GET /kurum/:adres — bir adres hangi kurum/firma? (ad, kategori, zaman).
async fn kurum(State(st): State<RpcState>, Path(adres_hex): Path<String>) -> Json<Value> {
    let adres_bytes = match hex::decode(adres_hex.trim()) {
        Ok(b) if b.len() == 20 => b,
        _ => return Json(json!({ "ok": false, "hata": "adres 20 bayt (40 hex) olmali" })),
    };
    let mut adres = [0u8; 20];
    adres.copy_from_slice(&adres_bytes);
    let node = st.node.read().await;
    match node.kurum_sorgula(&adres) {
        Some(k) => {
            let kat = match k.kategori {
                lsc_engine::KurumKategori::Devlet => "devlet",
                lsc_engine::KurumKategori::Ozel => "ozel",
            };
            Json(json!({
                "ok": true,
                "kayitli": true,
                "adres": adres_hex.trim(),
                "ad": k.ad,
                "kategori": kat,
                "zaman": k.zaman,
            }))
        }
        None => Json(json!({
            "ok": true,
            "kayitli": false,
            "adres": adres_hex.trim(),
        })),
    }
}

/// GET /belge/:hash — bir belge hash'i (64 hex = 32 bayt) zincirde kayitli mi?
/// Donus: kayitliysa kaydeden adres + zaman; degilse kayitli:false.
async fn belge(State(st): State<RpcState>, Path(hash_hex): Path<String>) -> Json<Value> {
    let hash_bytes = match hex::decode(hash_hex.trim()) {
        Ok(b) if b.len() == 32 => b,
        _ => return Json(json!({ "ok": false, "hata": "hash 32 bayt (64 hex) olmali" })),
    };
    let mut h = [0u8; 32];
    h.copy_from_slice(&hash_bytes);
    let node = st.node.read().await;
    match node.belge_dogrula(&h) {
        Some(kayit) => Json(json!({
            "ok": true,
            "kayitli": true,
            "hash": hash_hex.trim(),
            "kaydeden": hex::encode(kayit.kaydeden),
            "zaman": kayit.zaman,
        })),
        None => Json(json!({
            "ok": true,
            "kayitli": false,
            "hash": hash_hex.trim(),
        })),
    }
}

/// GET /bakiye/:adres — bir adresin serbest AIDAG bakiyesi (hex adres, 40 karakter).
/// GET /islemlerim/:pubkey — bir pubkey'in (64 hex) zincirdeki islemleri.
/// Kullanici kendi pubkey'iyle cagirir -> "Islemlerim" penceresi. Zincirden okur.
async fn islemlerim(State(st): State<RpcState>, Path(pubkey_hex): Path<String>) -> Json<Value> {
    let pk_bytes = match hex::decode(pubkey_hex.trim()) {
        Ok(b) if b.len() == 32 => b,
        _ => return Json(json!({ "ok": false, "hata": "pubkey 32 bayt (64 hex) olmali" })),
    };
    let mut pubkey = [0u8; 32];
    pubkey.copy_from_slice(&pk_bytes);
    let islemler = {
        let node = st.node.read().await;
        node.islemlerim(&pubkey)
    };
    let mut liste: Vec<Value> = Vec::new();
    for (tip, zaman, payload) in &islemler {
        let (tip_adi, detay) = match tip {
            4 => {
                let d = lsc_engine::tx::TransferKaydi::decode(payload).ok();
                (
                    "transfer".to_string(),
                    d.map(|t| json!({ "alici": hex::encode(t.alici), "miktar": t.miktar }))
                        .unwrap_or(json!({})),
                )
            }
            6 => {
                let d = lsc_engine::tx::FaucetKaydi::decode(payload).ok();
                (
                    "faucet".to_string(),
                    d.map(|f| json!({ "alici": hex::encode(f.alici), "miktar": f.miktar }))
                        .unwrap_or(json!({})),
                )
            }
            8 => {
                let d = lsc_engine::tx::EslestirmeKaydi::decode(payload).ok();
                ("eslestirme".to_string(), d.map(|e| json!({ "test": hex::encode(e.test_adresi), "gercek": hex::encode(e.gercek_adres) })).unwrap_or(json!({})))
            }
            7 => ("lsc_transfer".to_string(), json!({})),
            other => (format!("tip_{other}"), json!({})),
        };
        liste.push(json!({ "tip": tip_adi, "zaman": zaman, "detay": detay }));
    }
    Json(
        json!({ "ok": true, "pubkey": pubkey_hex.trim(), "islem_sayisi": liste.len(), "islemler": liste }),
    )
}

async fn bakiye(State(st): State<RpcState>, Path(adres_hex): Path<String>) -> Json<Value> {
    let adres_bytes = match hex::decode(adres_hex.trim()) {
        Ok(b) if b.len() == 20 => b,
        _ => return Json(json!({ "ok": false, "hata": "adres 20 bayt (40 hex) olmali" })),
    };
    let mut adres = [0u8; 20];
    adres.copy_from_slice(&adres_bytes);
    let node = st.node.read().await;
    Json(json!({
        "ok": true,
        "adres": adres_hex.trim(),
        "bakiye": node.bakiye(&adres).to_string(),
    }))
}

/// GET /lsc-bakiye/:adres — bir adresin serbest LSC bakiyesi (yakit/gas).
async fn lsc_bakiye(State(st): State<RpcState>, Path(adres_hex): Path<String>) -> Json<Value> {
    let adres_bytes = match hex::decode(adres_hex.trim()) {
        Ok(b) if b.len() == 20 => b,
        _ => return Json(json!({ "ok": false, "hata": "adres 20 bayt (40 hex) olmali" })),
    };
    let mut adres = [0u8; 20];
    adres.copy_from_slice(&adres_bytes);
    let node = st.node.read().await;
    Json(json!({
        "ok": true,
        "adres": adres_hex.trim(),
        "lsc_bakiye": node.lsc_bakiye(&adres),
    }))
}

/// GET /nonce/:adres — bir adresin BEKLENEN nonce'u (replay korumasi).
/// Istemci transfer kurmadan ONCE bunu okuyup payload'a koymali.
async fn nonce(State(st): State<RpcState>, Path(adres_hex): Path<String>) -> Json<Value> {
    let adres_bytes = match hex::decode(adres_hex.trim()) {
        Ok(b) if b.len() == 20 => b,
        _ => return Json(json!({ "ok": false, "hata": "adres 20 bayt (40 hex) olmali" })),
    };
    let mut adres = [0u8; 20];
    adres.copy_from_slice(&adres_bytes);
    let node = st.node.read().await;
    Json(json!({
        "ok": true,
        "adres": adres_hex.trim(),
        "nonce": node.beklenen_nonce(&adres),
    }))
}

/// GET /tips — mevcut DAG uclari (tip) id'leri. SDK bunlari parent yapar.
async fn tips(State(st): State<RpcState>) -> Json<Value> {
    let node = st.node.read().await;
    let tips: Vec<String> = node.tips().iter().map(hex::encode).collect();
    Json(json!({ "count": tips.len(), "tips": tips }))
}

/// Faucet basina sabit test AIDAG miktari (testnet kolayligi).
/// POST /eslestir — test adresini gercek (mainnet odul) adresine BIR KERELIK baglar.
/// Govde: {"test":"<40hex>","gercek":"<40hex>"}. Zaten eslesmisse degistirmez.
async fn eslestir(State(st): State<RpcState>, Json(govde): Json<Value>) -> Json<Value> {
    let test_hex = govde
        .get("test")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let gercek_hex = govde
        .get("gercek")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .to_lowercase();
    let test_b = match hex::decode(&test_hex) {
        Ok(b) if b.len() == 20 => b,
        _ => return Json(json!({ "ok": false, "hata": "test adresi 40 hex olmali" })),
    };
    let gercek_b = match hex::decode(&gercek_hex) {
        Ok(b) if b.len() == 20 => b,
        _ => return Json(json!({ "ok": false, "hata": "gercek adres 40 hex olmali" })),
    };
    let mut test = [0u8; 20];
    test.copy_from_slice(&test_b);
    let mut gercek = [0u8; 20];
    gercek.copy_from_slice(&gercek_b);
    let (yeni, mevcut) = {
        let mut node = st.node.write().await;
        let yeni = node.eslestir(test, gercek);
        let mevcut = node.eslesme_sorgula(&test);
        (yeni, mevcut)
    };
    Json(json!({
        "ok": true,
        "yeni_eslesme": yeni,
        "test": test_hex,
        "gercek": mevcut.map(hex::encode),
        "not": if yeni { "Eslesme kaydedildi (bir kerelik)." } else { "Bu test adresi zaten eslesmis; degistirilmedi." },
    }))
}

/// GET /eslesme/:adres — bir test adresinin eslesmis gercek odul adresi.
async fn eslesme(State(st): State<RpcState>, Path(adres_hex): Path<String>) -> Json<Value> {
    let b = match hex::decode(adres_hex.trim()) {
        Ok(b) if b.len() == 20 => b,
        _ => return Json(json!({ "ok": false, "hata": "adres 40 hex olmali" })),
    };
    let mut adres = [0u8; 20];
    adres.copy_from_slice(&b);
    let gercek = { st.node.read().await.eslesme_sorgula(&adres) };
    Json(json!({
        "ok": true,
        "test": adres_hex.trim(),
        "gercek": gercek.map(hex::encode),
        "eslesmis": gercek.is_some(),
    }))
}

/// GET /on-satis/:odeme_ref — bir on satis odeme referansinin dagitim kaydi.
/// Alici, satilan AIDAG, LSC hediye, zaman. Seffaflik/itiraz/denetim icin.
async fn on_satis_sorgu(
    State(st): State<RpcState>,
    Path(odeme_ref_str): Path<String>,
) -> Json<Value> {
    let odeme_ref: u64 = match odeme_ref_str.trim().parse() {
        Ok(v) => v,
        _ => return Json(json!({ "ok": false, "hata": "odeme_ref sayi olmali" })),
    };
    let kayit = { st.node.read().await.on_satis_sorgula(odeme_ref) };
    match kayit {
        Some(k) => Json(json!({
            "ok": true,
            "odeme_ref": odeme_ref,
            "bulundu": true,
            "alici": hex::encode(k.alici),
            "aidag": k.aidag.to_string(),
            "lsc_hediye": k.lsc_hediye.to_string(),
            "zaman": k.zaman,
        })),
        None => Json(json!({
            "ok": true,
            "odeme_ref": odeme_ref,
            "bulundu": false,
        })),
    }
}

/// Adres maskeleme: 0xABCDE...XYZ formati (bas 5 + son 3 hane gorunur, ortasi gizli).
/// Seffaflik (alimlar gercek, farkli kisiler gorunur) + gizlilik (kimlik saklanir).
fn mask_adres(adres: &[u8; 20]) -> String {
    let h = hex::encode(adres);
    format!("0x{}...{}", &h[0..5], &h[h.len() - 3..])
}

/// GET /on-satis-ozet — GENEL seffaflik: toplam satilan AIDAG, alim sayisi,
/// ve tum alimlar (adres MASKELI, zamana sirali). Hareket cizelgesi + seffaf liste icin.
async fn on_satis_ozet(State(st): State<RpcState>) -> Json<Value> {
    let (toplam, sayi, liste) = {
        let n = st.node.read().await;
        (
            n.on_satis_toplam_aidag(),
            n.on_satis_sayisi(),
            n.on_satis_liste(),
        )
    };
    let alimlar: Vec<Value> = liste
        .iter()
        .map(|(odeme_ref, k)| {
            json!({
                "odeme_ref": odeme_ref,
                "alici_maskeli": mask_adres(&k.alici),
                "aidag": k.aidag.to_string(),
                "lsc_hediye": k.lsc_hediye.to_string(),
                "zaman": k.zaman,
            })
        })
        .collect();
    Json(json!({
        "ok": true,
        "toplam_satilan_aidag": toplam.to_string(),
        "alim_sayisi": sayi,
        "alimlar": alimlar,
    }))
}

/// GET /on-satis-adres/:adres — KISISEL gorunum: bir alicinin kendi tum alimlari
/// (kendi adresi oldugu icin maskesiz) + toplam aldigi AIDAG. "Icim rahat" sorgusu.
async fn on_satis_adres(State(st): State<RpcState>, Path(adres_hex): Path<String>) -> Json<Value> {
    let b = match hex::decode(adres_hex.trim()) {
        Ok(b) if b.len() == 20 => b,
        _ => return Json(json!({ "ok": false, "hata": "adres 40 hex olmali" })),
    };
    let mut adres = [0u8; 20];
    adres.copy_from_slice(&b);
    let (alimlar_ham, toplam) = {
        let n = st.node.read().await;
        (
            n.on_satis_adrese_gore(&adres),
            n.on_satis_adres_toplam(&adres),
        )
    };
    let alimlar: Vec<Value> = alimlar_ham
        .iter()
        .map(|(odeme_ref, k)| {
            json!({
                "odeme_ref": odeme_ref,
                "aidag": k.aidag.to_string(),
                "lsc_hediye": k.lsc_hediye.to_string(),
                "zaman": k.zaman,
            })
        })
        .collect();
    Json(json!({
        "ok": true,
        "adres": adres_hex.trim(),
        "toplam_aldigi_aidag": toplam.to_string(),
        "alim_sayisi": alimlar.len(),
        "alimlar": alimlar,
    }))
}

/// GET /testnet-durum/:adres — kullanicinin testnet hesap ozeti (tek sorgu).
/// Test cuzdaniyla kendi durumunu gormek icin: AIDAG + LSC bakiye + stake.
async fn testnet_durum(State(st): State<RpcState>, Path(adres_hex): Path<String>) -> Json<Value> {
    let adres_bytes = match hex::decode(adres_hex.trim()) {
        Ok(b) if b.len() == 20 => b,
        _ => return Json(json!({ "ok": false, "hata": "adres 20 bayt (40 hex) olmali" })),
    };
    let mut adres = [0u8; 20];
    adres.copy_from_slice(&adres_bytes);
    let (aidag, lsc, stake) = {
        let node = st.node.read().await;
        (
            node.bakiye(&adres),
            node.lsc_bakiye(&adres),
            node.stake_miktari(&adres),
        )
    };
    Json(json!({
        "ok": true,
        "adres": adres_hex.trim(),
        "aidag_bakiye": aidag.to_string(),
        "lsc_bakiye": lsc.to_string(),
        "stake": stake.to_string(),
        "not": "Testnet hesap durumu — TEST AIDAG/LSC'nin gercek degeri yoktur.",
    }))
}

const FAUCET_MIKTAR: lsc_engine::registry::Tutar = 1000 * 1_000_000_000_000_000_000; // 1000 AIDAG (18 ondalik)
/// Anti-spam: bakiye bu esikteyse faucet TEKRAR vermez (once harca/transfer et).
const FAUCET_LIMIT: lsc_engine::registry::Tutar = 500 * 1_000_000_000_000_000_000; // 500 AIDAG (18 ondalik)

/// GET /faucet/:adres — TESTNET MUSLUGU: bir adrese sabit test AIDAG verir.
/// SADECE testnet/devnet icindir; test AIDAG'in GERCEK DEGERI YOKTUR (gercek
/// satis degeri/arz belirlenmedi). Gercege (mainnet) geciste KALDIRILMALIDIR.
async fn faucet(State(st): State<RpcState>, Path(adres_hex): Path<String>) -> Json<Value> {
    let adres_bytes = match hex::decode(adres_hex.trim()) {
        Ok(b) if b.len() == 20 => b,
        _ => return Json(json!({ "ok": false, "hata": "adres 20 bayt (40 hex) olmali" })),
    };
    let mut adres = [0u8; 20];
    adres.copy_from_slice(&adres_bytes);
    // ANTI-SPAM: bakiyesi zaten yuksekse verme (once harcasin/transfer etsin).
    {
        let node = st.node.read().await;
        let mevcut = node.bakiye(&adres);
        if mevcut >= FAUCET_LIMIT {
            return Json(json!({
                "ok": false,
                "adres": adres_hex.trim(),
                "mevcut_bakiye": mevcut,
                "hata": "Zaten yeterli TEST AIDAG'in var. Once harca veya transfer et, sonra tekrar iste.",
            }));
        }
    }
    // FAUCET ZINCIR: RAM'e degil, tip=6 imzali vertex olarak ZINCIRE yaz (kalici).
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (parents, net) = {
        let node = st.node.read().await;
        let mut t = node.tips();
        t.sort();
        (t, node.network_id())
    };
    let payload = lsc_engine::tx::FaucetKaydi::new(adres, FAUCET_MIKTAR).encode();
    let vertex = match lsc_engine::Vertex::new_signed(net, parents, payload, now, &st.signing_key) {
        Ok(v) => v,
        Err(e) => {
            return Json(json!({ "ok": false, "hata": format!("vertex uretilemedi: {e:?}") }))
        }
    };
    let bytes = lsc_engine::dag::wire::encode(&vertex);
    let (sonuc, yeni_bakiye) = {
        let mut node = st.node.write().await;
        let sonuc = node.ingest_networked(&bytes, now);
        (sonuc, node.bakiye(&adres))
    };
    let kabul = !format!("{sonuc:?}").contains("Rejected");
    if kabul {
        let _ = st.submit_tx.send(bytes); // aga yayinla
    }
    Json(json!({
        "ok": kabul,
        "adres": adres_hex.trim(),
        "verilen": FAUCET_MIKTAR.to_string(),
        "yeni_bakiye": yeni_bakiye.to_string(),
        "zincire_yazildi": kabul,
        "not": "TEST AIDAG — gercek degeri yoktur (testnet). Zincire yazildi (kalici).",
    }))
}

/// POST /test_bakiye — DEVNET/TEST: bir adrese bakiye basla (gercek arz DEGIL).
/// Govde: {"adres":"<hex40>","miktar":<u64>}. Sadece gelistirme/test icin.
/// Gercek arz/dagitim modeli sonra (audit+hukuk asamasi).
async fn test_bakiye(State(st): State<RpcState>, body: String) -> Json<Value> {
    let v: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return Json(json!({ "ok": false, "hata": format!("gecersiz json: {e}") })),
    };
    let adres_hex = v.get("adres").and_then(|a| a.as_str()).unwrap_or("");
    let miktar: lsc_engine::registry::Tutar = v
        .get("miktar")
        .and_then(|m| {
            m.as_str()
                .and_then(|x| x.parse::<u128>().ok()) // string: buyuk deger (18 ondalik)
                .or_else(|| m.as_u64().map(|n| n as u128))
        }) // sayi: geriye uyumlu
        .unwrap_or(0);
    let adres_bytes = match hex::decode(adres_hex.trim()) {
        Ok(b) if b.len() == 20 => b,
        _ => return Json(json!({ "ok": false, "hata": "adres 20 bayt (40 hex) olmali" })),
    };
    let mut adres = [0u8; 20];
    adres.copy_from_slice(&adres_bytes);
    let yeni = {
        let mut node = st.node.write().await;
        node.test_bakiye_ekle(adres, miktar)
    };
    Json(json!({ "ok": true, "adres": adres_hex.trim(), "yeni_bakiye": yeni.to_string() }))
}

/// POST /lsc_test_bakiye — DEVNET/TEST: bir adrese LSC bakiyesi basla (gercek arz DEGIL).
/// Govde: {"adres":"<hex40>","miktar":<u64>}. AVM/gas testleri icin (LSC = yakit).
async fn lsc_test_bakiye(State(st): State<RpcState>, body: String) -> Json<Value> {
    let v: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return Json(json!({ "ok": false, "hata": format!("gecersiz json: {e}") })),
    };
    let adres_hex = v.get("adres").and_then(|a| a.as_str()).unwrap_or("");
    let miktar: lsc_engine::registry::Tutar = v
        .get("miktar")
        .and_then(|m| {
            m.as_str()
                .and_then(|x| x.parse::<u128>().ok()) // string: buyuk deger (18 ondalik)
                .or_else(|| m.as_u64().map(|n| n as u128))
        }) // sayi: geriye uyumlu
        .unwrap_or(0);
    let adres_bytes = match hex::decode(adres_hex.trim()) {
        Ok(b) if b.len() == 20 => b,
        _ => return Json(json!({ "ok": false, "hata": "adres 20 bayt (40 hex) olmali" })),
    };
    let mut adres = [0u8; 20];
    adres.copy_from_slice(&adres_bytes);
    let yeni = {
        let mut node = st.node.write().await;
        node.lsc_test_bakiye_ekle(adres, miktar)
    };
    Json(json!({ "ok": true, "adres": adres_hex.trim(), "yeni_lsc_bakiye": yeni.to_string() }))
}

/// POST /submit — disaridan ham imzali vertex bayti (hex) al, zincire ingest et.
/// ASAMA B: SADECE kendi dugumune ingest (aga yayin sonraki adimda - kanal ile).
/// Govde: {"hex":"<vertex baytlari hex>"}  ya da duz hex string.
/// ingest_networked kullanir (imza + Kalkan dogrulamasi ZATEN ICINDE; RPC guvenli
/// bir kapi, konsensus kurallarini ATLAMAZ - bozuk/sahte vertex reddedilir).
async fn submit(State(st): State<RpcState>, body: String) -> Json<Value> {
    // Govde: ya {"hex":"..."} ya da duz hex. Once JSON dene, olmazsa duz al.
    let hex_str = match serde_json::from_str::<Value>(&body) {
        Ok(v) => v
            .get("hex")
            .and_then(|h| h.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| body.trim().to_string()),
        Err(_) => body.trim().to_string(),
    };
    let bytes = match hex::decode(hex_str.trim()) {
        Ok(b) => b,
        Err(e) => {
            return Json(json!({ "ok": false, "hata": format!("gecersiz hex: {e}") }));
        }
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (sonuc, vc) = {
        let mut node = st.node.write().await;
        let sonuc = node.ingest_networked(&bytes, now);
        (sonuc, node.vertex_count())
    };
    // ingest kabul ettiyse AGA yayinla (ag dongusu gossipsub publish eder).
    // Red/orphan olsa bile ag dongusune birakmak zararsiz; ama net olsun diye
    // sadece "kabul/ingest" durumunda yayinla. Sonuc string'inde "Rejected" yoksa yayinla.
    let kabul = !format!("{sonuc:?}").contains("Rejected");
    let mut yayinlandi = false;
    if kabul && st.submit_tx.send(bytes).is_ok() {
        yayinlandi = true;
    }
    Json(json!({
        "ok": true,
        "sonuc": format!("{sonuc:?}"),
        "vertex_count": vc,
        "aga_yayinlandi": yayinlandi,
    }))
}

/// AIDAG Chain ID (EVM uyumu). 3474 = 0xD92. MetaMask bu ID ile agi tanir.
/// NOT: mainnet'te chainlist.org'da rezerve edilmeli (cakisma kontrolu).
const AIDAG_CHAIN_ID: u64 = 3474;

/// eth_ JSON-RPC params dizisinin ILK elemanindan 20 baytlik adres cikar.
/// "0x<40 hex>" bekler. Hatali ise None.
/// eth_call params[0] = {"to":"0x<hex40>","data":"0x<hex>"} -> (hedef, data)
fn eth_params_call(istek: &Value) -> Option<([u8; 20], Vec<u8>)> {
    let p0 = istek.get("params")?.as_array()?.first()?;
    let to_str = p0.get("to")?.as_str()?;
    let to_temiz = to_str
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    let to_bytes = hex::decode(to_temiz).ok()?;
    if to_bytes.len() != 20 {
        return None;
    }
    let mut hedef = [0u8; 20];
    hedef.copy_from_slice(&to_bytes);
    // data opsiyonel (bos olabilir)
    let data = match p0.get("data").and_then(|d| d.as_str()) {
        Some(d) => {
            let temiz = d.trim().trim_start_matches("0x").trim_start_matches("0X");
            hex::decode(temiz).unwrap_or_default()
        }
        None => Vec::new(),
    };
    Some((hedef, data))
}

fn eth_params_adres(istek: &Value) -> Option<[u8; 20]> {
    let p0 = istek.get("params")?.as_array()?.first()?.as_str()?;
    let temiz = p0.trim().trim_start_matches("0x").trim_start_matches("0X");
    let bytes = hex::decode(temiz).ok()?;
    if bytes.len() != 20 {
        return None;
    }
    let mut a = [0u8; 20];
    a.copy_from_slice(&bytes);
    Some(a)
}

/// Ethereum JSON-RPC 2.0 ucu (POST /). MetaMask ve tum EVM cuzdanlari buraya
/// baglanir. Gelen {"jsonrpc","method","params","id"} -> method'a gore yanit.
/// Bu DILIM: agin TANINMASI icin minimum metotlar (chainId, net_version,
/// blockNumber). Henuz islem GONDERME yok (sonraki dilim).
async fn eth_rpc(State(st): State<RpcState>, Json(istek): Json<Value>) -> Json<Value> {
    let id = istek.get("id").cloned().unwrap_or(json!(1));
    let method = istek.get("method").and_then(|v| v.as_str()).unwrap_or("");

    // JSON-RPC 2.0 basarili yanit sarmalayici.
    fn ok(id: &Value, result: Value) -> Json<Value> {
        Json(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    }
    // JSON-RPC 2.0 hata yaniti.
    fn err(id: &Value, code: i64, msg: &str) -> Json<Value> {
        Json(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": msg } }))
    }

    match method {
        // Agin kimligi: MetaMask baglanirken ILK bunu sorar. Hex string.
        "eth_chainId" => ok(&id, json!(format!("0x{:x}", AIDAG_CHAIN_ID))),
        // Ag versiyonu: chainId'nin ondalik string hali.
        "net_version" => ok(&id, json!(AIDAG_CHAIN_ID.to_string())),
        // En son blok numarasi. Bizde "blok" = vertex sayisi (yaklasik gosterge).
        "eth_blockNumber" => {
            let n = st.node.read().await.vertex_count() as u64;
            ok(&id, json!(format!("0x{:x}", n)))
        }
        // Istemci surumu (bazi cuzdanlar sorar).
        "web3_clientVersion" => ok(&id, json!("AIDAG-Chain/v0.1.0")),
        // Bir adresin bakiyesi. params[0] = "0x<adres>". Hex (wei tarzi) doner.
        // MetaMask transfer ekraninda bakiyeyi GOSTERIR.
        "eth_getBalance" => match eth_params_adres(&istek) {
            Some(adres) => {
                let bakiye = st.node.read().await.bakiye(&adres) as u128;
                ok(&id, json!(format!("0x{:x}", bakiye)))
            }
            None => err(&id, -32602, "gecersiz adres parametresi"),
        },
        // Bir adresin islem sayisi = nonce. params[0] = "0x<adres>".
        // MetaMask islemi NUMARALANDIRMAK icin sorar (replay korumasi).
        "eth_getTransactionCount" => match eth_params_adres(&istek) {
            Some(adres) => {
                let nonce = st.node.read().await.beklenen_nonce(&adres);
                ok(&id, json!(format!("0x{:x}", nonce)))
            }
            None => err(&id, -32602, "gecersiz adres parametresi"),
        },
        // eth_call: OKUMA-ONLY sozlesme cagrisi (dApp/web3 sozlesme okur, zincir degismez).
        // params[0] = {"to":"0x<sozlesme>","data":"0x<calldata>"}
        "eth_call" => {
            match eth_params_call(&istek) {
                Some((hedef, data)) => {
                    // gonderen: okuma icin sifir adres yeterli
                    let gonderen = [0u8; 20];
                    let sonuc = st.node.read().await.avm_call(&gonderen, &hedef, &data);
                    match sonuc {
                        Ok(veri) => ok(&id, json!(format!("0x{}", hex::encode(veri)))),
                        Err(e) => err(&id, -32000, e),
                    }
                }
                None => err(&id, -32602, "gecersiz eth_call parametresi (to/data)"),
            }
        }
        // eth_sendRawTransaction: MetaMask/web3'ten imzali ham tx al -> AVM'de calistir.
        // params[0] = "0x<RLP-kodlu imzali tx>". Doner: tx_hash (0x...).
        "eth_sendRawTransaction" => {
            match istek
                .get("params")
                .and_then(|p| p.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
            {
                Some(raw_hex) => {
                    let temiz = raw_hex
                        .trim()
                        .trim_start_matches("0x")
                        .trim_start_matches("0X");
                    match hex::decode(temiz) {
                        Ok(raw) => {
                            // tx_hash = keccak256(raw) - eth standardi, hemen hesaplanir
                            let tx_hash = {
                                use lsc_engine::avm::ham_eth_tx_coz;
                                // once cozulebilir mi kontrol (gecersizse hemen red)
                                if ham_eth_tx_coz(&raw).is_err() {
                                    return err(&id, -32000, "raw tx cozulemedi (RLP/imza)");
                                }
                                lsc_engine::avm::eth_tx_hash(&raw)
                            };
                            // tip=12 payload olustur + vertex'e sar (DAG'a kalici)
                            let payload = lsc_engine::tx::ham_eth_tx_payload(&raw);
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0);
                            let (parents, net) = {
                                let node = st.node.read().await;
                                (node.tips(), node.network_id())
                            };
                            let vertex = match lsc_engine::Vertex::new_signed(
                                net,
                                parents,
                                payload,
                                now,
                                &st.signing_key,
                            ) {
                                Ok(v) => v,
                                Err(_) => return err(&id, -32000, "vertex uretilemedi"),
                            };
                            let bytes = lsc_engine::dag::wire::encode(&vertex);
                            let sonuc = st.node.write().await.ingest_networked(&bytes, now);
                            if format!("{sonuc:?}").contains("Rejected") {
                                return err(&id, -32000, "vertex reddedildi");
                            }
                            let _ = st.submit_tx.send(bytes); // aga yayinla
                            ok(&id, json!(format!("0x{}", hex::encode(tx_hash))))
                        }
                        Err(_) => err(&id, -32602, "gecersiz hex (raw tx)"),
                    }
                }
                None => err(&id, -32602, "raw tx parametresi gerekli"),
            }
        }

        // eth_gasPrice: gas fiyati. Testnette dusuk sabit deger (MetaMask sorar).
        "eth_gasPrice" => ok(&id, json!("0x3b9aca00")),
        // eth_maxPriorityFeePerGas: EIP-1559 tip (MetaMask sorar).
        "eth_maxPriorityFeePerGas" => ok(&id, json!("0x3b9aca00")),
        // eth_estimateGas: tahmini gas. Basit transfer icin sabit 21000.
        "eth_estimateGas" => ok(&id, json!("0x5208")),
        // eth_getBlockByNumber: MetaMask islem oncesi blok bilgisi ister.
        // Bizde blok = vertex; basit bir blok objesi doneriz (MetaMask'i tatmin eder).
        "eth_getBlockByNumber" => {
            let n = st.node.read().await.vertex_count() as u64;
            ok(
                &id,
                json!({
                    "number": format!("0x{:x}", n),
                    "hash": format!("0x{:064x}", n),
                    "parentHash": format!("0x{:064x}", n.saturating_sub(1)),
                    "timestamp": format!("0x{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)),
                    "gasLimit": "0x1c9c380",
                    "gasUsed": "0x0",
                    "baseFeePerGas": "0x3b9aca00",
                    "miner": "0x0000000000000000000000000000000000000000",
                    "transactions": [],
                    "difficulty": "0x0",
                    "totalDifficulty": "0x0",
                    "size": "0x0",
                    "extraData": "0x",
                    "nonce": "0x0000000000000000",
                    "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
                    "logsBloom": "0x0",
                    "stateRoot": format!("0x{:064x}", n),
                    "receiptsRoot": "0x0",
                    "transactionsRoot": "0x0",
                    "uncles": []
                }),
            )
        }
        // eth_getBlockByHash: benzer, hash ile.
        "eth_getBlockByHash" => {
            let n = st.node.read().await.vertex_count() as u64;
            ok(
                &id,
                json!({
                    "number": format!("0x{:x}", n),
                    "hash": format!("0x{:064x}", n),
                    "parentHash": format!("0x{:064x}", n.saturating_sub(1)),
                    "timestamp": format!("0x{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)),
                    "gasLimit": "0x1c9c380",
                    "gasUsed": "0x0",
                    "baseFeePerGas": "0x3b9aca00",
                    "miner": "0x0000000000000000000000000000000000000000",
                    "transactions": [],
                    "difficulty": "0x0",
                    "size": "0x0",
                    "extraData": "0x",
                    "nonce": "0x0000000000000000",
                    "uncles": []
                }),
            )
        }

        _ => err(
            &id,
            -32601,
            "method not found (bu dilimde desteklenmeyen metot)",
        ),
    }
}

/// RPC router'ini olusturur.
pub fn router(
    node: Arc<RwLock<lsc_engine::NodeState>>,
    submit_tx: UnboundedSender<Vec<u8>>,
    signing_key: ed25519_dalek::SigningKey,
) -> Router {
    let state = RpcState {
        node,
        submit_tx,
        signing_key,
    };
    let router = Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/tokens", get(tokens))
        .route("/avm-kontratlar", get(avm_kontratlar))
        .route("/tips", get(tips))
        .route("/bakiye/:adres", get(bakiye))
        .route("/lsc-bakiye/:adres", get(lsc_bakiye))
        .route("/nonce/:adres", get(nonce))
        .route("/belge/:hash", get(belge))
        .route("/kurum/:adres", get(kurum))
        .route("/submit", post(submit))
        .route("/", post(eth_rpc)) // Ethereum JSON-RPC (MetaMask + tum EVM cuzdanlari)
        .route("/islemlerim/:pubkey", get(islemlerim))
        .route("/testnet-durum/:adres", get(testnet_durum))
        .route("/eslestir", post(eslestir))
        .route("/eslesme/:adres", get(eslesme))
        .route("/on-satis/:odeme_ref", get(on_satis_sorgu))
        .route("/on-satis-ozet", get(on_satis_ozet))
        .route("/on-satis-adres/:adres", get(on_satis_adres));

    // FAUCET: anti-spam korumali (bakiye limitli), production da dahil ACIK.
    // test_bakiye/lsc_test_bakiye: sinirsiz basma -> sadece GELISTIRME modunda.
    let router = router.route("/faucet/:adres", get(faucet));
    let router = if std::env::var("LSC_PRODUCTION").is_ok() {
        tracing::warn!("PRODUCTION MODU: faucet ACIK (limitli), test_bakiye KAPALI.");
        router
    } else {
        tracing::info!("GELISTIRME MODU: tum test uclari ACIK.");
        router
            .route("/test_bakiye", post(test_bakiye))
            .route("/lsc_test_bakiye", post(lsc_test_bakiye))
    };

    router.with_state(state)
}

/// RPC sunucusunu verilen adreste baslatir (ornek: "0.0.0.0:8645").
pub async fn serve(
    addr: String,
    node: Arc<RwLock<lsc_engine::NodeState>>,
    submit_tx: UnboundedSender<Vec<u8>>,
    signing_key: ed25519_dalek::SigningKey,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = router(node, submit_tx, signing_key);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("RPC sunucusu dinliyor: http://{addr}  (GET /health, /status)");
    axum::serve(listener, app).await?;
    Ok(())
}
