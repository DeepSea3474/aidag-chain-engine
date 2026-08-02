//! on-satis-tahsis — OWNER offline TAHSIS (tip=10) imzalama araci.
//!
//! Kurucu ed25519 anahtariyla bir on-satis TAHSIS vertex'i imzalar; ciktisi
//! wire-hex'tir. Owner bunu node'un `/submit` ucuna POST eder.
//!
//! GUVENLIK: Bu arac SADECE imzalar — anahtar SUNUCUDA TUTULMAZ, owner'in kendi
//! makinesinde offline calisir. Ag baglantisi yoktur (tips disaridan verilir).
//!
//! Kullanim:
//!   on-satis-tahsis <key> <net_id> <alici_hex40> <aidag> <lsc_hediye> <odeme_ref> <ts_unix> <tips>
//!     key       : aidag-kurucu.key yolu ([algo=1][32 seed])
//!     net_id    : 3474 (mainnet) | 1 (devnet)
//!     alici_hex : alicinin 0x adresi (40 hex, 0x opsiyonel)
//!     aidag     : TAM AIDAG miktari (arac 10^18 ile carpar)
//!     lsc_hediye: TAM LSC hediye (gaz icin; 0 olabilir)
//!     odeme_ref : benzersiz odeme referansi (u64) — cifte-tahsis kilidi
//!     ts_unix   : simdiki unix saniye (`date +%s`)
//!     tips      : virgulle ayrilmis 64-hex vertex id (node /tips'ten). Bos: "-"
//!
//! Cikti (stdout): imzali vertex wire-hex. Sonra:
//!   curl -X POST <RPC>/submit -H 'Content-Type: application/json' -d "{\"hex\":\"<HEX>\"}"

use ed25519_dalek::SigningKey;
use lsc_engine::dag::wire;
use lsc_engine::tx::OnSatisDagitim;
use lsc_engine::Vertex;

const ONDALIK: u128 = 1_000_000_000_000_000_000; // 10^18

fn hata(m: &str) -> ! {
    eprintln!("HATA: {m}");
    eprintln!("Kullanim: on-satis-tahsis <key> <net_id> <alici_hex40> <aidag> <lsc_hediye> <odeme_ref> <ts_unix> <tips_virgul|->");
    std::process::exit(1);
}

fn hex20(s: &str) -> [u8; 20] {
    let s = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    let b = hex::decode(s).unwrap_or_else(|_| hata("alici_hex gecersiz"));
    if b.len() != 20 {
        hata("alici 20 bayt (40 hex) olmali");
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
    if args.len() != 10 {
        hata("8 arguman gerekli");
    }
    let key_path = &args[1];
    let net_id: u32 = args[2].parse().unwrap_or_else(|_| hata("net_id sayi olmali"));
    let alici = hex20(&args[3]);
    let odeme_adresi = hex20(&args[4]);
    let aidag_tam: u128 = args[5].parse().unwrap_or_else(|_| hata("aidag sayi olmali"));
    let lsc_tam: u128 = args[6].parse().unwrap_or_else(|_| hata("lsc_hediye sayi olmali"));
    let odeme_ref: u64 = args[7].parse().unwrap_or_else(|_| hata("odeme_ref sayi olmali"));
    let ts: u64 = args[8].parse().unwrap_or_else(|_| hata("ts sayi olmali"));
    let mut tips: Vec<[u8; 32]> = if args[9] == "-" || args[9].is_empty() {
        vec![]
    } else {
        args[9].split(',').filter(|t| !t.is_empty()).map(hex32).collect()
    };
    // Kanoniklik: parent seti KESIN ARTAN olmali (vertex check_bounds). Sirala.
    tips.sort();
    tips.dedup();

    // Kurucu anahtari: [algo=1][32 seed]
    let data = std::fs::read(key_path).unwrap_or_else(|_| hata("key dosyasi okunamadi"));
    if data.len() != 33 || data[0] != 1 {
        hata("key format [1][32 seed] olmali (33 bayt)");
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&data[1..33]);
    let sk = SigningKey::from_bytes(&seed);

    let aidag = aidag_tam
        .checked_mul(ONDALIK)
        .unwrap_or_else(|| hata("aidag miktari tasti"));
    let lsc = lsc_tam
        .checked_mul(ONDALIK)
        .unwrap_or_else(|| hata("lsc hediye tasti"));

    let payload = OnSatisDagitim::new(odeme_adresi, alici, aidag, lsc, odeme_ref).encode();
    let v = Vertex::new_signed(net_id, tips, payload, ts, &sk)
        .unwrap_or_else(|e| hata(&format!("vertex uretilemedi: {e:?}")));
    let bytes = wire::encode(&v);

    // stdout: sadece hex (script'te yakalanabilsin)
    println!("{}", hex::encode(&bytes));
    // stderr: ozet + owner adres teyidi
    let owner_adres = lsc_engine::public_key_to_adres(&sk.verifying_key().to_bytes());
    eprintln!(
        "OK  imzalayan(owner)=0x{}  net={net_id}  alici=0x{}  aidag={aidag_tam}  lsc_hediye={lsc_tam}  odeme_ref={odeme_ref}  parents={}",
        hex::encode(owner_adres),
        hex::encode(alici),
        v.parents().len()
    );
    eprintln!("-> curl -X POST <RPC>/submit -H 'Content-Type: application/json' -d '{{\"hex\":\"<yukaridaki>\"}}'");
}
