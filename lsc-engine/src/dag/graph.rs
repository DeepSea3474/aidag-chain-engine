//! Graph — DAG deposu ve yapısal invariantlar (Adım 2).
//!
//! `vertex.rs` tek bir düğümün KENDİ içsel bütünlüğünü garanti eder
//! (hash + imza + bounds + dedup). `graph.rs` ise düğümler ARASINDAKİ
//! ilişkilerin geçerliliğini zorlar.
//!
//! ## Belirlenimcilik ayrımı (Y1 — denetçi)
//! Kurallar iki sınıfa ayrılır; karıştırmak consensus-split'e yol açar:
//!
//! * **Belirlenimci yapısal geçerlilik** — `validate_structural`. TÜM dürüst
//!   düğümler aynı vertex için aynı sonucu verir. Yalnızca vertex içeriği +
//!   ağ-genelinde sabit parametrelere (network_id, max_clock_skew_secs)
//!   bağlıdır; yerel saat (`now`) İÇERMEZ. Consensus girdisi budur.
//!   Kurallar 1-6 ve 8.
//! * **Yerel saat politikası** — `within_clock_policy`. Düğümün yerel
//!   `now`'una bağlı (kural 7). Düğümden düğüme DEĞİŞEBİLİR → relay/mempool
//!   kabul filtresidir, consensus girdisi DEĞİLDİR.
//!
//! İki giriş kapısı:
//! * `insert(v, now)` — relay yolu: saat politikası + yapısal + mutasyon.
//! * `insert_synced(v)` — güvenilir senkron/replay yolu: yalnızca yapısal +
//!   mutasyon (saat filtresi yok; finalize edilmiş geçmiş ve güven-kökü
//!   genesis saat kaymasından bağımsız kabul edilir).
//!
//! ## Zorlanan invariantlar
//!   1. **Primitif** — `verify()` (bozuk hash/imza giremez) → `Vertex`.
//!   2. **Ağ kimliği** — `network_id` eşleşmesi → `NetworkMismatch`.
//!   3. **Tekillik** — aynı id iki kez yok → `DuplicateVertex`.
//!   4. **Tek/whitelisted genesis** — parent'sız vertex yalnızca İLK ve TEK
//!      genesis; ikincisi → `UnexpectedGenesis`. `GenesisPolicy::Whitelisted`
//!      ise id uymak zorunda → `GenesisMismatch`. `FirstSeenDevnet` yalnızca
//!      devnet/test içindir (Y2).
//!   5. **Parent varlığı** — non-genesis'in TÜM ebeveynleri depoda olmalı
//!      → `UnknownParent`.
//!   6. **Asiklik** — (5)'in DOĞRUDAN sonucu. id içerik-adresli ve ebeveyn
//!      id'leri preimage'in parçası; bir id'yi ebeveyn göstermek için o
//!      vertex'in önceden var olması gerekir → döngü kriptografik olarak
//!      imkânsız (A→B→A, A.id=H(..B.id..) ∧ B.id=H(..A.id..) → hash fixpoint).
//!   7. **Saat kayması (YEREL POLİTİKA)** — `ts ≤ now + max_clock_skew_secs`
//!      → `TimestampTooFarFuture`. Geçmişe sınır YOK. Whitelist'li genesis
//!      bu kontrolden MUAF (O3 — id zaten güven kökü, geride saatli düğüm
//!      bootstrap edebilmeli). `max_clock_skew_secs` AYARLANABİLİR yerel knob.
//!   8. **Nedensellik (BELİRLENİMCİ)** — `child_ts + CAUSALITY_MAX_SKEW_SECS ≥
//!      parent_ts` → `TimestampBeforeParent`. `now` İÇERMEZ ve AYARLANAMAZ
//!      sabit `CAUSALITY_MAX_SKEW_SECS`'i kullanır (YN1 — denetçi). Kural 7'nin
//!      yerel knob'una (`max_clock_skew_secs`) BAĞLI DEĞİLDİR; aksi halde
//!      operatör NTP toleransını ayarladığında consensus kuralı sessizce
//!      değişir ve düğümler ayrışırdı.
//!
//! ## Orphan / retry (O2 — denetçi)
//! `UnknownParent` gossip ağlarında NORMALDİR (parent'lar sırasız gelir).
//! `insert`/`insert_synced` hata durumunda vertex'i TÜKETİR (Err içinde geri
//! vermez). Çağıran, retry için kopya saklayıp bir orphan-buffer tutmalıdır;
//! parent geldiğinde yeniden denenir. Yapısal orphan-pool Adım 3+'a aittir.
//!
//! ## Anti-fake notu
//! Sahte yok. "now" dışarıdan verilir → testler deterministik ve gerçek.
//! Hiçbir invariant simüle edilmez; her biri ayrı testle ölçülür.

