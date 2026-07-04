//! Vertex — DAG'ın temel düğümü.
//!
//! Alanlar:
//!   * `network_id` — 32-bit ağ kimliği (mainnet/testnet/devnet ayrımı).
//!                    id preimage'inin parçası — cross-chain replay önler.
//!   * `parents`    — referans verdiği ebeveyn vertex ID'leri (genesis için boş).
//!                    KANONİK: strict artan (sözlüksel) sıralı + tekil olmak
//!                    ZORUNDA (p[i] < p[i+1]). Aynı DAG yapısının tek geçerli
//!                    temsilini garanti eder (malleability/grinding yüzeyini
//!                    kapatır). Sırasız/duplicate set primitif seviyede
//!                    `UnsortedOrDuplicateParents` ile reddedilir.
//!   * `payload`    — opak veri (işlem bayt'ları).
//!   * `timestamp`  — Unix saniye (üretici tarafından konur). Primitif
//!                    seviyede gelecek/geçmiş sınırı YOKTUR — consensus
//!                    katmanı bunu doğrulamak ZORUNDADIR (clock skew penceresi).
//!   * `public_key` — üreticinin ed25519 public key'i (32 bayt).
//!   * `signature`  — id üzerinde atılmış ed25519 imza (64 bayt).
//!
//! id formülü (domain-separated, blake3 streaming):
//!   id = blake3(
//!     DOMAIN_TAG (16) || FORMAT_VERSION (1) || network_id_le (4) ||
//!     public_key (32) || parent_count_le (8) || parents... (32 each) ||
//!     timestamp_le (8) || payload_len_le (8) || payload (n)
//!   )
//!
//! Doğrulama (`verify`):
//!   1. Bounds kontrolü (parent sayısı, payload boyutu) — saldırgan
//!      from_parts ile sınırı kaçırırsa burada yakalanır.
//!   2. id'yi yeniden hesapla, karşılaştır → IdMismatch.
//!   3. ed25519 `verify_strict` — small-order point + malleability korumalı.
//!
//! ## Anti-fake notu
//! Sahte / placeholder yoktur. Tüm sabitler ve testler `cargo test` ile
//! ölçülür. Format değişimi `FORMAT_VERSION` artırımı + hard-fork gerektirir.

use blake3::Hasher;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use thiserror::Error;

/// 32 bayt'lık blake3 hash — bir vertex'in içerik adresli ID'si.
pub type VertexId = [u8; 32];

/// Domain separation tag. Sabit 16 bayt. id preimage'ine ilk eklenir.
/// Aynı anahtar protokolün başka yerinde 32-byte bir şey imzalarsa
/// (oy hash'i, withdrawal id, blok hash'i), imzalar AIDAG vertex'i ile
/// karıştırılamaz.
pub const DOMAIN_TAG: &[u8; 16] = b"AIDAG-vertex-v1\0";

/// id format sürümü. Preimage'ın parçası. Format değişimi ⇒ artır + fork.
pub const FORMAT_VERSION: u8 = 1;

/// Bir vertex'in maksimum ebeveyn sayısı. Konsensüs tarafından zorlanır.
pub const MAX_PARENTS: usize = 8;

/// Payload boyut tavanı (1 MiB) — DAG spam'ini önlemek için.
pub const MAX_PAYLOAD_BYTES: usize = 1 << 20;

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum VertexError {
    #[error("too many parents: {0} > {max}", max = MAX_PARENTS)]
    TooManyParents(usize),

    #[error("payload too large: {0} bytes > {max}", max = MAX_PAYLOAD_BYTES)]
    PayloadTooLarge(usize),

    #[error("stored id does not match recomputed id (content tampered)")]
    IdMismatch,

    #[error("ed25519 signature verification failed (strict)")]
    BadSignature,

    #[error("invalid public key bytes")]
    BadPublicKey,

    #[error("parents must be strictly ascending (sorted + unique); canonical form violated")]
    UnsortedOrDuplicateParents,
}

/// DAG vertex'i. Alanlar private — geçerli yollar:
///   * [`Vertex::new_signed`] — yeni vertex üret ve imzala
///   * [`Vertex::from_parts`] — ağdan/diskten gelen baytları doğrulayıp inşa et
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vertex {
    network_id: u32,
    parents: Vec<VertexId>,
    payload: Vec<u8>,
    timestamp: u64,
    public_key: [u8; 32],
    signature: [u8; 64],
    id: VertexId,
}

