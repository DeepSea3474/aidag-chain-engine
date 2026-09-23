//! Worker cuzdan-sahipligi imzasinin MESAJ bicimi (koordinator + worker ortak).
//!
//! Imzalanan mesaj (UTF-8):   AIDAG-WORKER|<wallet>|<nonce>|<ts>
//!   wallet = "0x" + 40 hex, KUCUK harf
//!   nonce  = "kayit"                      (POST /worker/register)
//!          | "poll"                       (GET  /worker/poll/:wallet)
//!          | "benchmark:<ozet hex>"       (POST /worker/benchmark)
//!          | "is:<job_id>:<cevap ozeti>"  (POST /worker/submit)
//!   ts     = unix saniye (koordinator +-300 sn pencere uygular)
//!
//! Ozetler blake3 (hex). Cevap ozeti = blake3(answer.trim()) — koordinatorun
//! eslestirme hash'iyle ayni. Boylece imza belirli bir is + belirli bir cevaba baglidir.
//!
//! Imza turu:
//!   - ed25519 cuzdan (AIDAG adresi = blake3(pubkey)[..20]): `pubkey` (64 hex) +
//!     `imza` (128 hex) = ed25519(mesaj baytlari).
//!   - EVM cuzdan (MetaMask vb.): `imza` (130 hex, r|s|v) = EIP-191 personal_sign(mesaj);
//!     `pubkey` GONDERILMEZ.

pub const ONEK: &str = "AIDAG-WORKER";
pub const NONCE_KAYIT: &str = "kayit";
pub const NONCE_POLL: &str = "poll";

pub fn mesaj(wallet: &str, nonce: &str, ts: u64) -> String {
    format!("{ONEK}|{}|{nonce}|{ts}", wallet.trim().to_lowercase())
}

pub fn cevap_ozeti(answer: &str) -> String {
    hex::encode(blake3::hash(answer.trim().as_bytes()).as_bytes())
}

pub fn is_nonce(job_id: u64, answer: &str) -> String {
    format!("is:{job_id}:{}", cevap_ozeti(answer))
}

/// Benchmark cevaplarinin kanonik ozeti: her cevap icin id(u64 LE) | ms(u64 LE) |
/// uzunluk(u64 LE) | cevap baytlari, gonderim sirasiyla.
pub fn benchmark_nonce(cevaplar: &[(u64, u64, &str)]) -> String {
    let mut h = blake3::Hasher::new();
    for (id, ms, c) in cevaplar {
        h.update(&id.to_le_bytes());
        h.update(&ms.to_le_bytes());
        h.update(&(c.len() as u64).to_le_bytes());
        h.update(c.as_bytes());
    }
    format!("benchmark:{}", hex::encode(h.finalize().as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesaj_bicimi_sabit() {
        assert_eq!(mesaj(" 0xABCDEF0000000000000000000000000000000001 ", NONCE_KAYIT, 42),
            "AIDAG-WORKER|0xabcdef0000000000000000000000000000000001|kayit|42");
        // cevap ozeti trim'li: bosluk farki ayni is nonce'u
        assert_eq!(is_nonce(7, " cevap\n"), is_nonce(7, "cevap"));
        assert_ne!(is_nonce(7, "cevap"), is_nonce(8, "cevap"));
        assert_ne!(is_nonce(7, "cevap"), is_nonce(7, "cevap2"));
        assert_ne!(benchmark_nonce(&[(1, 5, "ab"), (2, 5, "c")]), benchmark_nonce(&[(1, 5, "a"), (2, 5, "bc")]));
    }
}
