//! Adım 4 — FINALITY (kesinlik) + reorg sınırı + pruning çıpası + slashing.
//!
//! Denetçinin (cross-AI) onayladığı ilk finality kuralı, weight() soyutlaması
//! (`blue_work`) üzerine kurulur. **Strict no-fabrication** — her karar gerçek
//! GHOSTDAG durumundan (blue_work, seçili-ebeveyn zinciri, gerçek anticone /
//! equivocation) türetilir; hiçbir kesinlik/derinlik uydurulmaz.
//!
//! Kurallar (denetçi):
//! 1. FINALITY_DEPTH (F) **weighted blue-work birimindedir** (blok sayısı
//!    değil) — O-sys: 0-ağırlık saldırgan blokları derinlik üretemez.
//! 2. **Final blok**: seçili-zincirde `blue_work(tip) − blue_work(B) ≥ F` olan
//!    en derin (tip'e en yakın) B. Bunun geçmişi (`past(B) ∪ {B}`) finaldir.
//! 3. **Reorg reddi**: final öneki içermeyen rakip tip reddedilir — yani final
//!    blok, aday tip'in atası (veya kendisi) olmalıdır.
//! 4. **Pruning çıpası**: final bloktan eski durum güvenle budanabilir.
//! 5. **Slashing/dışlama**: equivocation yapan yazarlar (gerçek paralel vertex
//!    çifti) tespit edilir → PoA komiteden çıkarma / PoS stake slashing girdisi.
//!
//! Çerçeve: **AI önerir → DAO/multisig onaylar ve uygular.** Bu modül yalnızca
//! kesinlik/reorg-sınırı hesaplar; hiçbir işlemi otomatik yürütmez.

use std::collections::BTreeSet;

use super::ghostdag::Ghostdag;
use super::{equivocations_by, is_ancestor};
use crate::dag::graph::Graph;
use crate::dag::vertex::VertexId;

/// Finality derinliği birimi (weighted blue-work). `u64`.
pub type FinalityDepth = u64;

/// Varsayılan finality derinliği (weighted blue-work). Denetçi: F ≥ 2 ile
/// 1-birim sınır beraberliği kapanır (gömülü blok artık reorg edilemez).
/// Geliştirme/test için küçük; mainnet'te güvenlik analizine göre büyütülür.
pub const DEFAULT_FINALITY_DEPTH: FinalityDepth = 2;

/// Varsayılan PRUNING derinliği — **finality derinliğinden AYRI ve DAHA BÜYÜK**
/// (denetçi A4: pruning ≥ finality + merge-gecikme marjı). Final bloğun ALTINI
/// değil, çok daha eski bir çıpanın altını budar; böylece final bloğun altında
/// dallanıp henüz birleşmemiş dürüst-ama-geç tip'ler öksüz kalmaz. **ASLA
/// finality derinliğini pruning için kullanmayın.** Mainnet'te ağ merge-gecikme
/// dağılımına göre büyütülür.
pub const DEFAULT_PRUNING_DEPTH: FinalityDepth = 6;

/// `target`, `tip`'in SEÇİLİ-EBEVEYN OMURGASINDA mı? (yalnızca ata değil —
/// denetçi SAFETY bulgusu.) Tip'ten genesis'e seçili-ebeveyn zinciri yürünür;
/// `target` bu zincirde görünürse `true`. `is_ancestor`'dan KATI ölçüde
/// güçlüdür: omurga-dışı bir yoldan ulaşılan ata `false` döner.
fn on_spine(gd: &Ghostdag, tip: &VertexId, target: &VertexId) -> bool {
    let mut cur = Some(*tip);
    while let Some(c) = cur {
        if c == *target {
            return true;
        }
        cur = gd.selected_parent(&c);
    }
    false
}

