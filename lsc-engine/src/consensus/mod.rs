//! # consensus — GHOSTDAG temeli (Adım 3a)
//!
//! Bu katman `dag::graph` üzerine BELİRLENİMCİ ata/erişilebilirlik (reachability)
//! sorguları kurar. GHOSTDAG-benzeri tip-selection (Adım 3b) ve finalite
//! (Adım 4) tamamen bu primitiflerin üzerinde çalışır. Graph saf yapısal
//! kalır; consensus mantığı buraya izole edilir.
//!
//! ## Tanımlar (kesin)
//! Bir DAG'da, `a → b` "a, b'nin ebeveyni" demektir (kenar çocuktan ebeveyne
//! `parents()` ile gezilir; ata yönü).
//! - **past(v)** — v'nin KESİN ataları: ebeveynler üzerinden geçişli olarak
//!   ulaşılan tüm vertex'ler. v dahil DEĞİL.
//! - **is_ancestor(a, b)** — `a ∈ past(b)`. (a, b'nin kesin atası mı?)
//! - **anticone(v) ∩ U** — U evreni içinde v ile ne ata ne torun olan
//!   (ve v olmayan) vertex'ler. GHOSTDAG'ın k-cluster mavi-küme hesabının özü.
//! - **topological_order** — her vertex kendi atalarından SONRA gelir;
//!   beraberlik `VertexId` ile bozulur → tüm düğümlerde AYNI sıra.
//!
//! ## Belirlenimcilik (KRİTİK)
//! Hiçbir fonksiyon `now`/saat/varış-sırası okumaz. Girdi yalnızca graph
//! topolojisi + `VertexId` sırasıdır. Aynı graph → her düğümde AYNI sonuç.
//! Bu, Adım 2'deki `validate_structural` belirlenimciliğinin doğal devamıdır.
//!
//! ## Equivocation (Adım 3 — denetçi yönü #1)
//! `equivocations_by(graph, pk)`: aynı yazardan birbirinin atası OLMAYAN
//! ("paralel") vertex çiftleri. GHOSTDAG bunları EKLEME anında engellemez;
//! sıralar ve equivocator'lar doğal olarak kırmızı/geç-sıralanır. Çakışma
//! (double-spend) semantiği Adım 4'e (execution/finality) bırakılır. Burada
//! yalnızca ucuz TESPİT sağlanır (graph'ın `by_author` indeksi + ata sorgusu).
//!
//! ## Karmaşıklık notu (DÜRÜSTLÜK)
//! `past`/`is_ancestor` BFS tabanlıdır → en kötü O(V+E). Doğru ama optimize
//! DEĞİL. Üretim ölçeğinde Kaspa-tarzı reachability (interval/ağaç indeksi ile
//! O(1) ata sorgusu) gerekir; bu Adım 3b/sonraki bir optimizasyon adımına
//! bilinçli olarak ERTELENMİŞTİR. Şu anki hedef: KANITLANABİLİR DOĞRULUK.

use std::collections::{BTreeSet, VecDeque};

use crate::dag::graph::Graph;
use crate::dag::vertex::VertexId;

pub mod finality;
pub mod ghostdag;

/// `v`'nin KESİN ataları (past kümesi). v dahil değil. Belirlenimci (BTreeSet).
/// Bilinmeyen id → boş küme (graph'ta olmayan vertex'in atası yoktur).
/// Sadece graph'ta GERÇEKTEN var olan ebeveynler gezilir (parent varlığı
/// Adım 2'de garanti; yine de savunmacı `get` ile yürürüz).
pub fn past(graph: &Graph, v: &VertexId) -> BTreeSet<VertexId> {
    let mut seen = BTreeSet::new();
    let mut queue: VecDeque<VertexId> = VecDeque::new();

    // v'nin kendisini DEĞİL, ebeveynlerini tohumla.
    if let Some(vx) = graph.get(v) {
        for p in vx.parents() {
            if seen.insert(*p) {
                queue.push_back(*p);
            }
        }
    }
    while let Some(cur) = queue.pop_front() {
        if let Some(vx) = graph.get(&cur) {
            for p in vx.parents() {
                if seen.insert(*p) {
                    queue.push_back(*p);
                }
            }
        }
    }
    seen
}