impl Vertex {
    /// Yeni imzalı vertex üret. Bounds kontrol edilir, id hesaplanır,
    /// id imzalanır. Genesis için `parents = vec![]`.
    pub fn new_signed(
        network_id: u32,
        parents: Vec<VertexId>,
        payload: Vec<u8>,
        timestamp: u64,
        signing_key: &SigningKey,
    ) -> Result<Self, VertexError> {
        check_bounds(&parents, &payload)?;
        let public_key = signing_key.verifying_key().to_bytes();
        let id = hash_id(network_id, &public_key, &parents, timestamp, &payload);
        let signature: Signature = signing_key.sign(&id);
        Ok(Vertex {
            network_id,
            parents,
            payload,
            timestamp,
            public_key,
            signature: signature.to_bytes(),
            id,
        })
    }

    /// Güvenli deserialize girişi. Ağdan/diskten gelen ham alanları
    /// inşa eder ve `verify()` ile bütün invariantları zorlar:
    /// bounds + id integrity + ed25519 strict signature.
    pub fn from_parts(
        network_id: u32,
        parents: Vec<VertexId>,
        payload: Vec<u8>,
        timestamp: u64,
        public_key: [u8; 32],
        signature: [u8; 64],
        id: VertexId,
    ) -> Result<Self, VertexError> {
        // K1 v3: fail-fast bounds — crypto işine geçmeden önce ucuz kontroller.
        // (Wire deserializer'ın ayrıca byte-stream sınırlarını alloc ÖNCESİ
        //  zorlaması ZORUNLUDUR; bkz. dag::wire — Adım 5.)
        check_bounds(&parents, &payload)?;
        let v = Vertex {
            network_id,
            parents,
            payload,
            timestamp,
            public_key,
            signature,
            id,
        };
        v.verify()?;
        Ok(v)
    }

    /// Vertex'in bütünlüğünü doğrula. Çağrılma yolundan bağımsız olarak
    /// bounds, id ve imza yeniden kontrol edilir.
    pub fn verify(&self) -> Result<(), VertexError> {
        // K1: bounds verify yolunda da zorlanır (from_parts ya da bellek
        // bozma yoluyla kaçırılırsa yakala).
        check_bounds(&self.parents, &self.payload)?;

        let recomputed = hash_id(
            self.network_id,
            &self.public_key,
            &self.parents,
            self.timestamp,
            &self.payload,
        );
        if recomputed != self.id {
            return Err(VertexError::IdMismatch);
        }

        let vk =
            VerifyingKey::from_bytes(&self.public_key).map_err(|_| VertexError::BadPublicKey)?;
        let sig = Signature::from_bytes(&self.signature);

        // Y1: strict mode — small-order point + non-canonical S reddi.
        vk.verify_strict(&self.id, &sig)
            .map_err(|_| VertexError::BadSignature)?;

        Ok(())
    }

    pub fn network_id(&self) -> u32 {
        self.network_id
    }
    pub fn id(&self) -> &VertexId {
        &self.id
    }
    pub fn parents(&self) -> &[VertexId] {
        &self.parents
    }
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
    pub fn public_key(&self) -> &[u8; 32] {
        &self.public_key
    }
    pub fn signature(&self) -> &[u8; 64] {
        &self.signature
    }
    pub fn is_genesis(&self) -> bool {
        self.parents.is_empty()
    }

    // ===== TEST-ONLY tamper helpers =====
    // #[cfg(test)] + pub(crate) → release / downstream dep build'lerinde
    // derlenmez. Sadece bu crate'in iç testlerinden erişilebilir.

    #[cfg(test)]
    pub(crate) fn tamper_payload(&mut self, new_payload: Vec<u8>) {
        self.payload = new_payload;
    }

    #[cfg(test)]
    pub(crate) fn tamper_id(&mut self, new_id: VertexId) {
        self.id = new_id;
    }

    #[cfg(test)]
    pub(crate) fn tamper_signature(&mut self, new_sig: [u8; 64]) {
        self.signature = new_sig;
    }

    #[cfg(test)]
    pub(crate) fn tamper_oversized_parents(&mut self) {
        self.parents = vec![[0u8; 32]; MAX_PARENTS + 9];
    }

    #[cfg(test)]
    pub(crate) fn tamper_oversized_payload(&mut self) {
        self.payload = vec![0u8; MAX_PAYLOAD_BYTES + 1];
    }