/// Seçili-zincirin FINAL bloğu: `blue_work(tip) − blue_work(B) ≥ depth` olan
/// en derin (tip'e en yakın) seçili-zincir bloğu B. Zincir henüz yeterince
/// derin değilse `None` (hiçbir şey finalize edilmemiş — erken zincir).
///
/// Tip'ten genesis'e doğru yürünür; `blue_work` azaldıkça fark büyür, bu yüzden
/// eşiği sağlayan İLK blok (tip'e en yakın) en derin final bloktur.
pub fn final_block(gd: &Ghostdag, graph: &Graph, depth: FinalityDepth) -> Option<VertexId> {
    let tip = gd.selected_tip(graph)?;
    let tip_work = gd.blue_work(&tip)?;
    let mut cur = Some(tip);
    while let Some(c) = cur {
        let cw = gd.blue_work(&c)?;
        // u64 taşmasından kaçın: çıkarma yerine karşılaştırma.
        if tip_work >= cw && tip_work - cw >= depth {
            return Some(c);
        }
        cur = gd.selected_parent(&c);
    }
    None
}

/// Bir blok FİNAL mi? Final blok mevcutsa ve `id` onun geçmişindeyse (veya
/// kendisiyse) `true`. Henüz finalize yoksa `false`.
pub fn is_final(gd: &Ghostdag, graph: &Graph, depth: FinalityDepth, id: &VertexId) -> bool {
    match final_block(gd, graph, depth) {
        None => false,
        Some(f) => *id == f || is_ancestor(graph, id, &f),
    }
}

/// Aday bir tip, final öneki UZATIYOR mu? (reorg sınırı — denetçi kural 3 +
/// SAFETY bulgusu). Henüz finalize yoksa her tip kabul (`true`). Aksi hâlde
/// final blok, aday tip'in SEÇİLİ-EBEVEYN OMURGASINDA olmalıdır — yalnızca
/// ata olması YETMEZ (denetçi: `is_ancestor` fazla zayıf; omurga-dışı bir
/// yoldan final bloğu içeren rakip tip finalize edilmiş bloğu geri alabilir).
/// Omurgadan geçmeyen tip geçersiz reorg'dur → `false` (reddet).
///
/// NOT: Bu STATELESS kontrol her çağrıda mevcut GHOSTDAG'tan yeniden hesaplar.
/// Ağ kabul-yolu için **yapışkan/monoton** garanti gereklidir →
/// [`FinalityState`] kullanın (denetçi: finality geri alınamaz olmalı).
pub fn extends_final(
    gd: &Ghostdag,
    graph: &Graph,
    depth: FinalityDepth,
    candidate_tip: &VertexId,
) -> bool {
    match final_block(gd, graph, depth) {
        None => true,
        Some(f) => on_spine(gd, candidate_tip, &f),
    }
}

/// Pruning çıpası: bu bloğun geçmişinden eski durum güvenle budanabilir.
/// `depth` **PRUNING derinliği olmalı** ([`DEFAULT_PRUNING_DEPTH`]), finality
/// derinliği DEĞİL (denetçi A4). Finality derinliği verilirse final bloğun
/// hemen altı budanır ve geç tip'ler öksüz kalabilir. Bu fonksiyon `final_block`
/// ile aynı "en derin eşik bloğu" mantığını kullanır; ayrım yalnızca verilen
/// (daha büyük) derinliktedir → çıpa final bloktan daha eski/gömülü çıkar.
/// Henüz yeterince derin değilse `None`.
pub fn pruning_anchor(gd: &Ghostdag, graph: &Graph, depth: FinalityDepth) -> Option<VertexId> {
    final_block(gd, graph, depth)
}

/// **Yapışkan / monoton finality durumu** (denetçi SAFETY bulgusu). Stateless
/// `final_block` her çağrıda omurga kayınca geriye gidebilir; bu yapı son
/// finalize bloğu KALICI tutar ve yalnızca İLERİ taşır. Ağ kabul-yolu bunu
/// kullanmalıdır → finalize edilmiş blok asla geri alınamaz (sert garanti).
#[derive(Debug, Clone)]
pub struct FinalityState {
    depth: FinalityDepth,
    finalized: Option<VertexId>,
}

