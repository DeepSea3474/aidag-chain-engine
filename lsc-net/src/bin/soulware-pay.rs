//! soulware-pay — TÜKETİCİ offline LSC ÖDEME (tip=7 LscTransfer) imzalama aracı.
//!
//! KUBRA'yı kullanmak için LSC harcayan tarafın (tüketici) kendi ed25519 anahtarıyla
//! bir LSC transfer vertex'i imzalar; çıktısı wire-hex. Tüketici bunu node'un
//! `/submit` ucuna POST eder → LSC gönderenden alıcıya (ödül havuzu) TAŞINIR.
//!
//! DÜRÜST: Bu SADECE VAR OLAN LSC'yi taşır — para BASMAZ (emisyon değil). Gönderenin
//! bakiyesi + nonce node tarafından zorlanır (çift-harcama/replay korumalı).
//!
//! GÜVENLİK: SADECE imzalar — anahtar SUNUCUDA TUTULMAZ, tüketicinin kendi makinesinde.
//! Bu, kazan↔harca kapalı döngüsünün "harca" ayağıdır (kazanç ayağı = koordinatör).
//!
//! Kullanım:
//!   soulware-pay <key> <net_id> <alici_hex40> <lsc_tam> <nonce> <ts_unix> <tips_virgul|->
//!     key     : tüketicinin aidag anahtarı ([algo=1][32 seed])
//!     net_id  : 3474 (mainnet) | devnet net_id
//!     alici   : ödül havuzu (koordinatör) adresi (40 hex, 0x opsiyonel)
//!     lsc_tam : TAM LSC miktarı (araç 10^18 ile çarpar)
//!     nonce   : gönderenin bir sonraki nonce'u (node GET /nonce/:adres)
//!     ts_unix : şimdiki unix saniye (`date +%s`)
//!     tips    : virgülle ayrılmış 64-hex vertex id (node /tips'ten). Boş: "-"
//!
//! Çıktı (stdout): imzalı vertex wire-hex → curl -X POST <RPC>/submit -d '{"hex":"..."}'

use ed25519_dalek::SigningKey;
use lsc_engine::dag::wire;
use lsc_engine::tx::LscTransferKaydi;
use lsc_engine::Vertex;

const ONDALIK: u128 = 1_000_000_000_000_000_000;

fn hata(m: &str) -> ! {
    eprintln!("HATA: {m}");
    eprintln!("Kullanim: soulware-pay <key> <net_id> <alici_hex40> <lsc_tam> <nonce> <ts_unix> <tips_virgul|->");
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
    if args.len() != 8 {
        hata("7 arguman gerekli");
    }
    let key_path = &args[1];
    let net_id: u32 = args[2].parse().unwrap_or_else(|_| hata("net_id sayi olmali"));
    let alici = hex20(&args[3]);
    let lsc_tam: u128 = args[4].parse().unwrap_or_else(|_| hata("lsc_tam sayi olmali"));
    let nonce: u64 = args[5].parse().unwrap_or_else(|_| hata("nonce sayi olmali"));
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

    let payload = LscTransferKaydi::new(alici, lsc, nonce).encode();
    let v = Vertex::new_signed(net_id, tips, payload, ts, &sk)
        .unwrap_or_else(|e| hata(&format!("vertex uretilemedi: {e:?}")));
    let bytes = wire::encode(&v);

    println!("{}", hex::encode(&bytes));
    let gonderen = lsc_engine::public_key_to_adres(&sk.verifying_key().to_bytes());
    eprintln!(
        "OK  gonderen(tuketici)=0x{}  net={net_id}  alici(havuz)=0x{}  lsc={lsc_tam}  nonce={nonce}  parents={}",
        hex::encode(gonderen),
        hex::encode(alici),
        v.parents().len()
    );
    eprintln!("-> curl -X POST <RPC>/submit -H 'Content-Type: application/json' -d '{{\"hex\":\"<yukaridaki>\"}}'");
}
