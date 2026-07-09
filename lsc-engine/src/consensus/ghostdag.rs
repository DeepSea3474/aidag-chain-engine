//! # ghostdag — GHOSTDAG k-cluster renklendirme + blue-score + toplam sıralama
//! (Adım 3b)
//!
//! PHANTOM/GHOSTDAG (Sompolinsky-Zohar, Kaspa) algoritmasının BELİRLENİMCİ,
//! DOĞRULUK-ÖNCELİKLİ bir uygulaması. Tamamen `consensus::{past, is_ancestor,
//! anticone_within, topological_order}` (Adım 3a) primitifleri üzerine kuruludur.
//!
//! ## Sezgi
//! Dürüst düğümler yeni vertex'i mevcut tüm tip'lere bağlar → dürüst alt-graf
//! sıkı bağlı bir "küme" oluşturur. Bir saldırganın gizli/çatallı vertex'leri
//! bu kümenin DIŞINDA kalır (anticone'ları büyük). GHOSTDAG her vertex için en
//! büyük **k-cluster**'ı (mavi küme) seçer: mavi kümedeki her bloğun, küme
//! içindeki anticone'u ≤ k. Mavi-küme dışı bloklar **kırmızı**. `blue_score` =
//! geçmişteki mavi blok sayısı → en yüksek blue-score'lu tip "seçili tip"tir ve
//! toplam sıra onun seçili-ebeveyn zinciri boyunca üretilir.
//!
//! ## Belirlenimcilik (KRİTİK — consensus)
//! Hiçbir adım `now`/saat/varış-sırası okumaz. Tüm beraberlikler `VertexId`
//! (içerik-adresli hash) ile bozulur. Aynı graph → her düğümde AYNI renk, AYNI
//! blue-score, AYNI toplam sıra. (Adım 2 `validate_structural` ve Adım 3a
//! belirlenimciliğinin doğal devamı.)
//!
//! ## k parametresi
//! `k`, eşzamanlı dürüst blokların beklenen anticone büyüklüğünü tolere eder
//! (ağ gecikmesi × blok hızı). Büyük k → daha çok eşzamanlılık toleransı ama
//! saldırgana daha geniş pencere. Mainnet k'sı LSC ağ parametreleriyle
//! kalibre edilecek (Adım 5/6). `DEFAULT_K` yalnızca bir başlangıç değeridir.
//!
//! ## DÜRÜSTLÜK — karmaşıklık
//! Bu bir DOĞRULUK PROTOTİPİDİR (denetçi O1/Soru 5). Renklendirme her aday için
//! `anticone_within`'i çağırır; `anticone_within` ata sorgularına dayanır ve
//! Adım 3a'daki ata sorgusu BFS O(V+E)'dir → toplam en kötü süper-kuadratik.
//! Performansa GÜVENİLMEZ; reachability-index optimizasyonu (Kaspa-tarzı
//! interval ağacı, O(1) ata) mainnet öncesi ZORUNLU bir güvenlik (liveness-DoS)
//! maddesi olarak izlenir. Şu anki hedef: kanıtlanabilir DOĞRULUK.
//!
//! ## AÇIK GÜVENLİK SORUSU — blok-üretim ağırlığı (denetçi O-sys, Adım 4)
//! GHOSTDAG güvenliği "dürüst ÇOĞUNLUĞUN blok-üretim ağırlığı" varsayımına
//! dayanır. Şu an blok üretmenin bir maliyeti YOK (PoW/PoS/stake yok); bu
//! yüzden `blue_score` serbest bir SAYIMDIR ve bir saldırgan gizli DOĞRUSAL bir
//! mavi zincir (her blok öncekini ebeveyn gösterir → küçük anticone, hepsi
//! mavi) kurup blue-score'u serbestçe şişirebilir. k-cluster yalnızca PARALEL
//! sybil bloklarını kırmızıya iter, doğrusal sahte zinciri değil. Bu nedenle
//! Adım 4 (finality) yazılmadan ÖNCE bir AĞIRLIK METRİĞİ (`blue_work` / stake-
//! ağırlıklı score) kararı verilmeli; bu, selected_parent/tip seçimini de
//! değiştirir ve Adım 1 vertex'ine work/stake ipucu eklemeyi gerektirebilir.
//! Bu bir 3b hatası DEĞİL; tüm tasarımın en büyük açık güvenlik sorusudur.
//!
//! ## Equivocation (denetçi O3)
//! GHOSTDAG equivocation'ı birinci-sınıf kavram olarak ele almaz; çatallayan
//! (paralel) vertex'ler doğal olarak büyük anticone'a sahip olur ve k-cluster
//! kuralıyla KIRMIZIYA itilir. `consensus::equivocations_by` yalnızca TELEMETRİ
//! sağlar; cezalandırma/slashing kararı Adım 4'e (execution/finality) aittir.

use std::collections::{BTreeMap, BTreeSet};

use crate::dag::graph::Graph;
use crate::dag::vertex::VertexId;

use super::{past, topological_order, topological_order_eksik_hizli};

/// Anticone büyüklüğü tipi. u16 — mainnet k'sı bunun çok altında olacak.
pub type KType = u16;

/// Başlangıç k değeri (yalnızca varsayılan; mainnet kalibre edilecek).
pub const DEFAULT_K: KType = 18;

/// Bir vertex'in BLOK-ÜRETİM AĞIRLIĞINI veren BELİRLENİMCİ fonksiyon (denetçi
/// O-sys). GHOSTDAG güvenliği "dürüst çoğunluğun üretim ağırlığı" varsayımına
/// dayanır; seçim ve finality ham SAYIM (`blue_score`) yerine AĞIRLIK
/// (`blue_work`) üzerine kurulur ki saldırgan gizli doğrusal mavi zincirle
/// blue-score şişiremesin. Metrik PoA→PoS göçünde DEĞİŞMEZ; yalnızca bu
/// fonksiyon değişir (denetçi önerisi: 3 → 2, weight() soyutlaması).
///
/// SÖZLEŞME: aynı `(graph, id)` daima aynı değeri vermeli (now/saat yok),
/// yoksa consensus belirlenimciliği bozulur.
pub trait Weigher {
    fn weight(&self, graph: &Graph, id: &VertexId) -> u64;

    /// Bu weigher'ın BELİRLENİMCİ kimliği. Artımlı GHOSTDAG önbelleği bu
    /// fingerprint'e bağlanır → aynı önbelleğin FARKLI bir weigher ile
    /// karıştırılması (ör. `UniformWeight` sonra `CommitteeWeight`, ya da farklı
    /// komite üyeliği) eski vertex'leri eski ağırlıkta bırakıp yeni vertex'leri
    /// yeni ağırlıkta hesaplardı → tam-compute eşitliğini ve belirlenimciliği
    /// bozardı. [`Ghostdag::update_with_weight`] uyuşmazlıkta DURUR (panic).
    /// SÖZLEŞME: ağırlık fonksiyonunu belirleyen TÜM durumu kapsamalıdır.
    fn fingerprint(&self) -> u64;
}

/// Bir weigher fingerprint'ini domain-ayrılmış blake3 ile türet (çakışmaya karşı
/// tag + tip durumu). İlk 8 bayt → u64 (belirlenimci, platform-bağımsız LE).
fn weigher_fingerprint(tag: &[u8], state: &[u8]) -> u64 {
    let mut h = blake3::Hasher::new();
    h.update(b"LSC-WEIGHER-FP-v1:");
    h.update(tag);
    h.update(b":");
    h.update(state);
    let out = h.finalize();
    let mut b = [0u8; 8];
    b.copy_from_slice(&out.as_bytes()[..8]);
    u64::from_le_bytes(b)
}

/// Başlangıç metriği: her vertex ağırlık 1. Bu durumda `blue_work == blue_score`
/// (tam geriye-uyum). Henüz üretici kümesi/stake tanımlı değilken varsayılan.
#[derive(Debug, Clone, Copy, Default)]
pub struct UniformWeight;

impl Weigher for UniformWeight {
    fn weight(&self, _graph: &Graph, _id: &VertexId) -> u64 {
        1
    }

    fn fingerprint(&self) -> u64 {
        weigher_fingerprint(b"uniform", &[])
    }
}

/// PoA başlangıç metriği (denetçi Seçenek 3): üreticisi yetkili komitede olan
/// vertex ağırlık 1, değilse 0. Sybil dirençlidir — komite dışı bir saldırgan
/// kaç blok üretirse üretsin ağırlığı 0'dır, gizli doğrusal zincirle blue-work
/// şişiremez. Komite başlangıçta DAO/multisig imzacı kümesi olabilir; ileride
/// `weight = stake` (PoS) ile değiştirilir, finality mantığı aynı kalır.
#[derive(Debug, Clone)]
pub struct CommitteeWeight {
    /// Yetkili üretici public key'leri.
    pub members: BTreeSet<[u8; 32]>,
}

impl Weigher for CommitteeWeight {
    fn weight(&self, graph: &Graph, id: &VertexId) -> u64 {
        match graph.get(id) {
            Some(vx) if self.members.contains(vx.public_key()) => 1,
            _ => 0,
        }
    }

    fn fingerprint(&self) -> u64 {
        // Üye kümesi BTreeSet → iterasyon sıralı/belirlenimci. Farklı üyelik =
        // farklı fingerprint → karıştırma yakalanır.
        let mut state = Vec::with_capacity(self.members.len() * 32);
        for m in &self.members {
            state.extend_from_slice(m);
        }
        weigher_fingerprint(b"committee", &state)
    }
}

/// Bir vertex'in GHOSTDAG renklendirme verisi. `mergeset_blues`/`reds`
/// topolojik sıralıdır ve seçili ebeveyni İÇERMEZ (o ayrı tutulur).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhostdagData {
    /// Geçmişteki (kesin past) mavi blok SAYISI. Genesis = 0. Telemetri/uyum.
    pub blue_score: u64,
    /// Geçmişteki (kesin past) mavi blokların AĞIRLIK toplamı (blue-work).
    /// Genesis = 0. Seçim ve finality bunu kullanır (denetçi O-sys).
    /// `UniformWeight` ile `blue_work == blue_score`.
    pub blue_work: u64,
    /// En yüksek blue-WORK'lü ebeveyn (tie-break min-id). Genesis = None.
    pub selected_parent: Option<VertexId>,
    /// Mergeset'in MAVİ blokları (seçili ebeveyn hariç), topolojik sıralı.
    pub mergeset_blues: Vec<VertexId>,
    /// Mergeset'in KIRMIZI blokları, topolojik sıralı.
    pub mergeset_reds: Vec<VertexId>,
}

/// Bir graph üzerinde hesaplanmış GHOSTDAG durumu (vertex → GhostdagData).
#[derive(Debug, Clone)]
pub struct Ghostdag {
    k: KType,
    data: BTreeMap<VertexId, GhostdagData>,
    /// Önbelleği üreten weigher'ın fingerprint'i — artımlı tutarlılık guard'ı.
    /// Henüz hiç vertex işlenmediyse `None`; ilk hesapta sabitlenir. Farklı
    /// weigher ile sonraki `update` DURUR (belirlenimcilik koruması).
    weigher_fp: Option<u64>,
    /// KALICI sp-agac interval (inkremental). Atalik sorgusu hizlandirma araci;
    /// GhostdagData'yi ETKILEMEZ (diske yazilmaz, konsensus hash'ine girmez),
    /// bu yuzden iki yolun (statik/inkremental) farkli DEGER uretmesi sorun
    /// degil — her yol kendi icinde tutarli atalik verir. update_with_weight
    /// bunu inkremental gunceller; bosluk dolunca sp_tree_intervals_gapped ile
    /// bir kez bastan kurulur (nadir).
    iv: BTreeMap<VertexId, (u64, u64)>,
    /// Her vertex'in cocuklarina dagitacagi boslukta SONRAKI bos baslangic.
    /// v'ye interval atandiginda iv_next[v] = v'nin boslugunun basi olarak
    /// baslar; her yeni cocuk bir dilim alip bu ilerler. Inkremental, O(1).
    iv_next: BTreeMap<VertexId, u64>,
    /// KALICI torba (covering set): her vertex'in sp-disi atalarina acilan
    /// tabelalar (miras + sikistirma). iv gibi: GhostdagData'yi ETKILEMEZ,
    /// sadece paralel-DAG atalik sorgusunu hizlandirir. update'te inkremental
    /// kurulur (sp-atasinin torbasi + kendi koprüleri, sonra sikistir).
    torba: BTreeMap<VertexId, BTreeSet<(u64, VertexId)>>,
    /// KALICI anticone_sizes (Kaspa blues_anticone_sizes, ama YAN-VERI). Her vertex
    /// icin, o vertex'in mergeset_blues'undaki her mavinin anticone boyutu (o vertex'in
    /// gorusunden). iv/torba gibi: GhostdagData'yi/konsensus hash'ini ETKILEMEZ.
    /// Bir mavinin guncel boyutu, sp-zincirinde geriye aranarak bulunur (Kaspa
    /// blue_anticone_size). Renklendirme baslangic dongusunu (10390 cagri) elemek icin.
    anticone_sizes: BTreeMap<VertexId, BTreeMap<VertexId, u32>>,
    /// BINARY LIFTING (yan-veri): her vertex icin sp-zincirinde 2^j atalar.
    /// up[v][0] = v'nin sp'si, up[v][j] = up[ up[v][j-1] ][j-1]. cand'in atasini
    /// sp-zincirinde O(log n) sicramayla bulmak icin. iv/torba gibi GhostdagData'ya
    /// DOKUNMAZ. Henuz PASIF (doldurulmuyor, kullanilmiyor).
    up: BTreeMap<VertexId, Vec<VertexId>>,
    /// KALICI sp-agac cocuk listesi: her vertex -> selected-parent'i o vertex
    /// olan cocuklar (id-sirali). Lokal rebuild (alt-agac yeniden numaralama)
    /// icin gerekli; iv/torba gibi GhostdagData'yi/konsensusu ETKILEMEZ.
    /// Henuz PASIF (dolduruluyor ama lokal rebuild devrede degil).
    children_sp: BTreeMap<VertexId, BTreeSet<VertexId>>,
    boyut_map: BTreeMap<VertexId, BTreeMap<VertexId, u32>>,
}

impl Ghostdag {
    /// `UniformWeight` (her vertex ağırlık 1) ile renklendir → `blue_work ==
    /// blue_score`. Geriye-uyumlu varsayılan giriş noktası.
    pub fn compute(graph: &Graph, k: KType) -> Self {
        Ghostdag::compute_with_weight(graph, k, &UniformWeight)
    }

    /// Verilen k ve AĞIRLIK metriği ile tüm graph'ı renklendir (denetçi O-sys).
    /// Vertex'ler topolojik sırada işlenir → her vertex işlenirken TÜM
    /// ebeveynlerinin verisi hazırdır (Adım 2 parent-varlığı garantisi + topo
    /// sıra). Tamamen belirlenimci. Renklendirme (k-cluster) YAPISALDIR ve
    /// ağırlıktan bağımsızdır; ağırlık yalnızca SEÇİM (selected_parent/tip) ve
    /// blue_work'ü etkiler — finality bunun üzerine kurulacak (Adım 4).
    pub fn compute_with_weight<W: Weigher>(graph: &Graph, k: KType, weigher: &W) -> Self {
        let mut data: BTreeMap<VertexId, GhostdagData> = BTreeMap::new();
        for id in topological_order(graph) {
            // Topo sıra → tüm ebeveynlerin verisi hazır. Her vertex BİR KEZ.
            let iv = sp_tree_intervals(&data);
            let (d, _out) = compute_vertex_data(graph, &id, k, weigher, &data, &iv, None, None);
            data.insert(id, d);
        }
        Ghostdag {
            k,
            data,
            weigher_fp: Some(weigher.fingerprint()),
            iv: BTreeMap::new(),
            iv_next: BTreeMap::new(),
            torba: BTreeMap::new(),
            anticone_sizes: BTreeMap::new(),
            up: BTreeMap::new(),
            children_sp: BTreeMap::new(),
            boyut_map: BTreeMap::new(),
        }
    }

    /// Boş artımlı GHOSTDAG durumu (denetçi: artımlı motor). Vertex verileri
    /// [`Ghostdag::update_with_weight`] / [`Ghostdag::update`] ile eklenir.
    pub fn new_incremental(k: KType) -> Self {
        Ghostdag {
            k,
            data: BTreeMap::new(),
            weigher_fp: None,
            iv: BTreeMap::new(),
            iv_next: BTreeMap::new(),
            torba: BTreeMap::new(),
            anticone_sizes: BTreeMap::new(),
            up: BTreeMap::new(),
            children_sp: BTreeMap::new(),
            boyut_map: BTreeMap::new(),
        }
    }

    /// **Artımlı GHOSTDAG güncellemesi** (denetçi Adım 5 ön koşulu). Graph'ta
    /// olup henüz HESAPLANMAMIŞ vertex'leri topolojik sırada işler; ZATEN
    /// hesaplanmışları ATLAR (önbellek değişmezliği — blok-başına `GhostdagData`
    /// yalnızca içerik-adresli/sabit past'ine bağlıdır, DAG append-only olduğundan
    /// asla değişmez). Böylece her blok ÖMÜR BOYU yalnızca bir kez işlenir;
    /// `compute_with_weight`'in tam yeniden-hesabından kaçınılır.
    ///
    /// Topolojik sıra her çağrıda graph'tan yeniden türetildiği için, herhangi
    /// bir ekleme sırasıyla eklenmiş bir vertex yığını TEK çağrıda doğru işlenir
    /// (ebeveynler her zaman çocuklardan önce gelir → ayrı orphan kuyruğuna gerek
    /// yok; eksik-ebeveyn vertex'i Adım 2 graph değişmezi zaten engeller).
    ///
    /// Differential garanti: sonuç, aynı graph üzerinde `compute_with_weight` ile
    /// **bit-bit aynıdır** (testlerle doğrulanır).
    /// Yeni `v`'ye sp-agac interval'i INKREMENTAL ata (bosluktan dilim).
    /// Donus: true = atandi; false = sp'nin boslugu doldu (cagiran bastan
    /// kurmali). Determinizm gerekmez (interval degerleri konsensusu etkilemez),
    /// ama ayni node icinde tutarli. O(1).
    /// LOKAL REBUILD: `kok`'un sp-alt-agacini, verilen genis `(yeni_s, yeni_e)`
    /// araligina yeniden numaralandirir (tum DAG'i DEGIL). children_sp (kalici)
    /// ile alt-agaci gezer; sp_tree_intervals_gapped ile AYNI bolme mantigi
    /// (bosluk cocuklara esit bolunur) -> ayni kapsama kurali (sa<=sb && eb<=ea)
    /// hem ic hem dis korunur. iv ve iv_next guncellenir. Henuz PASIF (cagrilmiyor).
    #[allow(dead_code)]
    fn subtree_reindex(&mut self, kok: VertexId, yeni_s: u64, yeni_e: u64) {
        let mut stack: Vec<(VertexId, u64, u64)> = vec![(kok, yeni_s, yeni_e)];
        while let Some((v, s, e)) = stack.pop() {
            self.iv.insert(v, (s, e));
            self.iv_next.insert(v, s); // v'nin cocuk boslugu bastan.
            if let Some(kids) = self.children_sp.get(&v).cloned() {
                let k = kids.len() as u64;
                if k > 0 && e > s {
                    let span = (e - 1 - s) / k;
                    let mut cs = s;
                    for c in kids.iter() {
                        let ce = if span == 0 { cs } else { cs + span - 1 };
                        let ce = ce.min(e - 1);
                        stack.push((*c, cs, ce));
                        cs = ce + 1;
                    }
                    // sp'nin sonraki bos noktasi: son cocugun bittigi yer+1.
                    self.iv_next.insert(v, cs.min(e));
                }
            }
        }
    }

