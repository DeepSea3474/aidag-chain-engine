//! SUREC-DUZEYI RESTART ENTEGRASYON TESTI (denetim raporu bloker #13).
//! Gercek `lsc-node` binary'si IZOLE bir agda (LSC_NETWORK_ID=99999) calisir:
//! baslat -> vertex gonder -> SURECI OLDUR -> ayni veri dosyasiyla yeniden
//! baslat. KANIT: genesis + vertex sayisi + orphan diskten birebir kurulur.
//! Izolasyon: ayri network_id -> ayri gossipsub topic'i + ag kapisi, canli
//! testnet (network_id=1) ile veri karismaz.

use std::process::{Child, Command};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const PORT: &str = "40099";
const RPC: &str = "127.0.0.1:8699";
const NET_ID: u32 = 99999;

fn simdi() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

fn log_yolu() -> String {
    "/tmp/aidag-restart-test-node.log".to_string()
}

fn dugum_baslat(veri: &str) -> Child {
    let f = std::fs::File::create(log_yolu()).expect("log dosyasi");
    let f2 = f.try_clone().expect("log clone");
    Command::new(env!("CARGO_BIN_EXE_lsc-node"))
        .arg(format!("/ip4/127.0.0.1/tcp/{PORT}"))
        .arg(veri)
        .env("LSC_RPC_ADDR", RPC)
        .env("LSC_NETWORK_ID", NET_ID.to_string())
        .stdout(std::process::Stdio::from(f))
        .stderr(std::process::Stdio::from(f2))
        .spawn()
        .expect("lsc-node baslatilamadi")
}

/// Hata durumunda dugum logunu ekrana bas (teshis).
fn log_bas() {
    if let Ok(t) = std::fs::read_to_string(log_yolu()) {
        eprintln!("--- DUGUM LOGU (son 30 satir) ---");
        for l in t.lines().rev().take(30).collect::<Vec<_>>().iter().rev() {
            eprintln!("{l}");
        }
        eprintln!("--- LOG SONU ---");
    }
}

/// genesis olusana kadar bekle (RPC, genesis'ten once ayaga kalkabiliyor).
fn genesis_bekle() -> serde_json::Value {
    for _ in 0..60 {
        let d = durum_bekle();
        if d["genesis"].as_str().unwrap_or("yok") != "yok" {
            return d;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    log_bas();
    panic!("genesis 30 sn icinde uretilmedi");
}

fn get(yol: &str) -> Option<serde_json::Value> {
    let c = Command::new("curl")
        .args(["-s", "--max-time", "3", &format!("http://{RPC}{yol}")])
        .output()
        .ok()?;
    serde_json::from_slice(&c.stdout).ok()
}

/// /status yanit verene kadar bekle (en fazla ~25 sn).
fn durum_bekle() -> serde_json::Value {
    for _ in 0..50 {
        if let Some(v) = get("/status") {
            if v.get("network_id").is_some() {
                return v;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    panic!("dugum /status'a {RPC} uzerinden yanit vermedi");
}

fn oldur(mut ch: Child) {
    let _ = ch.kill();
    let _ = ch.wait();
    std::thread::sleep(Duration::from_secs(2)); // port serbest kalsin
}

#[test]
#[ignore = "gercek surec + port acar; elle: cargo test --test restart_entegrasyon -- --ignored"]
fn surec_restart_diskten_ayni_state() {
    let veri = format!("/tmp/aidag-restart-test-{}.log", simdi());
    let _ = std::fs::remove_file(&veri);

    // --- 1) ILK CALISTIRMA ---
    let ch1 = dugum_baslat(&veri);
    let d1 = genesis_bekle();
    assert_eq!(d1["network_id"], NET_ID, "IZOLASYON: test agi disinda acildi!");
    let genesis_once = d1["genesis"].as_str().unwrap().to_string();
    assert_ne!(genesis_once, "yok", "genesis uretilmedi");
    assert_eq!(d1["vertex_count"], 1, "baslangicta sadece genesis olmali");

    // --- 2) BIR VERTEX GONDER ---
    let tips = get("/tips").expect("/tips");
    let tip_hex = tips["tips"][0].as_str().expect("tip yok").to_string();
    let mut parent = [0u8; 32];
    parent.copy_from_slice(&hex::decode(&tip_hex).expect("tip hex"));

    let sk = ed25519_dalek::SigningKey::from_bytes(&[57u8; 32]);
    let v = lsc_engine::dag::vertex::Vertex::new_signed(
        NET_ID,
        vec![parent],
        b"restart-testi".to_vec(),
        simdi(),
        &sk,
    )
    .expect("vertex imzalanamadi");
    let hexstr = hex::encode(lsc_engine::dag::wire::encode(&v));

    let out = Command::new("curl")
        .args(["-s", "--max-time", "5", "-X", "POST",
               &format!("http://{RPC}/submit"), "--data", &hexstr])
        .output()
        .expect("submit");
    let cevap = String::from_utf8_lossy(&out.stdout).to_string();

    let mut d2 = durum_bekle();
    for _ in 0..20 {
        if d2["vertex_count"] == 2 { break; }
        std::thread::sleep(Duration::from_millis(500));
        d2 = durum_bekle();
    }
    assert_eq!(d2["vertex_count"], 2, "vertex eklenmedi (submit cevabi: {cevap})");

    // --- 3) SURECI OLDUR ---
    oldur(ch1);

    // --- 4) AYNI VERI DOSYASIYLA YENIDEN BASLAT ---
    let ch2 = dugum_baslat(&veri);
    let d3 = genesis_bekle();

    // --- 5) KANIT: state diskten birebir kuruldu ---
    assert_eq!(d3["genesis"].as_str().unwrap(), genesis_once, "RESTART: genesis degisti!");
    assert_eq!(d3["vertex_count"], 2, "RESTART: vertex sayisi korunmadi");
    assert_eq!(d3["orphan_count"], 0, "RESTART: orphan olustu");
    assert_eq!(d3["network_id"], NET_ID, "RESTART: ag kimligi degisti");

    oldur(ch2);
    let _ = std::fs::remove_file(&veri);
}
