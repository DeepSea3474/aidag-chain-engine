//! Belge kayit TALEBI: imzasiz, tasinabilir kayit istegi.
//!
//! Akis: KUBRA (veya herhangi bir hazirlayici) talebi hazirlar ama IMZALAMAZ.
//! Kurum personeli talebi KENDI anahtariyla imzalar (tarayici, cevrimdisi
//! `belge-imzala` araci veya ileride e-Imza). Imzalayan taraf sunucudan gelen
//! "imzalanacak" degere guvenmez: id'yi talep alanlarindan KENDISI hesaplar.
//!
//! Mutabakat kurali DEGILDIR: zincire giden sey sıradan bir tip=1 Record
//! vertex'idir. Bu modul yalniz o vertex'i hazirlamayi/birlestirmeyi kolaylastirir.

use crate::dag::vertex::{hash_id, Vertex, VertexError, VertexId, MAX_PARENTS};
use crate::tx::Record;
use ed25519_dalek::SigningKey;

/// Imzasiz kayit talebi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KayitTalebi {
    pub network_id: u32,
    pub belge_hash: [u8; 32],
    /// Kanonik: kesin artan (sirali + tekrarsiz).
    pub parents: Vec<VertexId>,
    pub ts: u64,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TalepHatasi {
    #[error("en fazla {MAX_PARENTS} parent olabilir, {0} geldi")]
    CokParent(usize),
    #[error("vertex: {0}")]
    Vertex(#[from] VertexError),
}

impl KayitTalebi {
    /// Talep kur. Parent'lar kanonik bicime getirilir (sirala + tekrarlari at).
    pub fn yeni(network_id: u32, belge_hash: [u8; 32], mut parents: Vec<VertexId>, ts: u64) -> Result<Self, TalepHatasi> {
        parents.sort();
        parents.dedup();
        if parents.len() > MAX_PARENTS {
            return Err(TalepHatasi::CokParent(parents.len()));
        }
        Ok(KayitTalebi { network_id, belge_hash, parents, ts })
    }

    /// Vertex payload'i: `[tip=1][belge_hash:32]` (33 bayt).
    pub fn payload(&self) -> Vec<u8> {
        Record::new(self.belge_hash).encode()
    }

    /// Imzalayanin ed25519 ile imzalayacagi deger (= vertex id). Imzalayanin
    /// acik anahtarina baglidir; baska bir anahtarla imzalanmis id gecersizdir.
    pub fn imzalanacak_id(&self, imzalayan_pubkey: &[u8; 32]) -> VertexId {
        hash_id(self.network_id, imzalayan_pubkey, &self.parents, self.ts, &self.payload())
    }

    /// Anahtar dosyasiyla imzala (cevrimdisi arac / testler).
    pub fn imzala(&self, anahtar: &SigningKey) -> Result<Vertex, TalepHatasi> {
        Ok(Vertex::new_signed(self.network_id, self.parents.clone(), self.payload(), self.ts, anahtar)?)
    }

    /// Disarida (tarayici, donanim anahtari, ileride e-Imza koprusu) uretilmis
    /// imzayi talebe birlestir. Imza/anahtar yanlissa REDDEDILIR (verify_strict).
    pub fn imzayla_birlestir(&self, imzalayan_pubkey: [u8; 32], imza: [u8; 64]) -> Result<Vertex, TalepHatasi> {
        let id = self.imzalanacak_id(&imzalayan_pubkey);
        Ok(Vertex::from_parts(self.network_id, self.parents.clone(), self.payload(), self.ts, imzalayan_pubkey, imza, id)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::wire;
    use ed25519_dalek::Signer;

    fn anahtar(b: u8) -> SigningKey {
        SigningKey::from_bytes(&[b; 32])
    }

    #[test]
    fn payload_yalniz_record_ve_hash() {
        let t = KayitTalebi::yeni(3474, [9u8; 32], vec![], 100).unwrap();
        let p = t.payload();
        assert_eq!(p.len(), 33);
        assert_eq!(p[0], crate::tx::TX_TYPE_RECORD);
        assert_eq!(&p[1..], &[9u8; 32]);
    }

    #[test]
    fn parentlar_kanonik() {
        let t = KayitTalebi::yeni(1, [1u8; 32], vec![[3u8; 32], [1u8; 32], [3u8; 32]], 5).unwrap();
        assert_eq!(t.parents, vec![[1u8; 32], [3u8; 32]]);
        let cok: Vec<VertexId> = (0..=MAX_PARENTS as u32).map(|i| { let mut a = [0u8; 32]; a[..4].copy_from_slice(&i.to_le_bytes()); a }).collect();
        assert!(matches!(KayitTalebi::yeni(1, [1u8; 32], cok, 5), Err(TalepHatasi::CokParent(_))));
    }

    #[test]
    fn imzala_ve_dis_imza_ayni_vertex() {
        let t = KayitTalebi::yeni(3474, [7u8; 32], vec![[2u8; 32]], 1_758_650_000).unwrap();
        let k = anahtar(4);
        let pk = k.verifying_key().to_bytes();
        let v1 = t.imzala(&k).unwrap();
        // dis imzalayici: yalniz imzalanacak_id'yi imzalar
        let sig = k.sign(&t.imzalanacak_id(&pk)).to_bytes();
        let v2 = t.imzayla_birlestir(pk, sig).unwrap();
        assert_eq!(v1, v2);
        assert_eq!(v1.id(), &t.imzalanacak_id(&pk));
        // wire gidis-donus
        assert_eq!(wire::decode(&wire::encode(&v2)).unwrap(), v2);
    }

    #[test]
    fn id_imzalayana_bagli() {
        let t = KayitTalebi::yeni(3474, [7u8; 32], vec![], 1).unwrap();
        let a = anahtar(1).verifying_key().to_bytes();
        let b = anahtar(2).verifying_key().to_bytes();
        assert_ne!(t.imzalanacak_id(&a), t.imzalanacak_id(&b));
    }

    #[test]
    fn yanlis_imza_veya_anahtar_reddedilir() {
        let t = KayitTalebi::yeni(3474, [7u8; 32], vec![], 1).unwrap();
        let k = anahtar(1);
        let pk = k.verifying_key().to_bytes();
        let mut sig = k.sign(&t.imzalanacak_id(&pk)).to_bytes();
        // baska anahtarin pubkey'i ile birlestirme
        let baska = anahtar(2).verifying_key().to_bytes();
        assert!(t.imzayla_birlestir(baska, sig).is_err());
        // baska talebin imzasi
        let t2 = KayitTalebi::yeni(3474, [8u8; 32], vec![], 1).unwrap();
        assert!(t2.imzayla_birlestir(pk, sig).is_err());
        // bozuk imza
        sig[0] ^= 1;
        assert!(t.imzayla_birlestir(pk, sig).is_err());
    }
}