    /// Bosluk dolunca cagrilir. ONCE lokal: sp'nin alt-agacini sp'nin KENDI
    /// araligi icinde EShIT dagitimla yeniden paketler (kalan/2 israfini toplar
    /// -> bosluk acilir, sp babasinin icinde kalir -> kapsama/atalik korunur).
    /// Sonra id'yi tekrar dener. Lokal yetmezse TAM rebuild'e duser (guvenlik agi).
    /// Donus: true = id yerlesti, false = tam rebuild gerekti (cagiran yapar).
    fn lokal_rebuild_dene(&mut self, id: &VertexId, sp: Option<VertexId>) -> bool {
        if let Some(sp_id) = sp {
            if let Some(&(s_sp, e_sp)) = self.iv.get(&sp_id) {
                // sp'nin alt-agacini kendi araligina esit yeniden paketle.
                self.subtree_reindex(sp_id, s_sp, e_sp);
                // id'yi tekrar dene (artik sp'de yer acilmis olabilir).
                return self.assign_interval_incremental(id, sp);
            }
        }
        false // sp yok/araligi yok -> tam rebuild gerek.
    }

    fn assign_interval_incremental(&mut self, v: &VertexId, sp: Option<VertexId>) -> bool {
        const BIG: u64 = 1 << 60;
        match sp {
            None => {
                // Genesis/kok: yeni bir BIG blogu ver. Mevcut koklerden sonra.
                // base = simdiye dek atanmis en buyuk end + 1 (kok ayrimi).
                let base = self
                    .iv
                    .values()
                    .map(|&(_, e)| e)
                    .max()
                    .map(|m| m + 1)
                    .unwrap_or(0);
                self.iv.insert(*v, (base, base + BIG));
                self.iv_next.insert(*v, base); // bosluk basi.
                true
            }
            Some(sp) => {
                // KALICI children_sp: v'yi sp'nin cocuk listesine ekle. Lokal
                // rebuild'in alt-agaci bulmasi icin gerekli. rebuild (false) yolu
                // dahil HER durumda guncel olmali -> en basta, iv kontrolunden once.
                self.children_sp.entry(sp).or_default().insert(*v);
                let (_s_sp, e_sp) = match self.iv.get(&sp) {
                    Some(&iv) => iv,
                    None => return false, // sp'nin interval'i yok -> bastan kur.
                };
                let start = *self.iv_next.get(&sp).unwrap_or(&_s_sp);
                let limit = e_sp.saturating_sub(1); // sp'nin noktasi e_sp haric.
                if start >= limit {
                    return false; // bosluk bitti -> bastan kurma gerek.
                }
                // v'ye KUCUK SABIT oda ver (lineer zincir = en sik desen).
                // "yariya bolme" derin zincirde ~60 vertex sonra tukeniyordu
                // (2^60 -> her adim yarilanir -> 60 adim -> rebuild). Olculdu:
                // 10k vertex'te 166 rebuild, tam 60'ar arayla -> O(n^2).
                // Cozum: her cocuga sabit ODA (2^20). 2^60/2^20 = 2^40 vertex
                // rebuild'siz dayanir. Bir vertex'in cok cocugu olsa bile sp'nin
                // 2^60 boslugu 2^40 cocuga yeter. Kalan az ise yariya dus (idare).
                // GENIS ARAZI (lokal rebuild guvenlik agiyla): cocuga comert pay
                // (kalan-1). Lineer zincirde daralma durur (her nesil 1 azalir).
                // Kardes gelir de cakisirsa -> assign false doner -> lokal_rebuild
                // sp alt-agacini esit yeniden paketler (cakisma duzelir, atalik
                // korunur). Hiz (genis) + dogruluk (lokal rebuild) birlikte.
                let kalan = limit - start;
                let pay = kalan.saturating_sub(1).max(1);
                let end = start + pay;
                self.iv.insert(*v, (start, end));
                self.iv_next.insert(*v, start); // v'nin kendi boslugu start'tan.
                self.iv_next.insert(sp, end + 1); // sp'nin sonraki bos noktasi.
                true
            }
        }
    }

    pub fn update_with_weight<W: Weigher>(&mut self, graph: &Graph, weigher: &W) {
        // Önbellek-weigher tutarlılık guard'ı: aynı önbelleğin farklı bir weigher
        // ile karıştırılması (eski vertex'ler eski ağırlıkta kalır) belirlenimciliği
        // ve tam-compute eşitliğini bozardı → DURUR (sessiz yanlış sonuç yok).
        let fp = weigher.fingerprint();
        match self.weigher_fp {
            None => self.weigher_fp = Some(fp),
            Some(existing) => assert_eq!(
                existing, fp,
                "artımlı GHOSTDAG önbelleği farklı weigher ile karıştırıldı — \
                 belirlenimcilik ihlali (aynı örnekte tek weigher kullanın)"
            ),
        }
        // ARTIMLI: zaten hesaplanmış vertex'leri sıralamaya HİÇ koyma (eski
        // kod tüm grafı sıralayıp sonra atlıyordu → O(n) her çağrı → O(n^2)).
        // topological_order_eksik, AYNI belirlenimci sırayı korur; yalnızca
        // `data`'da olmayanları döndürür → sonuç compute_with_weight ile bit-bit
        // aynı (sıra değişmez, sadece zaten-hesaplanan israfı kalkar).
        let mevcut: std::collections::BTreeSet<VertexId> = self.data.keys().copied().collect();
        for id in topological_order_eksik_hizli(graph, &mevcut) {
            let (d, out) = compute_vertex_data(
                graph,
                &id,
                self.k,
                weigher,
                &self.data,
                &self.iv,
                Some(&self.torba),
                Some(&self.anticone_sizes),
            );
            let sp = d.selected_parent;
            self.data.insert(id, d);
            self.anticone_sizes.insert(id, out);
            // INKREMENTAL interval ata. Bosluk dolarsa (false) -> tum iv'yi
            // bosluklu sema ile bir kez bastan kur (NADIR; amortize ucuz).
            if !self.assign_interval_incremental(&id, sp) {
                // ONCE lokal rebuild dene (sp alt-agacini esit yeniden paketle).
                // Yetmezse TAM rebuild (guvenlik agi). Lokal cogu zaman yeter ->
                // tum-DAG rebuild NADIRen calisir -> O(n^2) kirilir.
                if !self.lokal_rebuild_dene(&id, sp) {
                    self.iv = sp_tree_intervals_gapped(&self.data);
                    self.iv_next = self.iv.iter().map(|(k, &(s, _))| (*k, s)).collect();
                }
            }
            // INKREMENTAL TORBA: v'nin torbasi = sp-atasinin torbasi (miras) +
            // v'nin sp-olmayan parent'lari (kopruleri), sonra sikistir. iv hazir
            // olmali (sikistirma interval kullanir) -> interval atamasindan SONRA.
            self.torba_guncelle_tek(graph, &id, sp);
            // [HIZ] up PASIF (uretimde okunmuyor, sadece test) - kaldirildi: self.up_guncelle_tek(&id, sp);
        }
    }