use std::collections::{BTreeSet, HashMap};

use thiserror::Error;

use super::vertex::{Vertex, VertexError, VertexId};

// =====================================================================
// D-d (denetçi) — UYARI: AŞAĞIDAKİ İKİ SABİT SEMANTİK OLARAK AYRIDIR.
// Biri AYARLANABİLİR yerel knob varsayılanı (kural 7), diğeri AYARLANAMAZ
// consensus sabiti (kural 8). Aynı değere (300) sahip olmaları TESADÜFTÜR.
// ASLA tek bir sabitte BİRLEŞTİRMEYİN — "DRY" diye birleştirmek YN1
// belirlenimsizlik sızıntısını geri getirir (with_max_clock_skew consensus
// kuralını sessizce değiştirir → düğümler ayrışır).
// =====================================================================

/// Kural 7 (yerel saat politikası) için VARSAYILAN ileri kayma penceresi
/// (saniye). 5 dakika — makul NTP toleransı. `with_max_clock_skew` ile
/// düğüm-yerel olarak ayarlanabilir; bu YALNIZCA relay/mempool filtresini
/// (kural 7) etkiler, consensus'a dokunmaz.
pub const MAX_CLOCK_SKEW_SECS: u64 = 300;

/// Kural 8 (nedensellik) için CONSENSUS SABİTİ (saniye). AYARLANAMAZ —
/// tüm düğümlerde AYNI olmak ZORUNDA, yoksa yapısal geçerlilik (ve onun
/// üzerine kurulan GHOSTDAG) düğümler arası ayrışır (YN1 — denetçi).
/// Bir çocuğun timestamp'i, ebeveyninden en fazla bu kadar eski olabilir.
/// NOT: MAX_CLOCK_SKEW_SECS ile aynı değerde olması tesadüf — bkz. yukarıdaki
/// D-d uyarısı; birleştirmeyin.
pub const CAUSALITY_MAX_SKEW_SECS: u64 = 300;

