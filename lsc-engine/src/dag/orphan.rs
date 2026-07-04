//! Yetim (orphan) vertex havuzu — ağdan gelen ama ebeveynleri henüz lokal
//! grafikte OLMAYAN vertex'leri geçici tutan gümrük kapısı.
//!
//! Güvenlik invaryantları (UYARI değil, YAPIYA gömülü):
//!   1. OOM koruması: havuz katı kapasiteli (MAX_ORPHANS). Doluysa yeni gelen
//!      REDDEDİLİR (PoolFull). Bellek kontrolsüz büyüyemez.
//!   2. TTL: her giriş `Instant` damgalı; `clean_expired` süreyi geçeni siler.
//!   3. Reaksiyon: ebeveyn entegre olunca, SADECE tüm ebeveynleri tamamlanan
//!      çocuklar serbest bırakılır (yarım vertex salınmaz).
//!   4. `unsafe` YOK; her şey HashMap + Result ile.
//!
//! CASCADE NOTU: `on_parent_integrated` TEK kuşak döndürür. Serbest kalan bir
//! vertex ana grafiğe işlenince, ÇAĞIRAN taraf onun id'siyle tekrar
//! `on_parent_integrated` çağırmalıdır (zincirleme çözülme çağıranda yönetilir).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::dag::vertex::{Vertex, VertexId};

/// Havuzun katı maksimum kapasitesi (OOM koruması).
pub const MAX_ORPHANS: usize = 1024;

/// Yetim havuzuna ekleme hatası.
#[derive(Debug, PartialEq, Eq)]
pub enum OrphanError {
    /// Havuz dolu (>= MAX_ORPHANS). Yeni yetim reddedildi.
    PoolFull,
    /// Bu vertex zaten havuzda (duplicate).
    AlreadyPresent,
}

impl std::fmt::Display for OrphanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrphanError::PoolFull => write!(f, "orphan pool full (>= {MAX_ORPHANS})"),
            OrphanError::AlreadyPresent => write!(f, "orphan already present"),
        }
    }
}
impl std::error::Error for OrphanError {}

/// Havuzdaki tek bir yetim girişi: vertex + eklenme zamanı.
struct OrphanEntry {
    vertex: Vertex,
    inserted_at: Instant,
}

/// Yetim vertex havuzu. Ebeveyni eksik vertex'leri geçici tutar.
pub struct OrphanPool {
    /// vertex id -> giriş (vertex + zaman damgası).
    orphans: HashMap<VertexId, OrphanEntry>,
    /// beklenen ebeveyn id -> onu bekleyen çocuk id'leri.
    /// (indeks: ebeveyn gelince O(1) bekleyen-liste erişimi.)
    waiting_on: HashMap<VertexId, Vec<VertexId>>,
}

impl Default for OrphanPool {
    fn default() -> Self {
        Self::new()
    }
}

impl OrphanPool {
    /// Boş havuz.
    pub fn new() -> Self {
        OrphanPool {
            orphans: HashMap::new(),
            waiting_on: HashMap::new(),
        }
    }

    /// Havuzdaki yetim sayısı.
    pub fn len(&self) -> usize {
        self.orphans.len()
    }

    /// Havuz boş mu.
    pub fn is_empty(&self) -> bool {
        self.orphans.is_empty()
    }

    /// Bu vertex id havuzda mı.
    pub fn contains(&self, id: &VertexId) -> bool {
        self.orphans.contains_key(id)
    }

    /// Bir yetim vertex ekle.
    /// - Havuz doluysa (>= MAX_ORPHANS) → `PoolFull` (OOM koruması).
    /// - Zaten varsa → `AlreadyPresent`.
    /// Aksi halde: zaman damgalı eklenir; eksik ebeveynleri `waiting_on`'a
    /// indekslenir.
    pub fn add_orphan(&mut self, v: Vertex) -> Result<(), OrphanError> {
        // OOM koruması: ekleme ÖNCESİ kapasite kontrolü.
        if self.orphans.len() >= MAX_ORPHANS {
            return Err(OrphanError::PoolFull);
        }
        let id = *v.id();
        if self.orphans.contains_key(&id) {
            return Err(OrphanError::AlreadyPresent);
        }

        // Eksik ebeveynler için bekleme indeksini güncelle.
        // (Hangi ebeveynin eksik olduğunu havuz BİLMEZ; çağıran sadece eksik
        //  olanları beklemek istiyorsa parents()'ı verir. Burada TÜM parent'lar
        //  indekslenir; entegre olduklarında düşülür — fazla indeks zararsız.)
        for parent in v.parents() {
            self.waiting_on.entry(*parent).or_default().push(id);
        }

        self.orphans.insert(
            id,
            OrphanEntry {
                vertex: v,
                inserted_at: Instant::now(),
            },
        );
        Ok(())
    }