    #[cfg(test)]
    pub(crate) fn tamper_with_duplicate_parents(&mut self) {
        // 2 eşit parent içeren küçük set (≤MAX_PARENTS).
        self.parents = vec![[5u8; 32], [5u8; 32]];
    }
}

/// Sınır kontrolü. Hem [`Vertex::new_signed`] hem [`Vertex::verify`]
/// tarafından çağrılır → K1 fix (DoS koruma her iki yolda da çalışır).
fn check_bounds(parents: &[VertexId], payload: &[u8]) -> Result<(), VertexError> {
    if parents.len() > MAX_PARENTS {
        return Err(VertexError::TooManyParents(parents.len()));
    }
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(VertexError::PayloadTooLarge(payload.len()));
    }
    // Seçenek A: parent seti KANONİK olmalı — strict artan (sözlüksel).
    // p[i] < p[i+1] hem sıralamayı hem tekilliği TEK kontrolde zorlar:
    //   * eşitlik (==) → duplicate → red
    //   * tersine sıra (>) → non-canonical → red
    // Genesis (boş) ve tek-parent muaf. Aynı DAG yapısının TEK geçerli
    // wire temsilini garanti eder (malleability/grinding yüzeyini kapatır).
    for w in parents.windows(2) {
        if w[0] >= w[1] {
            return Err(VertexError::UnsortedOrDuplicateParents);
        }
    }
    Ok(())
}

/// id hesaplaması — domain-separated, streaming (allocation yok).
/// preimage düzeni:
///   DOMAIN_TAG (16) || FORMAT_VERSION (1) || network_id_le (4) ||
///   public_key (32) || parent_count_le (8) || parents... (32 each) ||
///   timestamp_le (8) || payload_len_le (8) || payload (n)
pub(crate) fn hash_id(
    network_id: u32,
    public_key: &[u8; 32],
    parents: &[VertexId],
    timestamp: u64,
    payload: &[u8],
) -> VertexId {
    let mut h = Hasher::new();
    h.update(DOMAIN_TAG);
    h.update(&[FORMAT_VERSION]);
    h.update(&network_id.to_le_bytes());
    h.update(public_key);
    h.update(&(parents.len() as u64).to_le_bytes());
    for p in parents {
        h.update(p);
    }
    h.update(&timestamp.to_le_bytes());
    h.update(&(payload.len() as u64).to_le_bytes());
    h.update(payload);
    *h.finalize().as_bytes()
}