/// Genesis kabul politikası (Y2 — denetçi). Tip düzeyinde footgun engeli:
/// mainnet whitelist'i atlamak ancak `FirstSeenDevnet`'i AÇIKÇA seçerek
/// mümkün olur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenesisPolicy {
    /// Mainnet: genesis id'si önceden pinlenmiş (güven kökü).
    Whitelisted(VertexId),
    /// YALNIZCA devnet/test: ilk görülen parent'sız vertex genesis kabul
    /// edilir. Mainnet'te ASLA kullanmayın — iki düğüm farklı ilk genesis
    /// görürse zincirler kalıcı olarak ayrışır.
    FirstSeenDevnet,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum GraphError {
    #[error("vertex primitive verification failed: {0}")]
    Vertex(#[from] VertexError),

    #[error("vertex network_id {got} does not match graph network_id {expected}")]
    NetworkMismatch { expected: u32, got: u32 },

    #[error("vertex already present in graph")]
    DuplicateVertex(VertexId),

    #[error("unknown parent (not in graph): vertex references a missing parent")]
    UnknownParent(VertexId),

    #[error("a second genesis (parentless) vertex is not allowed")]
    UnexpectedGenesis,

    #[error("genesis id does not match the whitelisted genesis")]
    GenesisMismatch,

    #[error("timestamp {ts} is too far in the future (now={now}, max_skew={max_skew})")]
    TimestampTooFarFuture { ts: u64, now: u64, max_skew: u64 },

    #[error("timestamp {child_ts} is older than parent {parent_ts} beyond skew {max_skew}")]
    TimestampBeforeParent {
        child_ts: u64,
        parent_ts: u64,
        max_skew: u64,
    },
}

/// İçerik-adresli DAG deposu. Tüm yapısal invariantları `validate_structural`,
/// saat politikasını `within_clock_policy` zorlar.
#[derive(Debug, Clone)]
pub struct Graph {
    network_id: u32,
    vertices: HashMap<VertexId, Vertex>,
    /// Ebeveyn → çocuklar. Belirlenimci olması için id-SIRALI tutulur
    /// (Y3 — gossip varış sırasından bağımsız).
    children: HashMap<VertexId, Vec<VertexId>>,
    /// Tip kümesi (çocuğu olmayan vertex'ler). Artımlı güncellenir (O5).
    /// BTreeSet → sıralı iterasyon bedava.
    tips: BTreeSet<VertexId>,
    genesis: Option<VertexId>,
    policy: GenesisPolicy,
    max_clock_skew_secs: u64,
    /// Yazar (public key) → o yazara ait vertex id'leri (id-SIRALI BTreeSet).
    /// Equivocation tespitinin ucuz temeli (Adım 3 — denetçi yönü #1): aynı
    /// yazardan birbirinin atası OLMAYAN ("paralel") iki vertex equivocation
    /// kanıtıdır. İndeks burada tutulur; ata/anticone sorgusu `consensus`
    /// katmanındadır (graph saf yapısal kalır).
    by_author: HashMap<[u8; 32], BTreeSet<VertexId>>,
}

impl Graph {
    /// Mainnet graph'ı — genesis id'si pinlenmiş (güven kökü).
    /// D-a (denetçi): all-zero id pinlemek graph'ı kalıcı boş bırakır
    /// (gerçek hash asla all-zero olmaz) → dev/test'te yakala.
    pub fn mainnet(network_id: u32, genesis_id: VertexId) -> Self {
        debug_assert!(
            genesis_id != [0u8; 32],
            "mainnet genesis id all-zero olamaz — hiçbir gerçek vertex bu id'yi taşımaz"
        );
        Graph::with_policy(network_id, GenesisPolicy::Whitelisted(genesis_id))
    }

    /// YALNIZCA devnet/test graph'ı — ilk görülen parent'sız vertex genesis
    /// olur. Mainnet'te kullanmayın (bkz. `GenesisPolicy::FirstSeenDevnet`).
    pub fn devnet(network_id: u32) -> Self {
        Graph::with_policy(network_id, GenesisPolicy::FirstSeenDevnet)
    }

    fn with_policy(network_id: u32, policy: GenesisPolicy) -> Self {
        Graph {
            network_id,
            vertices: HashMap::new(),
            children: HashMap::new(),
            tips: BTreeSet::new(),
            genesis: None,
            policy,
            max_clock_skew_secs: MAX_CLOCK_SKEW_SECS,
            by_author: HashMap::new(),
        }
    }

    /// YEREL saat kayması penceresini (kural 7 — relay/mempool politikası)
    /// özelleştir. SADECE kural 7'yi etkiler; kural 8 (nedensellik) ayarlanamaz
    /// `CAUSALITY_MAX_SKEW_SECS` sabitini kullanır (YN1 — denetçi). Yani bu
    /// knob consensus'a DOKUNAMAZ.
    pub fn with_max_clock_skew(mut self, secs: u64) -> Self {
        self.max_clock_skew_secs = secs;
        self
    }

    /// Relay/mempool yolu: yerel saat politikası (kural 7) + belirlenimci
    /// yapısal geçerlilik (1-6, 8) + mutasyon. `now` doğrulayan düğümün Unix
    /// saniye saati. Saat politikası düğümden düğüme DEĞİŞEBİLİR → bu kararın
    /// kabul/ret sonucu consensus girdisi DEĞİLDİR (bkz. `insert_synced`).
    /// İhlalde depo HİÇ değişmez (atomik — tüm kontroller mutasyon öncesi).
    pub fn insert(&mut self, v: Vertex, now: u64) -> Result<(), GraphError> {
        self.within_clock_policy(&v, now)?;
        self.insert_synced(v)
    }

    /// Güvenilir senkron/replay yolu: SADECE belirlenimci yapısal kurallar
    /// (1-6, 8). Saat politikası UYGULANMAZ → finalize edilmiş geçmiş ve
    /// güven-kökü genesis saat kaymasından bağımsız kabul edilir. TÜM dürüst
    /// düğümler bu fonksiyondan aynı sonucu alır. Atomik.
    pub fn insert_synced(&mut self, v: Vertex) -> Result<(), GraphError> {
        self.validate_structural(&v)?;
        self.commit(v);
        Ok(())
    }

    /// `insert_synced` ile AYNI — fakat ed25519 imza dogrulamasi ATLANIR.
    /// ON KOSUL: `v`'nin imzasi cagiran tarafindan ZATEN (paralel toplu)
    /// dogrulanmis olmali. Diger TUM yapisal kontroller (ag, duplicate, parent,
    /// timestamp, genesis) YINE calisir. pub(crate) — disariya/aga KAPALI.
    /// Yanlis kullanim = dogrulanmamis imza sizmasi -> ASLA aga acilan yoldan
    /// (ingest_networked) cagrilmaz; SADECE imzasi onceden dogrulanmis toplu
    /// yukleme yolundan.
    pub(crate) fn insert_synced_preverified(&mut self, v: Vertex) -> Result<(), GraphError> {
        self.validate_structural_impl(&v, true)?;
        self.commit(v);
        Ok(())
    }

    /// Belirlenimci yapısal geçerlilik (kurallar 1-6, 8). `now` İÇERMEZ →
    /// tüm düğümlerde aynı sonuç (consensus girdisi). Mutasyon YAPMAZ.
    /// Ucuz O(1) kontroller (ağ, duplicate) pahalı kriptodan ÖNCE çalışır
    /// (O1 — fail-fast, DoS yüzeyini azaltır).
    pub fn validate_structural(&self, v: &Vertex) -> Result<(), GraphError> {
        // Varsayilan: imza DAHIL tam dogrulama (davranis degismez).
        self.validate_structural_impl(v, false)
    }

    /// Ic varyant: `skip_sig=true` ise ed25519 imza dogrulamasi ATLANIR.
    /// SADECE imza ZATEN baska yerde (paralel toplu) dogrulanmissa cagrilir.
    /// pub(crate) — disariya/aga KAPALI; yanlis kullanim imza atlamasina yol acar.
    pub(crate) fn validate_structural_impl(
        &self,
        v: &Vertex,
        skip_sig: bool,
    ) -> Result<(), GraphError> {
        // 2. Ağ kimliği (O(1)).
        if v.network_id() != self.network_id {
            return Err(GraphError::NetworkMismatch {
                expected: self.network_id,
                got: v.network_id(),
            });
        }

        // 3. Tekillik (O(1)).
        if self.vertices.contains_key(v.id()) {
            return Err(GraphError::DuplicateVertex(*v.id()));
        }

        // 1. Primitif bütünlük (pahalı: blake3 + ed25519).
        // skip_sig=true: imza ZATEN paralel toplu dogrulandi -> tekrar etme
        // (ATLAMA DEGIL; bir kez dogrula, iki kez etme). Diger TUM kontroller calisir.
        if !skip_sig {
            v.verify()?;
        }

        if v.is_genesis() {
            // 4. Tek genesis + policy.
            if self.genesis.is_some() {
                return Err(GraphError::UnexpectedGenesis);
            }
            if let GenesisPolicy::Whitelisted(expected) = &self.policy {
                if v.id() != expected {
                    return Err(GraphError::GenesisMismatch);
                }
            }
        } else {
            // 5. Parent varlığı + 8. nedensellik. (6 asiklik bunun sonucu.)
            for p in v.parents() {
                match self.vertices.get(p) {
                    None => return Err(GraphError::UnknownParent(*p)),
                    Some(parent) => {
                        // YN1: belirlenimci consensus sabiti — yerel knob DEĞİL.
                        if v.timestamp().saturating_add(CAUSALITY_MAX_SKEW_SECS)
                            < parent.timestamp()
                        {
                            return Err(GraphError::TimestampBeforeParent {
                                child_ts: v.timestamp(),
                                parent_ts: parent.timestamp(),
                                max_skew: CAUSALITY_MAX_SKEW_SECS,
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Yerel saat politikası (kural 7). Düğümden düğüme değişebilir.
    /// Whitelist'li genesis ileri-skew kontrolünden MUAF (O3).
    fn within_clock_policy(&self, v: &Vertex, now: u64) -> Result<(), GraphError> {
        let is_whitelisted_genesis =
            v.is_genesis() && matches!(&self.policy, GenesisPolicy::Whitelisted(g) if g == v.id());
        if is_whitelisted_genesis {
            return Ok(());
        }
        if v.timestamp() > now.saturating_add(self.max_clock_skew_secs) {
            return Err(GraphError::TimestampTooFarFuture {
                ts: v.timestamp(),
                now,
                max_skew: self.max_clock_skew_secs,
            });
        }
        Ok(())
    }

    /// Doğrulanmış vertex'i depoya yaz. Infallible — `validate_structural`
    /// başarılıysa çağrılır. children sıralı tutulur (Y3), tips artımlı (O5).
    fn commit(&mut self, v: Vertex) {
        let id = *v.id();

        if v.is_genesis() {
            self.genesis = Some(id);
        }
        for p in v.parents() {
            let kids = self.children.entry(*p).or_default();
            if let Err(pos) = kids.binary_search(&id) {
                kids.insert(pos, id); // sıralı ekleme → belirlenimci
            }
            self.tips.remove(p); // parent artık tip değil
        }
        self.children.entry(id).or_default();
        self.tips.insert(id); // yeni vertex tip olur
        self.by_author
            .entry(*v.public_key())
            .or_default()
            .insert(id);
        self.vertices.insert(id, v);
    }

    pub fn get(&self, id: &VertexId) -> Option<&Vertex> {
        self.vertices.get(id)
    }

    pub fn contains(&self, id: &VertexId) -> bool {
        self.vertices.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.vertices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    pub fn network_id(&self) -> u32 {
        self.network_id
    }

    pub fn genesis(&self) -> Option<&VertexId> {
        self.genesis.as_ref()
    }

    /// Bir vertex'in çocukları — id-SIRALI (belirlenimci, Y3).
    pub fn children(&self, id: &VertexId) -> &[VertexId] {
        self.children.get(id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Tips — çocuğu olmayan vertex'ler (Adım 3 tip-selection girdisi).
    /// Artımlı tutulan BTreeSet'ten sıralı döner (O5).
    pub fn tips(&self) -> Vec<VertexId> {
        self.tips.iter().copied().collect()
    }

    /// Tüm vertex id'leri üzerinde iterasyon (sırasız — HashMap). Belirlenimci
    /// sıra gerektiğinde `consensus::topological_order` kullanın.
    pub fn ids(&self) -> impl Iterator<Item = &VertexId> {
        self.vertices.keys()
    }

    /// Belirli bir yazara (public key) ait vertex id'leri — id-SIRALI.
    /// Equivocation tespiti için temel (Adım 3 — denetçi yönü #1).
    /// Yazar hiç vertex üretmediyse boş slice mantığında `None`.
    pub fn author_vertices(&self, public_key: &[u8; 32]) -> Option<&BTreeSet<VertexId>> {
        self.by_author.get(public_key)
    }
}

// ===================================================================
// Tests
// ===================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::vertex::Vertex;
    use ed25519_dalek::SigningKey;

    const NET: u32 = 0xA1DA6;

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn genesis(ts: u64) -> Vertex {
        Vertex::new_signed(NET, vec![], b"genesis".to_vec(), ts, &key(1)).unwrap()
    }

    fn child(parents: Vec<VertexId>, ts: u64, payload: &[u8], seed: u8) -> Vertex {
        Vertex::new_signed(NET, parents, payload.to_vec(), ts, &key(seed)).unwrap()
    }

    // ===== Temel ekleme =====

    #[test]
    fn empty_graph_is_empty() {
        let g = Graph::devnet(NET);
        assert!(g.is_empty());
        assert_eq!(g.len(), 0);
        assert!(g.genesis().is_none());
    }

    #[test]
    fn insert_genesis_then_child() {
        let mut g = Graph::devnet(NET);
        let gen = genesis(100);
        let gid = *gen.id();
        g.insert(gen, 100).unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g.genesis(), Some(&gid));

        let c = child(vec![gid], 101, b"tx1", 2);
        let cid = *c.id();
        g.insert(c, 101).unwrap();
        assert_eq!(g.len(), 2);
        assert!(g.contains(&cid));
        assert_eq!(g.children(&gid), &[cid]);
    }

    // ===== Tek genesis / policy (Y2) =====

    #[test]
    fn devnet_accepts_first_seen_genesis() {
        let mut g = Graph::devnet(NET);
        g.insert(genesis(100), 100).unwrap();
        assert!(g.genesis().is_some());
    }

    #[test]
    fn second_genesis_rejected() {
        let mut g = Graph::devnet(NET);
        g.insert(genesis(100), 100).unwrap();
        let g2 = Vertex::new_signed(NET, vec![], b"other".to_vec(), 100, &key(2)).unwrap();
        assert_eq!(g.insert(g2, 100), Err(GraphError::UnexpectedGenesis));
        assert_eq!(g.len(), 1);
    }

    #[test]
    fn whitelisted_genesis_accepted() {
        let gen = genesis(100);
        let gid = *gen.id();
        let mut g = Graph::mainnet(NET, gid);
        g.insert(gen, 100).unwrap();
        assert_eq!(g.genesis(), Some(&gid));
    }

    #[test]
    fn wrong_genesis_rejected_by_whitelist() {
        let expected = *genesis(100).id();
        let mut g = Graph::mainnet(NET, expected);
        let wrong = Vertex::new_signed(NET, vec![], b"wrong".to_vec(), 100, &key(3)).unwrap();
        assert_eq!(g.insert(wrong, 100), Err(GraphError::GenesisMismatch));
        assert!(g.is_empty());
    }

    /// O3 — whitelist'li genesis ileri-skew muafiyeti: saati GERİDE düğüm
    /// (now ≪ genesis_ts) güven-kökü genesis'i yine de bootstrap edebilmeli.
    #[test]
    fn whitelisted_genesis_bootstraps_with_lagging_clock() {
        let gen = genesis(10_000);
        let gid = *gen.id();
        let mut g = Graph::mainnet(NET, gid);
        // now=1; skew=300 → normalde 10_000 > 301 reddedilirdi. Muafiyet → kabul.
        g.insert(gen, 1).unwrap();
        assert_eq!(g.genesis(), Some(&gid));
    }

    // ===== Parent varlığı / asiklik =====

    #[test]
    fn non_genesis_with_unknown_parent_rejected() {
        let mut g = Graph::devnet(NET);
        g.insert(genesis(100), 100).unwrap();
        let missing = [0xABu8; 32];
        let orphan = child(vec![missing], 101, b"orphan", 2);
        assert_eq!(
            g.insert(orphan, 101),
            Err(GraphError::UnknownParent(missing))
        );
        assert_eq!(g.len(), 1);
    }

    #[test]
    fn first_vertex_nongenesis_is_unknown_parent() {
        let mut g = Graph::devnet(NET);
        let p = [0x01u8; 32];
        let v = child(vec![p], 100, b"x", 2);
        assert_eq!(g.insert(v, 100), Err(GraphError::UnknownParent(p)));
    }

    /// D1 — asiklik + döngü enjeksiyonu denemesi. İki vertex birbirini
    /// parent göstermeye çalışırsa ikisi de depoda olmadığından UnknownParent
    /// ile reddedilir; içerik-adresli id döngüyü zaten kriptografik olarak
    /// imkânsız kılar (A.id=H(..B.id..) ∧ B.id=H(..A.id..) → fixpoint).
    #[test]
    fn cycle_injection_rejected() {
        let mut g = Graph::devnet(NET);
        let gen = genesis(100);
        let gid = *gen.id();
        g.insert(gen, 100).unwrap();

        let a = child(vec![gid], 101, b"a", 2);
        let aid = *a.id();
        g.insert(a, 101).unwrap();

        // a'yı, henüz var olmayan "fake_future" id'sine bağlamaya çalış →
        // döngü kurma girişiminin temel taşı. Reddedilir.
        let fake_future = [0xCDu8; 32];
        let cyclic = child(vec![fake_future], 102, b"cyclic", 3);
        assert_eq!(
            g.insert(cyclic, 102),
            Err(GraphError::UnknownParent(fake_future))
        );
        // Mevcut zincir bozulmadı.
        assert_eq!(g.children(&gid), &[aid]);
    }

    #[test]
    fn multi_parent_dag() {
        let mut g = Graph::devnet(NET);
        let gen = genesis(100);
        let gid = *gen.id();
        g.insert(gen, 100).unwrap();

        let a = child(vec![gid], 101, b"a", 2);
        let aid = *a.id();
        g.insert(a, 101).unwrap();
        let b = child(vec![gid], 101, b"b", 3);
        let bid = *b.id();
        g.insert(b, 101).unwrap();

        let mut parents = vec![aid, bid];
        parents.sort_unstable();
        let m = child(parents, 102, b"merge", 4);
        let mid = *m.id();
        g.insert(m, 102).unwrap();
        assert_eq!(g.len(), 4);
        assert_eq!(g.children(&aid), &[mid]);
        assert_eq!(g.children(&bid), &[mid]);
    }

    // ===== Tekillik =====

    #[test]
    fn duplicate_vertex_rejected() {
        let mut g = Graph::devnet(NET);
        let gen = genesis(100);
        let gid = *gen.id();
        g.insert(gen.clone(), 100).unwrap();
        assert_eq!(g.insert(gen, 100), Err(GraphError::DuplicateVertex(gid)));
        assert_eq!(g.len(), 1);
    }

    // ===== Ağ kimliği =====

    #[test]
    fn network_mismatch_rejected() {
        let mut g = Graph::devnet(NET);
        let foreign = Vertex::new_signed(NET + 1, vec![], b"x".to_vec(), 100, &key(1)).unwrap();
        assert_eq!(
            g.insert(foreign, 100),
            Err(GraphError::NetworkMismatch {
                expected: NET,
                got: NET + 1
            })
        );
    }

    // ===== Saat kayması (yerel politika, Y1) =====

    #[test]
    fn timestamp_too_far_future_rejected() {
        let mut g = Graph::devnet(NET);
        let gen = genesis(401); // now=100, skew=300 → tavan 400
        assert_eq!(
            g.insert(gen, 100),
            Err(GraphError::TimestampTooFarFuture {
                ts: 401,
                now: 100,
                max_skew: 300
            })
        );
    }

    #[test]
    fn timestamp_within_skew_accepted() {
        let mut g = Graph::devnet(NET);
        let gen = genesis(400); // tam sınır
        g.insert(gen, 100).unwrap();
        assert_eq!(g.len(), 1);
    }

    #[test]
    fn old_timestamp_accepted() {
        let mut g = Graph::devnet(NET);
        let gen = genesis(1);
        g.insert(gen, 1_000_000).unwrap();
        assert_eq!(g.len(), 1);
    }

    /// Y1 — yapısal geçerlilik SAAT-BAĞIMSIZDIR. Aynı vertex:
    /// validate_structural OK, insert (yerel saat) reddeder,
    /// insert_synced (saat yok) kabul eder. Saate bağlı ayrışma yalnızca
    /// yerel politikada izole.
    #[test]
    fn structural_validation_is_clock_independent() {
        let mut g = Graph::devnet(NET);
        let gen = genesis(100);
        let gid = *gen.id();
        g.insert(gen, 100).unwrap();

        // Gelecek-zamanlı child: yapısal geçerli (nedensellik OK), saat değil.
        let future = child(vec![gid], 100_000, b"future", 2);
        assert!(g.validate_structural(&future).is_ok());
        assert!(matches!(
            g.insert(future.clone(), 100),
            Err(GraphError::TimestampTooFarFuture { .. })
        ));
        // Senkron yol saat filtresi uygulamaz → kabul.
        g.insert_synced(future).unwrap();
        assert_eq!(g.len(), 2);
    }

    // ===== Nedensellik (belirlenimci) =====

    #[test]
    fn child_before_parent_beyond_skew_rejected() {
        let mut g = Graph::devnet(NET);
        // parent ts=10_000; child_ts + 300 < 10_000 → reddedilir. child=9_000.
        let gen = genesis(10_000);
        let gid = *gen.id();
        g.insert(gen, 10_000).unwrap();
        let c = child(vec![gid], 9_000, b"old", 2); // 9000+300=9300 < 10000 → red
        assert_eq!(
            g.insert(c, 10_000),
            Err(GraphError::TimestampBeforeParent {
                child_ts: 9_000,
                parent_ts: 10_000,
                max_skew: CAUSALITY_MAX_SKEW_SECS
            })
        );
    }

    #[test]
    fn child_slightly_before_parent_within_skew_accepted() {
        let mut g = Graph::devnet(NET);
        let gen = genesis(10_000);
        let gid = *gen.id();
        g.insert(gen, 10_000).unwrap();
        // child=9_800; 9800+300=10100 ≥ 10000 → kabul (nedensellik penceresi içi).
        let c = child(vec![gid], 9_800, b"x", 2);
        g.insert(c, 10_000).unwrap();
        assert_eq!(g.len(), 2);
    }

    /// YN1 — kural 8 (nedensellik) yerel `with_max_clock_skew` knob'una BAĞLI
    /// DEĞİL. Knob'u devasa yapsak bile nedensellik AYARLANAMAZ
    /// `CAUSALITY_MAX_SKEW_SECS` sabitini kullanır → reddetmeye devam eder.
    /// Bu, Y1'in temizlediği belirlenimsizlik sınıfının geri sızmadığını
    /// kanıtlar.
    #[test]
    fn causality_ignores_local_clock_knob() {
        // Yerel knob çok büyük (kural 7'yi gevşetir) — kural 8'i ETKİLEMEMELİ.
        let mut g = Graph::devnet(NET).with_max_clock_skew(100_000);
        let gen = genesis(10_000);
        let gid = *gen.id();
        g.insert(gen, 10_000).unwrap();
        // child=9_000; kural 7 (knob=100_000) bunu geçirir; kural 8 (sabit 300)
        // 9000+300=9300 < 10000 → yine reddeder.
        let c = child(vec![gid], 9_000, b"x", 2);
        assert_eq!(
            g.insert(c, 10_000),
            Err(GraphError::TimestampBeforeParent {
                child_ts: 9_000,
                parent_ts: 10_000,
                max_skew: CAUSALITY_MAX_SKEW_SECS
            })
        );
    }

    // ===== Primitif doğrulama graph'ta da zorlanır =====

    #[test]
    fn primitive_invalid_vertex_rejected() {
        let mut g = Graph::devnet(NET);
        let gen = genesis(100);
        let gid = *gen.id();
        g.insert(gen, 100).unwrap();
        let mut bad = child(vec![gid], 101, b"orig", 2);
        bad.tamper_payload(b"hacked".to_vec());
        assert_eq!(
            g.insert(bad, 101),
            Err(GraphError::Vertex(VertexError::IdMismatch))
        );
        assert_eq!(g.len(), 1);
    }

    // ===== Tips (O5 — artımlı) =====

    #[test]
    fn tips_tracks_leaves() {
        let mut g = Graph::devnet(NET);
        let gen = genesis(100);
        let gid = *gen.id();
        g.insert(gen, 100).unwrap();
        assert_eq!(g.tips(), vec![gid]);

        let a = child(vec![gid], 101, b"a", 2);
        let aid = *a.id();
        g.insert(a, 101).unwrap();
        assert_eq!(g.tips(), vec![aid]);

        let b = child(vec![gid], 101, b"b", 3);
        let bid = *b.id();
        g.insert(b, 101).unwrap();
        let mut expected = vec![aid, bid];
        expected.sort_unstable();
        assert_eq!(g.tips(), expected);
    }

    // ===== children() belirlenimci sırası (Y3) =====

    #[test]
    fn children_returned_sorted_regardless_of_insertion_order() {
        let mut g = Graph::devnet(NET);
        let gen = genesis(100);
        let gid = *gen.id();
        g.insert(gen, 100).unwrap();

        // Farklı seed'ler → farklı (sırasız) id'ler. Ekleme sırası rastgele.
        let mut ids = Vec::new();
        for seed in [9u8, 2, 7, 4, 1] {
            let c = child(vec![gid], 101, b"c", seed);
            ids.push(*c.id());
            g.insert(c, 101).unwrap();
        }
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        // children() ekleme sırasından bağımsız, daima sıralı.
        assert_eq!(g.children(&gid), sorted.as_slice());
    }

    // ===== insert_synced yolu =====

    #[test]
    fn insert_synced_skips_clock_policy() {
        let mut g = Graph::devnet(NET);
        // now bilgisi yok; uzak gelecekteki genesis bile kabul (replay/sync).
        let gen = genesis(9_999_999);
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();
        assert_eq!(g.genesis(), Some(&gid));
    }

    // ===== Atomiklik =====

    #[test]
    fn failed_insert_does_not_mutate() {
        let mut g = Graph::devnet(NET);
        g.insert(genesis(100), 100).unwrap();
        let before = g.len();
        let missing = [0x77u8; 32];
        let orphan = child(vec![missing], 101, b"x", 2);
        let _ = g.insert(orphan, 101);
        assert_eq!(g.len(), before);
        assert!(g.children(&missing).is_empty());
        // tips de kirlenmemeli.
        assert_eq!(g.tips().len(), 1);
    }
}