    /// Bir ebeveyn ana grafiğe entegre olduğunda çağrılır. O ebeveyni bekleyen
    /// çocuklardan, ARTIK TÜM ebeveynleri tamamlanmış olanları havuzdan çıkarıp
    /// döndürür (ana grafiğe işlenmeye hazır). Hâlâ başka ebeveyn bekleyen
    /// çocuklar havuzda KALIR (yarım vertex salınmaz).
    ///
    /// TEK kuşak döndürür; cascade çağıranda (bkz. dosya başı not).
    pub fn on_parent_integrated(&mut self, parent_id: &VertexId) -> Vec<Vertex> {
        let waiters = match self.waiting_on.remove(parent_id) {
            Some(w) => w,
            None => return Vec::new(),
        };

        let mut ready = Vec::new();
        for child_id in waiters {
            // Çocuk hâlâ havuzda mı? (Başka yoldan çıkarılmış olabilir.)
            let still_waiting = match self.orphans.get(&child_id) {
                Some(entry) => {
                    // Bu çocuğun BAŞKA bekleyen ebeveyni var mı?
                    // (waiting_on'da hâlâ bu çocuğu bekleten bir ebeveyn varsa.)
                    entry
                        .vertex
                        .parents()
                        .iter()
                        .any(|p| self.parent_still_pending(p, &child_id))
                }
                None => continue, // zaten çıkmış
            };

            if !still_waiting {
                if let Some(entry) = self.orphans.remove(&child_id) {
                    ready.push(entry.vertex);
                }
            }
        }
        ready
    }

    /// `parent`'ı bekleyen ve `except_child` DIŞINDA bir çocuk var mı; ya da
    /// `parent` hâlâ `waiting_on`'da bu çocuğu bekletiyor mu. Yani bu çocuk
    /// için `parent` hâlâ "eksik ebeveyn" sayılıyor mu?
    fn parent_still_pending(&self, parent: &VertexId, child: &VertexId) -> bool {
        match self.waiting_on.get(parent) {
            Some(children) => children.contains(child),
            None => false,
        }
    }

