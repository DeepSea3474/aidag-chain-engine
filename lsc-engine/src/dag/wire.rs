//! Wire — Vertex'in kanonik, belirlenimci, saldırıya dayanıklı serileştirmesi.
//!
//! ## Sorumluluk sınırı
//! Wire kripto invariantını YENİDEN UYGULAMAZ. Decode sonunda
//! [`Vertex::from_parts`]'a delege eder; o da bounds + kanoniklik
//! (strict artan parent) + id integrity + ed25519 `verify_strict` zorlar.
//! Wire'ın kendi işi:
//!   * güvenli ayrıştırma (panik yok, hep `Result`)
//!   * alloc-ÖNCESİ sınır zorlaması (alloc-bomb / OOM koruması)
//!   * yalancı uzunluk reddi (declared > kalan buffer)
//!   * kanoniklik: explicit version + sabit düzen + trailing-byte reddi
//!   * id'yi TAŞIMAZ — içerikten `hash_id` ile yeniden hesaplar
//!
//! ## Wire formatı (tüm tamsayılar little-endian)
//! ```text
//!   version(1) || network_id(4) || parent_count(8) || parents(n×32) ||
//!   timestamp(8) || payload_len(8) || public_key(32) || signature(64) ||
//!   payload(payload_len)
//! ```
//! `id` taşınmaz (decode'da hesaplanır). parent_count/payload_len u64 —
//! vertex id preimage'i ile tutarlı.

use crate::dag::vertex::{hash_id, Vertex, VertexError, VertexId, MAX_PARENTS, MAX_PAYLOAD_BYTES};

/// Wire çerçeve sürümü. id preimage'indeki FORMAT_VERSION'dan ayrıdır
/// (bu, transport çerçevesinin sürümü). Şimdilik ikisi de 1.
pub const WIRE_VERSION: u8 = 1;

#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum WireError {
    /// version baytı beklenenden farklı (yanlış protokol/çerçeve sürümü).
    InvalidVersion(u8),
    /// Buffer beklenen alanı tamamlayacak kadar bayt içermiyor.
    UnexpectedEof { needed: usize, got: usize },
    /// parent_count başlığı MAX_PARENTS'ı aşıyor — ALLOC ÖNCESİ reddedildi.
    TooManyParents(u64),
    /// payload_len başlığı MAX_PAYLOAD_BYTES'ı aşıyor — ALLOC ÖNCESİ reddedildi.
    PayloadTooLarge(u64),
    /// Bildirilen uzunluk kalan buffer'dan büyük (yalancı uzunluk / truncation).
    DeclaredLengthExceedsBuffer { declared: usize, remaining: usize },
    /// Decode sonunda artık bayt var (malleability / replay yüzeyi).
    TrailingBytes(usize),
    /// usize taşması (32-bit hedefte parent_count*32 veya payload_len cast).
    LengthOverflow,
    /// from_parts delegasyonu — bounds, kanoniklik, id, imza hataları.
    Vertex(VertexError),
}

impl From<VertexError> for WireError {
    fn from(e: VertexError) -> Self {
        WireError::Vertex(e)
    }
}

/// İç imleç — panik-yok okuma yardımcıları.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Cursor { bytes, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    /// `n` bayt'lık dilim al; yetmezse UnexpectedEof.
    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        if self.remaining() < n {
            return Err(WireError::UnexpectedEof {
                needed: n,
                got: self.remaining(),
            });
        }
        let s = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn read_u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }

    fn read_u32_le(&mut self) -> Result<u32, WireError> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn read_u64_le(&mut self) -> Result<u64, WireError> {
        let s = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        Ok(u64::from_le_bytes(a))
    }

    fn read_32(&mut self) -> Result<[u8; 32], WireError> {
        let s = self.take(32)?;
        let mut a = [0u8; 32];
        a.copy_from_slice(s);
        Ok(a)
    }

    fn read_64(&mut self) -> Result<[u8; 64], WireError> {
        let s = self.take(64)?;
        let mut a = [0u8; 64];
        a.copy_from_slice(s);
        Ok(a)
    }
}

