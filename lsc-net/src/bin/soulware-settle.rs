//! soulware-settle — OWNER offline HESAPLAMA ÖDÜLÜ (tip=16) imzalama aracı.
//!
//! Kurucu ed25519 anahtarıyla bir ComputeReward vertex'i imzalar; çıktısı wire-hex.
//! Owner bunu node'un `/submit` ucuna POST eder → doğrulanan worker'a LSC BASILIR
//! (kontrollü emisyon; node emisyon tavanını + çifte-basım engelini uygular).
//!
//! GÜVENLİK: SADECE imzalar — anahtar SUNUCUDA TUTULMAZ, owner'ın kendi makinesinde
//! offline çalışır. Koordinatörün doğrulanmış kazançları → bu araçla ödül olarak basılır.
//!
//! Kullanım:
//!   soulware-settle <key> <net_id> <worker_hex40> <lsc_tam> <reward_id> <ts_unix> <tips_virgul|->
//!     key       : aidag-kurucu.key yolu ([algo=1][32 seed])
//!     net_id    : 3474 (mainnet) | 1 (devnet)
//!     worker    : ödül alan worker'ın 0x adresi (40 hex, 0x opsiyonel)
//!     lsc_tam   : TAM LSC miktarı (araç 10^18 ile çarpar)
//!     reward_id : benzersiz ödül referansı (u64) — çifte-basım kilidi
//!     ts_unix   : şimdiki unix saniye (`date +%s`)
//!     tips      : virgülle ayrılmış 64-hex vertex id (node /tips'ten). Boş: "-"
//!
//! Çıktı (stdout): imzalı vertex wire-hex. Sonra:
//!   curl -X POST <RPC>/submit -H 'Content-Type: application/json' -d '{"hex":"<HEX>"}'

use ed25519_dalek::SigningKey;
use lsc_engine::dag::wire;
use lsc_engine::tx::ComputeReward;
use lsc_engine::Vertex;

const ONDALIK: u128 = 1_000_000_000_000_000_000;

fn hata(m: &str) -> ! {
    eprintln!("HATA: {m}");
    eprintln!("Kullanim: soulware-settle <key> <net_id> <worker_hex40> <lsc_tam> <reward_id> <ts_unix> <tips_virgul|->");
    std::process::exit(1);
}

fn hex20(s: &str) -> [u8; 20] {
    let s = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    let b = hex::decode(s).unwrap_or_else(|_| hata("worker_hex gecersiz"));
    if b.len() != 20 {
        hata("worker 20 bayt (40 hex) olmali");
    }
    let mut a = [0u8; 20];
    a.copy_from_slice(&b);
    a
}

fn hex32(s: &str) -> [u8; 32] {
    let b = hex::decode(s.trim()).unwrap_or_else(|_| hata("tip id gecersiz hex"));
    if b.len() != 32 {
        hata("tip id 32 bayt (64 hex) olmali");
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&b);
    a
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 8 {
        hata("7 arguman gerekli");
    }
    let key_path = &args[1];
    let net_id: u32 = args[2].parse().unwrap_or_else(|_| hata("net_id sayi olmali"));
    let worker = hex20(&args[3]);
    let lsc_tam: u128 = args[4].parse().unwrap_or_else(|_| hata("lsc_tam sayi olmali"));
    let reward_id: u64 = args[5].parse().unwrap_or_else(|_| hata("reward_id sayi olmali"));
    let ts: u64 = args[6].parse().unwrap_or_else(|_| hata("ts sayi olmali"));
    let mut tips: Vec<[u8; 32]> = if args[7] == "-" || args[7].is_empty() {
        vec![]
    } else {
        args[7].split(',').filter(|t| !t.is_empty()).map(hex32).collect()
    };
    tips.sort();
    tips.dedup();

    let data = std::fs::read(key_path).unwrap_or_else(|_| hata("key dosyasi okunamadi"));
    if data.len() != 33 || data[0] != 1 {
        hata("key format [1][32 seed] olmali (33 bayt)");
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&data[1..33]);
    let sk = SigningKey::from_bytes(&seed);

    let lsc = lsc_tam
        .checked_mul(ONDALIK)
        .unwrap_or_else(|| hata("lsc miktari tasti"));

    let payload = ComputeReward::new(worker, lsc, reward_id).encode();
    let v = Vertex::new_signed(net_id, tips, payload, ts, &sk)
        .unwrap_or_else(|e| hata(&format!("vertex uretilemedi: {e:?}")));
    let bytes = wire::encode(&v);

    println!("{}", hex::encode(&bytes));
    let owner_adres = lsc_engine::public_key_to_adres(&sk.verifying_key().to_bytes());
    eprintln!(
        "OK  imzalayan(owner)=0x{}  net={net_id}  worker=0x{}  lsc={lsc_tam}  reward_id={reward_id}  parents={}",
        hex::encode(owner_adres),
        hex::encode(worker),
        v.parents().len()
    );
    eprintln!("-> curl -X POST <RPC>/submit -H 'Content-Type: application/json' -d '{{\"hex\":\"<yukaridaki>\"}}'");
}