/// `a`, `b`'nin KESİN atası mı? (`a ∈ past(b)`)
/// b'nin ebeveynlerinden geriye doğru erken-çıkışlı BFS — tüm past'i
/// kurmadan a'yı bulunca durur. a == b → false (kesin ata; refleksif değil).
pub fn is_ancestor(graph: &Graph, a: &VertexId, b: &VertexId) -> bool {
    if a == b {
        return false;
    }
    let mut seen = BTreeSet::new();
    let mut queue: VecDeque<VertexId> = VecDeque::new();

    if let Some(vx) = graph.get(b) {
        for p in vx.parents() {
            if p == a {
                return true;
            }
            if seen.insert(*p) {
                queue.push_back(*p);
            }
        }
    }
    while let Some(cur) = queue.pop_front() {
        if let Some(vx) = graph.get(&cur) {
            for p in vx.parents() {
                if p == a {
                    return true;
                }
                if seen.insert(*p) {
                    queue.push_back(*p);
                }
            }
        }
    }
    false
}

/// `universe` içindeki, `v` ile ne ata ne torun olan (ve v olmayan)
/// vertex'ler = anticone(v) ∩ universe. Belirlenimci (BTreeSet, id-sıralı).
/// GHOSTDAG k-cluster mavi-küme hesabının çekirdeği (Adım 3b).
///
/// ÖNKOŞUL (D-a — denetçi): `universe ⊆ graph.ids()`. Graph'ta OLMAYAN bir id
/// universe'e girerse `is_ancestor` her iki yönde de false döner ve o id
/// yanlışlıkla anticone'a dahil edilir. 3b'de universe daima graph alt-kümesi;
/// yine de debug build'de yakalanır.
pub fn anticone_within(
    graph: &Graph,
    v: &VertexId,
    universe: &BTreeSet<VertexId>,
) -> BTreeSet<VertexId> {
    debug_assert!(
        universe.iter().all(|u| graph.contains(u)),
        "anticone_within önkoşulu: universe ⊆ graph.ids()"
    );
    let mut out = BTreeSet::new();
    for u in universe {
        if u == v {
            continue;
        }
        // u, v'nin anticone'undaysa: ne u→v ata ne v→u ata.
        if !is_ancestor(graph, u, v) && !is_ancestor(graph, v, u) {
            out.insert(*u);
        }
    }
    out
}

/// Tüm graph'ın BELİRLENİMCİ topolojik sırası: her vertex atalarından SONRA;
/// beraberlik `VertexId` ile bozulur (Kahn algoritması, min-id seçimi). Aynı
/// graph → her düğümde AYNI sıra. GHOSTDAG mergeset sıralamasının temeli.
///
/// Kahn: in-degree = vertex'in graph'ta MEVCUT ebeveyn sayısı. Hazır küme
/// `BTreeSet<VertexId>` → her adımda min-id alınır (belirlenimci tie-break).
pub fn topological_order(graph: &Graph) -> Vec<VertexId> {
    use std::collections::HashMap;

    let mut in_degree: HashMap<VertexId, usize> = HashMap::new();
    for id in graph.ids() {
        let deg = graph
            .get(id)
            .map(|vx| vx.parents().iter().filter(|p| graph.contains(p)).count())
            .unwrap_or(0);
        in_degree.insert(*id, deg);
    }

    // in-degree 0 olanlar (genesis ve mevcut-ebeveyni olmayanlar) — id-sıralı.
    let mut ready: BTreeSet<VertexId> = in_degree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(id, _)| *id)
        .collect();

    let mut order = Vec::with_capacity(in_degree.len());
    while let Some(&next) = ready.iter().next() {
        ready.remove(&next);
        order.push(next);
        // next'in çocuklarının in-degree'sini düş (children id-sıralı, Y3).
        for child in graph.children(&next) {
            if let Some(d) = in_degree.get_mut(child) {
                *d -= 1;
                if *d == 0 {
                    ready.insert(*child);
                }
            }
        }
    }
    // D-b (denetçi): asiklik Adım 2'de kriptografik olarak garanti; bir döngü
    // sızsaydı Kahn kısmi sıra döndürürdü. Bu sigorta ileride bir regresyonu
    // (örn. yanlış children indeksi) debug build'de anında yakalar.
    debug_assert_eq!(
        order.len(),
        graph.len(),
        "topological_order tüm vertex'leri kapsamalı (döngü/indeks regresyonu?)"
    );
    order
}

