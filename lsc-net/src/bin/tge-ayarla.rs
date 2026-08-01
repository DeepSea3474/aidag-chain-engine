//! tge-ayarla — OWNER offline: ON-SATIS TGE (tip=15) ayarla.
//!
//! Kurucu ed25519 anahtariyla bir "TGE ayarla" vertex'i imzalar; ciktisi wire-hex.
//! Owner bunu /submit'e POST eder. Anahtar SUNUCUDA TUTULMAZ (offline imza).
//!
//! Kullanim:
//!   tge-ayarla <key> <net_id> <tge_unix> <ts_unix> <tips_virgul|->
//!     key     : aidag-kurucu.key ([1][32 seed])
//!     net_id  : 3474 (mainnet) | 1 (devnet)
//!     tge_unix: yeni TGE (Unix saniye) — on-satis claim/vesting bundan acilir
//!     ts_unix : simdiki zaman (`date +%s`)
//!     tips    : node /tips (virgulle). Bos: "-"
//! Cikti (stdout): imzali vertex hex -> curl -X POST <RPC>/submit -d '{"hex":"..."}'

use ed25519_dalek::SigningKey;
use lsc_engine::dag::wire;
use lsc_engine::tx::TgeAyarla;
use lsc_engine::Vertex;

fn hata(m: &str) -> ! {
    eprintln!("HATA: {m}");
    eprintln!("Kullanim: tge-ayarla <key> <net_id> <tge_unix> <ts_unix> <tips_virgul|->");
    std::process::exit(1);
}
fn hex32(s: &str) -> [u8; 32] {
    let b = hex::decode(s.trim()).unwrap_or_else(|_| hata("tip id gecersiz hex"));
    if b.len() != 32 {
        hata("tip id 32 bayt olmali");
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&b);
    a
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 6 {
        hata("5 arguman gerekli");
    }
    let net: u32 = a[2].parse().unwrap_or_else(|_| hata("net_id sayi olmali"));
    let tge: u64 = a[3].parse().unwrap_or_else(|_| hata("tge_unix sayi olmali"));
    let ts: u64 = a[4].parse().unwrap_or_else(|_| hata("ts_unix sayi olmali"));
    let mut tips: Vec<[u8; 32]> = if a[5] == "-" || a[5].is_empty() {
        vec![]
    } else {
        a[5].split(',').filter(|t| !t.is_empty()).map(hex32).collect()
    };
    tips.sort();
    tips.dedup();

    let data = std::fs::read(&a[1]).unwrap_or_else(|_| hata("key dosyasi okunamadi"));
    if data.len() != 33 || data[0] != 1 {
        hata("key format [1][32 seed] olmali");
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&data[1..33]);
    let sk = SigningKey::from_bytes(&seed);

    let payload = TgeAyarla::new(tge).encode();
    let v = Vertex::new_signed(net, tips, payload, ts, &sk)
        .unwrap_or_else(|e| hata(&format!("vertex uretilemedi: {e:?}")));
    println!("{}", hex::encode(wire::encode(&v)));
    eprintln!(
        "OK  owner=0x{}  net={net}  yeni TGE={tge}. Hex'i /submit'e POST et.",
        hex::encode(lsc_engine::public_key_to_adres(&sk.verifying_key().to_bytes()))
    );
}