impl FinalityState {
    /// Verilen finality derinliğiyle boş durum (henüz finalize yok).
    pub fn new(depth: FinalityDepth) -> Self {
        Self {
            depth,
            finalized: None,
        }
    }

    /// Kullanılan finality derinliği.
    pub fn depth(&self) -> FinalityDepth {
        self.depth
    }

    /// Şu ana kadar finalize edilmiş (yapışkan) blok.
    pub fn finalized(&self) -> Option<VertexId> {
        self.finalized
    }

    /// Mevcut GHOSTDAG'tan finality'yi MONOTON ilerlet. Aday = `final_block`.
    /// - İlk finalizasyon: adayı kabul et.
    /// - Sonraki: aday yalnızca önceki finalize bloğun OMURGASINDAN descend
    ///   ediyorsa (önceki blok adayın seçili-ebeveyn zincirinde) ileri taşınır;
    ///   aksi hâlde önceki finalize blok KORUNUR (geri/yan gitme yok — sert
    ///   garanti). Yeni finalize bloğu döndürür.
    pub fn advance(&mut self, gd: &Ghostdag, graph: &Graph) -> Option<VertexId> {
        let candidate = final_block(gd, graph, self.depth)?;
        match self.finalized {
            None => self.finalized = Some(candidate),
            Some(prev) => {
                // Monoton: aday, önceki finalize bloğun omurga-torunu olmalı.
                if candidate != prev && on_spine(gd, &candidate, &prev) {
                    self.finalized = Some(candidate);
                }
                // Aksi hâlde prev korunur (omurga kayması finality'yi geri alamaz).
            }
        }
        self.finalized
    }

    /// Ağ kabul-yolu guard'ı: aday tip yalnızca SEÇİLİ-EBEVEYN OMURGASI yapışkan
    /// finalize bloktan geçiyorsa kabul edilir. Finalizasyon öncesi her tip kabul.
    /// Bu, bir peer'dan gelen finality-ihlali reorg'unu reddeder.
    pub fn accepts_tip(&self, gd: &Ghostdag, tip: &VertexId) -> bool {
        match self.finalized {
            None => true,
            Some(f) => on_spine(gd, tip, &f),
        }
    }