/// Artımlı topolojik sıra: `topological_order` ile AYNI Kahn algoritmasını,
/// AYNI id-sıralı belirlenimci sırayı kullanır; ANCAK `mevcut` kümesinde
/// (zaten hesaplanmış) olan vertex'leri çıktıya KOYMAZ. Sonuç, tam sıranın
/// "henüz hesaplanmamış" vertex'lerinden oluşan, AYNI göreli sıradaki alt
/// dizisidir → `update_with_weight` için `compute_with_weight` ile bit-bit
/// aynı sonucu garanti eder (sadece zaten-hesaplananları atlar, sıra bozulmaz).
/// `topological_order_eksik`'in HIZLI versiyonu: tum grafi taramak yerine
/// SADECE `mevcut`'ta olmayan vertex'lerin alt-grafinda Kahn calistirir.
/// Mevcut'taki vertex'ler "zaten islenmis" (hazir) kabul edilir; bir eksik
/// vertex, mevcut-olmayan TUM parent'lari isleninceye dek bekler. Cikti, mevcut
/// `topological_order_eksik` ile BIREBIR AYNI olmali (id-sirali ready -> ayni
/// belirlenimci sira). Maliyet: O(eksik + eksik-kenarlari), tum graf DEGIL.
pub fn topological_order_eksik_hizli(
    graph: &Graph,
    mevcut: &std::collections::BTreeSet<VertexId>,
) -> Vec<VertexId> {
    use std::collections::HashMap;
    // Eksik kumesi.
    let eksik: BTreeSet<VertexId> = graph
        .ids()
        .filter(|id| !mevcut.contains(*id))
        .copied()
        .collect();
    // Her eksik vertex'in, MEVCUT-OLMAYAN (yani yine eksik) parent sayisi.
    // Mevcut'taki parent'lar zaten islenmis -> bagimlilik degil.
    let mut in_degree: HashMap<VertexId, usize> = HashMap::new();
    for id in eksik.iter() {
        let deg = graph
            .get(id)
            .map(|vx| {
                vx.parents()
                    .iter()
                    .filter(|p| graph.contains(p) && eksik.contains(*p))
                    .count()
            })
            .unwrap_or(0);
        in_degree.insert(*id, deg);
    }
    // Hazir = eksik olup, eksik-parent'i kalmayanlar (id-sirali).
    let mut ready: BTreeSet<VertexId> = in_degree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut order = Vec::new();
    while let Some(&next) = ready.iter().next() {
        ready.remove(&next);
        order.push(next);
        for child in graph.children(&next) {
            // child eksikse ve bu next onun eksik-parent'iysa in-degree azalt.
            if let Some(d) = in_degree.get_mut(child) {
                *d -= 1;
                if *d == 0 {
                    ready.insert(*child);
                }
            }
        }
    }
    order
}

pub fn topological_order_eksik(
    graph: &Graph,
    mevcut: &std::collections::BTreeSet<VertexId>,
) -> Vec<VertexId> {
    use std::collections::HashMap;

    let mut in_degree: HashMap<VertexId, usize> = HashMap::new();
    for id in graph.ids() {
        let deg = graph
            .get(id)
            .map(|vx| vx.parents().iter().filter(|p| graph.contains(p)).count())
            .unwrap_or(0);
        in_degree.insert(*id, deg);
    }

    let mut ready: BTreeSet<VertexId> = in_degree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(id, _)| *id)
        .collect();

    let mut order = Vec::new();
    while let Some(&next) = ready.iter().next() {
        ready.remove(&next);
        // SADECE mevcut'ta OLMAYANI çıktıya ekle (sıra mantığı birebir korunur).
        if !mevcut.contains(&next) {
            order.push(next);
        }
        for child in graph.children(&next) {
            if let Some(d) = in_degree.get_mut(child) {
                *d -= 1;
                if *d == 0 {
                    ready.insert(*child);
                }
            }
        }
    }
    order
}