    /// ARTIMLI TEK-VERTEX: update_with_weight dongu govdesinin aynisi, ama
    /// topological_order_eksik_hizli (O(n) tarama) YERINE dogrudan `yeni`.
    /// Ic islem BIREBIR ayni -> sonuc tam-tarama ile bit-bit ozdes.
    /// Onkosul: `yeni` data'da degil, parent'lari data'da (cascade garanti eder).
    pub fn update_one_with_weight<W: Weigher>(
        &mut self,
        graph: &Graph,
        yeni: &VertexId,
        weigher: &W,
    ) {
        let fp = weigher.fingerprint();
        match self.weigher_fp {
            None => self.weigher_fp = Some(fp),
            Some(existing) => assert_eq!(existing, fp, "weigher karisti"),
        }
        if self.data.contains_key(yeni) {
            return;
        }
        let id = *yeni;
        let (d, out) = compute_vertex_data(
            graph,
            &id,
            self.k,
            weigher,
            &self.data,
            &self.iv,
            Some(&self.torba),
            Some(&self.anticone_sizes),
        );
        let sp = d.selected_parent;
        self.data.insert(id, d);
        let _ta = std::time::Instant::now();
        self.anticone_sizes.insert(id, out);
        U_ANTI.fetch_add(_ta.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
        let _ti = std::time::Instant::now();
        if !self.assign_interval_incremental(&id, sp) && !self.lokal_rebuild_dene(&id, sp) {
            self.iv = sp_tree_intervals_gapped(&self.data);
            self.iv_next = self.iv.iter().map(|(k, &(s, _))| (*k, s)).collect();
        }
        U_IV.fetch_add(_ti.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
        let _tt = std::time::Instant::now();
        self.torba_guncelle_tek(graph, &id, sp);
        U_TORBA.fetch_add(_tt.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
        let _tu = std::time::Instant::now();
        // [HIZ] up PASIF - kaldirildi: self.up_guncelle_tek(&id, sp);
        U_UP.fetch_add(_tu.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    }

    /// Tek vertex icin inkremental torba: sp-atasinin torbasini devral + kendi
    /// sp-olmayan parent'larini ekle + sikistir. torba_hesapla'nin tek-vertex hali.
    /// BINARY LIFTING doldur: v icin sp-zincirinde 2^j atalar.
    /// up[v][0] = sp; up[v][j] = up[ up[v][j-1] ][j-1] (mevcut oldugu surece).
    /// Inkremental: v islendiginde cagrilir, atalari zaten dolu (topo sira).
    fn up_guncelle_tek(&mut self, v: &VertexId, sp: Option<VertexId>) {
        let sp = match sp {
            Some(s) => s,
            None => {
                // sp yok (genesis): bos tablo.
                self.up.insert(*v, Vec::new());
                return;
            }
        };
        let mut tablo: Vec<VertexId> = Vec::new();
        tablo.push(sp); // up[v][0] = sp
                        // up[v][j] = up[ tablo[j-1] ][j-1]
        let mut j = 1;
        loop {
            let onceki = tablo[j - 1];
            // onceki'nin 2^(j-1) atasi var mi?
            match self.up.get(&onceki).and_then(|t| t.get(j - 1)).copied() {
                Some(ata) => {
                    tablo.push(ata);
                    j += 1;
                }
                None => break, // daha fazla atlanamiyor (koke ulasildi)
            }
        }
        self.up.insert(*v, tablo);
    }

    fn torba_guncelle_tek(&mut self, graph: &Graph, v: &VertexId, sp: Option<VertexId>) {
        // Ic mantik VertexId ile (birebir eski); SADECE depolama (start,id) cifti.
        let mut bag: BTreeSet<VertexId> = BTreeSet::new();
        // 1. sp-atasinin torbasini devral. (torba artik (start,id) tutar -> id'leri al.)
        if let Some(spv) = sp {
            if let Some(sp_bag) = self.torba.get(&spv) {
                bag.extend(sp_bag.iter().map(|&(_, id)| id));
            }
        }
        // 2. kendi sp-olmayan parent'larini (kopruleri) ekle.
        if let Some(vx) = graph.get(v) {
            for pp in vx.parents() {
                if !graph.contains(pp) {
                    continue;
                }
                if Some(*pp) != sp {
                    bag.insert(*pp);
                }
            }
        }
        // 3. sikistir: bir tabela baska birinin sp-atasi ise at (iv ile).
        // O(t log t) hizli sikistirma (sikistir_referans ile bit-bit ozdes, test edildi).
        let elemanlar: Vec<VertexId> = bag.iter().copied().collect();
        let atilacak = sikistir_hizli(&elemanlar, &self.iv);
        for a in atilacak {
            bag.remove(&a);
        }
        TORBA_BOY.fetch_add(bag.len() as u64, std::sync::atomic::Ordering::Relaxed);
        TORBA_SAY.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // (start, id) cifti olarak sakla: interval-start sirali -> sorgu binary search.
        // start'i olmayan (iv'de yok) tabela olmamali; guvenlik icin 0 ata (sorgu yine id ile bulur).
        let bag_cift: BTreeSet<(u64, VertexId)> = bag
            .iter()
            .map(|&id| (self.iv.get(&id).map(|&(s, _)| s).unwrap_or(0), id))
            .collect();
        self.torba.insert(*v, bag_cift);
    }

    /// `UniformWeight` ile artımlı güncelleme (geriye-uyumlu varsayılan).
    pub fn update(&mut self, graph: &Graph) {
        self.update_with_weight(graph, &UniformWeight);
    }

    /// ARTIMLI HIZLI YOL: yalnizca `yeni` vertex'i isle (tum graf taramasi YOK).
    /// node ingest'te eklenen vertex'i bilir -> O(n) graf taramasi yerine O(1) hedef.
    /// `yeni`'nin TUM parent'lari zaten `data`'da olmali (cascade tek-tek garanti eder).
    /// Compute-yolu (None) icin eski tam-tarama korunur; bu yalnizca update hizli yolu.
    pub fn update_one(&mut self, graph: &Graph, yeni: &VertexId) {
        self.update_one_with_weight(graph, yeni, &UniformWeight);
    }

    /// Varsayılan k ile hesapla.
    pub fn compute_default(graph: &Graph) -> Self {
        Ghostdag::compute(graph, DEFAULT_K)
    }

    /// Kullanılan k.
    pub fn k(&self) -> KType {
        self.k
    }

    /// TEST: binary lifting up tablosuna erisim.
    #[cfg(test)]
    fn up_ref(&self) -> &BTreeMap<VertexId, Vec<VertexId>> {
        &self.up
    }

    /// Bir vertex'in GHOSTDAG verisi.
    pub fn data(&self, id: &VertexId) -> Option<&GhostdagData> {
        self.data.get(id)
    }

    /// Bir vertex'in blue-score'u (mavi blok SAYISI).
    pub fn blue_score(&self, id: &VertexId) -> Option<u64> {
        self.data.get(id).map(|d| d.blue_score)
    }

    /// Bir vertex'in blue-WORK'ü (mavi blok AĞIRLIK toplamı). Seçim/finality
    /// bunu kullanır (denetçi O-sys).
    pub fn blue_work(&self, id: &VertexId) -> Option<u64> {
        self.data.get(id).map(|d| d.blue_work)
    }

    /// Bir vertex'in seçili ebeveyni.
    pub fn selected_parent(&self, id: &VertexId) -> Option<VertexId> {
        self.data.get(id).and_then(|d| d.selected_parent)
    }

    /// Bir vertex MAVİ mi? (verisi yoksa None.) Bir blok, SEÇİLİ TİP'in
    /// görüşünde mavi sayılıyorsa true. NOT: renk gözlemciye (hangi tip'ten
    /// bakıldığına) görelidir; burada seçili-tip görüşü esas alınır.
    pub fn is_blue_in_selected_view(&self, graph: &Graph, id: &VertexId) -> Option<bool> {
        let tip = self.selected_tip(graph)?;
        self.data.get(id)?; // id'nin verisi olmalı
        let blue = blue_set_in_view(&self.data, tip);
        Some(blue.contains(id))
    }

    /// Seçili tip: en yüksek blue-WORK'lü tip, beraberlik min-id (denetçi
    /// O-sys). Tüm düğümler aynı tip kümesinden aynı seçimi yapar (belirlenimci).
    pub fn selected_tip(&self, graph: &Graph) -> Option<VertexId> {
        let mut best: Option<(u64, VertexId)> = None;
        for t in graph.tips() {
            let work = self.blue_work(&t).unwrap_or(0);
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

    /// Seçili tip'in geçmişinin (past(tip) ∪ {tip}) BELİRLENİMCİ TOPLAM SIRASI.
    /// Seçili-ebeveyn zinciri genesis→tip boyunca yürünür; her zincir bloğunun
    /// mergeset'i (mavi+kırmızı, topolojik sıralı) bloktan ÖNCE dizilir, sonra
    /// blok. Her blok tam bir kez görünür (her blok ya zincirde ya da tam bir
    /// zincir bloğunun mergeset'indedir — GHOSTDAG değişmezi).
    ///
    /// NOT: Seçili tip'in geçmişinde OLMAYAN diğer tip'ler bu sıraya girmez
    /// (gelecekte bir blok onları birleştirince sıralanır) — GHOSTDAG'da
    /// beklenen davranış.
    pub fn total_order(&self, graph: &Graph) -> Vec<VertexId> {
        let Some(tip) = self.selected_tip(graph) else {
            return Vec::new();
        };

        // Seçili-ebeveyn zinciri tip→genesis, sonra ters çevir (genesis ilk).
        let mut chain: Vec<VertexId> = Vec::new();
        let mut cur = Some(tip);
        while let Some(c) = cur {
            chain.push(c);
            cur = self.data.get(&c).and_then(|d| d.selected_parent);
        }
        chain.reverse();

        let mut order: Vec<VertexId> = Vec::new();
        for c in chain {
            if let Some(d) = self.data.get(&c) {
                // mergeset'i MAVİ-ÖNCELİKLİ ama TOPOLOJİYİ KORUYAN sırada diz
                // (YB1 — denetçi).
                order.extend(order_mergeset_blue_first(
                    graph,
                    &d.mergeset_blues,
                    &d.mergeset_reds,
                ));
            }
            order.push(c);
        }
        order
    }
}

/// Bir zincir bloğunun mergeset'ini MAVİ-ÖNCELİKLİ ama TOPOLOJİYİ KORUYAN
/// belirlenimci sırada diz (YB1 — denetçi). Anahtar = (rank, is_red, id) ve
/// `rank = |past(x) ∩ mergeset|`.
///
/// NEDEN TOPOLOJİYİ BOZMAZ: Adım 3a'da kanıtlandığı gibi x, y'nin atası ve
/// ikisi de alt-kümedeyse `rank(x) < rank(y)` (KESİN) → ata daima önce.
/// Dolayısıyla AYNI rank ⟹ aralarında ata ilişkisi YOK (karşılıklı anticone)
/// ⟹ onları renge göre yeniden sıralamak topolojiyi bozamaz. Aynı rank'ta önce
/// maviler (is_red=false), sonra kırmızılar; her grupta id ile beraberlik
/// bozulur. Böylece saldırgan bir kırmızıyı id-grind ile dürüst bir mavinin
/// ÖNÜNE sokamaz (naif "tüm maviler → tüm kırmızılar" ise kırmızı-ata/mavi-torun
/// durumunda topolojiyi bozardı; bu yaklaşım bozmaz).
fn order_mergeset_blue_first(
    graph: &Graph,
    blues: &[VertexId],
    reds: &[VertexId],
) -> Vec<VertexId> {
    let blue_set: BTreeSet<VertexId> = blues.iter().copied().collect();
    let all: BTreeSet<VertexId> = blues.iter().chain(reds.iter()).copied().collect();
    let mut v: Vec<VertexId> = all.iter().copied().collect();
    v.sort_by_cached_key(|x| {
        let rank = past(graph, x).iter().filter(|a| all.contains(*a)).count();
        let is_red = !blue_set.contains(x);
        (rank, is_red, *x)
    });
    v
}

/// Tek bir vertex'in `GhostdagData`'sını hesapla — TÜM ebeveynlerinin verisi
/// `data`'da hazır olmalı (çağıran topo sıra ile garantiler). `data` salt-okunur;
/// bu fonksiyon hiçbir mevcut girdiyi mutasyona uğratmaz → blok-başına veri
/// değişmezliği (denetçi artımlı GHOSTDAG değişmezi #1/#3). Hem tam
/// `compute_with_weight` hem artımlı `update_with_weight` bunu kullanır → tek
/// kaynak, differential eşitlik inşa gereği gelir.
/// Selected-parent AĞACINDA interval (DFS pre/post-order) etiketleri hesaplar.
/// Her vertex, `data[v].selected_parent` ile tek bir ebeveyne bağlanır → bu bir
/// AĞAÇ (orman: genesis kökü). `children_sp`: sp-ağacı çocuk listesi (id-sıralı).
/// Dönüş: vertex → (start, end). Kural: A, B'nin sp-ağaç atası  ⟺
/// `start[A] <= start[B] && end[B] <= end[A]` (O(1) ata kontrolü).
/// SADECE sp-ağacı; diğer parent'lar için ata kontrolü AYRI (hibrit) ele alınır.
/// Her vertex icin KOPRU listesi = sp-olmayan parent'lar (agac-disi kenarlar).
/// D'nin parent'lari {B, C} ve sp'si B ise, kopru = [C]. Interval sp-agacini
/// (B->D) gorur ama C->D'yi gormez; iste o "gorulmeyenler" buraya toplanir.
/// IZOLE: hicbir seye baglanmiyor, sadece hesaplar (ilk tugla: boyut olcumu icin).
#[allow(dead_code)]
fn bridge_lists(
    graph: &Graph,
    data: &BTreeMap<VertexId, GhostdagData>,
) -> BTreeMap<VertexId, Vec<VertexId>> {
    let mut out: BTreeMap<VertexId, Vec<VertexId>> = BTreeMap::new();
    for (id, gd) in data.iter() {
        let sp = gd.selected_parent;
        let mut kopruler: Vec<VertexId> = Vec::new();
        if let Some(vx) = graph.get(id) {
            for pp in vx.parents() {
                if !graph.contains(pp) {
                    continue;
                }
                // sp-olmayan parent -> kopru.
                if Some(*pp) != sp {
                    kopruler.push(*pp);
                }
            }
        }
        out.insert(*id, kopruler);
    }
    out
}

/// TORBA (covering set) hesabi: her vertex icin "sp-zinciri DISI atalarina acilan
/// kopru giris noktalari" (tabelalar). Miras: v'nin torbasi = sp-atasinin torbasi
/// + v'nin kendi koprüleri. Bu ILK versiyon SIKISTIRMASIZ (sadece kume - ayni
/// tabela tek kez). Interval-kapsama sikistirmasi sonraki tugla.
/// Topolojik sira sart (sp-atasi once islensin ki mirasi hazir olsun).
/// data: GhostdagData (sp), graph: parent'lar, topo: islem sirasi.
/// IZOLE REFERANS: eski O(t^2) sikistirma mantigi, AYNEN. Test referansi.
/// t1'i at eger bir t2 icin t1, t2'nin sp-agac atasi (s1<=s2 && e2<=e1).
#[allow(dead_code)]
fn sikistir_referans(
    elemanlar: &[VertexId],
    iv: &BTreeMap<VertexId, (u64, u64)>,
) -> BTreeSet<VertexId> {
    let mut atilacak: BTreeSet<VertexId> = BTreeSet::new();
    for &t1 in elemanlar {
        for &t2 in elemanlar {
            if t1 == t2 {
                continue;
            }
            if let (Some(&(s1, e1)), Some(&(s2, e2))) = (iv.get(&t1), iv.get(&t2)) {
                if s1 <= s2 && e2 <= e1 {
                    atilacak.insert(t1);
                }
            }
        }
    }
    atilacak
}

/// IZOLE HIZLI: O(t log t). sp-agac interval'leri ic ice (nested) ya da ayrik.
/// t1 atilir <=> t1 BASKA bir tabelayi kapsiyor (ata). Sirala (start artan,
/// end azalan); bir tabela, kendinden sonra gelen ve end'i <= kendi end'i olan
/// birini kapsiyorsa atilir. Stack ile tek gecis.
fn sikistir_hizli(
    elemanlar: &[VertexId],
    iv: &BTreeMap<VertexId, (u64, u64)>,
) -> BTreeSet<VertexId> {
    // (start, end, id) — iv'si olanlar.
    let mut v: Vec<(u64, u64, VertexId)> = elemanlar
        .iter()
        .filter_map(|&t| iv.get(&t).map(|&(s, e)| (s, e, t)))
        .collect();
    // start artan, esitlikte end AZALAN (kapsayan once).
    v.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let mut atilacak: BTreeSet<VertexId> = BTreeSet::new();
    // Stack: acik "kapsayici adaylari" (start, end, id), end azalan degil artan tut.
    // Her yeni eleman, stack'teki end'i >= kendi end'i olan (kapsayan) varsa,
    // o kapsayan(lar) atilir. Nested oldugu icin: yeni eleman x, stack tepesi y;
    // y.start<=x.start her zaman (sirali). y, x'i kapsiyorsa (y.end>=x.end) -> y ata -> at y.
    let mut stack: Vec<(u64, u64, VertexId)> = Vec::new();
    for (s, e, id) in v {
        // stack tepesindekiler, x'i kapsiyorsa atilir (ata). Ayrik ise pop (artik kapsayamaz).
        while let Some(&(_ts, te, tid)) = stack.last() {
            if te >= e {
                // tepe, x'i kapsiyor (ts<=s zaten, te>=e) -> tepe ata -> at.
                atilacak.insert(tid);
                // tepe hala daha sonraki elemanlari da kapsayabilir -> pop ETME.
                // Ama coklu kapsayici icin: tepe atildi, altindakiler de kapsayabilir.
                // Devam et: tepe disinda altindakileri de kontrol icin pop edip bakmali.
                // Basit ve dogru: pop et, ata listesine ekledik; altina gec.
                stack.pop();
            } else {
                // tepe x'i kapsamiyor (te<e): ayrik ya da x daha genis. Pop (gelecekte kapsayamaz).
                stack.pop();
            }
        }
        stack.push((s, e, id));
    }
    atilacak
}

#[allow(dead_code)]
fn torba_hesapla(
    graph: &Graph,
    data: &BTreeMap<VertexId, GhostdagData>,
    topo: &[VertexId],
    sp_iv: &BTreeMap<VertexId, (u64, u64)>,
) -> BTreeMap<VertexId, BTreeSet<(u64, VertexId)>> {
    let mut torba: BTreeMap<VertexId, BTreeSet<(u64, VertexId)>> = BTreeMap::new();
    for v in topo {
        let mut bag: BTreeSet<VertexId> = BTreeSet::new();
        // 1. sp-atasinin torbasini devral.
        if let Some(gd) = data.get(v) {
            if let Some(sp) = gd.selected_parent {
                if let Some(sp_bag) = torba.get(&sp) {
                    bag.extend(sp_bag.iter().map(|&(_, id)| id));
                }
            }
        }
        // 2. kendi koprülerini (sp-olmayan parent'lar) ekle.
        let sp = data.get(v).and_then(|g| g.selected_parent);
        if let Some(vx) = graph.get(v) {
            for pp in vx.parents() {
                if !graph.contains(pp) {
                    continue;
                }
                if Some(*pp) != sp {
                    bag.insert(*pp);
                }
            }
        }
        let elemanlar: Vec<VertexId> = bag.iter().copied().collect();
        let atilacak = sikistir_hizli(&elemanlar, sp_iv);
        for a in atilacak {
            bag.remove(&a);
        }
        let bag_cift: BTreeSet<(u64, VertexId)> = bag
            .iter()
            .map(|&id| (sp_iv.get(&id).map(|&(s, _)| s).unwrap_or(0), id))
            .collect();
        torba.insert(*v, bag_cift);
    }
    torba
}

fn sp_tree_intervals(data: &BTreeMap<VertexId, GhostdagData>) -> BTreeMap<VertexId, (u64, u64)> {
    use std::collections::BTreeMap as Map;
    // sp-agaci cocuk listesi: parent(sp) -> [child...], id-sirali (determinizm).
    let mut children_sp: Map<VertexId, BTreeSet<VertexId>> = Map::new();
    let mut roots: BTreeSet<VertexId> = BTreeSet::new();
    for (id, d) in data.iter() {
        match d.selected_parent {
            Some(sp) => {
                children_sp.entry(sp).or_default().insert(*id);
            }
            None => {
                roots.insert(*id); // genesis (sp yok) = kok.
            }
        }
    }
    // Iteratif DFS (pre/post sayac). Determinizm: kokler + cocuklar id-sirali.
    let mut intervals: Map<VertexId, (u64, u64)> = Map::new();
    let mut counter: u64 = 0;
    // (vertex, ziyaret_edildi_mi) — post icin ikinci gecis.
    let mut stack: Vec<(VertexId, bool)> = Vec::new();
    for r in roots.iter().rev() {
        stack.push((*r, false));
    }
    let mut start_of: Map<VertexId, u64> = Map::new();
    while let Some((v, visited)) = stack.pop() {
        if visited {
            let st = start_of[&v];
            intervals.insert(v, (st, counter));
            counter += 1;
        } else {
            let st = counter;
            counter += 1;
            start_of.insert(v, st);
            stack.push((v, true));
            if let Some(kids) = children_sp.get(&v) {
                for c in kids.iter().rev() {
                    stack.push((*c, false));
                }
            }
        }
    }
    intervals
}

/// BOSLUKLU aralik tahsisi ile sp-agac interval'i (inkremental-hazir sema).
/// Her dugume genis bir aralik [s, e] verilir; dugumun "noktasi" = e (en son),
/// cocuklara dagitilacak bosluk = [s, e-1]. Cocuklar id-sirali, boslugu esit
/// boler. Bu sema, yeni cocuk eklenince ESKI aralilari kaydirmaz (inkremental
/// icin temel). Ata kurali AYNI: a ata b <=> a.start<=b.start && b.end<=a.end.
/// Bu tugla STATIK (tum data'dan kurar); inkremental ekleme sonraki tugla.
fn sp_tree_intervals_gapped(
    data: &BTreeMap<VertexId, GhostdagData>,
) -> BTreeMap<VertexId, (u64, u64)> {
    use std::collections::BTreeMap as Map;
    // sp-agaci cocuk listesi (id-sirali) + kokler.
    let mut children_sp: Map<VertexId, BTreeSet<VertexId>> = Map::new();
    let mut roots: BTreeSet<VertexId> = BTreeSet::new();
    for (id, d) in data.iter() {
        match d.selected_parent {
            Some(sp) => {
                children_sp.entry(sp).or_default().insert(*id);
            }
            None => {
                roots.insert(*id);
            }
        }
    }
    const BIG: u64 = 1 << 60; // genis baslangic araligi.
    let mut intervals: Map<VertexId, (u64, u64)> = Map::new();
    // Iteratif: (dugum, s, e) yigini. Dugumun noktasi e; bosluk [s, e-1].
    let mut stack: Vec<(VertexId, u64, u64)> = Vec::new();
    // Kokleri [0,BIG], [BIG, 2BIG]... gibi ayri bloklara koy (id-sirali).
    let mut base: u64 = 0;
    for r in roots.iter() {
        stack.push((*r, base, base + BIG));
        base += BIG + 1;
    }
    while let Some((v, s, e)) = stack.pop() {
        intervals.insert(v, (s, e)); // v'nin araligi [s,e]; noktasi e.
        if let Some(kids) = children_sp.get(&v) {
            let k = kids.len() as u64;
            if k > 0 && e > s {
                // bosluk [s, e-1]'i k esit dilime bol (id-sirali cocuklara).
                let span = (e - 1 - s) / k; // her cocuga dusen genislik.
                let mut cs = s;
                for c in kids.iter() {
                    let ce = if span == 0 { cs } else { cs + span - 1 };
                    stack.push((*c, cs, ce.min(e - 1)));
                    cs = ce + 1;
                }
            }
        }
    }
    intervals
}

/// Tam DAG reachability: her vertex icin (sp-agac interval) + (sp-agaci DISI
/// dogrudan-ulasilan vertex'ler). "a, b'nin atasi mi?" -> sp-agac interval O(1)
/// VEYA b'nin reach-kapanisinda a var mi. Kapanis, parent'larin kapanisindan
/// artimli kurulur (topo sira). Tam-past ile AYNI atalik iliskisini verir.
struct ReachIndex<'a> {
    /// sp-agac interval: (start, end). a sp-agac atasi(b) <=> sa<=sb && eb<=ea.
    /// REFERANS (klonsuz) — cagiran hazir iv'yi verir.
    iv: &'a BTreeMap<VertexId, (u64, u64)>,
    /// Her vertex icin sp-olmayan parent'lar (kopruler). Atalik sorgusunda
    /// interval'in goremedigi agac-disi kenarlar buradan asilir. Opsiyonel:
    /// None ise eski davranis (sadece interval + recursive parent yuruyus).
    #[allow(dead_code)]
    bridges: Option<&'a BTreeMap<VertexId, Vec<VertexId>>>,
    /// TORBA (covering set): her vertex'in sp-disi atalarina acilan tabelalar
    /// (miras alinmis). is_ancestor_torba bunu kullanir: recursive YOK, tek gecis.
    torba: Option<&'a BTreeMap<VertexId, BTreeSet<(u64, VertexId)>>>,
}

use std::sync::atomic::{AtomicU64, Ordering as AtOrd};
#[allow(dead_code)]
static BRIDGED_CALLS: AtomicU64 = AtomicU64::new(0);
static REC_CALLS: AtomicU64 = AtomicU64::new(0);

impl<'a> ReachIndex<'a> {
    /// v'nin `universe` icindeki anticone'u: ne v'nin atasi ne torunu olanlar.
    /// is_ancestor_rec (interval hizli yol) ile — mevcut anticone_within ile
    /// AYNI kume. Her u icin iki atalik kontrolu (interval doluysa O(1)'e yakin).
    /// Atalik sorgusu - torba varsa is_ancestor_torba (hizli, recursive yok),
    /// yoksa is_ancestor_rec (eski, recursive). Tek noktadan kontrol: torba: None
    /// eski davranis (bit-bit), torba: Some hizli yol.
    fn atalik(&self, graph: &Graph, a: &VertexId, b: &VertexId) -> bool {
        if self.torba.is_some() {
            self.is_ancestor_torba(a, b)
        } else {
            let mut memo = BTreeMap::new();
            self.is_ancestor_rec(graph, a, b, &mut memo)
        }
    }

    fn anticone_within_ri(
        &self,
        graph: &Graph,
        v: &VertexId,
        universe: &BTreeSet<VertexId>,
    ) -> BTreeSet<VertexId> {
        let mut out = BTreeSet::new();
        for u in universe {
            if u == v {
                continue;
            }
            if !self.atalik(graph, u, v) && !self.atalik(graph, v, u) {
                out.insert(*u);
            }
        }
        out
    }

    /// mergeset(B) = past(B) \\ past(sp) \\ {sp} — TUM past kurmadan.
    /// B'nin parent'larindan geriye BFS; sp'ye ya da sp'nin atalarina (past(sp))
    /// degince budar. Yalnizca mergeset (kucuk) + sinirini gezer. is_ancestor_rec
    /// ile budama (bellek O(1)). Sonuc, past_b\\past_sp\\{sp} ile AYNI kume.
    fn saf_atalik_rec(&self, graph: &Graph, a: &VertexId, b: &VertexId) -> bool {
        if a == b {
            return false;
        }
        let mut stack = vec![*b];
        let mut seen: BTreeSet<VertexId> = BTreeSet::new();
        while let Some(x) = stack.pop() {
            if let Some(vx) = graph.get(&x) {
                for pp in vx.parents() {
                    if pp == a {
                        return true;
                    }
                    if graph.contains(pp) && seen.insert(*pp) {
                        stack.push(*pp);
                    }
                }
            }
        }
        false
    }

    fn mergeset_of(&self, graph: &Graph, id: &VertexId, sp: &VertexId) -> BTreeSet<VertexId> {
        use std::collections::VecDeque;
        let mut ms: BTreeSet<VertexId> = BTreeSet::new();
        let mut seen: BTreeSet<VertexId> = BTreeSet::new();
        let mut q: VecDeque<VertexId> = VecDeque::new();
        if let Some(vx) = graph.get(id) {
            for pp in vx.parents() {
                if graph.contains(pp) && seen.insert(*pp) {
                    q.push_back(*pp);
                }
            }
        }
        while let Some(cur) = q.pop_front() {
            if cur == *sp {
                continue; // sp mergeset'e dahil DEGIL, ardina gitme.
            }
            // cur, sp'nin atasi mi? (past(sp) icinde) -> mergeset disi, budama.
            let mut memo = BTreeMap::new();
            if self.is_ancestor_rec(graph, &cur, sp, &mut memo)
                && self.saf_atalik_rec(graph, &cur, sp)
            {
                continue;
            }
            // cur mergeset'te.
            ms.insert(cur);
            if let Some(vx) = graph.get(&cur) {
                for pp in vx.parents() {
                    if graph.contains(pp) && seen.insert(*pp) {
                        q.push_back(*pp);
                    }
                }
            }
        }
        ms
    }

    /// a, b'nin KESIN atasi mi? — SET SAKLAMADAN, sp-interval + ozyinelemeli
    /// parent yuruyusu. sp-agac hizli yolu O(1); sp-agaci disi icin parent'lara
    /// geriye gider (interval ile budanir). Bellek O(1) (anc kullanmaz).
    /// memo: ayni sorgu icinde tekrar ziyareti onler (dogruluk + hiz).
    fn is_ancestor_rec(
        &self,
        graph: &Graph,
        a: &VertexId,
        b: &VertexId,
        memo: &mut BTreeMap<VertexId, bool>,
    ) -> bool {
        REC_CALLS.fetch_add(1, AtOrd::Relaxed);
        if a == b {
            return false;
        }
        // sp-agac hizli yol: O(1).
        if let (Some(&(sa, ea)), Some(&(sb, eb))) = (self.iv.get(a), self.iv.get(b)) {
            if sa <= sb && eb <= ea {
                return true;
            }
        }
        if let Some(&cached) = memo.get(b) {
            return cached;
        }
        // memo'ya gecici false koy (dongu korumasi; DAG asiklik ama savunmaci).
        memo.insert(*b, false);
        let mut found = false;
        if let Some(vx) = graph.get(b) {
            for pp in vx.parents() {
                if !graph.contains(pp) {
                    continue;
                }
                if pp == a {
                    found = true;
                    break;
                }
                if self.is_ancestor_rec(graph, a, pp, memo) {
                    found = true;
                    break;
                }
            }
        }
        memo.insert(*b, found);
        found
    }

    /// KOPRU-DESTEKLI atalik: sp-interval O(1) hizli yol + interval-BUDAMALI
    /// recursive. Mevcut is_ancestor_rec ile AYNI sonucu vermeli (past ile
    /// birebir test edilecek), ama paralel DAG'da daha az dal gezerek.
    /// BUDAMA: bir parent p'ye inmeden once, a'nin p uzerinden ulasilabilir
    /// olup olmadigini interval ile eler. a, p'nin sp-agac atasi DEGILSE ve
    /// p, a'nin sp-agac atasi DEGILSE bile koprulerden ulasim olabilir -> bu
    /// ilk versiyonda GUVENLI taraf: budama yapma, tum parent'lara in (rec ile
    /// ayni). Sonraki tugla'da budama eklenecek. Simdilik bridges alani sadece
    /// ileride kullanilacak; bu versiyon dogruluk referansi.
    #[allow(dead_code)]
    fn is_ancestor_bridged(
        &self,
        graph: &Graph,
        a: &VertexId,
        b: &VertexId,
        memo: &mut BTreeMap<VertexId, bool>,
    ) -> bool {
        BRIDGED_CALLS.fetch_add(1, AtOrd::Relaxed);
        if a == b {
            return false;
        }
        // sp-agac hizli yol: O(1).
        if let (Some(&(sa, ea)), Some(&(sb, eb))) = (self.iv.get(a), self.iv.get(b)) {
            if sa <= sb && eb <= ea {
                return true;
            }
        }
        if let Some(&cached) = memo.get(b) {
            return cached;
        }
        memo.insert(*b, false);
        let mut found = false;
        if let Some(vx) = graph.get(b) {
            for pp in vx.parents() {
                if !graph.contains(pp) {
                    continue;
                }
                if pp == a {
                    found = true;
                    break;
                }
                // BUDAMA (guvenli): pp'nin koprusu YOKSA, pp'den a'ya tek yol
                // sp-agaci. O halde a, pp'nin sp-agac atasi degilse pp'ye inmeye
                // gerek yok (a oradan ulasilmaz). Koprusu varsa interval eksik
                // olabilir -> yine in. Dogruluk: birebir-test guvenlik agi.
                if let Some(br) = self.bridges {
                    // BUDAMA: pp'ye inmeden once, a oraya ulasabilir mi? interval ile s-z.
                    // a, pp uzerinden ulasilir ANCAK:
                    //  (1) a, pp'nin sp-agac atasi (interval), VEYA
                    //  (2) pp'nin koprulerinden biri c uzerinden: a, c'nin sp-atasi
                    //      (interval) VEYA a==c VEYA c'nin kendi koprusu var (a daha
                    //      derinde olabilir -> guvenli taraf: inmeye izin ver).
                    // Bunlarin HICBIRI olmuyorsa pp dali a'ya goturmez -> ATLA.
                    if let (Some(&(sa, ea)), Some(&(spp, epp))) = (self.iv.get(a), self.iv.get(pp))
                    {
                        let a_pp_nin_sp_atasi = sa <= spp && epp <= ea;
                        let mut koprulerden_olasi = false;
                        if let Some(kk) = br.get(pp) {
                            for c in kk {
                                if c == a {
                                    koprulerden_olasi = true;
                                    break;
                                }
                                // c'nin kendi koprusu varsa, a daha derinde olabilir.
                                let c_koprulu = br.get(c).map(|v| !v.is_empty()).unwrap_or(false);
                                if c_koprulu {
                                    koprulerden_olasi = true;
                                    break;
                                }
                                // a, c'nin sp-agac atasi mi? interval O(1).
                                if let (Some(&(sc, ec)), Some(&(sa2, ea2))) =
                                    (self.iv.get(c), self.iv.get(a))
                                {
                                    if sa2 <= sc && ec <= ea2 {
                                        koprulerden_olasi = true;
                                        break;
                                    }
                                }
                            }
                        }
                        if !a_pp_nin_sp_atasi && !koprulerden_olasi {
                            continue; // pp dali a'ya goturmez -> atla
                        }
                    }
                }
                if self.is_ancestor_bridged(graph, a, pp, memo) {
                    found = true;
                    break;
                }
            }
        }
        memo.insert(*b, found);
        found
    }

    /// TORBA-tabanli atalik: recursive YOK. a, b'nin sp-atasi (interval) VEYA
    /// b'nin torbasindaki bir tabela t icin a==t ya da a, t'nin sp-atasi (interval).
    /// Torba miras yoluyla TUM sp-disi atalari kapsadigi icin tek gecis yeter.
    fn is_ancestor_torba(&self, a: &VertexId, b: &VertexId) -> bool {
        if a == b {
            return false;
        }
        // 1. sp-agac hizli yol: O(1).
        if let (Some(&(sa, ea)), Some(&(sb, eb))) = (self.iv.get(a), self.iv.get(b)) {
            if sa <= sb && eb <= ea {
                return true;
            }
        }
        // 2. torbadaki tabelalardan biri uzerinden. BINARY SEARCH (O(log t)).
        // torba (start,id) sirali. a, t'yi kapsar <=> sa<=st && et<=ea. Nested interval'de
        // st in [sa,ea] => et<=ea otomatik (test: atalik_torba_bs_test bit-bit dogrulandi).
        // range((sa,0)..=(ea,MAX)) ile st in [sa,ea] olan tabelalara O(log t)+sonuc eris.
        if let Some(torba) = self.torba {
            if let Some(bag) = torba.get(b) {
                if let Some(&(sa, ea)) = self.iv.get(a) {
                    use std::ops::Bound::Included;
                    // GUVENLIK: sa<=ea olmali (bozuk/bos interval'de range panikler).
                    // sa>ea ise a gecerli bir aralik degil -> kapsayamaz.
                    if sa > ea {
                        return false;
                    }
                    // st in [sa, ea] olan ilk tabela varsa, a onu kapsar (nested) -> true.
                    let mut it =
                        bag.range((Included((sa, [0u8; 32])), Included((ea, [0xffu8; 32]))));
                    if let Some(&(_st, _id)) = it.next() {
                        // nested garanti: st in [sa,ea] => et<=ea => a kapsar.
                        return true;
                    }
                } else {
                    // a'nin interval'i yok (nadir): id ile lineer ara (guvenlik).
                    for &(_st, id) in bag {
                        if id == *a {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

/// YENI: Kaspa-mantikli renklendirme (izole). Eskiye DOKUNMAZ. Baslangic dongusu YOK;
/// her aday icin sp-zinciri yurunur, cand'in atasi olan zincir blokuna ulasinca durulur.
/// Donus: (mergeset_blues, mergeset_reds, bu vertex'in mavilerinin anticone boyutlari).
/// BINARY LIFTING ile cand'in atasi olan ILK sp-zinciri blogunu bul (O(log n)).
/// Eski chain'in KRITIK 1'de durdugu blogun AYNISINI dondurmeli. Izole, test icin.
/// sp'den baslar; cand'in atasi olmayan en yuksek bloga sicrar, sonra 1 adim = ata.
#[allow(dead_code)]
fn ata_bul_up(
    sp: &VertexId,
    cand: &VertexId,
    up: &BTreeMap<VertexId, Vec<VertexId>>,
    ri: &ReachIndex,
    graph: &Graph,
) -> Option<VertexId> {
    // sp zaten cand'in atasi ise, eski chain ilk adimda durur -> sp.
    if ri.atalik(graph, sp, cand) {
        return Some(*sp);
    }
    // sp cand'in atasi degil. sp-zincirinde, cand'in atasi OLMAYAN en yuksek bloga sicra.
    let mut cur = *sp;
    while let Some(tablo) = up.get(&cur) {
        let mut atladi = false;
        // buyukten kucuge: en buyuk guvenli sicramayi yap.
        for j in (0..tablo.len()).rev() {
            let aday = tablo[j];
            // aday hala cand'in atasi DEGIL ise, oraya sicra (guvenli).
            if !ri.atalik(graph, &aday, cand) {
                cur = aday;
                atladi = true;
                break;
            }
        }
        if !atladi {
            break; // hicbir sicrama guvenli degil -> cur'un sp'si (up[cur][0]) ata.
        }
    }
    // cur, cand'in atasi degil; up[cur][0] cand'in atasi olan ilk blok.
    up.get(&cur).and_then(|t| t.first()).copied()
}

// MAVI BONCUK: AIDAG'in kendi renklendirme sistemi. coloring_kaspa'nin chain-dongusu
// (sp-zinciri mergeset_blues toplama) GHOSTDAG invariant'i kirilinca anticone'u eksik
// sayiyordu (69ef: 14 vs gercek 23 -> overcount). Mavi boncuk, cand'in anticone'unu
// blue_view icinde DOGRUDAN, SAF-dogrulanmis atalik ile sayar (torba yanlis-pozitifinden
// bagimsiz). blue kumesi her mavi eklendikce buyur (compute_default ile ayni matematik).
static MB_INIT_SAY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static MB_CAND_SAY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

static BAS_ADIM: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static ODA_NZ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static T_BLUE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static T_MERGE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static T_CAND: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static T_SIGN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static T_INSERT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static T_GD: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static U_ANTI: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static U_IV: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static U_TORBA: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static U_UP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static TORBA_BOY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static TORBA_SAY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static ODA_TOP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn blue_anticone_size(
    b: &VertexId,
    sp: &VertexId,
    data: &BTreeMap<VertexId, GhostdagData>,
    anticone_sizes: &BTreeMap<VertexId, BTreeMap<VertexId, u32>>,
) -> Option<u32> {
    // Geriye yuru, b iceren ILK (en yakin/guncel) kayitta dur.
    let mut cur = Some(*sp);
    while let Some(c) = cur {
        BAS_ADIM.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if let Some(sz) = anticone_sizes.get(&c).and_then(|m| m.get(b)) {
            return Some(*sz);
        }
        cur = data.get(&c).and_then(|d| d.selected_parent);
    }
    None
}

fn mavi_boncuk(
    graph: &Graph,
    id: &VertexId,
    sp: &VertexId,
    k: KType,
    data: &BTreeMap<VertexId, GhostdagData>,
    ri: &ReachIndex,
    _anticone_sizes: &BTreeMap<VertexId, BTreeMap<VertexId, u32>>,
    boyut_map: &BTreeMap<VertexId, BTreeMap<VertexId, u32>>,
) -> (Vec<VertexId>, Vec<VertexId>, BTreeMap<VertexId, u32>) {
    let k_usize = k as usize;
    let _tm = std::time::Instant::now();
    let mergeset_unordered = ri.mergeset_of(graph, id, sp);
    let ordered_mergeset = topo_order_subset(graph, &mergeset_unordered);
    T_MERGE.fetch_add(_tm.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);

    // SAF-dogrulanmis anticone: u ve cand, saf recursive ile iliskisizse anticone'da.
    // DOGRULUK: saf recursive (torba'ya sorma). Torba, is_ancestor_torba fast-path
    // (sa<=sb && eb<=ea) interval cakismasi yuzunden mavi_boncuk baglaminda yanlis-pozitif
    // veriyor (overcount geri geliyor) VE bazen yanlis-negatif (undercount). Saf kesin.
    // [HIZ: sonraki tur - torba fast-path'i saf ile seçici dogrula ya da saf'i hizlandir.]
    let saf_iliskisiz = |ri: &ReachIndex, u: &VertexId, c: &VertexId| -> bool {
        !ri.saf_atalik_rec(graph, u, c) && !ri.saf_atalik_rec(graph, c, u)
    };

    let _t0 = std::time::Instant::now();
    // [HIZ] mergeset bossa blue HIC kullanilmaz (candidate dongusu donmez) -> kurma.
    // Zincir senaryosunda mergeset hep bos -> blue_set_in_view O(n^2) darbogazi atlanir.
    let mut blue: BTreeSet<VertexId> = if ordered_mergeset.is_empty() {
        BTreeSet::new()
    } else {
        blue_set_in_view(data, *sp)
    };
    T_BLUE.fetch_add(_t0.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    let _t1 = std::time::Instant::now();
    // [TUGLA2c HIZ] TEK GECIS: sp-zincirini BIR kez yuru, her b'nin ILK (en yakin/
    // guncel) boyutunu topla. Her b icin ayri yuruyus (O(n^2)) YERINE tek yuruyus O(n).
    // "ilk bulunan = en guncel" (blue_anticone_size ile ayni deger). Miras=saf kanitli.
    // [DEVRALMA] anticone_sizes[sp] artik TAM harita (return boyut). Dogrudan
    // devral, sp-zinciri yuruyusu YOK -> O(blue) per vertex. tek geciste yuruyus elendi.
    // [ODA] boyut = SADECE sifir-olmayan anticone boyutlari (varsayilan 0). Zincirde
    // hepsi 0 -> harita bos -> klon/kaydet O(sifir-olmayan) ~ O(1). O(n^2) bellek kirildi.
    // sp'den devral (zaten sadece sifir-olmayanlar), blue-disi at, 0 EKLEME.
    let _ = data;
    let mut boyut: BTreeMap<VertexId, u32> = _anticone_sizes.get(sp).cloned().unwrap_or_default();
    boyut.retain(|_kk, &mut vv| vv > 0);

    let mut mergeset_blues: Vec<VertexId> = Vec::new();
    let mut mergeset_reds: Vec<VertexId> = Vec::new();
    let mut out: BTreeMap<VertexId, u32> = BTreeMap::new();
    out.insert(*sp, 0);

    let _tc = std::time::Instant::now();
    for cand in &ordered_mergeset {
        let anticone: Vec<VertexId> = blue.iter().filter(|u| {
            if **u != *cand { MB_CAND_SAY.fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
            **u != *cand && saf_iliskisiz(ri, u, cand)
        }).copied().collect();
        let mut is_blue = anticone.len() <= k_usize;
        if is_blue {
            for b in &anticone {
                if *boyut.get(b).unwrap_or(&0) as usize + 1 > k_usize { is_blue = false; break; }
            }
        }
        if is_blue {
            for b in &anticone {
                let yeni = *boyut.entry(*b).or_insert(0) + 1;
                *boyut.get_mut(b).unwrap() = yeni;
                out.insert(*b, yeni); // +1 peer KALICI kaydet (Kaspa mantigi)
            }
            // [ODA] sadece sifir-olmayan yaz (0 saklama -> harita kompakt kalir)
            if !anticone.is_empty() {
                boyut.insert(*cand, anticone.len() as u32);
            }
            let _ = &mut out;
            blue.insert(*cand);
            mergeset_blues.push(*cand);
        } else {
            mergeset_reds.push(*cand);
        }
    }
    T_CAND.fetch_add(_tc.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    (mergeset_blues, mergeset_reds, boyut)
}

fn coloring_kaspa(
    graph: &Graph,
    id: &VertexId,
    sp: &VertexId,
    k: KType,
    data: &BTreeMap<VertexId, GhostdagData>,
    ri: &ReachIndex,
    anticone_sizes: &BTreeMap<VertexId, BTreeMap<VertexId, u32>>,
) -> (Vec<VertexId>, Vec<VertexId>, BTreeMap<VertexId, u32>) {
    let k_usize = k as usize;
    let mergeset_unordered = ri.mergeset_of(graph, id, sp);
    let ordered_mergeset = topo_order_subset(graph, &mergeset_unordered);

    let mut yeni_sizes: BTreeMap<VertexId, u32> = BTreeMap::new();
    yeni_sizes.insert(*sp, 0);

    let mut mergeset_blues: Vec<VertexId> = Vec::new();
    let mut mergeset_reds: Vec<VertexId> = Vec::new();

    for cand in &ordered_mergeset {
        // KASPA KISAYOLU: sp + k mavi zaten yerlestiyse (mergeset_blues sp'siz oldugu
        // icin esik = k), fazlasi k-cluster geregi kirmizi. Chain'e hic girme.
        // (Kaspa check_blue_candidate: mergeset_blues K+1 ise Red; bizde sp ayri -> k.)
        if mergeset_blues.len() >= k_usize {
            mergeset_reds.push(*cand);
            continue;
        }
        // GUVENLI ATLAMA: cand'in TUM parent'lari sp'nin atasi/kendisi ise anticone BOS
        // (sp hepsini gorur). Olcum: bu kosulda anticone HEP 0 (dolu_kosul=0, guvenli).
        // Chain'e girme: cand mavi, boyut 0 - chain'in bos durumda uretecegiyle BIREBIR.
        let atla = graph
            .get(cand)
            .map(|vx| {
                vx.parents()
                    .iter()
                    .all(|pp| pp == sp || ri.atalik(graph, pp, sp))
            })
            .unwrap_or(false);
        if atla {
            yeni_sizes.insert(*cand, 0);
            mergeset_blues.push(*cand);
            continue;
        }
        let mut cand_anticone_size: usize = 0;
        let mut cand_yeni_kayitlar: Vec<(VertexId, u32)> = Vec::new();
        let mut is_blue = true;

        let mut chain_cur = Some(*sp);
        while let Some(chain_block) = chain_cur {
            // KRITIK 1: chain_block cand'in atasi ise, kalan zincir cand'in gecmisinde.
            if ri.atalik(graph, &chain_block, cand) {
                break;
            }
            if let Some(cb_data) = data.get(&chain_block) {
                for peer in &cb_data.mergeset_blues {
                    if ri.atalik(graph, peer, cand) {
                        continue; // peer cand'in gecmisinde -> anticone'da degil.
                    }
                    cand_anticone_size += 1;
                    if cand_anticone_size > k_usize {
                        is_blue = false;
                        break;
                    }
                    // KRITIK 3: peer'in mevcut boyutu - once yeni_sizes, sonra sp-zinciri.
                    let peer_sz = {
                        let mut bulunan: Option<u32> = yeni_sizes.get(peer).copied();
                        if bulunan.is_none() {
                            let mut cur = Some(*sp);
                            while let Some(v) = cur {
                                if let Some(m) = anticone_sizes.get(&v) {
                                    if let Some(&sz) = m.get(peer) {
                                        bulunan = Some(sz);
                                        break;
                                    }
                                }
                                cur = data.get(&v).and_then(|d| d.selected_parent);
                            }
                        }
                        bulunan.unwrap_or(0)
                    };
                    // KRITIK 2: peer zaten k komsuya sahipse, cand eklenince k'yi asar.
                    if peer_sz as usize == k_usize {
                        is_blue = false;
                        break;
                    }
                    cand_yeni_kayitlar.push((*peer, peer_sz + 1));
                }
            }
            if !is_blue {
                break;
            }
            chain_cur = data.get(&chain_block).and_then(|d| d.selected_parent);
        }

        if is_blue {
            for (peer, yeni_sz) in cand_yeni_kayitlar {
                yeni_sizes.insert(peer, yeni_sz);
            }
            yeni_sizes.insert(*cand, cand_anticone_size as u32);
            mergeset_blues.push(*cand);
        } else {
            mergeset_reds.push(*cand);
        }
    }

    let mut out: BTreeMap<VertexId, u32> = BTreeMap::new();
    out.insert(*sp, 0);
    for b in &mergeset_blues {
        if let Some(&sz) = yeni_sizes.get(b) {
            out.insert(*b, sz);
        }
    }
    (mergeset_blues, mergeset_reds, out)
}

// Konsensus cekirdegi: 8 parametrenin hepsi GHOSTDAG hesabi icin gerekli
// (graph, id, k, weigher, data, iv, torba, anticone). Struct'a gruplamak
// referans/omur yonetimini gereksiz karmasiklastirirdi -> bilerek allow.
#[allow(clippy::too_many_arguments)]
fn compute_vertex_data<W: Weigher>(
    graph: &Graph,
    id: &VertexId,
    k: KType,
    weigher: &W,
    data: &BTreeMap<VertexId, GhostdagData>,
    iv: &BTreeMap<VertexId, (u64, u64)>,
    torba: Option<&BTreeMap<VertexId, BTreeSet<(u64, VertexId)>>>,
    anticone_sizes: Option<&BTreeMap<VertexId, BTreeMap<VertexId, u32>>>,
) -> (GhostdagData, BTreeMap<VertexId, u32>) {
    let vx = graph.get(id).expect("id graph'ta var");
    // Yalnızca graph'ta GERÇEKTEN mevcut ebeveynler (savunmacı).
    let parents: Vec<VertexId> = vx
        .parents()
        .iter()
        .copied()
        .filter(|p| graph.contains(p))
        .collect();

    if parents.is_empty() {
        // Genesis: mavi yok, score/work 0, seçili ebeveyn yok.
        return (GhostdagData {
            blue_score: 0,
            blue_work: 0,
            selected_parent: None,
            mergeset_blues: Vec::new(),
            mergeset_reds: Vec::new(),
        }, BTreeMap::new());
    }

    // 1. Seçili ebeveyn: max blue_work, beraberlik min-id.
    let sp = select_parent(&parents, data);

    // 2. mergeset(B) = past(B) \ past(sp) \ {sp} — TUM past kurmadan,
    //    sinirli BFS + is_ancestor_rec budama (mergeset_of). Bos interval'li
    //    ReachIndex: hizli-yol yok ama dogru (zincirde mergeset bos -> cok hizli).
    //    [test: mergeset_of_eski_yontemle_birebir — past_b\past_sp\{sp} ile ayni]
    // ReachIndex: o ana kadar islenmis data'dan interval kur (id henuz data'da
    // DEGIL — atalik kontrolleri atalara bakar, onlar data'da). anticone hizli
    // yolu (interval O(1)) icin DOLU interval sart.
    let ri = ReachIndex {
        iv,
        bridges: None,
        torba,
    };
    let mergeset_unordered: BTreeSet<VertexId> = ri.mergeset_of(graph, id, &sp);
    let ordered_mergeset = topo_order_subset(graph, &mergeset_unordered);

    // 3. k-cluster renklendirme.
    // Başlangıç mavi küme = seçili ebeveynin görüşündeki tüm maviler (sp dahil).
    // OPTIMIZASYON: blue YALNIZCA mergeset-aday dongusunde kullanilir; madde-4'te
    // (GhostdagData uretimi) kullanilmaz. Mergeset bossa o dongu calismaz -> blue
    // gereksiz. blue_set_in_view sp-zincirini yurur (zincirde O(n)); mergeset bossa
    // gereksiz -> bos birak (sonuc bit-bit ayni). [olcum: n=10000'de ~32s idi]

    // RENKLENDIRME: anticone_sizes verildiyse (update yolu) -> mavi_boncuk
    // (AIDAG'in kendi near-linear renklendirmesi: kosullu blue + oda sikistirma +
    // sp'den boyut devralma). Yoksa (compute_with_weight, toptan) -> eski
    // blue_set_in_view + tum-blue tarama. Ikisi de ayni sonucu verir
    // (fuzz_dogrula 2000 tur + birebir testler kanitladi); bit-bit compute-vs-update korunur.
    let (mergeset_blues, mergeset_reds, out_opt): (Vec<VertexId>, Vec<VertexId>, Option<BTreeMap<VertexId, u32>>) =
        if let Some(asz) = anticone_sizes {
            let (b, r, out_mb) = mavi_boncuk(graph, id, &sp, k, data, &ri, asz, asz);
            (b, r, Some(out_mb))
        } else {
            let mut blue: BTreeSet<VertexId> = if ordered_mergeset.is_empty() {
                BTreeSet::new()
            } else {
                blue_set_in_view(data, sp)
            };
            let mut anticone_size: BTreeMap<VertexId, u32> = BTreeMap::new();
            if !ordered_mergeset.is_empty() {
                for b in &blue {
                    anticone_size.insert(*b, ri.anticone_within_ri(graph, b, &blue).len() as u32);
                }
            }
            let mut mergeset_blues: Vec<VertexId> = Vec::new();
            let mut mergeset_reds: Vec<VertexId> = Vec::new();
            for cand in ordered_mergeset {
                let cand_anticone = ri.anticone_within_ri(graph, &cand, &blue);
                let k_usize = k as usize;
                let mut is_blue = cand_anticone.len() <= k_usize;
                if is_blue {
                    for b in &cand_anticone {
                        let cur = *anticone_size.get(b).unwrap_or(&0);
                        if cur as usize + 1 > k_usize {
                            is_blue = false;
                            break;
                        }
                    }
                }
                if is_blue {
                    for b in &cand_anticone {
                        *anticone_size.entry(*b).or_insert(0) += 1;
                    }
                    anticone_size.insert(cand, cand_anticone.len() as u32);
                    blue.insert(cand);
                    mergeset_blues.push(cand);
                } else {
                    mergeset_reds.push(cand);
                }
            }
            (mergeset_blues, mergeset_reds, Some(anticone_size))
        };

    // 4. blue_score = bs(sp) + 1 (sp'nin kendisi) + yeni maviler.
    let sp_data = data.get(&sp);
    let sp_score = sp_data.map(|d| d.blue_score).unwrap_or(0);
    let sp_work = sp_data.map(|d| d.blue_work).unwrap_or(0);
    let blue_score = sp_score + 1 + mergeset_blues.len() as u64;
    // blue_work = bw(sp) + weight(sp) + Σ weight(yeni maviler).
    let mergeset_blue_work: u64 = mergeset_blues
        .iter()
        .map(|b| weigher.weight(graph, b))
        .sum();
    let blue_work = sp_work + weigher.weight(graph, &sp) + mergeset_blue_work;

    (GhostdagData {
        blue_score,
        blue_work,
        selected_parent: Some(sp),
        mergeset_blues,
        mergeset_reds,
    }, out_opt.unwrap_or_default())
}

/// Ebeveynler içinde en yüksek blue-WORK'lü olanı seç; beraberlikte min-id
/// (denetçi O-sys). `data` çağrı anında tüm ebeveynleri içerir (topo sıra
/// garantisi). `UniformWeight` ile blue_work == blue_score olduğundan davranış
/// önceki sayım-tabanlı seçimle birebir aynıdır.
fn select_parent(parents: &[VertexId], data: &BTreeMap<VertexId, GhostdagData>) -> VertexId {
    let mut best: Option<(u64, VertexId)> = None;
    for p in parents {
        let work = data.get(p).map(|d| d.blue_work).unwrap_or(0);
        match best {
            None => best = Some((work, *p)),
            Some((bwork, bid)) => {
                if work > bwork || (work == bwork && *p < bid) {
                    best = Some((work, *p));
                }
            }
        }
    }
    best.map(|(_, id)| id).expect("parents boş değil")
}

/// Bir bloğun görüşündeki TÜM mavi blok kümesi (blok dahil): seçili-ebeveyn
/// zinciri boyunca her zincir bloğu + onun mergeset_blues'u birleştirilir.
/// GHOSTDAG değişmezi: her mavi blok ya zincirde ya da tam bir zincir bloğunun
/// mergeset_blues'undadır → bu yürüyüş eksiksizdir.
fn blue_set_in_view(
    data: &BTreeMap<VertexId, GhostdagData>,
    start: VertexId,
) -> BTreeSet<VertexId> {
    let mut set = BTreeSet::new();
    let mut cur = Some(start);
    while let Some(c) = cur {
        set.insert(c);
        match data.get(&c) {
            Some(d) => {
                for b in &d.mergeset_blues {
                    set.insert(*b);
                }
                cur = d.selected_parent;
            }
            None => break,
        }
    }
    set
}

/// Bir alt-kümeyi BELİRLENİMCİ topolojik sıraya diz: anahtar = (alt-küme
/// içindeki ata sayısı, VertexId). x, y'nin atası ve ikisi de alt-kümedeyse
/// `|past(x) ∩ S| < |past(y) ∩ S|` (kesin) → ata daima önce; beraberlik id
/// ile bozulur. (Adım 3a `topological_order`'ın alt-küme muadili.)
fn topo_order_subset(graph: &Graph, subset: &BTreeSet<VertexId>) -> Vec<VertexId> {
    let mut v: Vec<VertexId> = subset.iter().copied().collect();
    v.sort_by_cached_key(|x| {
        let rank = past(graph, x)
            .iter()
            .filter(|a| subset.contains(*a))
            .count();
        (rank, *x)
    });
    v
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

    fn signed(seed: u8, parents: Vec<VertexId>, ts: u64, payload: &[u8]) -> Vertex {
        // Seçenek A: vertex primitifi artık kanonik (strict artan) parent ister.
        // Gerçek üretici de parent'ları sıralı üretir; test helper'ı bunu modeller.
        let mut parents = parents;
        parents.sort_unstable();
        Vertex::new_signed(NET, parents, payload.to_vec(), ts, &key(seed)).unwrap()
    }

    /// Doğrusal zincir g0→g1→...; (graph, ids).
    fn linear_chain(n: usize, seed: u8) -> (Graph, Vec<VertexId>) {
        let mut g = Graph::devnet(NET);
        let gen = signed(seed, vec![], 1000, b"genesis");
        let mut last = *gen.id();
        let mut ids = vec![last];
        g.insert_synced(gen).unwrap();
        for i in 1..n {
            let v = signed(
                seed,
                vec![last],
                1000 + i as u64,
                format!("v{i}").as_bytes(),
            );
            last = *v.id();
            ids.push(last);
            g.insert_synced(v).unwrap();
        }
        (g, ids)
    }

    /// Diamond: A→B, A→C, (B,C)→D. (graph, [A,B,C,D]).
    fn diamond() -> (Graph, [VertexId; 4]) {
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
        (g, [aid, bid, cid, did])
    }

    // KOPRU BOYUTU OLCUMU: cok-katli paralel DAG'da bridge_lists ne kadar buyuk?
    // KRITIK SORU: kopru listesi kucuk mu kaliyor (umut) yoksa data ile patliyor
    // mu (cache tuzagi)? Buyuk olursa A2 yaklasimi belleksel olarak da riskli.
    #[test]
    #[ignore]
    fn kopru_boyutu_olcumu() {
        // W paralel vertex/kat, her vertex onceki katin TUM vertex'lerini parent alir.
        let katlar = 15usize;
        let w = 5usize;
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"gen");
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();
        let mut prev: Vec<VertexId> = vec![gid];
        let mut ts = 1001u64;
        for _k in 0..katlar {
            let mut bu_kat = Vec::new();
            let mut parents = prev.clone();
            parents.sort_unstable();
            for j in 0..w {
                let v = signed((j % 250) as u8 + 1, parents.clone(), ts, b"x");
                ts += 1;
                bu_kat.push(*v.id());
                g.insert_synced(v).unwrap();
            }
            prev = bu_kat;
        }
        let gd = Ghostdag::compute_default(&g);
        let data = data_map_of(&gd, &g);
        let bridges = bridge_lists(&g, &data);
        let toplam_vertex = data.len();
        let max_kopru = bridges.values().map(|v| v.len()).max().unwrap_or(0);
        let toplam_kopru: usize = bridges.values().map(|v| v.len()).sum();
        let ort = toplam_kopru as f64 / bridges.len().max(1) as f64;
        eprintln!(
            "KOPRU_OLCUM vertex={} max_kopru_listesi={} ortalama={:.2} toplam_kopru={}",
            toplam_vertex, max_kopru, ort, toplam_kopru
        );
    }

    #[test]
    fn genesis_has_zero_score_and_no_selected_parent() {
        let (g, ids) = linear_chain(1, 1);
        let gd = Ghostdag::compute_default(&g);
        let d = gd.data(&ids[0]).unwrap();
        assert_eq!(d.blue_score, 0);
        assert_eq!(d.selected_parent, None);
        assert!(d.mergeset_blues.is_empty() && d.mergeset_reds.is_empty());
    }

    // sp-agaci interval etiketleme DOGRULUK testi: interval kurali, sp-zincirini
    // elle yuruyerek bulunan gercek "sp-agac atasi mi" cevabiyla AYNI olmali.
    // (Sisteme baglamadan ONCE izole dogrulama — cache hatasindan ders.)
    fn data_map_of(gd: &Ghostdag, g: &Graph) -> BTreeMap<VertexId, GhostdagData> {
        let mut m = BTreeMap::new();
        for id in g.ids() {
            if let Some(d) = gd.data(id) {
                m.insert(*id, d.clone());
            }
        }
        m
    }

    // A, B'nin sp-agac atasi mi? (B'den selected_parent zincirini yukari yuru.)
    fn is_sp_ancestor(data: &BTreeMap<VertexId, GhostdagData>, a: &VertexId, b: &VertexId) -> bool {
        if a == b {
            return false;
        }
        let mut cur = data.get(b).and_then(|d| d.selected_parent);
        while let Some(c) = cur {
            if c == *a {
                return true;
            }
            cur = data.get(&c).and_then(|d| d.selected_parent);
        }
        false
    }

    #[test]
    fn anticone_within_ri_eski_ile_birebir() {
        // anticone_within_ri, mevcut anticone_within ile AYNI olmali.
        let cases: Vec<(Graph, Vec<VertexId>)> = vec![
            {
                let (g, ids) = linear_chain(7, 1);
                (g, ids)
            },
            {
                let (g, [a, b, c, d]) = diamond();
                (g, vec![a, b, c, d])
            },
        ];
        for (g, ids) in cases {
            let gd = Ghostdag::compute_default(&g);
            let data = data_map_of(&gd, &g);
            let iv = sp_tree_intervals(&data);
            let ri = ReachIndex {
                iv: &iv,
                bridges: None,
                torba: None,
            };
            // universe = tum vertex'ler (genel test).
            let universe: BTreeSet<VertexId> = ids.iter().copied().collect();
            for v in &ids {
                let yeni = ri.anticone_within_ri(&g, v, &universe);
                let eski = crate::consensus::anticone_within(&g, v, &universe);
                assert_eq!(yeni, eski, "anticone farkli: v={v:?}");
            }
        }
    }

    #[test]
    #[ignore]
    fn torba_seyrek_dag_olcumu() {
        // GERCEKCI (seyrek) DAG: her vertex son 2 vertex'i parent alir (yogun degil).
        // worst-case test'ten farkli: tabela sayisi kontrollu mu?
        for n in [50usize, 100, 200] {
            let mut g = Graph::devnet(NET);
            let gen = signed(1, vec![], 1000, b"gen");
            let gid = *gen.id();
            g.insert_synced(gen).unwrap();
            let mut ids = vec![gid];
            for (ts, i) in (1001u64..).zip(0..n) {
                // son 1-2 vertex'i parent al (seyrek).
                let mut parents = Vec::new();
                parents.push(ids[ids.len() - 1]);
                if ids.len() >= 2 {
                    parents.push(ids[ids.len() - 2]);
                }
                parents.sort_unstable();
                parents.dedup();
                let v = signed((i % 250) as u8 + 1, parents, ts, b"x");
                let vid = *v.id();
                g.insert_synced(v).unwrap();
                ids.push(vid);
            }
            let gd = Ghostdag::compute_default(&g);
            let data = data_map_of(&gd, &g);
            let iv = sp_tree_intervals(&data);
            let topo = super::topological_order(&g);
            let torba = torba_hesapla(&g, &data, &topo, &iv);
            let max_t = torba.values().map(|b| b.len()).max().unwrap_or(0);
            let ort: f64 =
                torba.values().map(|b| b.len()).sum::<usize>() as f64 / torba.len().max(1) as f64;
            eprintln!(
                "SEYREK_TORBA vertex={} max_torba={} ort_torba={:.2}",
                data.len(),
                max_t,
                ort
            );
        }
    }

    #[test]
    #[ignore = "regresyon: FUZZ_TUR=2000 ile elle calistir"]
    fn fuzz_dogrula() {
        let turlar: u64 = std::env::var("FUZZ_TUR").ok().and_then(|x| x.parse().ok()).unwrap_or(2000);
        let mut lcg: u64 = 0x9E3779B97F4A7C15;
        let mut rng = || { lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); lcg };
        for tur in 0..turlar {
            let n = 5 + (rng() % 60) as usize;
            let mut g = Graph::devnet(NET);
            let gen = signed(1, vec![], 1000, b"g"); let gid=*gen.id();
            g.insert_synced(gen).unwrap();
            let mut inc = Ghostdag::new_incremental(DEFAULT_K); inc.update_one(&g,&gid);
            let mut ids=vec![gid]; let mut ts=1001u64;
            for i in 1..n {
                let mevcut=ids.len(); let pmax=mevcut.min(8);
                let pk=1+(rng()%pmax as u64) as usize;
                let mut parents=Vec::new();
                for _ in 0..pk { let idx=(rng()%mevcut as u64) as usize; let c=ids[idx]; if !parents.contains(&c){parents.push(c);} }
                if parents.is_empty(){parents.push(ids[mevcut-1]);}
                let seed=(1+(rng()%250)) as u8;
                let v=signed(seed,parents,ts,format!("t{tur}v{i}").as_bytes()); ts+=1; let vid=*v.id();
                if g.insert_synced(v).is_err(){continue;}
                inc.update_one(&g,&vid); ids.push(vid);
            }
            let refr = Ghostdag::compute_default(&g);
            let d_inc = data_map_of(&inc, &g);
            let d_ref = data_map_of(&refr, &g);
            for (id, dref) in &d_ref {
                let dinc = d_inc.get(id).expect("eksik");
                if dinc.blue_score != dref.blue_score || dinc.mergeset_blues != dref.mergeset_blues {
                    panic!("FUZZ FARK tur={} n={} v={:02x}{:02x}: inc_bs={} ref_bs={} inc_blues={} ref_blues={}",
                        tur, n, id[0], id[1], dinc.blue_score, dref.blue_score, dinc.mergeset_blues.len(), dref.mergeset_blues.len());
                }
            }
        }
        eprintln!("FUZZ OK: {} tur, hepsi fark=0", turlar);
    }

    #[test]
    #[ignore]
    fn fuzz_determinizm() {
        // DETERMINIZM FUZZ: ayni vertex kumesi, FARKLI ekleme sirasi -> AYNI sonuc.
        let turlar: u64 = std::env::var("DET_TUR").ok().and_then(|x| x.parse().ok()).unwrap_or(2000);
        let mut lcg: u64 = 0xD1B54A32D192ED03;
        let mut rng = || { lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); lcg };
        for tur in 0..turlar {
            if tur % 1000 == 0 { eprintln!("[det] {}/{} tur", tur, turlar); }
            let n = 5 + (rng() % 60) as usize;
            let mut g0 = Graph::devnet(NET);
            let gen = signed(1, vec![], 1000, b"g"); let gid = *gen.id();
            g0.insert_synced(gen.clone()).unwrap();
            let mut ids = vec![gid]; let mut ts = 1001u64;
            let mut verts: Vec<Vertex> = vec![gen];
            for i in 1..n {
                let mevcut = ids.len(); let pmax = mevcut.min(8);
                let pk = 1 + (rng() % pmax as u64) as usize;
                let mut parents = Vec::new();
                for _ in 0..pk { let idx = (rng() % mevcut as u64) as usize; let c = ids[idx]; if !parents.contains(&c) { parents.push(c); } }
                if parents.is_empty() { parents.push(ids[mevcut - 1]); }
                let seed = (1 + (rng() % 250)) as u8;
                let v = signed(seed, parents, ts, format!("t{tur}v{i}").as_bytes()); ts += 1; let vid = *v.id();
                if g0.insert_synced(v.clone()).is_err() { continue; }
                ids.push(vid); verts.push(v);
            }
            // topolojik-gecerli ekleme (verilen ipucu sirasina gore, parent hazir olani ekle)
            let insert_ile = |ipucu: &[usize]| -> (Ghostdag, Graph) {
                let mut g = Graph::devnet(NET);
                let mut inc = Ghostdag::new_incremental(DEFAULT_K);
                let mut kalan: Vec<usize> = ipucu.to_vec();
                loop {
                    let mut yeni = Vec::new(); let mut ilerledi = false;
                    for &idx in &kalan {
                        let v = &verts[idx];
                        if v.parents().is_empty() || v.parents().iter().all(|p| g.contains(p)) {
                            let vid = *v.id();
                            if g.insert_synced(v.clone()).is_ok() { inc.update_one(&g, &vid); ilerledi = true; }
                        } else { yeni.push(idx); }
                    }
                    kalan = yeni;
                    if kalan.is_empty() || !ilerledi { break; }
                }
                (inc, g)
            };
            let sira_a: Vec<usize> = (0..verts.len()).collect();
            let mut sira_b = sira_a.clone();
            for i in (1..sira_b.len()).rev() { let j = (rng() % (i as u64 + 1)) as usize; sira_b.swap(i, j); }
            let (gd_a, ga) = insert_ile(&sira_a);
            let (gd_b, _gb) = insert_ile(&sira_b);
            let da = data_map_of(&gd_a, &ga);
            let db = data_map_of(&gd_b, &ga);
            for (id, a) in &da {
                let b = db.get(id).expect("det: eksik");
                if a.blue_score != b.blue_score || a.mergeset_blues != b.mergeset_blues {
                    panic!("DET FARK tur={} n={} v={:02x}{:02x}: a_bs={} b_bs={}", tur, n, id[0], id[1], a.blue_score, b.blue_score);
                }
            }
        }
        eprintln!("DET OK: {} tur, farkli sira -> ayni sonuc", turlar);
    }

    #[test]
    #[ignore]
    fn fuzz_invariant() {
        // PROPERTY FUZZ: INV1 monotonluk, INV2 mavi/kirmizi ayrik, INV3 k-cluster
        let turlar: u64 = std::env::var("PROP_TUR").ok().and_then(|x| x.parse().ok()).unwrap_or(2000);
        let mut lcg: u64 = 0x2545F4914F6CDD1D;
        let mut rng = || { lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); lcg };
        let k = DEFAULT_K as u32;
        for tur in 0..turlar {
            if tur % 1000 == 0 { eprintln!("[prop] {}/{} tur", tur, turlar); }
            let n = 5 + (rng() % 60) as usize;
            let mut g = Graph::devnet(NET);
            let gen = signed(1, vec![], 1000, b"g"); let gid = *gen.id();
            g.insert_synced(gen).unwrap();
            let mut inc = Ghostdag::new_incremental(DEFAULT_K); inc.update_one(&g, &gid);
            let mut ids = vec![gid]; let mut ts = 1001u64;
            for i in 1..n {
                let mevcut = ids.len(); let pmax = mevcut.min(8);
                let pk = 1 + (rng() % pmax as u64) as usize;
                let mut parents = Vec::new();
                for _ in 0..pk { let idx = (rng() % mevcut as u64) as usize; let c = ids[idx]; if !parents.contains(&c) { parents.push(c); } }
                if parents.is_empty() { parents.push(ids[mevcut - 1]); }
                let seed = (1 + (rng() % 250)) as u8;
                let v = signed(seed, parents, ts, format!("p{tur}v{i}").as_bytes()); ts += 1; let vid = *v.id();
                if g.insert_synced(v).is_err() { continue; }
                inc.update_one(&g, &vid); ids.push(vid);
            }
            let d = data_map_of(&inc, &g);
            for (id, gd) in &d {
                for b in &gd.mergeset_blues {
                    if gd.mergeset_reds.contains(b) { panic!("INV2 IHLAL tur={} v={:02x}{:02x}", tur, id[0], id[1]); }
                }
                if let Some(sp) = gd.selected_parent {
                    if let Some(spd) = d.get(&sp) {
                        if spd.blue_score > gd.blue_score { panic!("INV1 IHLAL tur={} v={:02x}{:02x}: {}>{}", tur, id[0], id[1], spd.blue_score, gd.blue_score); }
                    }
                }
            }
            for (vid, harita) in inc.anticone_sizes.iter() {
                for (bid, &sz) in harita.iter() {
                    if sz > k { panic!("INV3 IHLAL tur={} v={:02x}{:02x} b={:02x}{:02x}: {}>{}", tur, vid[0], vid[1], bid[0], bid[1], sz, k); }
                }
            }
        }
        eprintln!("PROP OK: {} tur, INV1+INV2+INV3 tuttu", turlar);
    }

    #[test]
    fn dogrula_test() {
        let senaryolar: Vec<(usize, usize)> = vec![(3, 2), (5, 3), (8, 4), (4, 6), (10, 2), (6, 5)];
        for (kat, w) in senaryolar {
            let mut g = Graph::devnet(NET);
            let gen = signed(1, vec![], 1000, b"gen");
            let gid = *gen.id();
            g.insert_synced(gen).unwrap();
            let mut inc = Ghostdag::new_incremental(DEFAULT_K);
            inc.update_one(&g, &gid);
            let mut prev = vec![gid];
            let mut ts = 1001u64;
            for _k in 0..kat {
                let mut bu = Vec::new();
                let mut parents = prev.clone();
                parents.sort_unstable();
                for j in 0..w {
                    let v = signed((j + 1) as u8, parents.clone(), ts, b"x");
                    ts += 1;
                    let vid = *v.id();
                    g.insert_synced(v).unwrap();
                    inc.update_one(&g, &vid);
                    bu.push(vid);
                }
                prev = bu;
            }
            let refr = Ghostdag::compute_default(&g);
            let d_inc = data_map_of(&inc, &g);
            let d_ref = data_map_of(&refr, &g);
            let mut fark = 0;
            for (id, dref) in &d_ref {
                let dinc = d_inc.get(id).expect("eksik");
                if dinc.blue_score != dref.blue_score || dinc.mergeset_blues != dref.mergeset_blues {
                    fark += 1;
                }
            }
            eprintln!("kat={} w={}: fark={}", kat, w, fark);
            assert_eq!(fark, 0, "kat={} w={} fark var", kat, w);
        }
    }

    #[test]
    #[ignore]
    fn torba_stres() {
        use std::time::Instant;
        let olcekler: Vec<usize> = std::env::var("TORBA_N").ok()
            .map(|x| x.split(',').filter_map(|t| t.trim().parse().ok()).collect())
            .unwrap_or_else(|| vec![20_000usize, 40_000, 80_000, 160_000]);
        for &n in &olcekler {
            let mut g = Graph::devnet(NET);
            let gen = signed(1, vec![], 1000, b"gen");
            let gid = *gen.id();
            g.insert_synced(gen).unwrap();
            let mut ids = vec![gid];
            let mut ts = 1001u64;
            let mut gd = Ghostdag::new_incremental(DEFAULT_K);
            gd.update_one(&g, &gid);
            let t_build = Instant::now();
            for i in 1..n {
                let mut parents = vec![ids[ids.len() - 1]];
                if ids.len() >= 2 { parents.push(ids[ids.len() - 2]); }
                parents.sort_unstable();
                parents.dedup();
                let _ts0 = Instant::now();
                let v = signed((i % 250) as u8 + 1, parents, ts, b"x");
                T_SIGN.fetch_add(_ts0.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
                ts += 1;
                let vid = *v.id();
                let _ti0 = Instant::now();
                g.insert_synced(v).unwrap();
                T_INSERT.fetch_add(_ti0.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
                let _tg0 = Instant::now();
                gd.update_one(&g, &vid);
                T_GD.fetch_add(_tg0.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
                ids.push(vid);
                if i % 10000 == 0 { let e = t_build.elapsed().as_secs_f64(); eprintln!("  [ingest] {}/{} t={:.1}s ({:.0} v/s)", i, n, e, i as f64 / e.max(1e-9)); }
            }
            let total_s = t_build.elapsed().as_secs_f64();
            let tps = n as f64 / total_s.max(1e-9);
            eprintln!("n={:>9} toplam={:.1}s TPS={:.0} (insert + artimli GHOSTDAG per-vertex = node gercek akisi)", n, total_s, tps);
            let ic = MB_INIT_SAY.load(std::sync::atomic::Ordering::Relaxed);
            let cc = MB_CAND_SAY.load(std::sync::atomic::Ordering::Relaxed);
            let ba = BAS_ADIM.load(std::sync::atomic::Ordering::Relaxed);
            let onz = ODA_NZ.load(std::sync::atomic::Ordering::Relaxed);
            let otop = ODA_TOP.load(std::sync::atomic::Ordering::Relaxed);
            let tb = T_BLUE.load(std::sync::atomic::Ordering::Relaxed);
            let tm = T_MERGE.load(std::sync::atomic::Ordering::Relaxed);
            let tcd = T_CAND.load(std::sync::atomic::Ordering::Relaxed);
            let tsg = T_SIGN.load(std::sync::atomic::Ordering::Relaxed);
            let tin = T_INSERT.load(std::sync::atomic::Ordering::Relaxed);
            let tgd = T_GD.load(std::sync::atomic::Ordering::Relaxed);
            let ua = U_ANTI.load(std::sync::atomic::Ordering::Relaxed);
            let ui = U_IV.load(std::sync::atomic::Ordering::Relaxed);
            let ut = U_TORBA.load(std::sync::atomic::Ordering::Relaxed);
            let uu = U_UP.load(std::sync::atomic::Ordering::Relaxed);
            let tboy = TORBA_BOY.load(std::sync::atomic::Ordering::Relaxed);
            let tsay = TORBA_SAY.load(std::sync::atomic::Ordering::Relaxed);
            eprintln!("   TORBA: ort_boyut={:.1} (toplam={} say={})", tboy as f64 / tsay.max(1) as f64, tboy, tsay);
            eprintln!("   UPDATE_ONE(ms): anti_kaydet={:.0} interval={:.0} torba={:.0} up={:.0}", ua as f64/1e6, ui as f64/1e6, ut as f64/1e6, uu as f64/1e6);
            eprintln!("   ANA(ms): imza_uretim={:.0} insert+dogrula={:.0} ghostdag={:.0}", tsg as f64/1e6, tin as f64/1e6, tgd as f64/1e6);
            eprintln!("   ZAMAN(ms): blue_set={:.0} mergeset={:.0} candidate={:.0}", tb as f64/1e6, tm as f64/1e6, tcd as f64/1e6);
            eprintln!("   ODA: sifir-olmayan={} toplam_blue={} (oran={:.3})", onz, otop, onz as f64 / otop.max(1) as f64);
            eprintln!("   PROFIL: baslangic_dongusu(a)={} cand_dongusu(b)={} blue_anticone_ADIM={} (saf_iliskisiz + sp-zinciri adimlari)", ic, cc, ba);
        }
    }

    #[test]
    #[ignore]
    fn imza_paralel_bench() {
        use std::time::Instant;
        use rayon::prelude::*;
        let n: usize = std::env::var("IMZA_N").ok().and_then(|x| x.parse().ok()).unwrap_or(100_000);
        // n vertex uret (imzali)
        let mut vs = Vec::with_capacity(n);
        let mut ts = 1000u64;
        for i in 0..n {
            vs.push(signed((i % 250) as u8 + 1, vec![], ts, b"x"));
            ts += 1;
        }
        // TEK TEK verify
        let t0 = Instant::now();
        for v in &vs { v.verify().unwrap(); }
        let tek = t0.elapsed().as_secs_f64();
        // PARALEL verify (rayon, 18 cekirdek)
        let t1 = Instant::now();
        vs.par_iter().for_each(|v| { v.verify().unwrap(); });
        let par = t1.elapsed().as_secs_f64();
        eprintln!("IMZA n={} TEK={:.2}s ({:.0}/s) PARALEL={:.2}s ({:.0}/s) HIZLANMA={:.1}x",
            n, tek, n as f64/tek, par, n as f64/par, tek/par);
    }


    #[test]
    #[ignore]
    fn budama_hiz_olcumu() {
        // Ayni paralel DAG'da bridged (budamali) vs rec (budamasiz) cagri sayisi.
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"gen");
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();
        let mut prev = vec![gid];
        let mut ts = 1001u64;
        let mut tum_ids = vec![gid];
        for _k in 0..25 {
            let mut bu = Vec::new();
            let mut parents = prev.clone();
            parents.sort_unstable();
            for j in 0..4 {
                let v = signed(j + 1, parents.clone(), ts, b"x");
                ts += 1;
                let vid = *v.id();
                g.insert_synced(v).unwrap();
                bu.push(vid);
                tum_ids.push(vid);
            }
            prev = bu;
        }
        let gd = Ghostdag::compute_default(&g);
        let data = data_map_of(&gd, &g);
        let iv = sp_tree_intervals(&data);
        let bridges = bridge_lists(&g, &data);

        // BRIDGED (budamali)
        BRIDGED_CALLS.store(0, AtOrd::Relaxed);
        let ri_b = ReachIndex {
            iv: &iv,
            bridges: Some(&bridges),
            torba: None,
        };
        for a in &tum_ids {
            for b in &tum_ids {
                let mut memo = BTreeMap::new();
                let _ = ri_b.is_ancestor_bridged(&g, a, b, &mut memo);
            }
        }
        let bridged = BRIDGED_CALLS.load(AtOrd::Relaxed);

        // REC (budamasiz)
        REC_CALLS.store(0, AtOrd::Relaxed);
        let ri_r = ReachIndex {
            iv: &iv,
            bridges: None,
            torba: None,
        };
        for a in &tum_ids {
            for b in &tum_ids {
                let mut memo = BTreeMap::new();
                let _ = ri_r.is_ancestor_rec(&g, a, b, &mut memo);
            }
        }
        let rec = REC_CALLS.load(AtOrd::Relaxed);

        eprintln!(
            "BUDAMA_OLCUM vertex={} bridged_cagri={} rec_cagri={} oran={:.2}x",
            tum_ids.len(),
            bridged,
            rec,
            rec as f64 / bridged.max(1) as f64
        );

        let topo = super::topological_order(&g);
        let torba = torba_hesapla(&g, &data, &topo, &iv);
        let max_torba = torba.values().map(|b| b.len()).max().unwrap_or(0);
        let ort_torba: f64 =
            torba.values().map(|b| b.len()).sum::<usize>() as f64 / torba.len().max(1) as f64;
        let ri_t = ReachIndex {
            iv: &iv,
            bridges: None,
            torba: Some(&torba),
        };
        let t0 = std::time::Instant::now();
        let mut dogru_say = 0u64;
        for a in &tum_ids {
            for b in &tum_ids {
                if ri_t.is_ancestor_torba(a, b) {
                    dogru_say += 1;
                }
            }
        }
        let torba_sure = t0.elapsed();
        let ri_r2 = ReachIndex {
            iv: &iv,
            bridges: None,
            torba: None,
        };
        let t1 = std::time::Instant::now();
        for a in &tum_ids {
            for b in &tum_ids {
                let mut memo = BTreeMap::new();
                let _ = ri_r2.is_ancestor_rec(&g, a, b, &mut memo);
            }
        }
        let rec_sure = t1.elapsed();
        eprintln!(
            "TORBA_OLCUM vertex={} max_torba={} ort_torba={:.2} torba_sure={:?} rec_sure={:?} hizlanma={:.1}x dogru={}",
            tum_ids.len(),
            max_torba,
            ort_torba,
            torba_sure,
            rec_sure,
            rec_sure.as_secs_f64() / torba_sure.as_secs_f64().max(1e-9),
            dogru_say
        );
    }

    #[test]
    #[ignore]
    fn ata_bul_up_eski_chain_ile_birebir() {
        // ata_bul_up (binary lifting), eski blok-blok chain yuruyusunun KRITIK 1'de
        // durdugu ata'nin AYNISINI buluyor mu? Cesitli DAG'larda bit-bit.
        let senaryolar: Vec<(usize, usize)> = vec![(3, 2), (5, 3), (6, 5), (10, 2), (12, 3)];
        for (kat, w) in senaryolar {
            let mut g = Graph::devnet(NET);
            let gen = signed(1, vec![], 1000, b"gen");
            let gid = *gen.id();
            g.insert_synced(gen).unwrap();
            let mut prev = vec![gid];
            let mut ts = 1001u64;
            for _r in 0..kat {
                let mut bu = Vec::new();
                let mut parents = prev.clone();
                parents.sort_unstable();
                for j in 0..w {
                    let v = signed((j % 250) as u8 + 1, parents.clone(), ts, b"x");
                    ts += 1;
                    let vid = *v.id();
                    g.insert_synced(v).unwrap();
                    bu.push(vid);
                }
                prev = bu;
            }
            // Inkremental Ghostdag (update yolu) ile up tablosunu kur.
            let mut gd = Ghostdag::new_incremental(DEFAULT_K);
            gd.update(&g);
            let data = data_map_of(&gd, &g);
            let iv = sp_tree_intervals(&data);
            let topo = super::topological_order(&g);
            let torba = torba_hesapla(&g, &data, &topo, &iv);
            let ri = ReachIndex {
                iv: &iv,
                bridges: None,
                torba: Some(&torba),
            };

            for v in &topo {
                let gdata = match data.get(v) {
                    Some(d) => d,
                    None => continue,
                };
                let sp = match gdata.selected_parent {
                    Some(s) => s,
                    None => continue,
                };
                let mergeset = ri.mergeset_of(&g, v, &sp);
                let ordered = topo_order_subset(&g, &mergeset);
                for cand in &ordered {
                    // ESKI: blok blok chain yuru, cand'in atasi olan ilk blogu bul.
                    let mut eski_ata = None;
                    let mut cur = Some(sp);
                    while let Some(cb) = cur {
                        if ri.atalik(&g, &cb, cand) {
                            eski_ata = Some(cb);
                            break;
                        }
                        cur = data.get(&cb).and_then(|d| d.selected_parent);
                    }
                    // YENI: ata_bul_up.
                    let yeni_ata = ata_bul_up(&sp, cand, gd.up_ref(), &ri, &g);
                    assert_eq!(
                        eski_ata, yeni_ata,
                        "ata farkli kat={kat} w={w} cand={cand:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn coloring_kaspa_genis_birebir() {
        // coloring_kaspa'yi COK CESITLI DAG'da eski renklendirme ile kiyasla.
        // Tek desen yeterli kanit degil (konsensus kalbi) -> diamond, zincir, yogun,
        // seyrek, farkli genislik. Hepsinde mergeset_blues+reds BIREBIR olmali.
        let senaryolar: Vec<(usize, usize)> = vec![
            (3, 2), // dar paralel
            (5, 3),
            (6, 5),  // genis paralel
            (10, 2), // uzun seyrek
            (4, 4),
            (12, 3),
        ];
        for (kat, w) in senaryolar {
            let mut g = Graph::devnet(NET);
            let gen = signed(1, vec![], 1000, b"gen");
            let gid = *gen.id();
            g.insert_synced(gen).unwrap();
            let mut prev = vec![gid];
            let mut ts = 1001u64;
            for _r in 0..kat {
                let mut bu = Vec::new();
                let mut parents = prev.clone();
                parents.sort_unstable();
                for j in 0..w {
                    let v = signed((j % 250) as u8 + 1, parents.clone(), ts, b"x");
                    ts += 1;
                    let vid = *v.id();
                    g.insert_synced(v).unwrap();
                    bu.push(vid);
                }
                prev = bu;
            }
            let gd = Ghostdag::compute_default(&g);
            let data = data_map_of(&gd, &g);
            let iv = sp_tree_intervals(&data);
            let topo = super::topological_order(&g);
            let torba = torba_hesapla(&g, &data, &topo, &iv);
            let ri = ReachIndex {
                iv: &iv,
                bridges: None,
                torba: Some(&torba),
            };
            let mut anti: BTreeMap<VertexId, BTreeMap<VertexId, u32>> = BTreeMap::new();
            for v in &topo {
                let gdata = match data.get(v) {
                    Some(d) => d,
                    None => continue,
                };
                let sp = match gdata.selected_parent {
                    Some(s) => s,
                    None => {
                        anti.insert(*v, BTreeMap::new());
                        continue;
                    }
                };
                let (blues, reds, out) = coloring_kaspa(&g, v, &sp, gd.k(), &data, &ri, &anti);
                anti.insert(*v, out);
                assert_eq!(
                    blues, gdata.mergeset_blues,
                    "blues farkli kat={kat} w={w} v={v:?}"
                );
                assert_eq!(
                    reds, gdata.mergeset_reds,
                    "reds farkli kat={kat} w={w} v={v:?}"
                );
            }
        }
    }

    #[test]
    fn coloring_kaspa_eski_ile_birebir() {
        // coloring_kaspa (sp-zinciri yuruyusu), eski compute_vertex_data renklendirmesi
        // ile BIREBIR mi? mergeset_blues + mergeset_reds esit olmali. Paralel DAG + zincir.
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"gen");
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();
        let mut prev = vec![gid];
        let mut ts = 1001u64;
        for _k in 0..8 {
            let mut bu = Vec::new();
            let mut parents = prev.clone();
            parents.sort_unstable();
            for j in 0..4 {
                let v = signed(j + 1, parents.clone(), ts, b"x");
                ts += 1;
                let vid = *v.id();
                g.insert_synced(v).unwrap();
                bu.push(vid);
            }
            prev = bu;
        }
        // Eski yol: tam compute (her vertex icin GhostdagData).
        let gd = Ghostdag::compute_default(&g);
        let data = data_map_of(&gd, &g);
        let iv = sp_tree_intervals(&data);
        let topo = super::topological_order(&g);
        let torba = torba_hesapla(&g, &data, &topo, &iv);
        let ri = ReachIndex {
            iv: &iv,
            bridges: None,
            torba: Some(&torba),
        };

        // Her vertex icin coloring_kaspa'yi cagir, eski GhostdagData ile kiyasla.
        // anticone_sizes'i topo sirada kademeli kur (her vertex kendi out'unu ekler).
        let mut anti: BTreeMap<VertexId, BTreeMap<VertexId, u32>> = BTreeMap::new();
        for v in &topo {
            let gdata = match data.get(v) {
                Some(d) => d,
                None => continue,
            };
            let sp = match gdata.selected_parent {
                Some(s) => s,
                None => {
                    anti.insert(*v, BTreeMap::new());
                    continue;
                }
            };
            let (blues, reds, out) = coloring_kaspa(&g, v, &sp, gd.k(), &data, &ri, &anti);
            anti.insert(*v, out);
            // Eski ile kiyasla (sp haric mergeset_blues; eski mergeset_blues sp'siz).
            assert_eq!(
                blues, gdata.mergeset_blues,
                "mergeset_blues farkli: v={v:?}"
            );
            assert_eq!(reds, gdata.mergeset_reds, "mergeset_reds farkli: v={v:?}");
        }
    }

    #[test]
    fn sikistir_hizli_referans_ile_birebir() {
        // sikistir_hizli, sikistir_referans (O(t^2)) ile AYNI kume mi?
        // Gercek DAG'lardan torba elemanlari + iv uret, ikisini karsilastir.
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"gen");
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();
        let mut prev = vec![gid];
        let mut ts = 1001u64;
        for _k in 0..10 {
            let mut bu = Vec::new();
            let mut parents = prev.clone();
            parents.sort_unstable();
            for j in 0..4 {
                let v = signed(j + 1, parents.clone(), ts, b"x");
                ts += 1;
                let vid = *v.id();
                g.insert_synced(v).unwrap();
                bu.push(vid);
            }
            prev = bu;
        }
        let gd = Ghostdag::compute_default(&g);
        let data = data_map_of(&gd, &g);
        let iv = sp_tree_intervals(&data);
        // Her vertex'in (sikistirma ONCESI) torba elemanlarini uret: sp-miras + kopruler.
        let topo = super::topological_order(&g);
        let mut torba_ham: BTreeMap<VertexId, BTreeSet<VertexId>> = BTreeMap::new();
        for v in &topo {
            let mut bag: BTreeSet<VertexId> = BTreeSet::new();
            if let Some(g_) = data.get(v) {
                if let Some(sp) = g_.selected_parent {
                    if let Some(b) = torba_ham.get(&sp) {
                        bag.extend(b.iter().copied());
                    }
                }
            }
            let sp = data.get(v).and_then(|g_| g_.selected_parent);
            if let Some(vx) = g.get(v) {
                for pp in vx.parents() {
                    if g.contains(pp) && Some(*pp) != sp {
                        bag.insert(*pp);
                    }
                }
            }
            // KARSILASTIR: iki sikistirma ayni atilacak kumeyi vermeli.
            let elemanlar: Vec<VertexId> = bag.iter().copied().collect();
            let a_ref = sikistir_referans(&elemanlar, &iv);
            let a_hiz = sikistir_hizli(&elemanlar, &iv);
            assert_eq!(
                a_ref, a_hiz,
                "sikistir farkli! v={v:?} elemanlar={elemanlar:?}"
            );
            // referans ile sikistirilmis bag'i miras icin sakla.
            for a in &a_ref {
                bag.remove(a);
            }
            torba_ham.insert(*v, bag);
        }
    }

    #[test]
    fn is_ancestor_torba_past_ile_birebir() {
        // TORBA-tabanli atalik, past() ile AYNI olmali. Paralel DAG + zincir + diamond.
        // Torba modelinin DOGRULUK sinavi (miras + tek-gecis sorgu dogru mu?).
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"gen");
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();
        let mut prev = vec![gid];
        let mut ts = 1001u64;
        let mut tum_ids = vec![gid];
        for _k in 0..8 {
            let mut bu = Vec::new();
            let mut parents = prev.clone();
            parents.sort_unstable();
            for j in 0..4 {
                let v = signed(j + 1, parents.clone(), ts, b"x");
                ts += 1;
                let vid = *v.id();
                g.insert_synced(v).unwrap();
                bu.push(vid);
                tum_ids.push(vid);
            }
            prev = bu;
        }
        let cases: Vec<(Graph, Vec<VertexId>)> = vec![
            (g, tum_ids),
            {
                let (g2, ids) = linear_chain(7, 1);
                (g2, ids)
            },
            {
                let (g2, [a, b, c, d]) = diamond();
                (g2, vec![a, b, c, d])
            },
        ];
        for (g, ids) in cases {
            let gd = Ghostdag::compute_default(&g);
            let data = data_map_of(&gd, &g);
            let iv = sp_tree_intervals(&data);
            // Topo sira: sp_tree_intervals ile ayni DFS sirasi yerine, basit
            // topolojik sira (parent once). topological_order kullan.
            let topo = super::topological_order(&g);
            let torba = torba_hesapla(&g, &data, &topo, &iv);
            let ri = ReachIndex {
                iv: &iv,
                bridges: None,
                torba: Some(&torba),
            };
            for a in &ids {
                for b in &ids {
                    let torba_sonuc = ri.is_ancestor_torba(a, b);
                    let gercek = super::past(&g, b).contains(a) && a != b;
                    assert_eq!(
                        torba_sonuc, gercek,
                        "torba atalik yanlis: a={a:?} b={b:?} (torba={torba_sonuc} gercek={gercek})"
                    );
                }
            }
        }
    }

    #[test]
    fn is_ancestor_bridged_past_ile_birebir() {
        // KOPRU-destekli atalik, mevcut past() ile AYNI olmali. Zincir + diamond +
        // COK-KATLI PARALEL DAG (asil hedef ortam). Yanlis atalik = sessiz
        // konsensus hatasi, o yuzden cok sayida (a,b) cifti taranir.
        // Cok-katli paralel DAG kur:
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"gen");
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();
        let mut prev = vec![gid];
        let mut ts = 1001u64;
        let mut tum_ids = vec![gid];
        for _k in 0..8 {
            let mut bu = Vec::new();
            let mut parents = prev.clone();
            parents.sort_unstable();
            for j in 0..4 {
                let v = signed(j + 1, parents.clone(), ts, b"x");
                ts += 1;
                let vid = *v.id();
                g.insert_synced(v).unwrap();
                bu.push(vid);
                tum_ids.push(vid);
            }
            prev = bu;
        }
        // Ayrica zincir + diamond da ekle (kucuk, ek guvence).
        let cases: Vec<(Graph, Vec<VertexId>)> = vec![
            (g, tum_ids),
            {
                let (g2, ids) = linear_chain(7, 1);
                (g2, ids)
            },
            {
                let (g2, [a, b, c, d]) = diamond();
                (g2, vec![a, b, c, d])
            },
        ];
        for (g, ids) in cases {
            let gd = Ghostdag::compute_default(&g);
            let data = data_map_of(&gd, &g);
            let iv = sp_tree_intervals(&data);
            let bridges = bridge_lists(&g, &data);
            let ri = ReachIndex {
                iv: &iv,
                bridges: Some(&bridges),
                torba: None,
            };
            for a in &ids {
                for b in &ids {
                    let mut memo = BTreeMap::new();
                    let bridged = ri.is_ancestor_bridged(&g, a, b, &mut memo);
                    let gercek = super::past(&g, b).contains(a) && a != b;
                    assert_eq!(
                        bridged, gercek,
                        "bridged atalik yanlis: a={a:?} b={b:?} (bridged={bridged} gercek={gercek})"
                    );
                }
            }
        }
    }

    #[test]
    fn mergeset_of_eski_yontemle_birebir() {
        // mergeset_of, mevcut past_b \\ past_sp \\ {sp} ile AYNI kume olmali.
        // Mergeset yanlissa TUM GHOSTDAG bozulur -> bu test hayati.
        let cases: Vec<(Graph, Vec<VertexId>)> = vec![
            {
                let (g, ids) = linear_chain(7, 1);
                (g, ids)
            },
            {
                let (g, [a, b, c, d]) = diamond();
                (g, vec![a, b, c, d])
            },
        ];
        for (g, ids) in cases {
            let gd = Ghostdag::compute_default(&g);
            let data = data_map_of(&gd, &g);
            let iv = sp_tree_intervals(&data);
            let ri = ReachIndex {
                iv: &iv,
                bridges: None,
                torba: None,
            };
            for id in &ids {
                // sp'yi gercek veriden al (genesis'in sp'si yok -> atla).
                let sp = match data.get(id).and_then(|d| d.selected_parent) {
                    Some(sp) => sp,
                    None => continue,
                };
                // YENI: mergeset_of
                let yeni = ri.mergeset_of(&g, id, &sp);
                // ESKI: past_b \ past_sp \ {sp}
                let past_b = super::past(&g, id);
                let past_sp = super::past(&g, &sp);
                let eski: BTreeSet<VertexId> = past_b
                    .iter()
                    .filter(|x| **x != sp && !past_sp.contains(*x))
                    .copied()
                    .collect();
                assert_eq!(yeni, eski, "mergeset farkli: id={id:?} sp={sp:?}");
            }
        }
    }

    #[test]
    fn is_ancestor_rec_past_ile_birebir() {
        // Set-saklamayan ozyinelemeli atalik, mevcut past() ile AYNI olmali.
        let cases: Vec<(Graph, Vec<VertexId>)> = vec![
            {
                let (g, ids) = linear_chain(7, 1);
                (g, ids)
            },
            {
                let (g, [a, b, c, d]) = diamond();
                (g, vec![a, b, c, d])
            },
        ];
        for (g, ids) in cases {
            let gd = Ghostdag::compute_default(&g);
            let data = data_map_of(&gd, &g);
            let iv = sp_tree_intervals(&data);
            let ri = ReachIndex {
                iv: &iv,
                bridges: None,
                torba: None,
            };
            for a in &ids {
                for b in &ids {
                    let mut memo = BTreeMap::new();
                    let rec = ri.is_ancestor_rec(&g, a, b, &mut memo);
                    let gercek = super::past(&g, b).contains(a) && a != b;
                    assert_eq!(rec, gercek, "rec atalik yanlis: a={a:?} b={b:?}");
                }
            }
        }
    }

    #[test]
    fn inkremental_iv_atalik_dogru() {
        // update ile INKREMENTAL kurulan iv, gercek sp-agac atalik ile birebir
        // olmali. Tek tek ekleyerek (her biri ayri update) inkremental yolu zorla.
        // Cesitli sekiller: zincir + diamond.
        let cases: Vec<(Graph, Vec<VertexId>)> = vec![
            {
                let (g, ids) = linear_chain(10, 1);
                (g, ids)
            },
            {
                let (g, [a, b, c, d]) = diamond();
                (g, vec![a, b, c, d])
            },
        ];
        for (g, ids) in cases {
            // Inkremental kur: tek update (topo sira ile hepsi islenir).
            let mut gd = Ghostdag::new_incremental(DEFAULT_K);
            gd.update(&g);
            // Referans: ayni data'dan gercek sp-agac atalik.
            let data = data_map_of(&gd, &g);
            for a in &ids {
                for b in &ids {
                    let (sa, ea) = gd.iv[a];
                    let (sb, eb) = gd.iv[b];
                    let iv_der = a != b && sa <= sb && eb <= ea;
                    let gercek = is_sp_ancestor(&data, a, b);
                    assert_eq!(
                        iv_der, gercek,
                        "inkremental iv atalik yanlis: a={a:?} b={b:?} iv_a=({sa},{ea}) iv_b=({sb},{eb})"
                    );
                }
            }
        }
    }

    #[test]
    fn gapped_intervals_ayni_atalik() {
        // Bosluklu sema, eski sp_tree_intervals ile AYNI atalik cevabi vermeli
        // (interval DEGERLERI farkli olabilir; ATALIK ILISKISI ayni olmali).
        for (g, _) in [linear_chain(8, 1), linear_chain(4, 2)] {
            let gd = Ghostdag::compute_default(&g);
            let data = data_map_of(&gd, &g);
            let gap = sp_tree_intervals_gapped(&data);
            let ids: Vec<VertexId> = g.ids().copied().collect();
            for a in &ids {
                for b in &ids {
                    let (sa, ea) = gap[a];
                    let (sb, eb) = gap[b];
                    let gap_der = a != b && sa <= sb && eb <= ea;
                    // gercek: sp-zincirini yuru.
                    let gercek = is_sp_ancestor(&data, a, b);
                    assert_eq!(gap_der, gercek, "gapped atalik yanlis: a={a:?} b={b:?}");
                }
            }
        }
    }

    #[test]
    fn gapped_intervals_diamond() {
        let (g, [a, b, c, d]) = diamond();
        let gd = Ghostdag::compute_default(&g);
        let data = data_map_of(&gd, &g);
        let gap = sp_tree_intervals_gapped(&data);
        for x in [a, b, c, d] {
            for y in [a, b, c, d] {
                let (sx, ex) = gap[&x];
                let (sy, ey) = gap[&y];
                let gap_der = x != y && sx <= sy && ey <= ex;
                let gercek = is_sp_ancestor(&data, &x, &y);
                assert_eq!(gap_der, gercek, "gapped diamond yanlis: x={x:?} y={y:?}");
            }
        }
    }

    #[test]
    fn sp_tree_intervals_dogru_ata_kontrolu() {
        for (g, _) in [linear_chain(6, 1), linear_chain(3, 2)] {
            let gd = Ghostdag::compute_default(&g);
            let data = data_map_of(&gd, &g);
            let iv = sp_tree_intervals(&data);
            let ids: Vec<VertexId> = g.ids().copied().collect();
            for a in &ids {
                for b in &ids {
                    let (sa, ea) = iv[a];
                    let (sb, eb) = iv[b];
                    // interval kurali: A, B'nin atasi <=> sa <= sb && eb <= ea (ve a!=b)
                    let interval_der = a != b && sa <= sb && eb <= ea;
                    let gercek = is_sp_ancestor(&data, a, b);
                    assert_eq!(
                        interval_der, gercek,
                        "interval ata-kontrolu yanlis: a={a:?} b={b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn diamond_sp_tree_intervals_dogru() {
        let (g, [a, b, c, d]) = diamond();
        let gd = Ghostdag::compute_default(&g);
        let data = data_map_of(&gd, &g);
        let iv = sp_tree_intervals(&data);
        // d'nin sp-agac atalari: sp(d) ve onun zinciri (a). diger paralel (b veya c)
        // sp-agacinda ata DEGIL.
        for x in [a, b, c, d] {
            for y in [a, b, c, d] {
                let (sx, ex) = iv[&x];
                let (sy, ey) = iv[&y];
                let interval_der = x != y && sx <= sy && ey <= ex;
                let gercek = is_sp_ancestor(&data, &x, &y);
                assert_eq!(
                    interval_der, gercek,
                    "diamond interval yanlis: x={x:?} y={y:?}"
                );
            }
        }
    }

    #[test]
    fn linear_chain_increments_blue_score() {
        let (g, ids) = linear_chain(5, 1);
        let gd = Ghostdag::compute_default(&g);
        // doğrusal: her adım +1 (mergeset boş, sadece seçili ebeveyn).
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(gd.blue_score(id), Some(i as u64), "blok {i}");
        }
        // her bloğun seçili ebeveyni bir öncekidir.
        for i in 1..ids.len() {
            assert_eq!(gd.selected_parent(&ids[i]), Some(ids[i - 1]));
        }
    }

    #[test]
    fn linear_chain_total_order_is_the_chain() {
        let (g, ids) = linear_chain(5, 1);
        let gd = Ghostdag::compute_default(&g);
        assert_eq!(gd.total_order(&g), ids);
        assert_eq!(gd.selected_tip(&g), Some(ids[4]));
    }

    #[test]
    fn diamond_merges_parallel_block_as_blue_with_default_k() {
        let (g, [a, b, c, d]) = diamond();
        let gd = Ghostdag::compute_default(&g); // k=18 ≫ 1
                                                // A score 0; B,C score 1; D seçili ebeveyni min(B,C).
        assert_eq!(gd.blue_score(&a), Some(0));
        assert_eq!(gd.blue_score(&b), Some(1));
        assert_eq!(gd.blue_score(&c), Some(1));
        let sp = gd.selected_parent(&d).unwrap();
        assert_eq!(sp, b.min(c)); // beraberlik min-id
                                  // diğer paralel blok mavi mergeset'te → D score = 1 + 1(sp) + 1 = 3.
        assert_eq!(gd.blue_score(&d), Some(3));
        let dd = gd.data(&d).unwrap();
        assert_eq!(dd.mergeset_blues, vec![b.max(c)]);
        assert!(dd.mergeset_reds.is_empty());
    }

    #[test]
    fn diamond_with_k_zero_paints_parallel_block_red() {
        let (g, [_a, b, c, d]) = diamond();
        let gd = Ghostdag::compute(&g, 0); // k=0: anticone>0 → kırmızı
        let sp = gd.selected_parent(&d).unwrap();
        assert_eq!(sp, b.min(c));
        // paralel blok (anticone size 1 > 0) kırmızı → D score = 1 + 1 + 0 = 2.
        assert_eq!(gd.blue_score(&d), Some(2));
        let dd = gd.data(&d).unwrap();
        assert!(dd.mergeset_blues.is_empty());
        assert_eq!(dd.mergeset_reds, vec![b.max(c)]);
    }

    #[test]
    fn total_order_is_valid_topological_order() {
        let (g, [a, b, c, d]) = diamond();
        let gd = Ghostdag::compute_default(&g);
        let order = gd.total_order(&g);
        assert_eq!(order.len(), 4);
        let pos = |x: &VertexId| order.iter().position(|y| y == x).unwrap();
        // A herkesten önce; B,C, D'den önce; D en son.
        assert!(pos(&a) < pos(&b) && pos(&a) < pos(&c));
        assert!(pos(&b) < pos(&d) && pos(&c) < pos(&d));
        assert_eq!(order[0], a);
        assert_eq!(order[3], d);
    }

    #[test]
    fn ghostdag_is_deterministic_across_insertion_order() {
        // Aynı vertex kümesi, farklı ekleme sırası → AYNI renk + score + sıra.
        let a = signed(1, vec![], 1000, b"a");
        let aid = *a.id();
        let b = signed(1, vec![aid], 1001, b"b");
        let c = signed(2, vec![aid], 1001, b"c");
        let bid = *b.id();
        let cid = *c.id();
        let d = signed(1, vec![bid, cid], 1002, b"d");

        let mut g1 = Graph::devnet(NET);
        g1.insert_synced(a.clone()).unwrap();
        g1.insert_synced(b.clone()).unwrap();
        g1.insert_synced(c.clone()).unwrap();
        g1.insert_synced(d.clone()).unwrap();

        let mut g2 = Graph::devnet(NET);
        g2.insert_synced(a).unwrap();
        g2.insert_synced(c).unwrap(); // c önce
        g2.insert_synced(b).unwrap();
        g2.insert_synced(d).unwrap();

        let gd1 = Ghostdag::compute_default(&g1);
        let gd2 = Ghostdag::compute_default(&g2);

        for id in [aid, bid, cid, *gd1.selected_tip(&g1).as_ref().unwrap()] {
            assert_eq!(gd1.data(&id), gd2.data(&id));
        }
        assert_eq!(gd1.total_order(&g1), gd2.total_order(&g2));
    }

    #[test]
    fn total_order_places_blue_mergeset_before_red_same_rank() {
        // YB1: G'den 4 paralel blok (hepsi aynı rank=0, karşılıklı anticone).
        // D hepsini birleştirir. k=1 → sp + 1 mavi, kalan 2 kırmızı. total_order
        // mavi mergeset bloğunu kırmızılardan ÖNCE koymalı (id'den bağımsız).
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"g");
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();
        let mut pids = Vec::new();
        for i in 0..4u8 {
            let p = signed(10 + i, vec![gid], 1001, format!("p{i}").as_bytes());
            pids.push(*p.id());
            g.insert_synced(p).unwrap();
        }
        let d = signed(1, pids.clone(), 1002, b"d");
        let did = *d.id();
        g.insert_synced(d).unwrap();

        let gd = Ghostdag::compute(&g, 1);
        let dd = gd.data(&did).unwrap();
        // sp seçili; mergeset = kalan 3; k=1 → tam 1 mavi, 2 kırmızı.
        assert_eq!(dd.mergeset_blues.len(), 1, "k=1: tam bir mavi merge");
        assert_eq!(dd.mergeset_reds.len(), 2, "k=1: kalan ikisi kırmızı");

        let order = gd.total_order(&g);
        let pos = |x: &VertexId| order.iter().position(|y| y == x).unwrap();
        let blue = dd.mergeset_blues[0];
        for red in &dd.mergeset_reds {
            assert!(
                pos(&blue) < pos(red),
                "mavi mergeset bloğu her kırmızıdan önce gelmeli (YB1)"
            );
        }
        // hâlâ geçerli topolojik sıra: G önce, D en son.
        assert_eq!(order[0], gid);
        assert_eq!(*order.last().unwrap(), did);
    }

    #[test]
    fn uniform_weight_makes_blue_work_equal_blue_score() {
        // UniformWeight (varsayılan) → her vertex'te blue_work == blue_score.
        let (g, ids) = linear_chain(5, 1);
        let gd = Ghostdag::compute_default(&g);
        for id in &ids {
            assert_eq!(gd.blue_work(id), gd.blue_score(id));
        }
        let (g2, [a, b, c, d]) = diamond();
        let gd2 = Ghostdag::compute_default(&g2);
        for id in [a, b, c, d] {
            assert_eq!(gd2.blue_work(&id), gd2.blue_score(&id));
        }
    }

    #[test]
    fn committee_weight_ignores_outsider_linear_chain() {
        // O-sys: komite dışı saldırgan gizli DOĞRUSAL mavi zincirle blue-work
        // şişiremez. Komite (seed 1) KISA zincir (h1→h2) vs komite-dışı (seed 9)
        // DAHA UZUN zincir (a1→a2→a3→a4). blue_score'da saldırgan önde ama tip
        // seçimi WORK'e göre → komite zinciri kazanır.
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"g"); // komite üyesi genesis
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();

        // komite (seed 1) iki blok: her biri work +1.
        let h1 = signed(1, vec![gid], 1001, b"h1");
        let h1id = *h1.id();
        g.insert_synced(h1).unwrap();
        let h2 = signed(1, vec![h1id], 1002, b"h2");
        let h2id = *h2.id();
        g.insert_synced(h2).unwrap();

        // komite-dışı (seed 9) DAHA UZUN zincir: a1..a4, hepsi work 0.
        let a1 = signed(9, vec![gid], 1001, b"a1");
        let a1id = *a1.id();
        g.insert_synced(a1).unwrap();
        let a2 = signed(9, vec![a1id], 1002, b"a2");
        let a2id = *a2.id();
        g.insert_synced(a2).unwrap();
        let a3 = signed(9, vec![a2id], 1003, b"a3");
        let a3id = *a3.id();
        g.insert_synced(a3).unwrap();
        let a4 = signed(9, vec![a3id], 1004, b"a4");
        let a4id = *a4.id();
        g.insert_synced(a4).unwrap();

        let committee = CommitteeWeight {
            members: [*g.get(&gid).unwrap().public_key()].into_iter().collect(),
        };
        let gd = Ghostdag::compute_with_weight(&g, DEFAULT_K, &committee);

        // work: h2 = weight(genesis)+weight(h1) = 2; a4 = weight(genesis)+0 = 1.
        assert_eq!(gd.blue_work(&h2id), Some(2));
        assert_eq!(gd.blue_work(&a4id), Some(1));
        // blue_score (sayım) saldırganda DAHA YÜKSEK (daha uzun zincir):
        assert!(gd.blue_score(&a4id).unwrap() > gd.blue_score(&h2id).unwrap());
        // ama tip seçimi WORK'e göre → komite zinciri kazanır (saldırgan
        // gizli doğrusal zincirle blue-work şişiremedi — O-sys).
        assert_eq!(gd.selected_tip(&g), Some(h2id));
    }

    #[test]
    fn higher_blue_score_tip_is_selected() {
        // İki tip: uzun zincir (yüksek score) vs kısa dal (düşük score).
        let mut g = Graph::devnet(NET);
        let gen = signed(1, vec![], 1000, b"g");
        let gid = *gen.id();
        g.insert_synced(gen).unwrap();
        // uzun kol: gid→l1→l2
        let l1 = signed(1, vec![gid], 1001, b"l1");
        let l1id = *l1.id();
        g.insert_synced(l1).unwrap();
        let l2 = signed(1, vec![l1id], 1002, b"l2");
        let l2id = *l2.id();
        g.insert_synced(l2).unwrap();
        // kısa kol: gid→s1
        let s1 = signed(2, vec![gid], 1001, b"s1");
        let s1id = *s1.id();
        g.insert_synced(s1).unwrap();

        let gd = Ghostdag::compute_default(&g);
        assert_eq!(gd.blue_score(&l2id), Some(2));
        assert_eq!(gd.blue_score(&s1id), Some(1));
        // seçili tip yüksek score → l2.
        assert_eq!(gd.selected_tip(&g), Some(l2id));
    }

    #[test]
    fn equivocation_fork_is_painted_red_under_small_k() {
        // Yazar 3 çatallıyor: g0'dan iki paralel (x,y). Sonra dürüst bir blok
        // z her ikisini birleştirir (parents x,y). k=0 → biri kırmızı olmalı
        // (anticone'ları birbiri → k-cluster bozulur).
        let mut g = Graph::devnet(NET);
        let g0 = signed(1, vec![], 1000, b"g0");
        let g0id = *g0.id();
        g.insert_synced(g0).unwrap();
        let x = signed(3, vec![g0id], 1001, b"x");
        let xid = *x.id();
        g.insert_synced(x).unwrap();
        let y = signed(3, vec![g0id], 1001, b"y");
        let yid = *y.id();
        g.insert_synced(y).unwrap();
        let z = signed(1, vec![xid, yid], 1002, b"z");
        let zid = *z.id();
        g.insert_synced(z).unwrap();

        let gd = Ghostdag::compute(&g, 0);
        // z'nin seçili ebeveyni min(x,y); diğeri (anticone size 1 > k=0) kırmızı.
        let dd = gd.data(&zid).unwrap();
        assert_eq!(dd.selected_parent, Some(xid.min(yid)));
        assert_eq!(dd.mergeset_reds, vec![xid.max(yid)]);
        assert!(dd.mergeset_blues.is_empty());
        // telemetri: equivocation tespit edilmiş olmalı (Adım 3a).
        let pk = g.get(&xid).unwrap().public_key();
        assert!(!super::super::equivocations_by(&g, pk).is_empty());
    }

    // ===== Adım 5: ARTIMLI GHOSTDAG differential testleri =====

    /// Çeşitli merge/red üretecek karmaşık DAG vertex'leri (oluşturma sırasında
    /// topo sıralı): gen → (h1,h2) → m1 → (a,b,c) → t.
    fn complex_dag_vertices() -> Vec<Vertex> {
        let gen = signed(1, vec![], 1000, b"gen");
        let g0 = *gen.id();
        let h1 = signed(1, vec![g0], 1001, b"h1");
        let h2 = signed(2, vec![g0], 1001, b"h2");
        let (h1id, h2id) = (*h1.id(), *h2.id());
        let m1 = signed(1, vec![h1id, h2id], 1002, b"m1");
        let m1id = *m1.id();
        let a = signed(1, vec![m1id], 1003, b"a");
        let b = signed(2, vec![m1id], 1003, b"b");
        let c = signed(3, vec![m1id], 1003, b"c");
        let (aid, bid, cid) = (*a.id(), *b.id(), *c.id());
        let t = signed(1, vec![aid, bid, cid], 1004, b"t");
        vec![gen, h1, h2, m1, a, b, c, t]
    }

    /// Verilen vertex'lerden GEÇERLİ bir topolojik ekleme sırası üret (ebeveynler
    /// daima çocuklardan önce); `rot` hazır adaylar arasında farklı seçim yaparak
    /// farklı geçerli sıralar verir (graph.insert ebeveyn-varlığı ister → sıra
    /// geçerli olmalı).
    fn valid_order(verts: &[Vertex], rot: usize) -> Vec<Vertex> {
        let mut remaining: Vec<Vertex> = verts.to_vec();
        let mut inserted: BTreeSet<VertexId> = BTreeSet::new();
        let mut out = Vec::new();
        while !remaining.is_empty() {
            let ready: Vec<usize> = remaining
                .iter()
                .enumerate()
                .filter(|(_, v)| v.parents().iter().all(|p| inserted.contains(p)))
                .map(|(i, _)| i)
                .collect();
            let pick = ready[rot % ready.len()];
            let v = remaining.remove(pick);
            inserted.insert(*v.id());
            out.push(v);
        }
        out
    }

    fn assert_same(inc: &Ghostdag, full: &Ghostdag, gi: &Graph, gf: &Graph) {
        for id in gf.ids() {
            assert_eq!(inc.data(id), full.data(id), "blok-başına veri uyuşmadı");
        }
        assert_eq!(
            inc.total_order(gi),
            full.total_order(gf),
            "total_order uyuşmadı"
        );
        assert_eq!(
            inc.selected_tip(gi),
            full.selected_tip(gf),
            "selected_tip uyuşmadı"
        );
    }

    #[test]
    fn incremental_equals_full_across_insertion_orders() {
        // EN KRİTİK differential test (denetçi değişmez #1): aynı graph, farklı
        // (geçerli) ekleme sıralarıyla artımlı işlendiğinde her vertex'in verisi +
        // total_order + selected_tip tam-compute() ile BİT-BİT aynı.
        let base = complex_dag_vertices();
        let mut gfull = Graph::devnet(NET);
        for v in &base {
            gfull.insert_synced(v.clone()).unwrap();
        }
        let full = Ghostdag::compute_default(&gfull);

        for rot in 0..6 {
            let order = valid_order(&base, rot);
            let mut g = Graph::devnet(NET);
            let mut inc = Ghostdag::new_incremental(DEFAULT_K);
            for v in order {
                g.insert_synced(v).unwrap();
                inc.update(&g); // her eklemeden sonra artımlı işle.
            }
            assert_same(&inc, &full, &g, &gfull);
        }
    }

    #[test]
    fn incremental_equals_full_with_committee_weight() {
        // Differential eşitlik ağırlıktan bağımsız olmalı: CommitteeWeight ile
        // artımlı == tam-compute (denetçi O-sys + değişmez #1).
        let base = complex_dag_vertices();
        // h1/a/t (seed1) ve b (seed2) komiteden; c (seed3) değil → ağırlık çeşitli.
        let members: BTreeSet<[u8; 32]> = [1u8, 2u8]
            .iter()
            .map(|s| key(*s).verifying_key().to_bytes())
            .collect();
        let weigher = CommitteeWeight { members };

        let mut gfull = Graph::devnet(NET);
        for v in &base {
            gfull.insert_synced(v.clone()).unwrap();
        }
        let full = Ghostdag::compute_with_weight(&gfull, DEFAULT_K, &weigher);

        for rot in 0..6 {
            let order = valid_order(&base, rot);
            let mut g = Graph::devnet(NET);
            let mut inc = Ghostdag::new_incremental(DEFAULT_K);
            for v in order {
                g.insert_synced(v).unwrap();
                inc.update_with_weight(&g, &weigher);
            }
            assert_same(&inc, &full, &g, &gfull);
        }
    }

    #[test]
    fn cached_data_never_mutates_after_future_merges() {
        // Değişmez #3: hesaplanan GhostdagData bir daha ASLA mutasyona uğramaz —
        // bir blok sonradan başka bloklarca merge edilse bile. Akış ortasında
        // alınan anlık görüntü, tüm DAG eklendikten sonra hâlâ aynı olmalı.
        let base = complex_dag_vertices();
        let snapshot_at = 3; // m1 eklendiğinde (sonradan a/b/c/t tarafından merge)
        let mut g = Graph::devnet(NET);
        let mut inc = Ghostdag::new_incremental(DEFAULT_K);
        let mut snap: Option<(VertexId, GhostdagData)> = None;
        for (i, v) in base.into_iter().enumerate() {
            let id = *v.id();
            g.insert_synced(v).unwrap();
            inc.update(&g);
            if i == snapshot_at {
                snap = Some((id, inc.data(&id).unwrap().clone()));
            }
        }
        let (sid, sdata) = snap.expect("anlık görüntü alındı");
        assert_eq!(
            inc.data(&sid).unwrap(),
            &sdata,
            "önbellekli veri sonradan değişti — değişmezlik ihlali"
        );
    }

    #[test]
    #[should_panic(expected = "farklı weigher")]
    fn mixing_weighers_on_incremental_cache_halts() {
        // KORUMA (architect bulgusu): aynı artımlı örnek önce Uniform sonra
        // Committee ile güncellenirse → fingerprint uyuşmaz → DURUR. Bu, eski
        // vertex'lerin eski ağırlıkta kalıp sessizce yanlış sonuç vermesini
        // (belirlenimcilik ihlali) engeller.
        let (g, _ids) = linear_chain(4, 1);
        let mut inc = Ghostdag::new_incremental(DEFAULT_K);
        inc.update(&g); // UniformWeight → fp sabitlenir.
        let members: BTreeSet<[u8; 32]> = [1u8]
            .iter()
            .map(|s| key(*s).verifying_key().to_bytes())
            .collect();
        inc.update_with_weight(&g, &CommitteeWeight { members }); // farklı → panik.
    }

    #[test]
    fn same_committee_weigher_reused_is_consistent() {
        // Aynı weigher (aynı üyelik) tekrar kullanılırsa fingerprint eşleşir →
        // panik yok, sonuç tam-compute ile aynı (false-positive guard kontrolü).
        let base = complex_dag_vertices();
        let members: BTreeSet<[u8; 32]> = [1u8, 2u8]
            .iter()
            .map(|s| key(*s).verifying_key().to_bytes())
            .collect();
        let weigher = CommitteeWeight { members };
        let mut gfull = Graph::devnet(NET);
        for v in &base {
            gfull.insert_synced(v.clone()).unwrap();
        }
        let full = Ghostdag::compute_with_weight(&gfull, DEFAULT_K, &weigher);
        let mut g = Graph::devnet(NET);
        let mut inc = Ghostdag::new_incremental(DEFAULT_K);
        for v in base {
            g.insert_synced(v).unwrap();
            inc.update_with_weight(&g, &weigher); // aynı weigher tekrar tekrar.
        }
        assert_same(&inc, &full, &g, &gfull);
    }

    #[test]
    fn update_is_idempotent_and_skips_cached() {
        // Değişmez #3 (önbellek): aynı graph üzerinde tekrar update çağırmak
        // hiçbir şeyi değiştirmez (zaten-hesaplanmış atlanır).
        let (g, _ids) = linear_chain(6, 1);
        let mut inc = Ghostdag::new_incremental(DEFAULT_K);
        inc.update(&g);
        let snapshot: Vec<GhostdagData> = g.ids().map(|id| inc.data(id).unwrap().clone()).collect();
        inc.update(&g); // tekrar — değişiklik olmamalı.
        inc.update(&g);
        let after: Vec<GhostdagData> = g.ids().map(|id| inc.data(id).unwrap().clone()).collect();
        assert_eq!(snapshot, after, "tekrar update veriyi değiştirdi");
        let full = Ghostdag::compute_default(&g);
        assert_same(&inc, &full, &g, &g);
    }
}