    /// **Finality-kısıtlı tip seçimi** (denetçi Adım 5 entegrasyon maddesi a).
    /// Saf `Ghostdag::selected_tip` finality'yi UMURSAMAZ (yalnızca max
    /// blue_work) → teorik olarak finality-ihlali bir tip baş olarak seçilebilir.
    /// Bu fonksiyon yalnızca [`accepts_tip`](Self::accepts_tip) onaylı (omurgası
    /// yapışkan finalize bloktan geçen) tip'ler arasından max blue_work (tie
    /// min-id) seçer → finality-ihlali tip ASLA baş olmaz. Onaylı tip yoksa
    /// `None` (kritik durum — bkz. [`has_finality_conflict`](Self::has_finality_conflict)).
    pub fn selected_tip_final(&self, gd: &Ghostdag, graph: &Graph) -> Option<VertexId> {
        let mut best: Option<(u64, VertexId)> = None;
        for t in graph.tips() {
            if !self.accepts_tip(gd, &t) {
                continue;
            }
            let work = gd.blue_work(&t).unwrap_or(0);
            match best {
                None => best = Some((work, t)),
                Some((bwork, bid)) => {
                    if work > bwork || (work == bwork && t < bid) {
                        best = Some((work, t));
                    }
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// **Finality ÇATIŞMA alarmı** (denetçi Adım 5 entegrasyon maddesi b).
    /// Yapışkan finalize blok varken, ağın EN AĞIR zinciri (saf `selected_tip`)
    /// bu bloğu omurgasında BARINDIRMIYORSA `true`. Bu, `advance`'ın sessizce
    /// "prev'i koru" ile geçiştirdiği iki durumdan KRİTİK olanıdır: en-ağır
    /// zincir finalize bloğu TERK ETMİŞ (eclipse / partition / saldırı). `advance`
    /// güvenle geri-alma YAPMAZ; ama düğüm bu sinyalde DURMALI / alarm vermeli —
    /// gerçek karar (devam/rollback/manuel müdahale) DAO/multisig'dedir (AI
    /// önerir, imzalamaz). Finalizasyon öncesi veya tip yoksa `false`.
    pub fn has_finality_conflict(&self, gd: &Ghostdag, graph: &Graph) -> bool {
        match self.finalized {
            None => false,
            Some(f) => match gd.selected_tip(graph) {
                None => false,
                Some(tip) => !on_spine(gd, &tip, &f),
            },
        }
    }
}

/// Equivocation yapan TÜM yazarların pubkey'leri (gerçek paralel vertex çifti
/// olanlar). PoA komiteden çıkarma / PoS stake slashing için aday listesi
/// (denetçi kural 5). **Strict no-fabrication** — yalnızca graph'ta fiilen
/// gözlemlenen paralel (anticone) çiftleri sayılır.
///
/// Semantik uyarı (denetçi B7): "paralel vertex" otomatik "kötü niyet" DEĞİL —
/// dürüst bir düğüm çökme/restart, HA-failover veya anahtarın iki cihazda olması
/// nedeniyle kazara paralel vertex üretebilir. Bu yüzden çıktı yalnızca ÖNERİ
/// girdisidir: gerçek slashing/çıkarma kararı DAO/multisig onayıyla uygulanır
/// (AI imzalamaz) — çerçevenin doğal koruması.
pub fn equivocators(graph: &Graph) -> BTreeSet<[u8; 32]> {
    let mut authors: BTreeSet<[u8; 32]> = BTreeSet::new();
    for id in graph.ids() {
        if let Some(vx) = graph.get(id) {
            authors.insert(*vx.public_key());
        }
    }
    authors
        .into_iter()
        .filter(|pk| !equivocations_by(graph, pk).is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::ghostdag::{CommitteeWeight, Ghostdag, DEFAULT_K};
    use crate::dag::vertex::Vertex;
    use ed25519_dalek::SigningKey;

    const NET: u32 = 0xA1DA6;

    fn signed(seed: u8, parents: Vec<VertexId>, ts: u64, payload: &[u8]) -> Vertex {
        let sk = SigningKey::from_bytes(&[seed; 32]);
        Vertex::new_signed(NET, parents, payload.to_vec(), ts, &sk).unwrap()
    }

    /// Tek üretici (seed 1), doğrusal zincir uzunluğu `n` (genesis dahil).
    fn linear_chain(n: usize) -> (Graph, Vec<VertexId>) {
        let mut g = Graph::devnet(NET);
        let mut ids = Vec::new();
        let gen = signed(1, vec![], 1000, b"g");
        let mut prev = *gen.id();
        ids.push(prev);
        g.insert_synced(gen).unwrap();
        for i in 1..n {
            let v = signed(1, vec![prev], 1000 + i as u64, format!("b{i}").as_bytes());
            prev = *v.id();
            ids.push(prev);
            g.insert_synced(v).unwrap();
        }
        (g, ids)
    }

    #[test]
    fn short_chain_has_no_final_block() {
        // Zincir derinliği < F → finalize yok.
        let (g, _ids) = linear_chain(2); // genesis + 1 blok, blue_work tip = 1
        let gd = Ghostdag::compute_default(&g);
        assert_eq!(final_block(&gd, &g, DEFAULT_FINALITY_DEPTH), None);
    }

    #[test]
    fn deep_chain_finalizes_prefix() {
        // F = 2, blue_work tip = 5 (genesis=0..tip). Final blok =
        // blue_work(tip)−blue_work(B) ≥ 2 olan en derin (tip'e en yakın) B.
        // tip work=5: B work 3 (gap 2) tip'e en yakın eşik → final = work-3 blok.
        let (g, ids) = linear_chain(6); // work: 0,1,2,3,4,5
        let gd = Ghostdag::compute_default(&g);
        let f = final_block(&gd, &g, 2).expect("finalize olmalı");
        // tip = ids[5] (work 5); eşik gap≥2 → work≤3 ilk = work 3 = ids[3].
        assert_eq!(f, ids[3]);
        // ids[0..=3] final; ids[4], ids[5] henüz değil.
        for fin in &ids[0..=3] {
            assert!(is_final(&gd, &g, 2, fin), "{fin:?} final olmalı");
        }
        assert!(!is_final(&gd, &g, 2, &ids[4]));
        assert!(!is_final(&gd, &g, 2, &ids[5]));
    }

    #[test]
    fn pruning_anchor_equals_final_block() {
        let (g, _ids) = linear_chain(6);
        let gd = Ghostdag::compute_default(&g);
        assert_eq!(
            pruning_anchor(&gd, &g, 2),
            final_block(&gd, &g, 2),
            "pruning çıpası = final blok"
        );
    }

    #[test]
    fn competing_tip_not_extending_final_is_rejected() {
        // Final blok F belirlendikten sonra, F'yi DIŞLAYAN rakip tip reddedilir.
        // Komite ağırlığıyla: dürüst zincir final üretir; saldırgan F-öncesinden
        // çatallı 0-ağırlık tip → final öneki uzatmaz → extends_final = false.
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"g");
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();

        // dürüst komite zinciri h1..h4 (her biri work +1).
        let mut prev = gid;
        let mut honest = Vec::new();
        for i in 1..=4u64 {
            let v = signed(1, vec![prev], 1000 + i, format!("h{i}").as_bytes());
            prev = *v.id();
            honest.push(prev);
            g.insert_synced(v).unwrap();
        }

        // saldırgan (komite-dışı seed 9) genesis'ten çatallı tek blok (work 0).
        let atk = signed(9, vec![gid], 1001, b"atk");
        let atkid = *atk.id();
        g.insert_synced(atk).unwrap();

        let committee = CommitteeWeight {
            members: [*g.get(&gid).unwrap().public_key()].into_iter().collect(),
        };
        let gd = Ghostdag::compute_with_weight(&g, DEFAULT_K, &committee);

        let f = final_block(&gd, &g, 2).expect("finalize olmalı");
        // dürüst tip final öneki uzatır:
        let tip = gd.selected_tip(&g).unwrap();
        assert!(extends_final(&gd, &g, 2, &tip));
        // saldırgan tip final bloğu içermez (final genesis sonrası komite bloğu):
        assert_ne!(atkid, f);
        assert!(!is_ancestor(&g, &f, &atkid));
        assert!(
            !extends_final(&gd, &g, 2, &atkid),
            "final öneki uzatmayan rakip tip reddedilmeli"
        );
    }

    #[test]
    fn no_finality_accepts_any_tip() {
        // Henüz finalize yoksa her tip kabul (extends_final = true).
        let (g, ids) = linear_chain(2);
        let gd = Ghostdag::compute_default(&g);
        assert_eq!(final_block(&gd, &g, DEFAULT_FINALITY_DEPTH), None);
        assert!(extends_final(&gd, &g, DEFAULT_FINALITY_DEPTH, &ids[1]));
    }

    #[test]
    fn spine_membership_is_stricter_than_ancestry() {
        // Diamond: g → b1, g → b2, m = merge(b1,b2). m'in seçili-ebeveyni biri;
        // DİĞERİ m'in ATASI ama OMURGASINDA DEĞİL → extends_final omurga
        // kullanmalı, sadece is_ancestor değil (denetçi SAFETY bulgusu).
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"g");
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();
        let b1 = signed(2, vec![gid], 1001, b"b1");
        let b1id = *b1.id();
        g.insert_synced(b1).unwrap();
        let b2 = signed(3, vec![gid], 1001, b"b2");
        let b2id = *b2.id();
        g.insert_synced(b2).unwrap();
        let m = signed(4, vec![b1id, b2id], 1002, b"m");
        let mid = *m.id();
        g.insert_synced(m).unwrap();

        let gd = Ghostdag::compute_default(&g);
        let sp = gd.selected_parent(&mid).expect("m'in seçili ebeveyni");
        let other = if sp == b1id { b2id } else { b1id };

        // other m'in ATASI (is_ancestor true) ...
        assert!(is_ancestor(&g, &other, &mid));
        // ... ama m'in OMURGASINDA DEĞİL (on_spine false) → katı ölçüde güçlü.
        assert!(!on_spine(&gd, &mid, &other));
        // seçili ebeveyn ve genesis omurgada:
        assert!(on_spine(&gd, &mid, &sp));
        assert!(on_spine(&gd, &mid, &gid));
    }

    #[test]
    fn finality_state_is_monotone_and_idempotent() {
        let (g, ids) = linear_chain(6); // work 0..5
        let gd = Ghostdag::compute_default(&g);
        let mut fs = FinalityState::new(2);
        assert_eq!(fs.finalized(), None);
        let f1 = fs.advance(&gd, &g);
        assert_eq!(f1, Some(ids[3])); // stateless final_block ile aynı
                                      // Aynı graph'la tekrar advance → ileri/geri gitmez (idempotent).
        let f2 = fs.advance(&gd, &g);
        assert_eq!(f2, Some(ids[3]));
        assert_eq!(fs.finalized(), Some(ids[3]));
    }

    #[test]
    fn finality_state_advances_forward_as_chain_grows() {
        // Kısa zincirde finalize yok; büyüdükçe ileri taşınır (monoton).
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"g");
        let mut prev = *gen.id();
        let mut ids = vec![prev];
        g.insert_synced(gen).unwrap();
        let mut fs = FinalityState::new(2);
        assert_eq!(fs.advance(&Ghostdag::compute_default(&g), &g), None);

        for i in 1..6u64 {
            let v = signed(1, vec![prev], 1000 + i, format!("b{i}").as_bytes());
            prev = *v.id();
            ids.push(prev);
            g.insert_synced(v).unwrap();
        }
        let gd = Ghostdag::compute_default(&g);
        let f = fs.advance(&gd, &g).expect("finalize olmalı");
        assert_eq!(f, ids[3]);
        // önceki finalize bloğun omurga-torunu (monoton ileri).
        assert!(on_spine(&gd, &ids[5], &f));
    }

    #[test]
    fn accepts_tip_rejects_off_spine_tip_after_finalization() {
        // Komite zinciri finalize olur; saldırgan (komite-dışı) genesis'ten
        // çatallı tip yapışkan finality omurgasından geçmez → reddedilir.
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"g");
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();
        let mut prev = gid;
        for i in 1..=4u64 {
            let v = signed(1, vec![prev], 1000 + i, format!("h{i}").as_bytes());
            prev = *v.id();
            g.insert_synced(v).unwrap();
        }
        let atk = signed(9, vec![gid], 1001, b"atk");
        let atkid = *atk.id();
        g.insert_synced(atk).unwrap();

        let committee = CommitteeWeight {
            members: [*g.get(&gid).unwrap().public_key()].into_iter().collect(),
        };
        let gd = Ghostdag::compute_with_weight(&g, DEFAULT_K, &committee);
        let mut fs = FinalityState::new(2);
        let f = fs.advance(&gd, &g).expect("finalize olmalı");

        // dürüst tip kabul; saldırgan tip ret.
        let tip = gd.selected_tip(&g).unwrap();
        assert!(fs.accepts_tip(&gd, &tip));
        assert!(!fs.accepts_tip(&gd, &atkid), "off-spine reorg reddedilmeli");
        assert_ne!(atkid, f);
    }

    #[test]
    fn finality_constrained_tip_selection_and_conflict_alarm() {
        // UniformWeight: önce A zinciri finalize edilir; sonra DAHA AĞIR paralel
        // B zinciri (genesis'ten çatallı) en-ağır olur ve finalize bloğu
        // omurgasında BARINDIRMAZ → (1) has_finality_conflict alarmı, (2)
        // selected_tip_final B'yi reddedip onaylı A tip'ini seçer, (3) advance
        // monoton — geri-alma YOK (eclipse/partition senaryosu).
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"g");
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();

        // A zinciri (seed 1): a1..a5.
        let mut prev = gid;
        let mut a = Vec::new();
        for i in 1..=5u64 {
            let v = signed(1, vec![prev], 1000 + i, format!("a{i}").as_bytes());
            prev = *v.id();
            a.push(prev);
            g.insert_synced(v).unwrap();
        }
        let mut fs = FinalityState::new(2);
        let gd_a = Ghostdag::compute_default(&g);
        let fa = fs.advance(&gd_a, &g).expect("A finalize olmalı");
        assert!(!fs.has_finality_conflict(&gd_a, &g), "henüz çatışma yok");
        // sadece A varken finality-kısıtlı tip = A tip'i (a5).
        assert_eq!(fs.selected_tip_final(&gd_a, &g), Some(*a.last().unwrap()));

        // B zinciri (seed 2): b1..b7 — daha uzun/ağır, genesis'ten çatallı.
        let mut prevb = gid;
        for i in 1..=7u64 {
            let v = signed(2, vec![prevb], 1000 + i, format!("b{i}").as_bytes());
            prevb = *v.id();
            g.insert_synced(v).unwrap();
        }
        let gd_b = Ghostdag::compute_default(&g);
        // saf selected_tip artık B (daha ağır) → finalize blok omurgada değil.
        assert!(
            fs.has_finality_conflict(&gd_b, &g),
            "en-ağır zincir finalize bloğu terk etti → alarm"
        );
        // monoton: advance geri ALMAZ, fa korunur.
        assert_eq!(fs.advance(&gd_b, &g), Some(fa));
        // finality-kısıtlı seçim: ağır B tip'i REDDEDİLİR; onaylı A tip'i seçilir.
        assert_eq!(
            fs.selected_tip_final(&gd_b, &g),
            Some(*a.last().unwrap()),
            "finality-ihlali ağır tip baş olmamalı"
        );
    }