/// Bir yazarın (public key) EQUIVOCATION çiftleri: aynı yazardan, birbirinin
/// atası OLMAYAN ("paralel") vertex'ler (Adım 3 — denetçi yönü #1). Her çift
/// id-sıralı `(a, b)` (a < b) olarak döner; tüm çiftler de id-sıralı.
///
/// Dürüst zincir bir yazarın vertex'lerini tek bir ata zinciri yapar (her
/// yeni vertex öncekini ebeveyn gösterir) → equivocation YOK. Çatallayan
/// (forking) yazar paralel vertex üretir → burada yakalanır. GHOSTDAG bunu
/// ekleme anında engellemez; sıralama + state katmanı çözer.
pub fn equivocations_by(graph: &Graph, public_key: &[u8; 32]) -> Vec<(VertexId, VertexId)> {
    let mut pairs = Vec::new();
    let Some(set) = graph.author_vertices(public_key) else {
        return pairs;
    };
    let ids: Vec<VertexId> = set.iter().copied().collect();
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let (a, b) = (ids[i], ids[j]); // a < b (BTreeSet sıralı)
            if !is_ancestor(graph, &a, &b) && !is_ancestor(graph, &b, &a) {
                pairs.push((a, b));
            }
        }
    }
    pairs
}

/// Graph'ta HERHANGİ bir yazarın equivocation yapıp yapmadığı (ucuz ön-kontrol).
/// `true` → en az bir yazarın paralel vertex çifti var.
pub fn has_any_equivocation(graph: &Graph) -> bool {
    let mut authors: BTreeSet<[u8; 32]> = BTreeSet::new();
    for id in graph.ids() {
        if let Some(vx) = graph.get(id) {
            authors.insert(*vx.public_key());
        }
    }
    authors
        .iter()
        .any(|pk| !equivocations_by(graph, pk).is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::vertex::Vertex;
    use ed25519_dalek::SigningKey;

    const NET: u32 = 0xA1DA6;

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    /// Belirli bir anahtarla imzalı vertex üret (yazar = pk(seed)).
    fn signed(seed: u8, parents: Vec<VertexId>, ts: u64, payload: &[u8]) -> Vertex {
        // Seçenek A: vertex primitifi artık kanonik (strict artan) parent ister.
        // Gerçek üretici de parent'ları sıralı üretir; test helper'ı bunu modeller.
        let mut parents = parents;
        parents.sort_unstable();
        Vertex::new_signed(NET, parents, payload.to_vec(), ts, &key(seed)).unwrap()
    }

    /// Tek-yazarlı doğrusal zincir kuran devnet graph + id listesi döndür.
    /// g0 (genesis) → g1 → g2 ... her biri öncekini ebeveyn gösterir.
    fn linear_chain(n: usize, seed: u8) -> (Graph, Vec<VertexId>) {
        let mut g = Graph::devnet(NET);
        let mut ids = Vec::new();
        let gen = signed(seed, vec![], 1000, b"genesis");
        let mut last = *gen.id();
        g.insert_synced(gen).unwrap();
        ids.push(last);
        for i in 1..n {
            let v = signed(
                seed,
                vec![last],
                1000 + i as u64,
                format!("v{i}").as_bytes(),
            );
            last = *v.id();
            g.insert_synced(v).unwrap();
            ids.push(last);
        }
        (g, ids)
    }

    #[test]
    fn topo_eksik_hizli_eskiyle_birebir() {
        // Yeni hizli versiyon, eski topological_order_eksik ile AYNI cikti vermeli.
        // Cesitli "mevcut" alt-kumeleri ile dogrula.
        let (g, ids) = linear_chain(8, 1);
        // Farkli mevcut senaryolari: bos, ilk-yari, hepsi-bir-eksik, hepsi.
        let senaryolar: Vec<BTreeSet<VertexId>> = vec![
            BTreeSet::new(),                       // hicbiri islenmemis
            ids.iter().take(4).copied().collect(), // ilk 4 islenmis
            ids.iter().take(7).copied().collect(), // son 1 eksik
            ids.iter().copied().collect(),         // hepsi islenmis
            ids.iter().take(1).copied().collect(), // sadece genesis
        ];
        for mevcut in senaryolar {
            let eski = topological_order_eksik(&g, &mevcut);
            let yeni = topological_order_eksik_hizli(&g, &mevcut);
            assert_eq!(eski, yeni, "topo eksik farkli, mevcut.len={}", mevcut.len());
        }
    }

    // ===== past =====

    #[test]
    fn past_of_genesis_is_empty() {
        let (g, ids) = linear_chain(1, 1);
        assert!(past(&g, &ids[0]).is_empty());
    }

    #[test]
    fn past_accumulates_all_strict_ancestors() {
        let (g, ids) = linear_chain(4, 1); // g0→g1→g2→g3
        let p = past(&g, &ids[3]);
        assert_eq!(p.len(), 3);
        assert!(p.contains(&ids[0]) && p.contains(&ids[1]) && p.contains(&ids[2]));
        assert!(!p.contains(&ids[3])); // kendisi DAHİL DEĞİL (kesin ata)
    }

    #[test]
    fn past_of_unknown_id_is_empty() {
        let (g, _) = linear_chain(2, 1);
        assert!(past(&g, &[0xEE; 32]).is_empty());
    }

    #[test]
    fn past_merges_diamond() {
        // A → B, A → C, (B,C) → D. past(D) = {A,B,C}.
        let mut g = Graph::devnet(NET);
        let a = signed(1, vec![], 1000, b"a");
        let aid = *a.id();
        g.insert_synced(a).unwrap();
        let b = signed(1, vec![aid], 1001, b"b");
        let bid = *b.id();
        g.insert_synced(b).unwrap();
        let c = signed(2, vec![aid], 1001, b"c");
        let cid = *c.id();
        g.insert_synced(c).unwrap();
        let d = signed(1, vec![bid, cid], 1002, b"d");
        let did = *d.id();
        g.insert_synced(d).unwrap();

        let p = past(&g, &did);
        assert_eq!(p.len(), 3);
        assert!(p.contains(&aid) && p.contains(&bid) && p.contains(&cid));
    }

    // ===== is_ancestor =====

    #[test]
    fn is_ancestor_transitive_and_irreflexive() {
        let (g, ids) = linear_chain(3, 1); // g0→g1→g2
        assert!(is_ancestor(&g, &ids[0], &ids[2])); // geçişli
        assert!(is_ancestor(&g, &ids[0], &ids[1]));
        assert!(is_ancestor(&g, &ids[1], &ids[2]));
        assert!(!is_ancestor(&g, &ids[0], &ids[0])); // refleksif DEĞİL
        assert!(!is_ancestor(&g, &ids[2], &ids[0])); // ters yön DEĞİL
    }

    // ===== anticone =====

    #[test]
    fn anticone_finds_parallel_branches() {
        // A → B, A → C (B ve C paralel). universe={A,B,C}.
        // anticone(B) ∩ U = {C}; anticone(C) ∩ U = {B}; anticone(A) ∩ U = {}.
        let mut g = Graph::devnet(NET);
        let a = signed(1, vec![], 1000, b"a");
        let aid = *a.id();
        g.insert_synced(a).unwrap();
        let b = signed(1, vec![aid], 1001, b"b");
        let bid = *b.id();
        g.insert_synced(b).unwrap();
        let c = signed(2, vec![aid], 1001, b"c");
        let cid = *c.id();
        g.insert_synced(c).unwrap();

        let universe: BTreeSet<VertexId> = [aid, bid, cid].into_iter().collect();
        assert_eq!(
            anticone_within(&g, &bid, &universe),
            [cid].into_iter().collect()
        );
        assert_eq!(
            anticone_within(&g, &cid, &universe),
            [bid].into_iter().collect()
        );
        assert!(anticone_within(&g, &aid, &universe).is_empty());
    }

    // ===== topological_order =====

    #[test]
    fn topo_order_respects_ancestry() {
        let (g, ids) = linear_chain(4, 1);
        let order = topological_order(&g);
        assert_eq!(order.len(), 4);
        // her vertex atasından SONRA gelmeli.
        for (a, b) in [(0, 1), (1, 2), (2, 3)] {
            let pa = order.iter().position(|x| *x == ids[a]).unwrap();
            let pb = order.iter().position(|x| *x == ids[b]).unwrap();
            assert!(pa < pb, "ata {a} torundan {b} önce gelmeli");
        }
    }

    #[test]
    fn topo_order_is_deterministic_across_graphs() {
        // Aynı vertex kümesi farklı EKLEME sırasıyla iki graph'a koyulsa bile
        // topolojik sıra AYNI olmalı (id tie-break → varış-sırasından bağımsız).
        let mut g1 = Graph::devnet(NET);
        let mut g2 = Graph::devnet(NET);
        let a = signed(1, vec![], 1000, b"a");
        let aid = *a.id();
        let b = signed(1, vec![aid], 1001, b"b");
        let c = signed(2, vec![aid], 1001, b"c");
        let (bid, cid) = (*b.id(), *c.id());

        g1.insert_synced(a.clone()).unwrap();
        g1.insert_synced(b.clone()).unwrap();
        g1.insert_synced(c.clone()).unwrap();
        // g2: b ve c ters sırada eklenir.
        g2.insert_synced(a).unwrap();
        g2.insert_synced(c).unwrap();
        g2.insert_synced(b).unwrap();

        assert_eq!(topological_order(&g1), topological_order(&g2));
        // genesis her zaman ilk; b/c id-sıralı.
        let order = topological_order(&g1);
        assert_eq!(order[0], aid);
        let mut tail = [order[1], order[2]];
        tail.sort();
        assert_eq!(tail, {
            let mut t = [bid, cid];
            t.sort();
            t
        });
    }

    // ===== equivocation =====

    #[test]
    fn honest_linear_author_has_no_equivocation() {
        let (g, ids) = linear_chain(5, 7);
        // tek yazar (seed 7), doğrusal → equivocation YOK.
        let pk = g.get(&ids[0]).unwrap().public_key();
        assert!(equivocations_by(&g, pk).is_empty());
        assert!(!has_any_equivocation(&g));
    }

    #[test]
    fn forking_author_is_detected() {
        // Yazar seed=3: genesis g0, sonra g0'dan İKİ paralel çocuk (x, y).
        // x ve y birbirinin atası değil → equivocation çifti.
        let mut g = Graph::devnet(NET);
        let g0 = signed(3, vec![], 1000, b"g0");
        let g0id = *g0.id();
        g.insert_synced(g0).unwrap();
        let x = signed(3, vec![g0id], 1001, b"x");
        let xid = *x.id();
        g.insert_synced(x).unwrap();
        let y = signed(3, vec![g0id], 1001, b"y");
        let yid = *y.id();
        g.insert_synced(y).unwrap();

        let pk = g.get(&g0id).unwrap().public_key();
        let pairs = equivocations_by(&g, pk);
        // tek çift: (min(x,y), max(x,y)). g0 her ikisinin de atası → çift değil.
        let mut expect = [xid, yid];
        expect.sort();
        assert_eq!(pairs, vec![(expect[0], expect[1])]);
        assert!(has_any_equivocation(&g));
    }

    #[test]
    fn distinct_authors_not_confused() {
        // İki FARKLI yazar paralel vertex üretirse bu equivocation DEĞİL
        // (her biri kendi indeksinde tek vertex).
        let mut g = Graph::devnet(NET);
        let a = signed(1, vec![], 1000, b"a");
        let aid = *a.id();
        g.insert_synced(a).unwrap();
        let b = signed(1, vec![aid], 1001, b"b"); // yazar 1
        let c = signed(2, vec![aid], 1001, b"c"); // yazar 2
        g.insert_synced(b).unwrap();
        g.insert_synced(c).unwrap();

        let pk1 = key(1).verifying_key().to_bytes();
        let pk2 = key(2).verifying_key().to_bytes();
        assert!(equivocations_by(&g, &pk1).is_empty()); // yazar1: g0+b doğrusal
        assert!(equivocations_by(&g, &pk2).is_empty()); // yazar2: tek vertex
        assert!(!has_any_equivocation(&g));
    }
}