    /// Süresi (TTL) geçmiş yetimleri temizle. Silinen sayısını döndürür.
    /// TTL koruması: eski yetimler bellekte sonsuza kadar kalamaz.
    pub fn clean_expired(&mut self, ttl: Duration) -> usize {
        let now = Instant::now();
        // Süresi geçen id'leri topla.
        let expired: Vec<VertexId> = self
            .orphans
            .iter()
            .filter(|(_, e)| now.duration_since(e.inserted_at) >= ttl)
            .map(|(id, _)| *id)
            .collect();

        for id in &expired {
            if let Some(entry) = self.orphans.remove(id) {
                // waiting_on indeksinden bu çocuğu düş.
                for parent in entry.vertex.parents() {
                    if let Some(children) = self.waiting_on.get_mut(parent) {
                        children.retain(|c| c != id);
                        if children.is_empty() {
                            self.waiting_on.remove(parent);
                        }
                    }
                }
            }
        }
        expired.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    const NET: u32 = 1;

    // Belirli parent'larla imzali bir vertex uret. Her cagri farkli payload
    // -> farkli id (cakisma yok).
    fn mk_vertex(parents: Vec<VertexId>, tag: u8) -> Vertex {
        let sk = SigningKey::from_bytes(&[tag; 32]);
        Vertex::new_signed(NET, parents, vec![tag, tag, tag], 1_000_000, &sk).expect("vertex")
    }

    // Gecerli ama var olmayan bir parent id (sabit, all-zero degil).
    fn fake_parent(seed: u8) -> VertexId {
        [seed; 32]
    }

    #[test]
    fn new_pool_is_empty() {
        let pool = OrphanPool::new();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn add_orphan_increases_len() {
        let mut pool = OrphanPool::new();
        let v = mk_vertex(vec![fake_parent(9)], 1);
        let id = *v.id();
        pool.add_orphan(v).expect("add");
        assert_eq!(pool.len(), 1);
        assert!(pool.contains(&id));
    }

    #[test]
    fn duplicate_add_rejected() {
        let mut pool = OrphanPool::new();
        let v = mk_vertex(vec![fake_parent(9)], 1);
        let v2 = mk_vertex(vec![fake_parent(9)], 1); // ayni tag -> ayni id
        pool.add_orphan(v).expect("first");
        assert_eq!(pool.add_orphan(v2), Err(OrphanError::AlreadyPresent));
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn pool_full_rejects_new_orphan() {
        let mut pool = OrphanPool::new();
        // MAX_ORPHANS kadar farkli vertex ekle (her biri farkli tag/payload).
        // Not: u8 256 ile sinirli; bu yuzden parent'i de degistirerek id'yi
        // farklilastiriyoruz. Daha basit: timestamp/parent kombinasyonu.
        // Burada MAX'i dogrudan test etmek icin kucuk bir kapasite kanit
        // senaryosu: havuzu MAX_ORPHANS'a kadar doldur, +1 reddedilsin.
        let mut count = 0usize;
        // Farkli id'ler icin parent seed + tag kombinasyonu kullan.
        'outer: for a in 0u8..=255 {
            for b in 0u8..=255 {
                if count >= MAX_ORPHANS {
                    break 'outer;
                }
                let sk = SigningKey::from_bytes(&[a; 32]);
                let v = Vertex::new_signed(NET, vec![fake_parent(b)], vec![a, b], 1_000_000, &sk)
                    .expect("v");
                // Cakisma olursa atla (ayni id).
                if pool.add_orphan(v).is_ok() {
                    count += 1;
                }
            }
        }
        assert_eq!(pool.len(), MAX_ORPHANS);
        // Havuz dolu: yeni gelen REDDEDILMELI (OOM koruması).
        let extra = mk_vertex(vec![fake_parent(200)], 200);
        assert_eq!(pool.add_orphan(extra), Err(OrphanError::PoolFull));
        assert_eq!(pool.len(), MAX_ORPHANS);
    }

    #[test]
    fn single_parent_child_released_on_integration() {
        let mut pool = OrphanPool::new();
        let parent = fake_parent(50);
        let child = mk_vertex(vec![parent], 1);
        let child_id = *child.id();
        pool.add_orphan(child).expect("add");

        // Ebeveyn entegre oldu -> cocuk serbest kalmali.
        let released = pool.on_parent_integrated(&parent);
        assert_eq!(released.len(), 1);
        assert_eq!(*released[0].id(), child_id);
        assert!(pool.is_empty());
    }

    #[test]
    fn no_waiters_returns_empty() {
        let mut pool = OrphanPool::new();
        let released = pool.on_parent_integrated(&fake_parent(123));
        assert!(released.is_empty());
    }

    #[test]
    fn two_parent_child_not_released_until_both_arrive() {
        let mut pool = OrphanPool::new();
        // parents canonical (artan) sirali olmali — kucukten buyuge.
        let p1 = fake_parent(10);
        let p2 = fake_parent(20);
        let child = mk_vertex(vec![p1, p2], 1);
        let child_id = *child.id();
        pool.add_orphan(child).expect("add");

        // Sadece p1 geldi -> cocuk HALA serbest kalmamali (p2 eksik).
        let r1 = pool.on_parent_integrated(&p1);
        assert!(r1.is_empty(), "tek ebeveyn yetmez, yarim vertex salinmaz");
        assert_eq!(pool.len(), 1);

        // Simdi p2 de geldi -> cocuk serbest kalmali.
        let r2 = pool.on_parent_integrated(&p2);
        assert_eq!(r2.len(), 1);
        assert_eq!(*r2[0].id(), child_id);
        assert!(pool.is_empty());
    }

    #[test]
    fn clean_expired_removes_old_keeps_fresh() {
        let mut pool = OrphanPool::new();
        let v = mk_vertex(vec![fake_parent(9)], 1);
        pool.add_orphan(v).expect("add");
        assert_eq!(pool.len(), 1);

        // Cok kisa TTL (0) ile temizle: giris zaten gecmiste -> silinmeli.
        std::thread::sleep(std::time::Duration::from_millis(5));
        let removed = pool.clean_expired(std::time::Duration::from_millis(1));
        assert_eq!(removed, 1);
        assert!(pool.is_empty());
    }

    #[test]
    fn clean_expired_keeps_recent() {
        let mut pool = OrphanPool::new();
        let v = mk_vertex(vec![fake_parent(9)], 1);
        pool.add_orphan(v).expect("add");
        // Cok uzun TTL: taze giris KALMALI.
        let removed = pool.clean_expired(std::time::Duration::from_secs(3600));
        assert_eq!(removed, 0);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn cascade_resolved_by_caller_loop() {
        // A <- B <- C zinciri (B, A'yi bekler; C, B'yi bekler).
        // Cascade CAGIRAN tarafindan dongu ile cozulur (dosya basi not).
        let mut pool = OrphanPool::new();
        let a = fake_parent(1);
        let b_vertex = mk_vertex(vec![a], 2);
        let b_id = *b_vertex.id();
        let c_vertex = mk_vertex(vec![b_id], 3);
        let c_id = *c_vertex.id();

        pool.add_orphan(b_vertex).expect("add b");
        pool.add_orphan(c_vertex).expect("add c");
        assert_eq!(pool.len(), 2);

        // A entegre oldu -> B serbest.
        let r1 = pool.on_parent_integrated(&a);
        assert_eq!(r1.len(), 1);
        assert_eq!(*r1[0].id(), b_id);

        // Cagiran, B'yi ana grafige isleyip B'nin id'siyle TEKRAR cagirir -> C serbest.
        let r2 = pool.on_parent_integrated(&b_id);
        assert_eq!(r2.len(), 1);
        assert_eq!(*r2[0].id(), c_id);
        assert!(pool.is_empty());
    }
}