    #[test]
    fn pruning_depth_is_separate_from_finality() {
        // pruning derinliği finality'den BÜYÜK → pruning çıpası finality
        // bloğundan daha eski/gömülü (denetçi A4: geç tip'leri öksüz bırakma).
        let (g, _ids) = linear_chain(8); // work 0..7
        let gd = Ghostdag::compute_default(&g);
        let fin = final_block(&gd, &g, DEFAULT_FINALITY_DEPTH).expect("final");
        let prune = pruning_anchor(&gd, &g, DEFAULT_PRUNING_DEPTH).expect("prune");
        // pruning çıpası finality bloğunun atası (daha eski) ve farklı.
        assert!(
            is_ancestor(&g, &prune, &fin),
            "pruning çıpası daha eski olmalı"
        );
        assert_ne!(prune, fin);
    }

    #[test]
    fn honest_chain_has_no_equivocators() {
        let (g, _ids) = linear_chain(5);
        assert!(equivocators(&g).is_empty());
    }

    #[test]
    fn equivocator_is_detected() {
        // Aynı yazar (seed 7) genesis'ten İKİ paralel blok → equivocation.
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"g");
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();

        let e1 = signed(7, vec![gid], 1001, b"e1");
        let e2 = signed(7, vec![gid], 1001, b"e2"); // paralel (farklı payload)
        g.insert_synced(e1).unwrap();
        g.insert_synced(e2).unwrap();

        let eqs = equivocators(&g);
        let sk7 = SigningKey::from_bytes(&[7u8; 32]);
        let pk7 = sk7.verifying_key().to_bytes();
        assert!(
            eqs.contains(&pk7),
            "seed 7 equivocator olarak tespit edilmeli"
        );
        // dürüst seed 1 (genesis) listede olmamalı.
        let sk1 = SigningKey::from_bytes(&[1u8; 32]);
        let pk1 = sk1.verifying_key().to_bytes();
        assert!(!eqs.contains(&pk1));
    }
}