/// Vertex'i kanonik wire baytlarına serileştir.
/// Geçerli bir `Vertex` her zaman serileştirilebilir (panik/başarısızlık yok).
pub fn encode(v: &Vertex) -> Vec<u8> {
    let parents = v.parents();
    let payload = v.payload();
    let mut out =
        Vec::with_capacity(1 + 4 + 8 + parents.len() * 32 + 8 + 8 + 32 + 64 + payload.len());
    out.push(WIRE_VERSION);
    out.extend_from_slice(&v.network_id().to_le_bytes());
    out.extend_from_slice(&(parents.len() as u64).to_le_bytes());
    for p in parents {
        out.extend_from_slice(p);
    }
    out.extend_from_slice(&v.timestamp().to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(v.public_key());
    out.extend_from_slice(v.signature());
    out.extend_from_slice(payload);
    out
}

/// Wire baytlarını güvenli biçimde Vertex'e çöz.
///
/// Alloc-ÖNCESİ sınır zorlaması ve yalancı-uzunluk reddi yapar; son olarak
/// `from_parts`'a delege ederek tüm kripto invariantlarını zorlar. id
/// taşınmaz — içerikten yeniden hesaplanır.
pub fn decode(bytes: &[u8]) -> Result<Vertex, WireError> {
    let mut c = Cursor::new(bytes);

    // 1) version
    let version = c.read_u8()?;
    if version != WIRE_VERSION {
        return Err(WireError::InvalidVersion(version));
    }

    // 2) network_id
    let network_id = c.read_u32_le()?;

    // 3) parent_count — ALLOC ÖNCESİ sınır
    let parent_count = c.read_u64_le()?;
    if parent_count > MAX_PARENTS as u64 {
        return Err(WireError::TooManyParents(parent_count));
    }
    let parent_count = parent_count as usize; // MAX_PARENTS'a sığar, güvenli

    // 4) Yalancı uzunluk: parent_count*32 kalan buffer'a sığıyor mu?
    let parents_bytes = parent_count
        .checked_mul(32)
        .ok_or(WireError::LengthOverflow)?;
    if c.remaining() < parents_bytes {
        return Err(WireError::DeclaredLengthExceedsBuffer {
            declared: parents_bytes,
            remaining: c.remaining(),
        });
    }

    // 5) parent'ları oku (sınır doğrulandı → güvenli alloc)
    let mut parents: Vec<VertexId> = Vec::with_capacity(parent_count);
    for _ in 0..parent_count {
        parents.push(c.read_32()?);
    }

    // 6) timestamp
    let timestamp = c.read_u64_le()?;

    // 7) payload_len — ALLOC ÖNCESİ sınır
    let payload_len = c.read_u64_le()?;
    if payload_len > MAX_PAYLOAD_BYTES as u64 {
        return Err(WireError::PayloadTooLarge(payload_len));
    }
    // 32-bit hedefte güvenli daraltma
    let payload_len: usize = payload_len
        .try_into()
        .map_err(|_| WireError::LengthOverflow)?;

    // 8) public_key + signature (sabit boyut)
    let public_key = c.read_32()?;
    let signature = c.read_64()?;

    // 9) Yalancı uzunluk: payload kalan buffer'a sığıyor mu?
    if c.remaining() < payload_len {
        return Err(WireError::DeclaredLengthExceedsBuffer {
            declared: payload_len,
            remaining: c.remaining(),
        });
    }
    let payload = c.take(payload_len)?.to_vec();

    // 10) Trailing bayt reddi — tek kanonik temsil.
    if c.remaining() != 0 {
        return Err(WireError::TrailingBytes(c.remaining()));
    }

    // 11) id'yi içerikten hesapla (taşınmaz). from_parts ayrıca yeniden
    //     hesaplayıp doğrular (IdMismatch) + imza + kanoniklik zorlar.
    let id = hash_id(network_id, &public_key, &parents, timestamp, &payload);

    let v = Vertex::from_parts(
        network_id, parents, payload, timestamp, public_key, signature, id,
    )?;
    Ok(v)
}

// ===================================================================
// Tests
// ===================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    const NET: u32 = 0xA1DA6;

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn sample(parents: Vec<VertexId>, payload: &[u8]) -> Vertex {
        // Kanonik sıra (Seçenek A) — parent'ları sırala.
        let mut parents = parents;
        parents.sort_unstable();
        Vertex::new_signed(NET, parents, payload.to_vec(), 12345, &key(1)).unwrap()
    }

    // ===== Pozitif: roundtrip =====

    #[test]
    fn roundtrip_genesis() {
        let v = sample(vec![], b"genesis");
        assert_eq!(decode(&encode(&v)).unwrap(), v);
    }

    #[test]
    fn roundtrip_single_parent() {
        let v = sample(vec![[3u8; 32]], b"tx");
        assert_eq!(decode(&encode(&v)).unwrap(), v);
    }

    #[test]
    fn roundtrip_multi_parent_sorted() {
        let v = sample(vec![[1u8; 32], [2u8; 32], [3u8; 32]], b"merge");
        assert_eq!(decode(&encode(&v)).unwrap(), v);
    }

    #[test]
    fn roundtrip_empty_payload() {
        let v = sample(vec![[7u8; 32]], b"");
        assert_eq!(decode(&encode(&v)).unwrap(), v);
    }

    #[test]
    fn max_parents_max_payload_roundtrip() {
        // 8 farklı (sıralı) parent + 1 MiB payload — sınır değerleri.
        let parents: Vec<VertexId> = (0u8..8).map(|i| [i + 1; 32]).collect();
        let payload = vec![0xABu8; MAX_PAYLOAD_BYTES];
        let v = sample(parents, &payload);
        assert_eq!(decode(&encode(&v)).unwrap(), v);
    }

    #[test]
    fn encode_is_deterministic() {
        let v = sample(vec![[1u8; 32], [2u8; 32]], b"x");
        assert_eq!(encode(&v), encode(&v));
    }

    // ===== Negatif: version =====

    #[test]
    fn wrong_version_rejected() {
        let v = sample(vec![], b"x");
        let mut b = encode(&v);
        b[0] = 2; // version baytını boz
        assert_eq!(decode(&b), Err(WireError::InvalidVersion(2)));
    }

    #[test]
    fn empty_buffer_rejected() {
        assert_eq!(
            decode(&[]),
            Err(WireError::UnexpectedEof { needed: 1, got: 0 })
        );
    }

    // ===== Negatif: alloc-bomb (sınır ALLOC ÖNCESİ) =====

    #[test]
    fn oversized_parent_count_rejected_before_alloc() {
        // version + network_id + parent_count=u64::MAX
        let mut b = Vec::new();
        b.push(WIRE_VERSION);
        b.extend_from_slice(&NET.to_le_bytes());
        b.extend_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(decode(&b), Err(WireError::TooManyParents(u64::MAX)));
    }

    #[test]
    fn oversized_payload_len_rejected_before_alloc() {
        // Geçerli başlık + parent_count=0 + timestamp + payload_len=u64::MAX
        let mut b = Vec::new();
        b.push(WIRE_VERSION);
        b.extend_from_slice(&NET.to_le_bytes());
        b.extend_from_slice(&0u64.to_le_bytes()); // parent_count
        b.extend_from_slice(&12345u64.to_le_bytes()); // timestamp
        b.extend_from_slice(&u64::MAX.to_le_bytes()); // payload_len
        assert_eq!(decode(&b), Err(WireError::PayloadTooLarge(u64::MAX)));
    }

    // ===== Negatif: yalancı uzunluk (declared > buffer) =====

    #[test]
    fn declared_parent_count_exceeds_buffer_rejected() {
        // parent_count=5 ama hiç parent bayt'ı yok.
        let mut b = Vec::new();
        b.push(WIRE_VERSION);
        b.extend_from_slice(&NET.to_le_bytes());
        b.extend_from_slice(&5u64.to_le_bytes());
        assert!(matches!(
            decode(&b),
            Err(WireError::DeclaredLengthExceedsBuffer { .. })
        ));
    }

    #[test]
    fn declared_payload_len_exceeds_buffer_rejected() {
        // Geçerli vertex'i al, payload_len'i şişir ama payload ekleme.
        let v = sample(vec![], b"hello");
        let good = encode(&v);
        // payload_len alanı: 1+4+8(pc=0)+8(ts)=21. offset 21..29.
        let mut b = good[..21 + 8 + 32 + 64].to_vec(); // header+pk+sig, payload'sız
                                                       // payload_len'i 1000 yap (gerçekte 0 payload var)
        b[21..29].copy_from_slice(&1000u64.to_le_bytes());
        assert!(matches!(
            decode(&b),
            Err(WireError::DeclaredLengthExceedsBuffer { .. })
        ));
    }

    // ===== Negatif: trailing bytes (malleability) =====

    #[test]
    fn trailing_bytes_rejected() {
        let v = sample(vec![[1u8; 32]], b"x");
        let mut b = encode(&v);
        b.push(0xFF); // arkaya çöp
        assert_eq!(decode(&b), Err(WireError::TrailingBytes(1)));
    }

    // ===== Negatif: truncation (her alan sınırında) =====

    #[test]
    fn truncated_at_every_length_rejected() {
        let v = sample(vec![[1u8; 32], [2u8; 32]], b"payload");
        let full = encode(&v);
        // Her prefix (tam uzunluk hariç) bir hata vermeli, panik DEĞİL.
        for len in 0..full.len() {
            let res = decode(&full[..len]);
            assert!(res.is_err(), "prefix len {len} kabul edildi (panik/ok)!");
        }
        // Tam uzunluk geçerli.
        assert_eq!(decode(&full).unwrap(), v);
    }

    // ===== Negatif: from_parts delegasyonu (kripto + kanoniklik) =====

    #[test]
    fn tampered_payload_in_wire_rejected() {
        // payload'ı boz → hesaplanan id imzayla uyuşmaz → IdMismatch/BadSignature.
        let v = sample(vec![], b"original");
        let mut b = encode(&v);
        // payload son 8 bayt ("original"). Son baytı değiştir.
        let n = b.len();
        b[n - 1] ^= 0xFF;
        // id decode'da yeniden hesaplanır; imza eski id'ye atılmıştı →
        // verify_strict başarısız → BadSignature (id tutarsızlığı imzayı bozar).
        assert!(matches!(
            decode(&b),
            Err(WireError::Vertex(VertexError::BadSignature))
        ));
    }

    #[test]
    fn forged_signature_in_wire_rejected() {
        let v = sample(vec![], b"x");
        let mut b = encode(&v);
        // signature offset: 1+4+8(pc=0)+8(ts)+8(plen)+32(pk) = 61. 64 bayt.
        for byte in &mut b[61..61 + 64] {
            *byte ^= 0xFF;
        }
        assert!(matches!(
            decode(&b),
            Err(WireError::Vertex(VertexError::BadSignature))
        ));
    }

    #[test]
    fn unsorted_parents_in_wire_rejected() {
        // Manuel olarak SIRASIZ parent'lı bir wire kur (sample sıralar, biz kurmuyoruz).
        // Geçerli imzalı bir vertex'i alıp parent baytlarını ters çevirmek imzayı
        // bozar; bunun yerine sırasız parent'ı new_signed reddedeceği için, wire'a
        // doğrudan sırasız parent enjekte edip from_parts'ın yakaladığını gösteririz.
        // p2 > p1 sırayla yazılır.
        let p1 = [1u8; 32];
        let p2 = [2u8; 32];
        // Kanonik vertex (sıralı) üret, sonra wire'da parent sırasını ters çevir.
        let v = sample(vec![p1, p2], b"x");
        let good = encode(&v);
        let mut b = good.clone();
        // parents offset: 1+4+8 = 13. İki parent: [13..45]=p1, [45..77]=p2. Ters çevir.
        let (a_start, b_start) = (13usize, 45usize);
        let first: Vec<u8> = b[a_start..a_start + 32].to_vec();
        let second: Vec<u8> = b[b_start..b_start + 32].to_vec();
        b[a_start..a_start + 32].copy_from_slice(&second);
        b[b_start..b_start + 32].copy_from_slice(&first);
        // Artık parent'lar [p2, p1] sırasında → kanonik değil. id yeniden
        // hesaplanır; from_parts check_bounds'ta UnsortedOrDuplicateParents verir.
        assert!(matches!(
            decode(&b),
            Err(WireError::Vertex(VertexError::UnsortedOrDuplicateParents))
        ));
    }

    #[test]
    fn duplicate_parents_in_wire_rejected() {
        // Manuel olarak [p1, p1] (duplicate) wire kur. sample() sıralar ama
        // duplicate'i sıralama düzeltmez; new_signed zaten reddeder. Bu yüzden
        // geçerli tek-parent'lı vertex'i alıp parent_count'u 2 yapar ve ikinci
        // parent olarak aynısını enjekte ederiz — id yeniden hesaplanır,
        // from_parts check_bounds'ta strict-< ihlalini (==) yakalar.
        let p1 = [9u8; 32];
        let v = sample(vec![p1], b"x"); // tek parent, kanonik
        let good = encode(&v);
        // good düzeni: ver(1)+net(4)+pc(8)=13, parent[13..45], ts[45..53], ...
        // Yeni buffer: pc=2, iki kez p1, sonra orijinalin ts'den sonrası.
        let mut b = Vec::new();
        b.push(WIRE_VERSION);
        b.extend_from_slice(&NET.to_le_bytes());
        b.extend_from_slice(&2u64.to_le_bytes()); // parent_count = 2
        b.extend_from_slice(&p1); // parent 1
        b.extend_from_slice(&p1); // parent 2 (duplicate!)
        b.extend_from_slice(&good[45..]); // ts + payload_len + pk + sig + payload
        assert!(matches!(
            decode(&b),
            Err(WireError::Vertex(VertexError::UnsortedOrDuplicateParents))
        ));
    }
}