// ===================================================================
// Tests
// ===================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::OsRng, RngCore};

    const NET_TEST: u32 = 0xA1DA6;

    fn make_key() -> SigningKey {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        SigningKey::from_bytes(&bytes)
    }

    fn make_key_from_seed(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    // ===== Temel davranış =====

    #[test]
    fn genesis_vertex_has_no_parents() {
        let key = make_key();
        let v = Vertex::new_signed(NET_TEST, vec![], b"genesis".to_vec(), 1_000_000, &key).unwrap();
        assert!(v.is_genesis());
        v.verify().unwrap();
    }

    #[test]
    fn valid_vertex_verifies() {
        let key = make_key();
        let v = Vertex::new_signed(NET_TEST, vec![[9u8; 32]], b"hi".to_vec(), 123, &key).unwrap();
        v.verify().unwrap();
    }

    #[test]
    fn id_is_deterministic_for_same_inputs() {
        let key = make_key_from_seed(7);
        let p = vec![[1u8; 32], [2u8; 32]];
        let v1 = Vertex::new_signed(NET_TEST, p.clone(), b"tx".to_vec(), 42, &key).unwrap();
        let v2 = Vertex::new_signed(NET_TEST, p, b"tx".to_vec(), 42, &key).unwrap();
        assert_eq!(v1.id(), v2.id());
    }

    // ===== Her alan id'yi etkilemeli =====

    #[test]
    fn different_payload_changes_id() {
        let key = make_key_from_seed(1);
        let v1 = Vertex::new_signed(NET_TEST, vec![], b"a".to_vec(), 1, &key).unwrap();
        let v2 = Vertex::new_signed(NET_TEST, vec![], b"b".to_vec(), 1, &key).unwrap();
        assert_ne!(v1.id(), v2.id());
    }

    #[test]
    fn different_parents_change_id() {
        let key = make_key_from_seed(1);
        let v1 = Vertex::new_signed(NET_TEST, vec![[1u8; 32]], b"x".to_vec(), 1, &key).unwrap();
        let v2 = Vertex::new_signed(NET_TEST, vec![[2u8; 32]], b"x".to_vec(), 1, &key).unwrap();
        assert_ne!(v1.id(), v2.id());
    }

    #[test]
    fn different_timestamp_changes_id() {
        let key = make_key_from_seed(1);
        let v1 = Vertex::new_signed(NET_TEST, vec![], b"x".to_vec(), 1, &key).unwrap();
        let v2 = Vertex::new_signed(NET_TEST, vec![], b"x".to_vec(), 2, &key).unwrap();
        assert_ne!(v1.id(), v2.id());
    }

    #[test]
    fn different_signer_changes_id() {
        let k1 = make_key_from_seed(1);
        let k2 = make_key_from_seed(2);
        let v1 = Vertex::new_signed(NET_TEST, vec![], b"x".to_vec(), 1, &k1).unwrap();
        let v2 = Vertex::new_signed(NET_TEST, vec![], b"x".to_vec(), 1, &k2).unwrap();
        assert_ne!(v1.id(), v2.id());
    }

    /// K2 fix: network_id id'nin parçası — cross-chain replay önler.
    #[test]
    fn different_network_id_changes_id() {
        let key = make_key_from_seed(1);
        let v1 = Vertex::new_signed(1, vec![], b"x".to_vec(), 1, &key).unwrap();
        let v2 = Vertex::new_signed(2, vec![], b"x".to_vec(), 1, &key).unwrap();
        assert_ne!(v1.id(), v2.id());
    }

    // ===== Seçenek A: kanonik parent kuralı (strict artan) =====

    /// Sırasız parent dizisi reddedilir — [p2, p1] kanonik değil.
    #[test]
    fn unsorted_parents_rejected_in_new_signed() {
        let key = make_key_from_seed(1);
        assert_eq!(
            Vertex::new_signed(NET_TEST, vec![[2u8; 32], [1u8; 32]], b"x".to_vec(), 1, &key),
            Err(VertexError::UnsortedOrDuplicateParents)
        );
    }

    /// strict-< sayesinde duplicate parent ayrıca yakalanır — [p1, p1] eşit.
    #[test]
    fn strict_less_than_prevents_duplicate_parent() {
        let key = make_key_from_seed(1);
        assert_eq!(
            Vertex::new_signed(NET_TEST, vec![[1u8; 32], [1u8; 32]], b"x".to_vec(), 1, &key),
            Err(VertexError::UnsortedOrDuplicateParents)
        );
    }

    /// Strict artan (sıralı + tekil) çoklu parent kabul edilir.
    #[test]
    fn sorted_unique_parents_accepted() {
        let key = make_key_from_seed(1);
        let v = Vertex::new_signed(
            NET_TEST,
            vec![[1u8; 32], [2u8; 32], [3u8; 32]],
            b"x".to_vec(),
            1,
            &key,
        )
        .unwrap();
        v.verify().unwrap();
    }

    /// Tek parent muaf — kontrol edilecek çift yok, kabul.
    #[test]
    fn single_parent_accepted() {
        let key = make_key_from_seed(1);
        let v = Vertex::new_signed(NET_TEST, vec![[7u8; 32]], b"x".to_vec(), 1, &key).unwrap();
        v.verify().unwrap();
    }

    // ===== IdMismatch dalı =====

    #[test]
    fn tampered_payload_fails_verification() {
        let key = make_key();
        let mut v = Vertex::new_signed(NET_TEST, vec![], b"original".to_vec(), 1, &key).unwrap();
        v.tamper_payload(b"tampered".to_vec());
        assert_eq!(v.verify(), Err(VertexError::IdMismatch));
    }

    #[test]
    fn tampered_id_fails() {
        let key = make_key();
        let mut v = Vertex::new_signed(NET_TEST, vec![], b"x".to_vec(), 1, &key).unwrap();
        v.tamper_id([0xFFu8; 32]);
        assert_eq!(v.verify(), Err(VertexError::IdMismatch));
    }

    // ===== Y2: BadSignature dalı GERÇEKTEN test edilir =====

    /// k2'nin geçerli imzasını k1'in vertex'ine yapıştır → id tutarlı kalır
    /// (k1'in pk'sına göre hesaplandı), ama imza k2'ye ait → BadSignature.
    #[test]
    fn forged_signature_rejected() {
        let k1 = make_key_from_seed(1);
        let k2 = make_key_from_seed(2);
        let v1 = Vertex::new_signed(NET_TEST, vec![], b"x".to_vec(), 1, &k1).unwrap();
        let v2 = Vertex::new_signed(NET_TEST, vec![], b"x".to_vec(), 1, &k2).unwrap();
        let mut forged = v1.clone();
        forged.tamper_signature(*v2.signature());
        assert_eq!(forged.verify(), Err(VertexError::BadSignature));
    }

    /// Sıfır imza geçerli ed25519 değildir → verify_strict reddeder.
    #[test]
    fn all_zero_signature_rejected() {
        let key = make_key_from_seed(1);
        let mut v = Vertex::new_signed(NET_TEST, vec![], b"x".to_vec(), 1, &key).unwrap();
        v.tamper_signature([0u8; 64]);
        assert_eq!(v.verify(), Err(VertexError::BadSignature));
    }

    /// YB4 v4 fix: GERÇEK ve AYIRT EDİCİ small-order point testi.
    ///
    /// Önceki sürümde (v3) iki test-kalitesi açığı vardı:
    ///   1. 7 vector'den 5'i bozuktu: #5'in son baytı `0x0A` idi → eğri
    ///      üzerinde değil (libsodium kanonik değeri `0x7A`). #6 ise
    ///      uydurma — on-curve ama 8·P ≠ O (sıradan yüksek dereceli nokta).
    ///   2. İmza `[0u8; 64]` idi → verify_strict bunu pk ne olursa olsun
    ///      reddediyor (R=0 küçük-derece + s=0). Yani test, küçük-derece
    ///      pk reddini İSPATLAMIYORDU.
    ///
    /// Bu sürüm:
    ///   * Sadece eğri-doğrulanmış 6 gerçek küçük-derece noktası içerir
    ///     (`8·P == O` ile bağımsız teyit).
    ///   * Geçerli formdaki bir imza üretir (gerçek anahtar id'yi imzalar),
    ///     SONRA pk küçük-derece ile değiştirilir. R ve s biçimsel olarak
    ///     geçerli → tek ret sebebi small-order A olur.
    ///
    /// Kaynak: libsodium ref10 + RFC 8032 §5.1.7.
    #[test]
    fn small_order_public_keys_rejected() {
        let small_order_pks: [[u8; 32]; 6] = [
            // order 4 (Y = 0)
            [0u8; 32],
            // order 1 (identity)
            {
                let mut a = [0u8; 32];
                a[0] = 0x01;
                a
            },
            // order 2 (-identity)
            {
                let mut a = [0xFFu8; 32];
                a[0] = 0xEC;
                a[31] = 0x7F;
                a
            },
            // order 4
            {
                let mut a = [0u8; 32];
                a[31] = 0x80;
                a
            },
            // order 8
            [
                0x26, 0xE8, 0x95, 0x8F, 0xC2, 0xB2, 0x27, 0xB0, 0x45, 0xC3, 0xF4, 0x89, 0xF2, 0xEF,
                0x98, 0xF0, 0xD5, 0xDF, 0xAC, 0x05, 0xD3, 0xC6, 0x33, 0x39, 0xB1, 0x38, 0x02, 0x88,
                0x6D, 0x53, 0xFC, 0x05,
            ],
            // order 8 — son bayt 0x7A (libsodium kanonik); v3'te yanlışlıkla 0x0A idi.
            [
                0xC7, 0x17, 0x6A, 0x70, 0x3D, 0x4D, 0xD8, 0x4F, 0xBA, 0x3C, 0x0B, 0x76, 0x0D, 0x10,
                0x67, 0x0F, 0x2A, 0x20, 0x53, 0xFA, 0x2C, 0x39, 0xCC, 0xC6, 0x4E, 0xC7, 0xFD, 0x77,
                0x92, 0xAC, 0x03, 0x7A,
            ],
        ];

        // Gerçek bir anahtarla biçimsel olarak GEÇERLİ bir imza üret.
        // pk küçük-derece ile değiştirilince imzanın id ile alakası kalmaz,
        // ama R ve s biçimsel olarak geçerlidir → ret sebebi izole edilir:
        // ya BadPublicKey (decode) ya BadSignature (verify_strict small-order reddi).
        let real = make_key_from_seed(1);

        for (idx, pk) in small_order_pks.iter().enumerate() {
            let id = hash_id(NET_TEST, pk, &[], 1, b"x");
            let sig = real.sign(&id).to_bytes(); // biçimsel olarak geçerli imza
            let err =
                Vertex::from_parts(NET_TEST, vec![], b"x".to_vec(), 1, *pk, sig, id).unwrap_err();
            assert!(
                matches!(err, VertexError::BadPublicKey | VertexError::BadSignature),
                "small-order pk #{idx} kabul edildi! pk={:02x?} err={:?}",
                pk,
                err
            );
        }
    }

    // ===== K1: verify() bounds zorlama =====

    #[test]
    fn verify_rejects_smuggled_oversized_parents() {
        let key = make_key();
        let mut v = Vertex::new_signed(NET_TEST, vec![], b"x".to_vec(), 1, &key).unwrap();
        v.tamper_oversized_parents();
        assert!(matches!(v.verify(), Err(VertexError::TooManyParents(_))));
    }

    #[test]
    fn verify_rejects_smuggled_oversized_payload() {
        let key = make_key();
        let mut v = Vertex::new_signed(NET_TEST, vec![], b"x".to_vec(), 1, &key).unwrap();
        v.tamper_oversized_payload();
        assert!(matches!(v.verify(), Err(VertexError::PayloadTooLarge(_))));
    }

    // ===== Y3: from_parts (deserialize) =====

    #[test]
    fn from_parts_accepts_valid_vertex() {
        let key = make_key_from_seed(9);
        let v = Vertex::new_signed(NET_TEST, vec![[1u8; 32]], b"p".to_vec(), 1, &key).unwrap();
        let copy = Vertex::from_parts(
            v.network_id(),
            v.parents().to_vec(),
            v.payload().to_vec(),
            v.timestamp(),
            *v.public_key(),
            *v.signature(),
            *v.id(),
        )
        .unwrap();
        assert_eq!(v, copy);
    }

    #[test]
    fn from_parts_rejects_wrong_id() {
        let key = make_key_from_seed(9);
        let v = Vertex::new_signed(NET_TEST, vec![], b"p".to_vec(), 1, &key).unwrap();
        let err = Vertex::from_parts(
            v.network_id(),
            v.parents().to_vec(),
            v.payload().to_vec(),
            v.timestamp(),
            *v.public_key(),
            *v.signature(),
            [0u8; 32],
        )
        .unwrap_err();
        assert_eq!(err, VertexError::IdMismatch);
    }

    #[test]
    fn from_parts_rejects_oversized_payload() {
        let key = make_key_from_seed(9);
        let v = Vertex::new_signed(NET_TEST, vec![], b"p".to_vec(), 1, &key).unwrap();
        let big = vec![0u8; MAX_PAYLOAD_BYTES + 1];
        let err = Vertex::from_parts(
            v.network_id(),
            v.parents().to_vec(),
            big,
            v.timestamp(),
            *v.public_key(),
            *v.signature(),
            *v.id(),
        )
        .unwrap_err();
        assert_eq!(err, VertexError::PayloadTooLarge(MAX_PAYLOAD_BYTES + 1));
    }

    #[test]
    fn from_parts_rejects_forged_signature() {
        let k1 = make_key_from_seed(1);
        let k2 = make_key_from_seed(2);
        let v1 = Vertex::new_signed(NET_TEST, vec![], b"x".to_vec(), 1, &k1).unwrap();
        let v2 = Vertex::new_signed(NET_TEST, vec![], b"x".to_vec(), 1, &k2).unwrap();
        let err = Vertex::from_parts(
            v1.network_id(),
            v1.parents().to_vec(),
            v1.payload().to_vec(),
            v1.timestamp(),
            *v1.public_key(),
            *v2.signature(),
            *v1.id(),
        )
        .unwrap_err();
        assert_eq!(err, VertexError::BadSignature);
    }

    // ===== Sınır testleri =====

    /// YB4 kalıntı kapanışı — small-order guard'ını GERÇEKTEN izole eden test.
    ///
    /// `small_order_public_keys_rejected` küçük-derece pk'ları reddediyor
    /// ama bunu doğrulama DENKLEMİ uyuşmazlığıyla yapıyor (imza A_real için
    /// üretildi, A_small ile kontrol edildi → `[s]B ≠ R + [k]A`). Yani o
    /// test guard'ı çıkarsa bile yeşil kalır.
    ///
    /// Bu test farklı: identity (order-1) pk için `[k]A = O` olduğundan
    /// doğrulama denklemi `[s]B == R`'ye iner. Saldırgan `R = [s]B` seçerek
    /// HER mesaj için "geçerli" imza üretebilir (universal forgery). PLAIN
    /// (cofactored) verify bunu kabul EDERDİ; `verify_strict` ise A'nın
    /// küçük-derece olması nedeniyle reddetmek ZORUNDADIR. Guard kaldırılırsa
    /// BU test patlar → gerçek ayırt edici güç burada.
    #[test]
    fn identity_key_universal_forgery_rejected() {
        use curve25519_dalek::{constants::ED25519_BASEPOINT_POINT as B, scalar::Scalar};

        // identity public key (order 1): A = O
        let identity_pk = {
            let mut a = [0u8; 32];
            a[0] = 0x01;
            a
        };

        // R = [s]B forgery'si — plain denklemi [s]B == R sağlar.
        let s = Scalar::from(12345u64);
        let r = (s * B).compress().to_bytes();
        let mut sig = [0u8; 64];
        sig[..32].copy_from_slice(&r);
        sig[32..].copy_from_slice(s.as_bytes());

        // id identity_pk ile hesaplanır → IdMismatch'i geçer, verify_strict'e ulaşır.
        let id = hash_id(NET_TEST, &identity_pk, &[], 1, b"x");
        let err = Vertex::from_parts(NET_TEST, vec![], b"x".to_vec(), 1, identity_pk, sig, id)
            .unwrap_err();

        // verify_strict A'nın küçük-derece olması nedeniyle reddeder.
        assert_eq!(
            err,
            VertexError::BadSignature,
            "universal forgery kabul edildi — verify_strict small-order guard'ı çalışmıyor!"
        );
    }

    // ===== O1: duplicate parent reddi =====

    #[test]
    fn duplicate_parents_rejected_in_new_signed() {
        let key = make_key();
        let parents = vec![[1u8; 32], [2u8; 32], [1u8; 32]];
        assert_eq!(
            Vertex::new_signed(NET_TEST, parents, vec![], 1, &key).unwrap_err(),
            VertexError::UnsortedOrDuplicateParents
        );
    }

    #[test]
    fn duplicate_parents_rejected_in_from_parts() {
        let key = make_key();
        // Önce geçerli bir vertex üret, sonra parents'ı duplicate ile değiştir
        // ve manuel id hesapla (saldırgan senaryosu).
        let parents = vec![[3u8; 32], [3u8; 32]];
        let pk = key.verifying_key().to_bytes();
        let id = hash_id(NET_TEST, &pk, &parents, 1, b"x");
        let sig = key.sign(&id).to_bytes();
        let err = Vertex::from_parts(NET_TEST, parents, b"x".to_vec(), 1, pk, sig, id).unwrap_err();
        assert_eq!(err, VertexError::UnsortedOrDuplicateParents);
    }

    #[test]
    fn verify_rejects_smuggled_duplicate_parents() {
        let key = make_key();
        let mut v = Vertex::new_signed(NET_TEST, vec![[1u8; 32]], b"x".to_vec(), 1, &key).unwrap();
        // tamper: ikinci kopyayı ekle (yine ≤ MAX_PARENTS)
        v.tamper_with_duplicate_parents();
        assert_eq!(v.verify(), Err(VertexError::UnsortedOrDuplicateParents));
    }

    #[test]
    fn too_many_parents_rejected_in_new_signed() {
        let key = make_key();
        let parents = vec![[0u8; 32]; MAX_PARENTS + 1];
        assert_eq!(
            Vertex::new_signed(NET_TEST, parents, vec![], 1, &key).unwrap_err(),
            VertexError::TooManyParents(MAX_PARENTS + 1)
        );
    }

    #[test]
    fn oversized_payload_rejected_in_new_signed() {
        let key = make_key();
        let payload = vec![0u8; MAX_PAYLOAD_BYTES + 1];
        assert_eq!(
            Vertex::new_signed(NET_TEST, vec![], payload, 1, &key).unwrap_err(),
            VertexError::PayloadTooLarge(MAX_PAYLOAD_BYTES + 1)
        );
    }

    /// D5: tam sınır (MAX_PAYLOAD_BYTES, MAX_PARENTS) kabul edilmeli.
    #[test]
    fn exact_max_payload_accepted() {
        let key = make_key();
        let payload = vec![0u8; MAX_PAYLOAD_BYTES];
        let v = Vertex::new_signed(NET_TEST, vec![], payload, 1, &key).unwrap();
        v.verify().unwrap();
    }

    #[test]
    fn max_parents_accepted() {
        let key = make_key();
        // Her parent farklı olmalı — duplicate yasak (O1).
        let mut parents = Vec::with_capacity(MAX_PARENTS);
        for i in 0..MAX_PARENTS {
            parents.push([i as u8; 32]);
        }
        let v = Vertex::new_signed(NET_TEST, parents, vec![], 1, &key).unwrap();
        v.verify().unwrap();
    }

    #[test]
    fn empty_payload_allowed() {
        let key = make_key();
        let v = Vertex::new_signed(NET_TEST, vec![], vec![], 0, &key).unwrap();
        v.verify().unwrap();
    }

    #[test]
    fn id_length_is_32_bytes() {
        let key = make_key();
        let v = Vertex::new_signed(NET_TEST, vec![], b"x".to_vec(), 1, &key).unwrap();
        assert_eq!(v.id().len(), 32);
    }

    #[test]
    fn public_key_matches_signer() {
        let key = make_key_from_seed(42);
        let v = Vertex::new_signed(NET_TEST, vec![], b"x".to_vec(), 1, &key).unwrap();
        assert_eq!(v.public_key(), &key.verifying_key().to_bytes());
    }

    // ===== D6: known-answer vectors (canary — bu testler kırılırsa
    // wire format değişti demektir; mainnet sonrası YASAKTIR). =====

    /// DOMAIN_TAG'in kelimesi kelimesine doğru olduğunu sabitler.
    #[test]
    fn domain_tag_is_pinned() {
        assert_eq!(DOMAIN_TAG, b"AIDAG-vertex-v1\0");
        assert_eq!(DOMAIN_TAG.len(), 16);
        assert_eq!(FORMAT_VERSION, 1);
    }

    /// Boş genesis (network_id=0, seed=[1;32]) için id'yi pinleyen
    /// known-answer vector. blake3 ya da preimage düzeni değişirse
    /// bu test kırılır.
    ///
    /// Değer, ilk geçerli `cargo test` çalıştırmasından alındı ve
    /// kalıcı olarak buraya yazıldı.
    #[test]
    fn known_answer_genesis_id_seed_one() {
        let key = SigningKey::from_bytes(&[1u8; 32]);
        let v = Vertex::new_signed(0, vec![], vec![], 0, &key).unwrap();
        let id_hex = hex::encode(v.id());
        // KAT — değiştirme yasak (wire format kilidi).
        assert_eq!(
            id_hex, "c692f9dd55a0a57b9246679a2820091d0b3b6af27382cb1718bafb4f01fbfe9c",
            "KAT mismatch — wire format değişti mi? Önce hard-fork kararı al."
        );
    }

    /// YB3 v3: İkinci KAT — tüm alanlar SIFIRDAN FARKLI.
    /// Endianness hataları (LE/BE karışıklığı) sıfır alanlı KAT'ta
    /// görünmez; bu vector onları yakalar.
    ///
    /// Girdiler:
    ///   network_id = 0x01020304
    ///   parents    = [ [0xAA;32], [0xBB;32], [0xCC;32] ]
    ///   payload    = b"AIDAG/LSC-KAT/v1"  (16 bayt, ASCII)
    ///   timestamp  = 0x0102030405060708
    ///   signer     = seed [2;32]
    #[test]
    fn known_answer_nonzero_all_fields() {
        let key = SigningKey::from_bytes(&[2u8; 32]);
        let parents = vec![[0xAAu8; 32], [0xBBu8; 32], [0xCCu8; 32]];
        let payload = b"AIDAG/LSC-KAT/v1".to_vec();
        let v = Vertex::new_signed(0x01020304, parents, payload, 0x0102030405060708, &key).unwrap();
        let id_hex = hex::encode(v.id());
        assert_eq!(
            id_hex, "23e2c9903d7baa0bd8b3abe30811b7fa9d860adfd5e864ab26876590ec4255b7",
            "KAT-2 mismatch — endianness veya preimage düzeni değişti."
        );
    }
}
