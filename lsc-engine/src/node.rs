//! Node DAG durumu — `Graph` + `Ghostdag`'ı tek bir çalıştırılabilir düğüm
//! seviyesinde birleştirir. Ağ katmanı (`lsc-net`) bu yapıyı kullanır.
//!
//! Tasarım: bu modül AĞDAN bağımsızdır (async yok, libp2p yok). Saf, test
//! edilebilir DAG durumu + ingest sarmalayıcı. Ağ kodu bunu çağırır.
//!
//! Genesis politikası (devnet): graph BOŞ başlar (`genesis: None`); ilk
//! görülen parent'sız vertex genesis olur (FirstSeenDevnet). Yani yeni
//! düğüm "henüz genesis yok" durumundadır — bu dürüst/gerçek davranıştır.

use std::time::Duration;

use crate::consensus::ghostdag::{Ghostdag, DEFAULT_K};
use crate::dag::graph::Graph;
use crate::dag::orphan::OrphanPool;
use crate::dag::pipeline::{ingest_bytes, IngestError};
use crate::dag::vertex::{Vertex, VertexId};
use crate::dag::wire;

/// Bir LSC düğümünün DAG durumu: graf + ghostdag konsensüs durumu.
/// Ağ katmanından bağımsızdır (saf çekirdek).
pub struct NodeState {
    graph: Graph,
    ghostdag: Ghostdag,
    network_id: u32,
    /// Ebeveyni henuz gelmemis vertex'lerin bekleme havuzu.
    orphans: OrphanPool,
    /// KALKAN: kanonik token kayit defteri. Taklit token'lar (ayni sembol,
    /// farkli adres) buraya karsi kontrol edilip reddedilir.
    token_registry: crate::registry::TokenRegistry,
    /// STAKING: hangi adres ne kadar AIDAG kilitlemis (teminat defteri).
    /// Kalkan bagi (9d): sadece stake etmis adresler kanonik token kaydedebilir.
    stake_registry: crate::registry::StakeRegistry,

    /// Bakiye defteri: adres -> serbest AIDAG bakiyesi (transfer/odeme).
    /// Stake'ten ayri: stake KILITLI teminat, bakiye SERBEST/transfer edilebilir.
    bakiye_registry: crate::registry::BakiyeRegistry,

    /// LSC defteri: adres -> serbest LSC bakiyesi (yakit/gas coini). AIDAG ile
    /// AYNI saglam BakiyeRegistry kodu, ayri ornek (Yol C). Iki varlik ayri tutulur.
    lsc_registry: crate::registry::BakiyeRegistry,
    /// Replay korumasi defteri: adres -> beklenen nonce. DAG replay'i ile
    /// yeniden kurulur (diske ayrica persist edilmez).
    nonce_registry: crate::registry::NonceRegistry,

    /// Belge/veri dogrulama defteri: hash -> (kaydeden, zaman). Gercek dunya.
    record_registry: crate::registry::RecordRegistry,

    /// Kurum/firma kimlik defteri: adres -> (ad, kategori, zaman).
    kurum_registry: crate::registry::KurumRegistry,

    /// Testnet eslestirme: test_adresi -> gercek (mainnet odul) adresi. Bir kerelik.
    eslestirme_registry: crate::registry::EslestirmeRegistry,
    /// On satis dagitim defteri: odeme_ref -> kayit (cifte dagitim engeli, seffaflik).
    on_satis_registry: crate::registry::OnSatisRegistry,

    /// Faucet owner adresi. On-satis (tip=10) ve faucet (tip=6) yetki kapisi.
    /// Mainnet'te new_mainnet() PINLI kurucu adrese sabitler (env override YOK) ->
    /// tum node'larda AYNI (A2: owner-gating konsensus-deterministik).
    faucet_owner: Option<[u8; 20]>,
    /// Faucet CIFTE-DAMLA engeli: bir adrese bir kez.
    faucet_verildi: std::collections::HashSet<[u8; 20]>,
    /// MAINNET modu mu? new_mainnet()=true, new_devnet()=false. Deterministik launch
    /// konfigu (network_id ile korele). Faucet (tip=6, MINT) mainnet'te KAPALI ->
    /// 21M sabit AIDAG arzi korunur (yoktan bakiye basilmaz).
    mainnet: bool,

    /// AVM state: sozlesme kodu + storage (KALICI kaynak). LSC bakiyesi BURADA
    /// TUTULMAZ; her cagrida lsc_registry'den yuklenir (tek kaynak = lsc_registry).
    /// Kod+storage DAG replay'i ile yeniden kurulur (vertex'ler kalici).
    avm_db: crate::avm::AidagDatabase,

    /// BASLANGIC DAGITIMI (genesis / test). DAG'da vertex karsiligi YOK,
    /// bu yuzden durum yeniden uygulanirken ONCE bunlar yuklenir.
    baslangic_bakiyeler: Vec<([u8; 20], crate::registry::Tutar)>,
    /// Baslangic LSC dagitimi (DAG disi).
    baslangic_lsc: Vec<([u8; 20], crate::registry::Tutar)>,
    /// DAG disi eklenen stake kayitlari (test/bootstrap).
    baslangic_stake: Vec<crate::tx::StakeKaydi>,
    /// DAG disi eklenen vesting kayitlari.
    baslangic_vesting: Vec<([u8; 20], crate::registry::VestingKaydi)>,
    /// ARTIMLI: en son uygulanan total_order. Yeni sira bunun uzantisiysa
    /// (append) sadece kuyruk islenir; degilse (reorg) tam yeniden hesap.
    son_uygulanan_sira: Vec<VertexId>,
}

impl NodeState {
    /// Devnet düğüm durumu: boş graf (genesis ilk vertex'le kurulur) +
    /// artımlı ghostdag. Mainnet için ayrı kurucu (genesis pinli) gerekir.
    pub fn new_devnet(network_id: u32) -> Self {
        Self::yeni_ic(Graph::devnet(network_id), network_id, false)
    }

    /// MAINNET düğüm durumu: genesis id'si PINLI (`GenesisPolicy::Whitelisted`)
    /// + `network_id = 3474`. Genesis vertex çalışma anında `crate::mainnet`
    /// baked wire baytlarından yüklenir (özel anahtara ihtiyaç yok). Whitelisted
    /// politika: id uymayan hiçbir parent'sız vertex genesis olamaz → güven kökü
    /// sabittir, "first-seen" devnet açığı YOK. En güvenli mainnet kuruluşu.
    pub fn new_mainnet() -> Self {
        let mut s = Self::yeni_ic(
            Graph::mainnet(
                crate::mainnet::MAINNET_NETWORK_ID,
                crate::mainnet::genesis_id(),
            ),
            crate::mainnet::MAINNET_NETWORK_ID,
            true,
        );
        // A2 (owner-gating konsensus-deterministik): on-satis owner'i PINLI kurucu
        // adrese sabitle. Tum mainnet node'lari AYNI owner'i kullanir -> env'e bagli
        // node-yerel ayrisma (konsensus bolunmesi) YOK. Env override mainnet'te
        // devre disi (lib.rs). Faucet (mint) mainnet'te kapali (mainnet=true) ->
        // owner pinlemek arzi kirmaz; yalniz on-satis (arz-korumali) etkinlesir.
        s.faucet_owner = Some(crate::mainnet::kurucu_adres());
        // A1 (fon determinizmi): PINLI 7-dilim genesis dagitimini + vesting'i OTOMATIK
        // yukle. Env (LSC_GENESIS_*) YOK -> tum mainnet node'lari AYNI dagitim + owner
        // bakiyesi -> on-satis konsensus bolunmesi kapanir. Kapalilik (21M) debug'da dogrulanir.
        s.mainnet_dagitim_yukle();
        s
    }

    /// A1: PINLI 7-dilim mainnet dagitimini + vesting'i yukle. baslangic_bakiyeler/
    /// baslangic_vesting'e islenir -> her state-rebuild'de deterministik korunur.
    /// Adresler mainnet.rs'te PINLI (env DEGIL). on-satis (idx 6) = kurucu native.
    fn mainnet_dagitim_yukle(&mut self) {
        let adresler = crate::mainnet::dagitim_adresleri();
        let dagitim = crate::genesis::GenesisDagitim::planla(adresler);
        debug_assert!(
            dagitim.kapali_mi(),
            "MAINNET dagitim toplam 21M degil (kapali_mi=false)!"
        );
        let vesting_bas = crate::mainnet::MAINNET_VESTING_BASLANGIC;
        for (idx, (adres, miktar)) in dagitim.dilimler().iter().enumerate() {
            self.test_bakiye_ekle(*adres, *miktar);
            if let Some((cliff_sure, toplam_sure)) = crate::genesis::dilim_vesting(idx) {
                self.vesting_ekle(
                    *adres,
                    crate::registry::VestingKaydi {
                        toplam: *miktar,
                        baslangic: vesting_bas,
                        cliff_sure,
                        toplam_sure,
                        tge_acik: 0,
                    },
                );
            }
        }
        self.vesting_zaman_ayarla(vesting_bas);
    }

    /// Ortak kurulum: verilen graf + network_id ile boş defterli NodeState.
    /// `new_devnet`/`new_mainnet` yalnızca graf POLİTİKASINDA ayrışır; geri kalan
    /// tüm defterler (bakiye/stake/nonce/...) aynı → tek yerden kurulur (drift yok).
    fn yeni_ic(graph: Graph, network_id: u32, mainnet: bool) -> Self {
        NodeState {
            graph,
            ghostdag: Ghostdag::new_incremental(DEFAULT_K),
            network_id,
            mainnet,
            orphans: OrphanPool::new(),
            token_registry: crate::registry::TokenRegistry::yeni(),
            stake_registry: crate::registry::StakeRegistry::yeni(),
            bakiye_registry: crate::registry::BakiyeRegistry::yeni(),
            lsc_registry: crate::registry::BakiyeRegistry::yeni(),
            nonce_registry: crate::registry::NonceRegistry::yeni(),
            record_registry: crate::registry::RecordRegistry::yeni(),
            kurum_registry: crate::registry::KurumRegistry::yeni(),
            eslestirme_registry: crate::registry::EslestirmeRegistry::yeni(),
            on_satis_registry: crate::registry::OnSatisRegistry::yeni(),
            faucet_owner: None,
            faucet_verildi: std::collections::HashSet::new(),
            avm_db: crate::avm::AidagDatabase::yeni(),
            baslangic_bakiyeler: Vec::new(),
            baslangic_lsc: Vec::new(),
            baslangic_stake: Vec::new(),
            baslangic_vesting: Vec::new(),
            son_uygulanan_sira: Vec::new(),
        }
    }

    /// Ağdan/diskten gelen ham baytları ingest eder:
    /// decode → insert → ghostdag update. Hata aşama tipini korur.
    pub fn ingest(&mut self, bytes: &[u8], now: u64) -> Result<VertexId, IngestError> {
        let id = ingest_bytes(&mut self.graph, &mut self.ghostdag, bytes, now)?;
        // DURUM = ghostdag.total_order()'dan TURETILIR (belirlenimci, idempotent).
        // KRITIK (K1): eskiden burada dogrudan `kalkana_yonlendir` cagriliyordu AMA
        // `son_uygulanan_sira` guncellenmiyordu; sonraki `durumu_yeniden_uygula`
        // append fast-path'i AYNI vertex'i TEKRAR isliyordu. Nonce'suz STAKE bu
        // yuzden CIFT sayiliyordu (transferler nonce ile korunuyordu, stake degil)
        // -> uretici dugum agdan ayrisiyordu (konsensus bolunmesi). Cozum: ingest de
        // tek yol olan total_order-turevi yeniden-uygulamaya guvenir (idempotent).
        self.durumu_yeniden_uygula();
        Ok(id)
    }

    /// KALKAN: bir token kaydini defere ekleme girisimi. Taklit (ayni sembol
    /// farkli adres) ise PROTOKOL SEVIYESINDE reddedilir (deftere girmez).
    /// Donus: Kabul / TaklitReddedildi / ZatenKayitli.
    pub fn token_kaydet(&mut self, token: crate::tx::TokenKaydi) -> crate::registry::KayitSonucu {
        self.token_registry.kaydet(token)
    }

    /// Bir token, kayitli bir sembolun taklidi mi? (deftere eklemeden sorgu)
    /// Sahteyse taklit edilen gercek adresi doner.
    pub fn token_taklit_mi(&self, token: &crate::tx::TokenKaydi) -> Option<[u8; 20]> {
        self.token_registry.taklit_mi(token)
    }

    /// Defterdeki kanonik token sayisi.
    pub fn token_sayisi(&self) -> usize {
        self.token_registry.len()
    }

    /// KALKAN: kayitli kanonik token'lar (adres, sembol) — RPC/okuma.
    pub fn tum_tokenlar(&self) -> Vec<([u8; 20], [u8; 8])> {
        self.token_registry.tum_tokenlar()
    }

    /// STAKING: bir adresin teminatini ekle (birikimli). Donus: yeni toplam.
    pub fn stake_ekle(&mut self, kayit: crate::tx::StakeKaydi) -> crate::registry::Tutar {
        self.baslangic_stake.push(kayit.clone());
        self.stake_registry.stake_ekle(kayit)
    }

    /// Bir adresin toplam kilitli (stake) AIDAG miktari.
    pub fn stake_miktari(&self, adres: &[u8; 20]) -> crate::registry::Tutar {
        self.stake_registry.stake_miktari(adres)
    }

    /// Bir adresin serbest AIDAG bakiyesi (transfer edilebilir). Yoksa 0.
    pub fn bakiye(&self, adres: &[u8; 20]) -> crate::registry::Tutar {
        self.bakiye_registry.bakiye(adres)
    }

    /// Bir adresin BEKLENEN nonce'u (replay korumasi). SDK/RPC transfer
    /// olustururken bunu okuyup TransferKaydi/LscTransferKaydi'ye koymali.
    pub fn beklenen_nonce(&self, adres: &[u8; 20]) -> u64 {
        self.nonce_registry.beklenen(adres)
    }

    /// TEST/DEVNET: bir adrese bakiye basla (gercek arz DEGIL; mekanik testi).
    /// Gercek arz/dagitim modeli sonra (audit+hukuk). Birikimli.
    pub fn test_bakiye_ekle(
        &mut self,
        adres: [u8; 20],
        miktar: crate::registry::Tutar,
    ) -> crate::registry::Tutar {
        self.baslangic_bakiyeler.push((adres, miktar));
        self.bakiye_registry.test_bakiye_ekle(adres, miktar)
    }

    /// GENESIS VESTING: bir adrese vesting plani ekle (kurucu/destekci/likidite).
    /// Kilitli AIDAG transfer edilemez; cliff+dogrusal ile zamanla acilir.
    pub fn vesting_ekle(&mut self, adres: [u8; 20], kayit: crate::registry::VestingKaydi) {
        self.baslangic_vesting.push((adres, kayit.clone()));
        self.bakiye_registry.vesting_ekle(adres, kayit);
    }

    /// Zincir zamanini bakiye_registry'ye ver (transfer'de vesting kontrolu).
    pub fn vesting_zaman_ayarla(&mut self, simdi: u64) {
        self.bakiye_registry.zaman_ayarla(simdi);
    }

    /// Bir adresin su an kilitli (vesting) miktari.
    pub fn vesting_kilitli(&self, adres: &[u8; 20], simdi: u64) -> crate::registry::Tutar {
        self.bakiye_registry.vesting_kilitli(adres, simdi)
    }

    /// Toplam serbest AIDAG arzi (bakiye defteri). Test/denetim.
    pub fn toplam_bakiye_arzi(&self) -> crate::registry::Tutar {
        self.bakiye_registry.toplam_arz()
    }

    /// Kac farkli adresin bakiyesi var.
    pub fn bakiye_hesap_sayisi(&self) -> usize {
        self.bakiye_registry.hesap_sayisi()
    }

    /// Bir adresin serbest LSC bakiyesi (yakit/gas). Yoksa 0.
    pub fn lsc_bakiye(&self, adres: &[u8; 20]) -> crate::registry::Tutar {
        self.lsc_registry.bakiye(adres)
    }

    /// TEST/DEVNET: bir adrese LSC basla (gercek arz DEGIL; mekanik testi).
    pub fn lsc_test_bakiye_ekle(
        &mut self,
        adres: [u8; 20],
        miktar: crate::registry::Tutar,
    ) -> crate::registry::Tutar {
        self.baslangic_lsc.push((adres, miktar));
        self.lsc_registry.test_bakiye_ekle(adres, miktar)
    }

    /// Toplam serbest LSC arzi (LSC defteri). Test/denetim.
    pub fn lsc_toplam_arzi(&self) -> crate::registry::Tutar {
        self.lsc_registry.toplam_arz()
    }

    /// TEST/DOGRULAMA: bir adreste AVM sozlesme kodu var mi? (canli deploy kontrolu)
    pub fn avm_kod_var_mi(&self, adres: &[u8; 20]) -> bool {
        self.avm_db.kod_oku(adres).is_some()
    }

    /// TEST/DOGRULAMA: deploy edilmis tum kontrat adresleri.
    pub fn avm_kontrat_adresleri(&self) -> Vec<[u8; 20]> {
        self.avm_db.kontrat_adresleri()
    }

    /// eth_call icin: OKUMA-ONLY sozlesme cagrisi (state degismez).
    /// gonderen genelde sifir adres olabilir (okuma), hedef = sozlesme.
    pub fn avm_call(
        &self,
        gonderen: &[u8; 20],
        hedef: &[u8; 20],
        data: &[u8],
    ) -> Result<Vec<u8>, &'static str> {
        crate::avm::avm_call_oku(&self.avm_db, gonderen, hedef, data)
    }

    /// Kac farkli adresin LSC bakiyesi var.
    pub fn lsc_hesap_sayisi(&self) -> usize {
        self.lsc_registry.hesap_sayisi()
    }

    /// GERCEK DUNYA: bir belge hash'i zincirde kayitli mi? (kim, ne zaman).
    pub fn belge_dogrula(&self, data_hash: &[u8; 32]) -> Option<crate::registry::BelgeKaydi> {
        self.record_registry.dogrula(data_hash)
    }

    /// Kayitli belge sayisi.
    pub fn belge_sayisi(&self) -> usize {
        self.record_registry.len()
    }

    /// Bir adres hangi kurum/firma? (ad, kategori, zaman). Kayitli degilse None.
    pub fn kurum_sorgula(&self, adres: &[u8; 20]) -> Option<&crate::registry::KurumKaydi> {
        self.kurum_registry.sorgula(adres)
    }

    /// Kayitli kurum/firma sayisi.
    pub fn kurum_sayisi(&self) -> usize {
        self.kurum_registry.len()
    }

    /// Faucet owner adresini ayarla (TESTNET). Sadece bu adres faucet basabilir.
    /// Ayarlanmazsa faucet kapali kalir (mainnet guvenligi).
    pub fn faucet_owner_ayarla(&mut self, owner: [u8; 20]) {
        self.faucet_owner = Some(owner);
    }

    /// Faucet owner ayarli mi (faucet acik mi)?
    pub fn faucet_owner(&self) -> Option<[u8; 20]> {
        self.faucet_owner
    }

    /// Testnet eslestirme: test adresi -> gercek odul adresi (BIR KERELIK).
    /// true = yeni eslesme eklendi; false = zaten eslesmis (degistirilmedi).
    pub fn eslestir(&mut self, test_adresi: [u8; 20], gercek_adres: [u8; 20]) -> bool {
        self.eslestirme_registry.eslestir(test_adresi, gercek_adres)
    }

    /// Bir test adresinin eslesmis gercek odul adresi (yoksa None).
    pub fn eslesme_sorgula(&self, test_adresi: &[u8; 20]) -> Option<[u8; 20]> {
        self.eslestirme_registry.sorgula(test_adresi)
    }

    /// Toplam eslesme sayisi.
    pub fn eslesme_sayisi(&self) -> usize {
        self.eslestirme_registry.sayisi()
    }

    /// On satis: bir odeme_ref'in dagitim kaydi (yoksa None). Alici/aidag/lsc/zaman.
    pub fn on_satis_sorgula(&self, odeme_ref: u64) -> Option<crate::registry::OnSatisKaydi> {
        self.on_satis_registry.sorgula(odeme_ref).cloned()
    }

    /// On satis: toplam dagitim sayisi.
    pub fn on_satis_sayisi(&self) -> usize {
        self.on_satis_registry.sayisi()
    }

    /// On satis: toplam dagitilan AIDAG (seffaflik/denetim).
    pub fn on_satis_toplam_aidag(&self) -> u128 {
        self.on_satis_registry.toplam_aidag()
    }

    /// On satis: bir alicinin TUM alimlari (kisisel gorunum, zamana sirali).
    pub fn on_satis_adrese_gore(
        &self,
        alici: &[u8; 20],
    ) -> Vec<(u64, crate::registry::OnSatisKaydi)> {
        self.on_satis_registry.adrese_gore(alici)
    }

    /// On satis: tum alimlar zamana sirali (genel seffaf liste + hareket cizelgesi).
    pub fn on_satis_liste(&self) -> Vec<(u64, crate::registry::OnSatisKaydi)> {
        self.on_satis_registry.tum_kayitlar_sirali()
    }

    /// On satis: bir alicinin toplam aldigi AIDAG (kisisel ozet).
    pub fn on_satis_adres_toplam(&self, alici: &[u8; 20]) -> u128 {
        self.on_satis_registry.adres_toplam_aidag(alici)
    }

    /// Adres stake etmis mi? (kalkan icin: kanonik kayit hakki var mi?)
    pub fn stake_var_mi(&self, adres: &[u8; 20]) -> bool {
        self.stake_registry.stake_var_mi(adres)
    }

    /// Agdaki toplam kilitli AIDAG.
    pub fn toplam_stake(&self) -> crate::registry::Tutar {
        self.stake_registry.toplam_stake()
    }

    /// Kac farkli adres stake etmis (gozlemlenebilirlik / DURUM logu icin).
    pub fn staker_sayisi(&self) -> usize {
        self.stake_registry.staker_sayisi()
    }

    /// Graf'taki toplam vertex sayısı (genesis dahil).
    pub fn vertex_count(&self) -> usize {
        self.graph.len()
    }

    /// Genesis vertex id'si — henüz hiç vertex yoksa `None`.
    pub fn genesis_id(&self) -> Option<&VertexId> {
        self.graph.genesis()
    }

    /// Bu düğümün ağ kimliği (network_id).
    pub fn network_id(&self) -> u32 {
        self.network_id
    }

    /// Bir vertex id'sinin graf'ta olup olmadığı.
    pub fn contains(&self, id: &VertexId) -> bool {
        self.graph.contains(id)
    }

    /// Salt-okunur graf erişimi (ileride sorgular/explorer için).
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Bir pubkey'in URETTIGI (imzaladigi) islemleri dondur: (tip, zaman, payload).
    /// Frontend "Islemlerim" penceresi icin. Zincirden okur (gercek, kalici).
    pub fn islemlerim(&self, public_key: &[u8; 32]) -> Vec<(u8, u64, Vec<u8>)> {
        let mut sonuc = Vec::new();
        if let Some(ids) = self.graph().author_vertices(public_key) {
            for id in ids {
                if let Some(v) = self.graph().get(id) {
                    let payload = v.payload();
                    let tip = payload.first().copied().unwrap_or(0);
                    sonuc.push((tip, v.timestamp(), payload.to_vec()));
                }
            }
        }
        sonuc
    }

    /// Graf'in mevcut uclari (tips) — yeni vertex'lerin parent adaylari.
    /// Bos graf'ta bos Vec. Cagiran, canonical (artan) sira gerekiyorsa siralar.
    pub fn tips(&self) -> Vec<VertexId> {
        self.graph.tips()
    }

    /// Graf'taki TUM vertex'leri ham (wire) bayt halinde disa aktarir.
    /// Kaliciliga (diske kaydetme) temel: bu baytlar daha sonra `ingest_networked`
    /// ile geri yuklenebilir. SIRA garantisi YOK (HashMap) — yukleme orphan-bilincli
    /// oldugu icin sirasiz yukleme de dogru sonuc verir (cascade cozer).
    pub fn export_vertices(&self) -> Vec<Vec<u8>> {
        // SENKRONIZASYON: topolojik sira (genesis -> parent -> cocuk).
        // Boylece alici dugum her vertex'i islerken parent'lari ZATEN gelmis olur;
        // orphan havuzuna dusmez. Sirasiz ids() kullanmak orphan tasmasina yol acardi.
        crate::consensus::topological_order(&self.graph)
            .iter()
            .filter_map(|id| self.graph.get(id))
            .map(wire::encode)
            .collect()
    }

    /// Havuzdaki bekleyen yetim sayisi (gozlem icin).
    pub fn orphan_count(&self) -> usize {
        self.orphans.len()
    }

    /// Suresi (TTL) gecmis yetimleri temizle. Silinen sayisini doner.
    /// Ag katmani bunu periyodik cagirir (bellek korumasi).
    pub fn clean_orphans(&mut self, ttl: Duration) -> usize {
        self.orphans.clean_expired(ttl)
    }

    /// AGDAN gelen ham baytlari orphan-bilincli sekilde isler:
    ///   - decode hatasi  -> Rejected (graf'a dokunulmadi)
    ///   - eksik ebeveyn  -> Buffered (orphan havuzuna kondu) veya OrphanPoolFull
    ///   - tum ebeveynler hazir -> Integrated (graf+ghostdag) + CASCADE cozum
    ///
    /// Cascade: yeni eklenen vertex baska yetimlerin bekledigi ebeveyn olabilir;
    /// dongu ile zincirleme cozulur (A gelince B, B islenince C ...).
    pub fn ingest_networked(&mut self, bytes: &[u8], now: u64) -> NetworkIngestOutcome {
        // 1) Decode. Bozuksa graf'a HIC dokunma.
        let vertex = match wire::decode(bytes) {
            Ok(v) => v,
            Err(e) => return NetworkIngestOutcome::Rejected(IngestError::Decode(e)),
        };
        let id = *vertex.id();

        // AG KAPISI: yabanci network_id'li vertex ORPHAN HAVUZUNA GIRMEDEN reddedilir.
        // Onceden ag kontrolu yalnizca graph.insert icindeydi; eksik-ebeveynli yabanci
        // vertex oraya hic ulasmadigi icin havuza sizip birikiyordu (mainnet<->testnet
        // mDNS karisimi, 2026-07-18: 0 entegre / 4 orphan). Havuz doldurma yuzeyi kapandi.
        if vertex.network_id() != self.graph.network_id() {
            return NetworkIngestOutcome::Rejected(IngestError::Graph(
                crate::dag::graph::GraphError::NetworkMismatch {
                    expected: self.graph.network_id(),
                    got: vertex.network_id(),
                },
            ));
        }

        // 2) Zaten varsa (graf'ta veya havuzda) tekrar isleme.
        if self.graph.contains(&id) || self.orphans.contains(&id) {
            return NetworkIngestOutcome::Duplicate(id);
        }

        // 3) Eksik ebeveyn var mi? (genesis'in parent'i yok -> eksik sayilmaz)
        let has_missing_parent = vertex.parents().iter().any(|p| !self.graph.contains(p));

        if has_missing_parent {
            // Henuz islenemez -> yetim havuzuna al (reddetme!).
            return match self.orphans.add_orphan(vertex) {
                Ok(()) => NetworkIngestOutcome::Buffered(id),
                Err(_) => NetworkIngestOutcome::OrphanPoolFull(id),
            };
        }

        // 4) Tum ebeveynler hazir -> graf'a entegre et + cascade coz.
        match self.integrate_vertex(vertex, now, false, false) {
            Ok(()) => {
                self.resolve_cascade(id, now, false);
                NetworkIngestOutcome::Integrated(id)
            }
            Err(e) => NetworkIngestOutcome::Rejected(e),
        }
    }

    /// REPLAY ingest: diskten/finalize gecmisten vertex yukleme yolu.
    /// `ingest_networked` ile AYNI mantik (decode, duplicate, orphan, cascade)
    /// fakat SAAT POLITIKASI UYGULANMAZ (insert_synced). Eski timestamp'li
    /// vertex'ler "cok eski/ileri" diye reddedilmez — kalicilik icin sart.
    /// `now` gerekmez (synced yol timestamp'e bakmaz).
    pub fn ingest_synced(&mut self, bytes: &[u8]) -> NetworkIngestOutcome {
        let vertex = match wire::decode(bytes) {
            Ok(v) => v,
            Err(e) => return NetworkIngestOutcome::Rejected(IngestError::Decode(e)),
        };
        let id = *vertex.id();

        // AG KAPISI: yabanci network_id'li vertex ORPHAN HAVUZUNA GIRMEDEN reddedilir.
        // Onceden ag kontrolu yalnizca graph.insert icindeydi; eksik-ebeveynli yabanci
        // vertex oraya hic ulasmadigi icin havuza sizip birikiyordu (mainnet<->testnet
        // mDNS karisimi, 2026-07-18: 0 entegre / 4 orphan). Havuz doldurma yuzeyi kapandi.
        if vertex.network_id() != self.graph.network_id() {
            return NetworkIngestOutcome::Rejected(IngestError::Graph(
                crate::dag::graph::GraphError::NetworkMismatch {
                    expected: self.graph.network_id(),
                    got: vertex.network_id(),
                },
            ));
        }

        if self.graph.contains(&id) || self.orphans.contains(&id) {
            return NetworkIngestOutcome::Duplicate(id);
        }

        let has_missing_parent = vertex.parents().iter().any(|p| !self.graph.contains(p));

        if has_missing_parent {
            return match self.orphans.add_orphan(vertex) {
                Ok(()) => NetworkIngestOutcome::Buffered(id),
                Err(_) => NetworkIngestOutcome::OrphanPoolFull(id),
            };
        }

        // synced=true: saat politikasi YOK; now kullanilmaz (0 placeholder).
        match self.integrate_vertex(vertex, 0, true, false) {
            Ok(()) => {
                self.resolve_cascade(id, 0, true);
                NetworkIngestOutcome::Integrated(id)
            }
            Err(e) => NetworkIngestOutcome::Rejected(e),
        }
    }

    /// `ingest_synced` ile AYNI — fakat ed25519 imza dogrulamasi ATLANIR.
    /// ON KOSUL (CAGIRANIN SORUMLULUGU): `bytes`'in vertex'inin imzasi ZATEN
    /// (paralel toplu) dogrulanmis olmali. Diger TUM yapisal kontroller (ag,
    /// duplicate, orphan, parent, timestamp, genesis) YINE calisir.
    /// SADECE imzasi onceden dogrulanmis TOPLU YUKLEME yolundan cagrilir;
    /// aga acilan/guvenilmeyen kaynaktan ASLA. Yanlis kullanim = imza sizmasi.
    pub fn ingest_synced_preverified(&mut self, bytes: &[u8]) -> NetworkIngestOutcome {
        let vertex = match wire::decode(bytes) {
            Ok(v) => v,
            Err(e) => return NetworkIngestOutcome::Rejected(IngestError::Decode(e)),
        };
        let id = *vertex.id();

        // AG KAPISI: yabanci network_id'li vertex ORPHAN HAVUZUNA GIRMEDEN reddedilir.
        // Onceden ag kontrolu yalnizca graph.insert icindeydi; eksik-ebeveynli yabanci
        // vertex oraya hic ulasmadigi icin havuza sizip birikiyordu (mainnet<->testnet
        // mDNS karisimi, 2026-07-18: 0 entegre / 4 orphan). Havuz doldurma yuzeyi kapandi.
        if vertex.network_id() != self.graph.network_id() {
            return NetworkIngestOutcome::Rejected(IngestError::Graph(
                crate::dag::graph::GraphError::NetworkMismatch {
                    expected: self.graph.network_id(),
                    got: vertex.network_id(),
                },
            ));
        }
        if self.graph.contains(&id) || self.orphans.contains(&id) {
            return NetworkIngestOutcome::Duplicate(id);
        }
        let has_missing_parent = vertex.parents().iter().any(|p| !self.graph.contains(p));
        if has_missing_parent {
            return match self.orphans.add_orphan(vertex) {
                Ok(()) => NetworkIngestOutcome::Buffered(id),
                Err(_) => NetworkIngestOutcome::OrphanPoolFull(id),
            };
        }
        // skip_sig=true: imza zaten paralel dogrulandi (cascade da skip_sig=false
        // birakilir -> guvenli taraf; cascade'deki vertex'ler nadir/az).
        match self.integrate_vertex(vertex, 0, true, true) {
            Ok(()) => {
                self.resolve_cascade(id, 0, true);
                NetworkIngestOutcome::Integrated(id)
            }
            Err(e) => NetworkIngestOutcome::Rejected(e),
        }
    }

    /// `ingest_synced_preverified` ile AYNI — fakat ZATEN decode edilmis `Vertex`
    /// alir (wire::decode TEKRAR YAPILMAZ). Toplu yukleme zaten paralel verify
    /// fazinda decode+verify yapti; vertex'i tekrar decode etmek %73 israfti.
    /// ON KOSUL: `vertex`'in imzasi cagiran tarafindan ZATEN dogrulanmis olmali.
    /// pub(crate) — sadece guvenilir toplu yukleme yolundan.
    pub fn ingest_decoded_preverified(&mut self, vertex: Vertex) -> NetworkIngestOutcome {
        let id = *vertex.id();

        // AG KAPISI: yabanci network_id'li vertex ORPHAN HAVUZUNA GIRMEDEN reddedilir.
        // Onceden ag kontrolu yalnizca graph.insert icindeydi; eksik-ebeveynli yabanci
        // vertex oraya hic ulasmadigi icin havuza sizip birikiyordu (mainnet<->testnet
        // mDNS karisimi, 2026-07-18: 0 entegre / 4 orphan). Havuz doldurma yuzeyi kapandi.
        if vertex.network_id() != self.graph.network_id() {
            return NetworkIngestOutcome::Rejected(IngestError::Graph(
                crate::dag::graph::GraphError::NetworkMismatch {
                    expected: self.graph.network_id(),
                    got: vertex.network_id(),
                },
            ));
        }
        if self.graph.contains(&id) || self.orphans.contains(&id) {
            return NetworkIngestOutcome::Duplicate(id);
        }
        let has_missing_parent = vertex.parents().iter().any(|p| !self.graph.contains(p));
        if has_missing_parent {
            return match self.orphans.add_orphan(vertex) {
                Ok(()) => NetworkIngestOutcome::Buffered(id),
                Err(_) => NetworkIngestOutcome::OrphanPoolFull(id),
            };
        }
        match self.integrate_vertex(vertex, 0, true, true) {
            Ok(()) => {
                self.resolve_cascade(id, 0, true);
                NetworkIngestOutcome::Integrated(id)
            }
            Err(e) => NetworkIngestOutcome::Rejected(e),
        }
    }

    /// Tek bir vertex'i graf'a ekle + ghostdag guncelle (ic yardimci).
    /// `synced=true` ise REPLAY yolu (insert_synced): saat politikasi YOK —
    /// diskten/finalize gecmisten yukleme icin (eski timestamp'ler reddedilmez).
    /// `synced=false` ise CANLI yol (insert): saat kaymasi politikasi uygulanir.
    fn integrate_vertex(
        &mut self,
        vertex: Vertex,
        now: u64,
        synced: bool,
        skip_sig: bool,
    ) -> Result<(), IngestError> {
        // KALKAN: vertex graf'a girmeden ONCE payload'i kopyala (insert move eder).
        // Entegrasyon basariliysa, tip=2 (TokenKaydi) payload registry'ye yonlenir.
        // integrate_vertex TUM yollardan (ag/replay/yerel/cascade) gectigi icin,
        // burada yapilan yonlendirme her tip=2 vertex'i otomatik kalkandan gecirir.
        let payload_kopya: Vec<u8> = vertex.payload().to_vec();
        // STAKING bagi (9d): imzalayanin public key'ini de kopyala (move oncesi).
        // Token kaydinda "kaydeden = imzalayan"; kalkan, imzalayanin stake edip
        // etmedigini bu key'den turetilen adresle dogrular.
        let signer_kopya: [u8; 32] = *vertex.public_key();
        // Belge kaydi icin NE ZAMAN: vertex'in timestamp'i (move oncesi yakala).
        let zaman_kopya: u64 = vertex.timestamp();
        // ARTIMLI: eklenen vertex'in id'sini move ONCESI yakala -> update_one ile
        // dogrudan isle (tum graf taramasi yok; O(n^2) -> O(n)).
        let yeni_id: VertexId = *vertex.id();

        if synced {
            // skip_sig=true: imza ZATEN paralel toplu dogrulandi -> insert_synced_preverified
            // (ATLAMA DEGIL; bir kez dogrula). Diger TUM kontroller calisir. SADECE
            // imzasi onceden dogrulanmis toplu yukleme yolundan true gelir.
            if skip_sig {
                self.graph
                    .insert_synced_preverified(vertex)
                    .map_err(IngestError::Graph)?;
            } else {
                self.graph
                    .insert_synced(vertex)
                    .map_err(IngestError::Graph)?;
            }
        } else {
            self.graph.insert(vertex, now).map_err(IngestError::Graph)?;
        }
        self.ghostdag.update_one(&self.graph, &yeni_id);

        // KONSENSUS DUZELTMESI: state ARTIK burada uygulanmiyor.
        // Neden: ingest sirasi = ag gelis sirasi. Iki node ayni vertex'leri
        // farkli sirada alirsa FARKLI duruma giderdi (konsensus bolunmesi).
        // Dogrusu: state, ghostdag.total_order()'dan TURETILIR (belirlenimci).
        let _ = (&payload_kopya, &signer_kopya, zaman_kopya, synced);
        self.durumu_yeniden_uygula();

        Ok(())
    }

    /// DURUMU YENIDEN UYGULA — konsensus belirlenimciligi.
    ///
    /// Tum turetilmis defterleri SIFIRLAR, sonra ghostdag'in BELIRLENIMCI
    /// toplam siralamasi (total_order) ile vertex'leri bastan isler.
    /// Boylece durum, vertex'lerin AG'DAN GELIS SIRASINA degil, DAG'in
    /// uzlasilmis sirasina baglidir -> iki node AYNI duruma yakinsar.
    ///
    /// NOT: naif O(n) — her ingest'te tam yeniden hesap. Once DOGRULUK.
    /// Artimli hale getirme (sadece reorg olan kismi yeniden uygula) sonraki adim.
    fn durumu_yeniden_uygula(&mut self) {
        let yeni_sira = self.ghostdag.total_order(&self.graph);

        // APPEND FAST-PATH: yeni sira, son uygulanan siranin uzantisi mi?
        // Oyleyse onceki state gecerli; sadece YENI kuyrugu isle (sifirlama yok).
        let onceki = &self.son_uygulanan_sira;
        let append_mi = yeni_sira.len() >= onceki.len() && yeni_sira[..onceki.len()] == onceki[..];

        if append_mi && !onceki.is_empty() {
            let baslangic = onceki.len();
            for id in &yeni_sira[baslangic..] {
                if let Some(v) = self.graph.get(id) {
                    let payload: Vec<u8> = v.payload().to_vec();
                    let signer: [u8; 32] = *v.public_key();
                    let zaman: u64 = v.timestamp();
                    self.kalkana_yonlendir(&payload, &signer, zaman);
                }
            }
            self.son_uygulanan_sira = yeni_sira;
            return;
        }

        // REORG (veya ilk kez): TAM yeniden hesap.
        self.tam_yeniden_uygula(yeni_sira);
    }

    /// TAM yeniden hesap: tum defterleri sifirla, baslangic durumunu yukle,
    /// verilen siradaki TUM vertex'leri bastan isle. Reorg'da veya ilk
    /// uygulamada cagrilir. HER ZAMAN dogru (yavas yol).
    fn tam_yeniden_uygula(&mut self, sira: Vec<VertexId>) {
        // 1) Turetilmis defterleri sifirla.
        self.token_registry = crate::registry::TokenRegistry::yeni();
        self.stake_registry = crate::registry::StakeRegistry::yeni();
        self.bakiye_registry = crate::registry::BakiyeRegistry::yeni();
        self.lsc_registry = crate::registry::BakiyeRegistry::yeni();
        self.nonce_registry = crate::registry::NonceRegistry::yeni();
        self.record_registry = crate::registry::RecordRegistry::yeni();
        self.kurum_registry = crate::registry::KurumRegistry::yeni();
        self.eslestirme_registry = crate::registry::EslestirmeRegistry::yeni();
        self.on_satis_registry = crate::registry::OnSatisRegistry::yeni();
        self.faucet_verildi = std::collections::HashSet::new();
        self.avm_db = crate::avm::AidagDatabase::yeni();

        // 2) BASLANGIC DURUMU (genesis/test) — DAG'da vertex karsiligi YOK.
        for (adres, miktar) in self.baslangic_bakiyeler.clone() {
            self.bakiye_registry.test_bakiye_ekle(adres, miktar);
        }
        for (adres, miktar) in self.baslangic_lsc.clone() {
            self.lsc_registry.test_bakiye_ekle(adres, miktar);
        }
        for kayit in self.baslangic_stake.clone() {
            self.stake_registry.stake_ekle(kayit);
        }
        for (adres, kayit) in self.baslangic_vesting.clone() {
            self.bakiye_registry.vesting_ekle(adres, kayit);
        }

        // 3) BELIRLENIMCI sira ile tum vertex'leri yeniden isle.
        for id in &sira {
            let Some(v) = self.graph.get(id) else {
                continue;
            };
            let payload: Vec<u8> = v.payload().to_vec();
            let signer: [u8; 32] = *v.public_key();
            let zaman: u64 = v.timestamp();
            // synced=FALSE: disk-replay DEGIL; state'in sifirdan TAM KURALLARLA
            // yeniden hesabi (gas kesilir, nonce ilerler, bakiye kontrol edilir).
            self.kalkana_yonlendir(&payload, &signer, zaman);
        }
        self.son_uygulanan_sira = sira;
    }

    /// KALKAN yonlendirme: payload tip=2 (TokenKaydi) ise registry'ye kaydet.
    /// Taklit (ayni sembol farkli adres) protokol seviyesinde reddedilir.
    /// Bu metod durumu_yeniden_uygula'dan (belirlenimci total_order) cagrilir ->
    /// TUM ingest yollari (ag/replay/yerel/cascade) ayni deterministik yoldan gecer.
    /// NOT (B7): eski `synced` param'i kaldirildi — state HER ZAMAN total_order'dan
    /// sifirdan turetilir; "replay" ozel yolu yoktu (olu koddu), silindi.
    fn kalkana_yonlendir(&mut self, payload: &[u8], signer: &[u8; 32], zaman: u64) {
        // DETERMINIZM: vesting kilit kontrolu, islenmekte olan vertex'in KENDI
        // timestamp'ine gore yapilir. `zaman` konsensus verisidir (vertex preimage'i
        // + her dugumde AYNI) → kilitli/serbest miktar tum dugumlerde birebir ayni
        // hesaplanir. Yerel saat (SystemTime::now) ASLA kullanilmaz; aksi halde
        // dugumler ayni transfer'i farkli kilit durumuyla degerlendirir → ayrisma.
        // EVM `block.timestamp` mantigi: islem, kendi zaman damgasina gore degerlenir.
        self.bakiye_registry.zaman_ayarla(zaman);
        match payload.first() {
            // tip=2: token kimlik kaydi -> KALKAN (STAKE-KONTROLLU + taklit reddi)
            Some(&crate::tx::TX_TYPE_TOKEN) => {
                if let Ok(token) = crate::tx::TokenKaydi::decode(payload) {
                    // STAKING KAPISI (9d): token kaydeden = imzalayan. Imzalayanin
                    // adresini public key'den turet; STAKE etmemisse kayit HAKKI
                    // YOK -> reddet (deftere girmez). "Kanonik adresi kim belirler?"
                    // = TEMINAT yatiranlar. Imza sahte olamaz -> baskasinin stake'i
                    // kullanilamaz. Bedavaya kayit YOK.
                    let kaydeden = crate::registry::public_key_to_adres(signer);
                    if !self.stake_registry.stake_var_mi(&kaydeden) {
                        // Stake yok -> kayit reddedilir (sessizce; deftere girmez).
                        return;
                    }
                    // Stake var -> kalkandan gecir. SAHTECILIK (taklit) ise SLASH:
                    // kaydedenin TUM stake'i yakilir (sahteciligin bedeli agir).
                    // Taklit ise (TaklitReddedildi) tum stake yakilir; Kabul/
                    // ZatenKayitli'de ceza yok.
                    if let crate::registry::KayitSonucu::TaklitReddedildi { .. } =
                        self.token_registry.kaydet(token)
                    {
                        let _yakilan = self.stake_registry.slash(&kaydeden);
                    }
                }
            }
            // tip=3: stake kaydi -> STAKING defteri (teminat birikir)
            Some(&crate::tx::TX_TYPE_STAKE) => {
                if let Ok(stake) = crate::tx::StakeKaydi::decode(payload) {
                    let _yeni_toplam = self.stake_registry.stake_ekle(stake);
                }
            }
            // tip=4: transfer (odeme) -> BAKIYE defteri.
            // GUVENLIK (B modeli): GONDEREN = imzalayan (signer'dan turetilir),
            // payload'daki adres ALICI'dir. Boylece "baskasi adina transfer"
            // IMKANSIZ -> imza sahte olamaz, yalnizca kendi bakiyeni harcarsin.
            // BakiyeRegistry.transfer() cift harcamayi (yetersiz bakiye) reddeder;
            // basarisizsa graf DEGISMEZ (vertex zaten DAG'da, ama bakiye guncellenmez).
            Some(&crate::tx::TX_TYPE_TRANSFER) => {
                if let Ok(t) = crate::tx::TransferKaydi::decode(payload) {
                    let gonderen = crate::registry::public_key_to_adres(signer);
                    // REPLAY KORUMASI: nonce beklenenle eslesmezse transfer islenmez.
                    // Vertex DAG'da kalir; sadece bakiye degismez (gecersiz transfer
                    // para yaratmaz/kaybetmez felsefesiyle ayni). (A) yalniz BASARILI
                    // transfer nonce'u ilerletir.
                    if self.nonce_registry.dogru_mu(&gonderen, t.nonce) {
                        let sonuc = self.bakiye_registry.transfer(&gonderen, &t.alici, t.miktar);
                        if matches!(sonuc, crate::registry::TransferSonuc::Basarili { .. }) {
                            self.nonce_registry.ilerlet(&gonderen);
                        }
                    }
                    // _sonuc YetersizBakiye olabilir -> sessizce gecersiz (deftere
                    // yansimaz). Vertex DAG'da kalir ama bakiye degismez (dogru:
                    // gecersiz transfer para yaratmaz/kaybetmez).
                }
            }
            // tip=7: LSC transfer -> LSC defteri (lsc_registry). AIDAG transferiyle
            // AYNI mantik (gonderen=imzalayan, cift-harcama korumali) ama ayri defter.
            Some(&crate::tx::TX_TYPE_LSC_TRANSFER) => {
                if let Ok(t) = crate::tx::LscTransferKaydi::decode(payload) {
                    let gonderen = crate::registry::public_key_to_adres(signer);
                    if self.nonce_registry.dogru_mu(&gonderen, t.nonce) {
                        let sonuc = self.lsc_registry.transfer(&gonderen, &t.alici, t.miktar);
                        if matches!(sonuc, crate::registry::TransferSonuc::Basarili { .. }) {
                            self.nonce_registry.ilerlet(&gonderen);
                        }
                    }
                    // YetersizBakiye -> sessizce gecersiz (AIDAG ile ayni davranis).
                }
            }
            // tip=11: EVM-UYUMLU TRANSFER (secp256k1 imzali — MetaMask/Trust/Ledger).
            // tip=4'un KARDESI. TEK FARK: gonderen, vertex imzalayanindan DEGIL,
            // EVM transferin KENDI secp256k1 imzasindan (ecrecover) cikar. Boylece
            // bir MetaMask kullanicisi, AIDAG zincirinde kendi adresinden transfer
            // yapar. Cift-harcama (bakiye_registry.transfer), nonce/replay
            // (nonce_registry) — hepsi MEVCUT, test edilmis yollar. Yeni para
            // mantigi YOK. Imza gecersizse gonderen None -> hicbir sey degismez.
            Some(&crate::tx::TX_TYPE_EVM_TRANSFER) => {
                if let Ok(t) = crate::tx::EvmTransfer::decode(payload) {
                    // ecrecover: imzadan gondereni kurtar (Secenek B). Gecersizse None.
                    if let Some(gonderen) = t.gonderen_adres() {
                        // Replay korumasi: kendi nonce sistemimiz (tip=4 ile AYNI).
                        if self.nonce_registry.dogru_mu(&gonderen, t.nonce) {
                            let sonuc =
                                self.bakiye_registry.transfer(&gonderen, &t.alici, t.miktar);
                            if matches!(sonuc, crate::registry::TransferSonuc::Basarili { .. }) {
                                self.nonce_registry.ilerlet(&gonderen);
                            }
                        }
                        // YetersizBakiye/yanlis nonce -> sessizce gecersiz (tip=4 ile ayni).
                    }
                    // Imza gecersiz (gonderen None) -> hicbir sey degismez.
                }
            }
            // tip=8: ESLESTIRME -> eslestirme_registry (test -> gercek odul adresi).
            // Zincire yazilir: kalici + denetlenebilir. Restart'ta replay edilince
            // eslesme geri gelir (transfer gibi). BIR KERELIK + anti-Sybil registry'de.
            Some(&crate::tx::TX_TYPE_ESLESTIRME) => {
                if let Ok(e) = crate::tx::EslestirmeKaydi::decode(payload) {
                    let _yeni = self
                        .eslestirme_registry
                        .eslestir(e.test_adresi, e.gercek_adres);
                    // _yeni false olabilir (zaten eslesmis ya da gercek adres kullanilmis):
                    // sessizce yok sayilir (kural registry'de, para/odul etkilenmez).
                }
            }
            // tip=9: AVM CAGRISI (Kopru 4). EVM-yolu LSC deger transferi + gas.
            // GONDEREN = imzalayan. deger = LSC, hedefe transfer. Gas (sabit 21000)
            // LSC olarak gonderenden kesilir: %50 yakim adresine, %50 gelistirme
            // havuzuna (gas_ucreti_bol). Hepsi lsc_registry.transfer() ile (test
            // edilmis, arz-korumali yol). nonce replay korumasi (transfer ile ayni).
            // Yetersiz bakiye / yanlis nonce -> hicbir sey degismez (vertex DAG'da kalir).
            Some(&crate::tx::TX_TYPE_AVM_CAGRI) => {
                if let Ok(c) = crate::tx::AvmCagri::decode(payload) {
                    let gonderen = crate::registry::public_key_to_adres(signer);
                    if self.nonce_registry.dogru_mu(&gonderen, c.nonce) {
                        const AVM_GAS: u64 = 21_000;
                        let ucret = crate::avm::gas_ucreti_hesapla(AVM_GAS);
                        let (yakilan, gelistirme) = crate::avm::gas_ucreti_bol(ucret);

                        if c.data.is_empty() {
                            // --- DATA BOS: basit LSC deger transferi (ESKI YOL, korunur) ---
                            let lsc_gerekli = ucret as crate::registry::Tutar;
                            if self.bakiye_registry.bakiye(&gonderen) >= c.deger
                                && self.lsc_registry.bakiye(&gonderen) >= lsc_gerekli
                            {
                                let s1 =
                                    self.bakiye_registry.transfer(&gonderen, &c.hedef, c.deger);
                                if matches!(s1, crate::registry::TransferSonuc::Basarili { .. }) {
                                    let _ = self.lsc_registry.transfer(
                                        &gonderen,
                                        &crate::avm::YAKIM_ADRESI,
                                        yakilan as crate::registry::Tutar,
                                    );
                                    let _ = self.lsc_registry.transfer(
                                        &gonderen,
                                        &crate::avm::GELISTIRME_HAVUZU,
                                        gelistirme as crate::registry::Tutar,
                                    );
                                    self.nonce_registry.ilerlet(&gonderen);
                                }
                            }
                        } else {
                            // --- DATA DOLU: KONTRAT calistirma. Kod/storage avm_db'de KALICI.
                            // GAS (B2 duzeltmesi): sabit 21000 DEGIL, GERCEK gas_used'dan ucret.
                            //  * Upfront: kullanici gas TAVANINI (AVM_GAS_LIMIT) LSC olarak
                            //    karsilayabilmeli (aksi halde islem calistirilmaz).
                            //  * Kesinti GERCEK gas_used'dan (fazlasi kesilmez, refund yok cunku
                            //    hic reserve edilmedi; gas_price=0 revm-ici, kesinti node'da).
                            //  * BASARISIZ tx'te de gas KESILIR + nonce ILERLER -> "bedava basarisiz
                            //    tx" DoS'u kapanir. Deger transferi yalnizca basari + deger>0'da.
                            let azami_ucret =
                                crate::avm::gas_ucreti_hesapla(crate::avm::AVM_GAS_LIMIT)
                                    as crate::registry::Tutar;
                            if self.bakiye_registry.bakiye(&gonderen) >= c.deger
                                && self.lsc_registry.bakiye(&gonderen) >= azami_ucret
                            {
                                // B1 (SEED): EVM'e TAM AIDAG gorunumu ver. Yalniz gonderen
                                // degil, TUM hesaplar yuklenir ki kontrat-ici hareketler
                                // (payable/withdraw/ucuncu-tarafa odeme) dogru bakiyelerle
                                // yurusun. gas_price=0 -> EVM native yaratmaz/yakmaz.
                                self.avm_db
                                    .aidag_yukle_hepsi(self.bakiye_registry.tum_bakiyeler());
                                // B6: CREATE nonce'unu BIRLESIK nonce_registry'ye senkronla
                                // (c.nonce == beklenen, dogru_mu ile dogrulandi). Boylece
                                // CREATE adresi = keccak(gonderen, birlesik_nonce) =
                                // eth_getTransactionCount -> MetaMask/arac adres tahmini tutar.
                                // (Aksi halde native tx'ler avm_db nonce'undan ayrisirdi.)
                                self.avm_db.nonce_koy(gonderen, c.nonce);
                                // KONTRAT calistir: deploy (hedef=sifir) ya da call. deger EVM'e
                                // verilir ki kontrat mantigi (payable vb.) dogru tetiklensin.
                                let sonuc = crate::avm::avm_calistir(
                                    &mut self.avm_db,
                                    &gonderen,
                                    &c.hedef,
                                    c.deger,
                                    &c.data,
                                    zaman,
                                );
                                if let Ok(r) = sonuc {
                                    // GERCEK gas_used'dan ucret (basari/basarisiz FARK ETMEZ).
                                    let ucret_ger = crate::avm::gas_ucreti_hesapla(r.gas_used);
                                    let (yak_g, gel_g) = crate::avm::gas_ucreti_bol(ucret_ger);
                                    let _ = self.lsc_registry.transfer(
                                        &gonderen,
                                        &crate::avm::YAKIM_ADRESI,
                                        yak_g as crate::registry::Tutar,
                                    );
                                    let _ = self.lsc_registry.transfer(
                                        &gonderen,
                                        &crate::avm::GELISTIRME_HAVUZU,
                                        gel_g as crate::registry::Tutar,
                                    );
                                    // nonce HER DURUMDA ilerler (basarisiz tx replay'i de engellenir).
                                    self.nonce_registry.ilerlet(&gonderen);
                                    // B1 (MIRROR): EVM'in urettigi TUM AIDAG state-diff'i
                                    // gercek deftere geri aynala. Ust-seviye `deger` dahil TUM
                                    // kontrat-ici hareketler burada yansir -> fon donmasi biter.
                                    // Basarisiz/revert'te EVM state'i geri sarilmis olur ->
                                    // aynalama seed ile ayni kalir (guvenli no-op). Eski
                                    // ust-seviye `transfer` KALDIRILDI (deger'i EVM zaten tasidi;
                                    // aksi halde CIFT sayim olurdu).
                                    self.bakiye_registry.aidag_aynala(self.avm_db.aidag_tumu());
                                }
                            }
                        }
                    }
                }
            }
            // tip=10: ON SATIS DAGITIM. SADECE owner cagirir (odeme zincir-disi onaylanir).
            // Owner (hazine) -> aliciya AIDAG satilir + LSC hediye. Arz-korumali transfer.
            Some(&crate::tx::TX_TYPE_ON_SATIS) => {
                if let Ok(d) = crate::tx::OnSatisDagitim::decode(payload) {
                    let cagiran = crate::registry::public_key_to_adres(signer);
                    if self.faucet_owner == Some(cagiran) {
                        // CIFTE DAGITIM ENGELI: bu odeme_ref daha once kullanildiysa HICBIR SEY YAPMA.
                        // (Owner yanlislikla ayni odemeyi iki kez gonderse bile cifte AIDAG gitmez.)
                        if !self.on_satis_registry.kullanilmis(d.odeme_ref) {
                            // 1) AIDAG transferi ZORUNLU basarili olmali. Owner bakiyesi
                            //    yetersizse transfer HATA verir -> KAYIT TUTMA, dagitma.
                            //    (Aksi halde kayit "dagitildi" der ama AIDAG gitmemis olur =
                            //    seffafliga ihanet. "Gerceklesmeyen dagitim kaydedilmez.")
                            let aidag_ok = if d.aidag > 0 {
                                matches!(
                                    self.bakiye_registry.transfer(&cagiran, &d.alici, d.aidag),
                                    crate::registry::TransferSonuc::Basarili { .. }
                                )
                            } else {
                                true // 0 AIDAG: gecerli (sadece LSC hediye senaryosu)
                            };

                            if aidag_ok {
                                // 2) LSC hediye (AIDAG basariliysa). Hediye basarisiz olsa bile
                                //    AIDAG gitti -> kayit tutulur (asil urun AIDAG'dir).
                                // A4 (SEFFAFLIK): transfer sonucu KONTROL edilir; kayda GERCEKTEN
                                // gonderilen tutar yazilir. Owner LSC'si yetersizse hediye gitmez
                                // ve kayda 0 yazilir -> "gonderildi" YALANI kaydedilmez.
                                // DETERMINIZM: owner bakiyesi tum dugumlerde ayni -> gonderilen
                                // tutar da ayni -> on_satis_registry ayrisimaz.
                                let lsc_gonderilen = if d.lsc_hediye > 0 {
                                    match self.lsc_registry.transfer(
                                        &cagiran,
                                        &d.alici,
                                        d.lsc_hediye,
                                    ) {
                                        crate::registry::TransferSonuc::Basarili { .. } => {
                                            d.lsc_hediye
                                        }
                                        _ => {
                                            eprintln!(
                                                "[UYARI] ON SATIS LSC HEDIYE BASARISIZ: owner LSC bakiyesi yetersiz olabilir. odeme_ref={}, istenen_lsc={}. AIDAG dagitildi; hediye 0 kaydedildi.",
                                                d.odeme_ref, d.lsc_hediye
                                            );
                                            0
                                        }
                                    }
                                } else {
                                    0
                                };
                                // 3) KAYDET: AIDAG gercekten gitti + GERCEK hediye tutari.
                                //    Kayit = gercek dagitim (seffaf).
                                let _ = self.on_satis_registry.kaydet(
                                    d.odeme_ref,
                                    d.alici,
                                    d.aidag,
                                    lsc_gonderilen,
                                    zaman,
                                );
                                // 4) FAZ2 VESTING: dagitilan AIDAG KILITLI (%20 TGE hemen +
                                //    kalan %80 12 ay dogrusal). Birden cok dagitim alan alici
                                //    icin kilit BIRIKIR (vesting_biriktir). Takvim SABIT (TGE
                                //    = MAINNET_VESTING_BASLANGIC) -> tum dugumlerde deterministik.
                                if d.aidag > 0 {
                                    let tge_acik =
                                        d.aidag * crate::genesis::ON_SATIS_TGE_YUZDE / 100;
                                    self.bakiye_registry.vesting_biriktir(
                                        d.alici,
                                        crate::registry::VestingKaydi {
                                            toplam: d.aidag,
                                            baslangic: crate::mainnet::MAINNET_VESTING_BASLANGIC,
                                            cliff_sure: 0,
                                            toplam_sure: crate::genesis::ON_SATIS_VESTING_SURE,
                                            tge_acik,
                                        },
                                    );
                                }
                            } else {
                                // AIDAG transferi basarisiz (owner bakiyesi yetersiz vb.):
                                // SESSIZ GECME -> uyar. Dagitim YAPILMADI, kayit YOK.
                                eprintln!(
                                    "[UYARI] ON SATIS BASARISIZ: owner bakiyesi yetersiz olabilir. odeme_ref={}, istenen_aidag={}. Dagitim YAPILMADI, kayit TUTULMADI.",
                                    d.odeme_ref, d.aidag
                                );
                            }
                        }
                    }
                }
            }

            // tip=1: belge/veri kaydi -> RECORD defteri (gercek dunya dogrulama).
            // KAYDEDEN = imzalayan (signer'dan turetilir); ZAMAN = vertex timestamp.
            // ILK KAYIT KAZANIR (registry icinde); kanit bozulmaz.
            Some(&crate::tx::TX_TYPE_RECORD) => {
                if let Ok(rec) = crate::tx::Record::decode(payload) {
                    let kaydeden = crate::registry::public_key_to_adres(signer);
                    let _yeni = self.record_registry.kaydet(rec.data_hash, kaydeden, zaman);
                }
            }
            // tip=5: kurum/firma kimlik kaydi -> KURUM defteri.
            // KAYDEDEN = imzalayan (signer'dan turetilir) -> baskasi adina
            // kurum kaydi IMKANSIZ. kategori 0=Devlet,1=Ozel. ILK KAYIT KAZANIR.
            Some(&crate::tx::TX_TYPE_KURUM) => {
                if let Ok(k) = crate::tx::KurumKaydiTx::decode(payload) {
                    let kaydeden = crate::registry::public_key_to_adres(signer);
                    let kategori = if k.kategori == 0 {
                        crate::registry::KurumKategori::Devlet
                    } else {
                        crate::registry::KurumKategori::Ozel
                    };
                    let _yeni = self.kurum_registry.kaydet(kaydeden, k.ad, kategori, zaman);
                }
            }
            // tip=6: FAUCET (TESTNET test AIDAG). GUVENLIK: sadece imzalayan ==
            // faucet_owner ise bakiye eklenir; owner degilse ya da owner ayarli
            // degilse REDDEDILIR (sessizce yok sayilir). Boylece faucet vertex'i
            // aga yayilir, tum dugumlerde ayni bakiye olusur (senkron).
            Some(&crate::tx::TX_TYPE_FAUCET) => {
                // MAINNET GUVENLIGI: faucet MINT'tir (yoktan test AIDAG). Mainnet'te
                // TAMAMEN KAPALI -> 21M sabit arz korunur. Yalniz testnet'te calisir.
                if !self.mainnet {
                    if let Some(owner) = self.faucet_owner {
                        if let Ok(f) = crate::tx::FaucetKaydi::decode(payload) {
                            let imzalayan = crate::registry::public_key_to_adres(signer);
                            if imzalayan == owner && self.faucet_verildi.insert(f.alici) {
                                // Owner dogrulandi -> test bakiyesi bas (aga yayilan).
                                self.bakiye_registry.test_bakiye_ekle(f.alici, f.miktar);
                                // GAS: AIDAG yaninda birkac islemlik LSC (gas) de ver.
                                self.lsc_registry
                                    .test_bakiye_ekle(f.alici, 100_000_000_000_000_000);
                                // 0.1 LSC gas (~4700 transfer)
                            }
                            // owner degilse: sessizce reddet (yetkisiz faucet).
                        }
                    }
                }
                // owner ayarsiz ya da MAINNET: faucet kapali, hicbir sey yapma.
            }
            // diger tipler: kalkan/staking/record disi, dokunma.
            // tip=12: HAM ETHEREUM TX (eth_sendRawTransaction). Payload = RLP eth tx.
            // GONDEREN eth tx'in secp256k1 imzasindan gelir (vertex signer'i DEGIL).
            // Hem canli hem replay'de AVM'de calisir -> DAG'da kalici + restart'ta geri gelir.
            Some(&crate::tx::TX_TYPE_HAM_ETH_TX) => {
                if let Some(raw) = crate::tx::ham_eth_tx_coz_payload(payload) {
                    if let Ok(islem) = crate::avm::ham_eth_tx_coz(raw) {
                        let gonderen = islem.gonderen;
                        // Nonce replay korumasi (canli+replay ayni)
                        if self.nonce_registry.dogru_mu(&gonderen, islem.nonce) {
                            // B2: upfront affordability gas TAVANINA (AVM_GAS_LIMIT) gore;
                            // GERCEK kesinti gas_used'dan. AIDAG (deger) + LSC (gas) ayri defter.
                            let azami_ucret =
                                crate::avm::gas_ucreti_hesapla(crate::avm::AVM_GAS_LIMIT)
                                    as crate::registry::Tutar;
                            if self.bakiye_registry.bakiye(&gonderen) >= islem.deger
                                && self.lsc_registry.bakiye(&gonderen) >= azami_ucret
                            {
                                // B1 (SEED): EVM'e TAM AIDAG gorunumu ver (kontrat-ici
                                // hareketler ucuncu-taraflar dahil dogru bakiyelerle yurusun).
                                self.avm_db
                                    .aidag_yukle_hepsi(self.bakiye_registry.tum_bakiyeler());
                                // B6: CREATE nonce'unu BIRLESIK nonce_registry'ye senkronla
                                // (islem.nonce == beklenen). CREATE adresi eth_getTransactionCount
                                // ile tutarli olur -> MetaMask/arac adres tahmini dogru.
                                self.avm_db.nonce_koy(gonderen, islem.nonce);
                                if let Ok((_h, r)) =
                                    crate::avm::ham_eth_tx_isle(&mut self.avm_db, raw, zaman)
                                {
                                    // B2: GERCEK gas_used (basari/basarisiz FARK ETMEZ) -> LSC.
                                    let ucret_ger = crate::avm::gas_ucreti_hesapla(r.gas_used);
                                    let (yak_g, gel_g) = crate::avm::gas_ucreti_bol(ucret_ger);
                                    let _ = self.lsc_registry.transfer(
                                        &gonderen,
                                        &crate::avm::YAKIM_ADRESI,
                                        yak_g as crate::registry::Tutar,
                                    );
                                    let _ = self.lsc_registry.transfer(
                                        &gonderen,
                                        &crate::avm::GELISTIRME_HAVUZU,
                                        gel_g as crate::registry::Tutar,
                                    );
                                    // nonce HER DURUMDA ilerler (basarisiz tx replay'i de engellenir).
                                    self.nonce_registry.ilerlet(&gonderen);
                                    // B1 (MIRROR): EVM'in urettigi TUM AIDAG state-diff'i (ust
                                    // seviye deger dahil) gercek deftere aynala -> fon donmasi biter.
                                    // Eski ust-seviye transfer KALDIRILDI (cift sayim olurdu).
                                    self.bakiye_registry.aidag_aynala(self.avm_db.aidag_tumu());
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// `parent_id` entegre olduktan sonra, onu bekleyen yetimleri zincirleme
    /// coz. Serbest kalan her cocuk graf'a islenir; o da yeni serbest birakmalar
    /// tetikleyebilir (BFS benzeri dongu).
    fn resolve_cascade(&mut self, parent_id: VertexId, now: u64, synced: bool) {
        let mut frontier = vec![parent_id];
        while let Some(pid) = frontier.pop() {
            let ready = self.orphans.on_parent_integrated(&pid);
            for child in ready {
                let child_id = *child.id();
                // Cocuk artik islenebilir; graf'a ekle.
                if self.integrate_vertex(child, now, synced, false).is_ok() {
                    // Bu cocuk da baska yetimlerin ebeveyni olabilir.
                    frontier.push(child_id);
                }
                // integrate hata verirse (gecersiz): sessizce dusur; havuzdan
                // zaten cikti, graf degismedi (atomik insert).
            }
        }
    }
}

/// `ingest_networked` sonucu. Her durum acikca ayrilir (sahte/sessiz yok).
/// (PartialEq/Eq turetilmez: IngestError ic tipleri Eq degil. Testlerde
///  `matches!` ve id karsilastirmasi kullanilir.)
#[derive(Debug)]
pub enum NetworkIngestOutcome {
    /// Vertex graf'a eklendi (tum ebeveynleri hazirdi).
    Integrated(VertexId),
    /// Ebeveyni eksik -> yetim havuzunda bekliyor.
    Buffered(VertexId),
    /// Bu vertex zaten graf'ta veya havuzda.
    Duplicate(VertexId),
    /// Yetim havuzu dolu (OOM korumasi) -> vertex dusuruldu.
    OrphanPoolFull(VertexId),
    /// Decode veya graf dogrulamasi reddetti.
    Rejected(IngestError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::KayitSonucu;
    use crate::tx::TokenKaydi;

    fn sym(s: &str) -> [u8; 8] {
        let mut out = [0u8; 8];
        let b = s.as_bytes();
        out[..b.len()].copy_from_slice(b);
        out
    }

    #[test]
    fn node_gercek_token_kabul_eder() {
        let mut node = NodeState::new_devnet(1);
        let usdc = TokenKaydi::new([0xAA; 20], sym("USDC"));
        assert_eq!(node.token_kaydet(usdc), KayitSonucu::Kabul);
        assert_eq!(node.token_sayisi(), 1);
    }

    #[test]
    fn node_sahte_token_protokol_seviyesinde_reddeder() {
        let mut node = NodeState::new_devnet(1);
        // Gercek USDC kaydedilir
        node.token_kaydet(TokenKaydi::new([0xAA; 20], sym("USDC")));
        // Sahte USDC: ayni sembol, farkli adres -> REDDEDILMELI
        let sahte = TokenKaydi::new([0xBB; 20], sym("USDC"));
        assert!(matches!(
            node.token_kaydet(sahte),
            KayitSonucu::TaklitReddedildi { .. }
        ));
        // KRITIK: sahte node'un defterine GIRMEDI
        assert_eq!(node.token_sayisi(), 1);
    }

    #[test]
    fn node_taklit_sorgusu_calisir() {
        let mut node = NodeState::new_devnet(1);
        node.token_kaydet(TokenKaydi::new([0xAA; 20], sym("USDC")));
        let sahte = TokenKaydi::new([0xBB; 20], sym("USDC"));
        assert_eq!(node.token_taklit_mi(&sahte), Some([0xAA; 20]));
        let temiz = TokenKaydi::new([0xCC; 20], sym("DAI"));
        assert_eq!(node.token_taklit_mi(&temiz), None);
    }

    // KALKAN AG ENTEGRASYONU: tip=2 TokenKaydi payload'li bir vertex INGEST
    // yoluyla gelince, integrate_vertex -> kalkana_yonlendir otomatik calisir
    // ve token registry'ye islenir. Bu, dagitik kalkanin temeli: token kaydi
    // bir vertex olarak agda yayilir, her node ingest edince ayni registry'yi kurar.
    #[test]
    fn ingest_yoluyla_gelen_token_kalkana_islenir() {
        use crate::registry::public_key_to_adres;
        use crate::tx::StakeKaydi;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        // 9d: token kaydeden ONCE stake etmeli. Kaydeden = imzalayan (sk2).
        let sk2 = SigningKey::from_bytes(&[2u8; 32]);
        let kaydeden_adres = public_key_to_adres(&sk2.verifying_key().to_bytes());
        // Once stake et (dogrudan defter; gercekte tip=3 vertex ile de olur)
        node.stake_ekle(StakeKaydi::new(kaydeden_adres, 1000));

        // tip=2 token vertex'i sk2 ile imzalanir -> kaydeden_adres ile eslesir
        let payload = TokenKaydi::new([0xAA; 20], sym("USDC")).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk2).expect("token vertex");
        assert!(matches!(
            node.ingest_networked(&wire::encode(&v), now),
            NetworkIngestOutcome::Integrated(_)
        ));

        // KANIT: stake'li kaydeden -> token registry'ye islendi
        assert_eq!(node.token_sayisi(), 1);
    }

    // 9d KANIT: STAKE ETMEYEN kaydedicinin token'i REDDEDILIR (deftere girmez).
    #[test]
    fn ingest_stake_etmeyen_token_reddedilir() {
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        // sk3 STAKE ETMEDEN token kaydetmeye calisir
        let sk3 = SigningKey::from_bytes(&[3u8; 32]);
        let payload = TokenKaydi::new([0xAA; 20], sym("USDC")).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk3).expect("token vertex");
        node.ingest_networked(&wire::encode(&v), now);

        // KANIT: stake yok -> kayit HAKKI yok -> token registry'ye GIRMEDI
        assert_eq!(node.token_sayisi(), 0);
    }

    #[test]
    fn ingest_yoluyla_gelen_sahte_token_reddedilir() {
        use crate::registry::public_key_to_adres;
        use crate::tx::StakeKaydi;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        // Gercek USDC kaydeden (sk_a) stake eder + kaydeder
        let sk_a = SigningKey::from_bytes(&[10u8; 32]);
        let adr_a = public_key_to_adres(&sk_a.verifying_key().to_bytes());
        node.stake_ekle(StakeKaydi::new(adr_a, 1000));
        let p1 = TokenKaydi::new([0xAA; 20], sym("USDC")).encode();
        let v1 = Vertex::new_signed(NET, vec![gid], p1, now, &sk_a).expect("v1");
        node.ingest_networked(&wire::encode(&v1), now);
        assert_eq!(node.token_sayisi(), 1);

        // Sahte USDC kaydeden (sk_b) stake eder + kaydetmeye calisir (ayni sembol farkli adres)
        let sk_b = SigningKey::from_bytes(&[11u8; 32]);
        let adr_b = public_key_to_adres(&sk_b.verifying_key().to_bytes());
        node.stake_ekle(StakeKaydi::new(adr_b, 1000));
        let p2 = TokenKaydi::new([0xBB; 20], sym("USDC")).encode();
        let v2 = Vertex::new_signed(NET, vec![*v1.id()], p2, now + 1, &sk_b).expect("v2");
        node.ingest_networked(&wire::encode(&v2), now + 1);

        // KANIT: stake'li olsa bile TAKLIT reddedilir (sahte deftere girmez)
        assert_eq!(node.token_sayisi(), 1);
        assert_eq!(
            node.token_taklit_mi(&TokenKaydi::new([0xBB; 20], sym("USDC"))),
            Some([0xAA; 20])
        );
    }

    // 9e SLASHING KANIT: stake'li bir adres TAKLIT token kaydetmeye kalkisir ->
    // token reddedilir VE adresin TUM stake'i YAKILIR (sahteciligin bedeli agir).
    #[test]
    fn taklit_deneyenin_stakei_yakilir() {
        use crate::registry::public_key_to_adres;
        use crate::tx::StakeKaydi;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        // STAKE ARTIK DAG'DAN (tip=3 vertex) — arka kapi DEGIL. Boylece
        // durum yeniden uygulanirken stake dogru sirada kurulur ve SLASH
        // (yakma) geri alinmaz. Vertex'ler ZINCIR olusturur (kardes degil).

        // Gercek USDC kaydeden (sk_a): once STAKE vertex'i, sonra token kaydi
        let sk_a = SigningKey::from_bytes(&[10u8; 32]);
        let adr_a = public_key_to_adres(&sk_a.verifying_key().to_bytes());
        let ps_a = StakeKaydi::new(adr_a, 1000).encode();
        let vs_a = Vertex::new_signed(NET, vec![gid], ps_a, now, &sk_a).expect("vs_a");
        node.ingest_networked(&wire::encode(&vs_a), now);
        assert_eq!(
            node.stake_miktari(&adr_a),
            1000,
            "adr_a stake DAG'dan geldi"
        );

        let p1 = TokenKaydi::new([0xAA; 20], sym("USDC")).encode();
        let v1 = Vertex::new_signed(NET, vec![*vs_a.id()], p1, now + 1, &sk_a).expect("v1");
        node.ingest_networked(&wire::encode(&v1), now + 1);
        assert_eq!(node.token_sayisi(), 1);

        // Sahteci (sk_b): STAKE vertex'i (5000), sonra TAKLIT USDC kaydi
        let sk_b = SigningKey::from_bytes(&[11u8; 32]);
        let adr_b = public_key_to_adres(&sk_b.verifying_key().to_bytes());
        let ps_b = StakeKaydi::new(adr_b, 5000).encode();
        let vs_b = Vertex::new_signed(NET, vec![*v1.id()], ps_b, now + 2, &sk_b).expect("vs_b");
        node.ingest_networked(&wire::encode(&vs_b), now + 2);
        assert_eq!(node.stake_miktari(&adr_b), 5000); // stake'i var

        let p2 = TokenKaydi::new([0xBB; 20], sym("USDC")).encode(); // ayni sembol farkli adres = TAKLIT
        let v2 = Vertex::new_signed(NET, vec![*vs_b.id()], p2, now + 3, &sk_b).expect("v2");
        node.ingest_networked(&wire::encode(&v2), now + 3);

        // KANIT 1: taklit token deftere GIRMEDI (hala 1)
        assert_eq!(node.token_sayisi(), 1);
        // KANIT 2: sahtecinin TUM stake'i YAKILDI (5000 -> 0)
        assert_eq!(node.stake_miktari(&adr_b), 0);
        assert!(!node.stake_var_mi(&adr_b));
        // KANIT 3: durust kaydedicinin (adr_a) stake'i DOKUNULMADI
        assert_eq!(node.stake_miktari(&adr_a), 1000);
    }

    // STAKING node-seviyesi: dogrudan stake_ekle + sorgu
    #[test]
    fn node_stake_ekle_ve_sorgu() {
        use crate::tx::StakeKaydi;
        let mut node = NodeState::new_devnet(NET);
        assert!(!node.stake_var_mi(&[0xAA; 20]));
        node.stake_ekle(StakeKaydi::new([0xAA; 20], 1000));
        assert!(node.stake_var_mi(&[0xAA; 20]));
        assert_eq!(node.stake_miktari(&[0xAA; 20]), 1000);
        assert_eq!(node.toplam_stake(), 1000);
    }

    // STAKING ag entegrasyonu: tip=3 StakeKaydi payload'li vertex INGEST yoluyla
    // gelince, kalkana_yonlendir otomatik stake defterine isler (token gibi).
    #[test]
    fn ingest_yoluyla_gelen_stake_islenir() {
        use crate::tx::StakeKaydi;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);
        assert_eq!(node.toplam_stake(), 0);

        let sk = SigningKey::from_bytes(&[5u8; 32]);
        let payload = StakeKaydi::new([0xAA; 20], 5000).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk).expect("stake vertex");
        assert!(matches!(
            node.ingest_networked(&wire::encode(&v), now),
            NetworkIngestOutcome::Integrated(_)
        ));

        assert_eq!(node.stake_miktari(&[0xAA; 20]), 5000);
        assert!(node.stake_var_mi(&[0xAA; 20]));
        assert_eq!(node.toplam_stake(), 5000);
    }
    use crate::dag::vertex::Vertex;
    use crate::dag::wire;
    use ed25519_dalek::SigningKey;

    const NET: u32 = 1;

    fn signed_genesis_bytes(now: u64) -> Vec<u8> {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let v = Vertex::new_signed(NET, vec![], b"genesis-payload".to_vec(), now, &sk)
            .expect("genesis vertex");
        wire::encode(&v)
    }

    #[test]
    fn new_devnet_starts_empty_no_genesis() {
        let node = NodeState::new_devnet(NET);
        assert_eq!(node.vertex_count(), 0);
        assert!(node.genesis_id().is_none());
        assert_eq!(node.network_id(), NET);
    }

    #[test]
    fn ingest_first_vertex_establishes_genesis() {
        let mut node = NodeState::new_devnet(NET);
        let now = 1_000_000;
        let bytes = signed_genesis_bytes(now);
        let id = node.ingest(&bytes, now).expect("ingest genesis");
        assert_eq!(node.vertex_count(), 1);
        assert_eq!(node.genesis_id(), Some(&id));
        assert!(node.contains(&id));
    }

    #[test]
    fn ingest_garbage_leaves_state_untouched() {
        let mut node = NodeState::new_devnet(NET);
        let now = 1_000_000;
        let res = node.ingest(b"not-a-valid-vertex", now);
        assert!(res.is_err());
        assert_eq!(node.vertex_count(), 0);
        assert!(node.genesis_id().is_none());
    }

    #[test]
    fn duplicate_ingest_rejected_count_stable() {
        let mut node = NodeState::new_devnet(NET);
        let now = 1_000_000;
        let bytes = signed_genesis_bytes(now);
        node.ingest(&bytes, now).expect("first ingest ok");
        assert_eq!(node.vertex_count(), 1);
        let res = node.ingest(&bytes, now);
        assert!(res.is_err());
        assert_eq!(node.vertex_count(), 1);
    }

    // ===== ORPHAN-BILINCLI AG INGEST TESTLERI (ingest_networked) =====

    // Genesis (parent'siz) baytlari uret.
    fn genesis_bytes(tag: u8, now: u64) -> (Vec<u8>, VertexId) {
        let sk = SigningKey::from_bytes(&[tag; 32]);
        let v = Vertex::new_signed(NET, vec![], vec![tag, 1], now, &sk).expect("genesis");
        let id = *v.id();
        (wire::encode(&v), id)
    }

    // Belirli parent'larla child baytlari uret.
    fn child_bytes(parents: Vec<VertexId>, tag: u8, now: u64) -> (Vec<u8>, VertexId) {
        let sk = SigningKey::from_bytes(&[tag; 32]);
        let v = Vertex::new_signed(NET, parents, vec![tag, 2], now, &sk).expect("child");
        let id = *v.id();
        (wire::encode(&v), id)
    }

    // ===== PARALEL/DIAMOND HIZ OLCUMU (dolu mergeset) =====
    // Zincirden farkli: her kat W paralel vertex; her vertex onceki katin TUM
    // vertex'lerini parent alir -> mergeset DOLU -> anticone/blue kisa-devreleri
    // DEVREYE GIRMEZ, gercek hesap yolu test edilir. Hem dogruluk hem hiz.
    #[test]
    #[ignore]
    fn tps_olcum_paralel() {
        use std::time::Instant;
        let now = 1_000_000u64;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let katlar: u64 = 10; // kat sayisi
        let w: u64 = 5; // her katta paralel vertex
        let sk = SigningKey::from_bytes(&[7u8; 32]);

        let t_uret = Instant::now();
        let mut prev_kat: Vec<VertexId> = vec![gid];
        let mut vertices: Vec<Vec<u8>> = Vec::new();
        let mut ts = now + 1;
        for _k in 0..katlar {
            let mut bu_kat: Vec<VertexId> = Vec::new();
            // parent = onceki katin TUM vertex'leri (sirali, kanonik).
            let mut parents = prev_kat.clone();
            parents.sort_unstable();
            for j in 0..w {
                let payload = (j as u32).to_le_bytes().to_vec();
                let v = Vertex::new_signed(NET, parents.clone(), payload, ts, &sk).expect("vertex");
                ts += 1;
                bu_kat.push(*v.id());
                vertices.push(wire::encode(&v));
            }
            prev_kat = bu_kat;
        }
        let uret_sure = t_uret.elapsed().as_secs_f64();
        let toplam = vertices.len() as u64;

        let t_ingest = Instant::now();
        for bytes in &vertices {
            let _ = node.ingest_networked(bytes, ts);
        }
        let ingest_sure = t_ingest.elapsed().as_secs_f64();

        println!("\n===== PARALEL HIZ OLCUMU =====");
        println!("Kat={katlar} W={w} Toplam vertex={toplam}");
        println!("Uretim : {uret_sure:.3}s");
        println!(
            "INGEST : {ingest_sure:.3}s  ({:.0} TPS)",
            toplam as f64 / ingest_sure
        );
        println!("vertex_count: {}", node.vertex_count());
        println!("==============================\n");
        assert_eq!(node.vertex_count() as u64, toplam + 1);
    }

    // ===== OLCEK EGRISI: DAG buyudukce TPS nasil degisiyor =====
    // Elle calistir: cargo test --release olcek_egrisi -- --ignored --nocapture
    // Farkli n degerleri icin saf ingest TPS'i olcer (lineer zincir, W=1).
    // Amac: GHOSTDAG maliyeti DAG buyudukce TPS'i dusuruyor mu gormek.
    #[test]
    #[ignore]
    fn olcek_egrisi() {
        use std::time::Instant;
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        println!("\n===== OLCEK EGRISI (lineer zincir, saf ingest) =====");
        println!("{:>8} | {:>10} | {:>12}", "vertex", "sure(s)", "TPS");
        for &n in &[100u64, 1000, 5000, 10_000] {
            let now = 1_000_000u64;
            let mut node = NodeState::new_devnet(NET);
            let (gen, gid) = genesis_bytes(1, now);
            node.ingest_networked(&gen, now);
            // onceden uret
            let mut parent = gid;
            let mut vertices: Vec<Vec<u8>> = Vec::with_capacity(n as usize);
            for i in 0..n {
                let payload = (i as u32).to_le_bytes().to_vec();
                let v =
                    Vertex::new_signed(NET, vec![parent], payload, now + 1 + i, &sk).expect("v");
                parent = *v.id();
                vertices.push(wire::encode(&v));
            }
            // sadece ingest olc
            let t = Instant::now();
            for bytes in &vertices {
                let _ = node.ingest_networked(bytes, now + 1 + n);
            }
            let sure = t.elapsed().as_secs_f64();
            let tps = n as f64 / sure;
            println!("{:>8} | {:>10.3} | {:>12.0}", n, sure, tps);
        }
        println!("====================================================");
        println!("NOT: TPS dususse DAG-buyume maliyeti (GHOSTDAG) var demektir.");
    }

    #[test]
    #[ignore]
    fn diskli_olcum() {
        use std::io::Read;
        use std::io::Write;
        use std::time::Instant;
        let dizin = "/tmp/aidag_diskli_test";
        let _ = std::fs::remove_dir_all(dizin);
        std::fs::create_dir_all(dizin).expect("dizin");
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        println!("\n===== DISKLI UCTAN-UCA OLCUM (imzali + disk I/O) =====");
        println!(
            "{:>8} | {:>10} | {:>12} | {:>12}",
            "vertex", "yaz(s)", "oku+ing(s)", "TPS"
        );
        for &n in &[1000u64, 5000, 10_000] {
            let now = 1_000_000u64;
            let mut node = NodeState::new_devnet(NET);
            let (gen, gid) = genesis_bytes(1, now);
            node.ingest_networked(&gen, now);
            let mut parent = gid;
            let mut paths: Vec<String> = Vec::with_capacity(n as usize);
            let t_yaz = Instant::now();
            for i in 0..n {
                let payload = (i as u32).to_le_bytes().to_vec();
                let v =
                    Vertex::new_signed(NET, vec![parent], payload, now + 1 + i, &sk).expect("v");
                parent = *v.id();
                let bytes = wire::encode(&v);
                let p = format!("{}/v{:08}.bin", dizin, i);
                let mut f = std::fs::File::create(&p).expect("create");
                f.write_all(&bytes).expect("write");
                f.sync_all().expect("sync");
                paths.push(p);
            }
            let yaz_sure = t_yaz.elapsed().as_secs_f64();
            let t_oku = Instant::now();
            for p in &paths {
                let mut buf = Vec::new();
                let mut f = std::fs::File::open(p).expect("open");
                f.read_to_end(&mut buf).expect("read");
                let _ = node.ingest_networked(&buf, now + 1 + n);
            }
            let oku_sure = t_oku.elapsed().as_secs_f64();
            let tps = n as f64 / (yaz_sure + oku_sure);
            println!(
                "{:>8} | {:>10.3} | {:>12.3} | {:>12.0}",
                n, yaz_sure, oku_sure, tps
            );
            assert_eq!(node.vertex_count() as u64, n + 1);
        }
        let _ = std::fs::remove_dir_all(dizin);
        println!("====================================================");
        println!("NOT: uctan-uca (imza + disk yaz + disk oku + decode + GHOSTDAG ingest)");
    }

    #[test]
    #[ignore]
    fn diskli_olcum_batch() {
        use std::io::Read;
        use std::io::Write;
        use std::time::Instant;
        let yol = "/tmp/aidag_batch_test.bin";
        let _ = std::fs::remove_file(yol);
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        println!("\n===== DISKLI BATCH OLCUM (imzali + tek dosya + tek sync) =====");
        println!(
            "{:>8} | {:>10} | {:>12} | {:>12}",
            "vertex", "yaz(s)", "oku+ing(s)", "TPS"
        );
        for &n in &[1000u64, 5000, 10_000] {
            let now = 1_000_000u64;
            let mut node = NodeState::new_devnet(NET);
            let (gen, gid) = genesis_bytes(1, now);
            node.ingest_networked(&gen, now);
            let mut parent = gid;
            let mut kayitlar: Vec<Vec<u8>> = Vec::with_capacity(n as usize);
            for i in 0..n {
                let payload = (i as u32).to_le_bytes().to_vec();
                let v =
                    Vertex::new_signed(NET, vec![parent], payload, now + 1 + i, &sk).expect("v");
                parent = *v.id();
                kayitlar.push(wire::encode(&v));
            }
            // BATCH YAZMA: tek dosyaya [uzunluk][veri]... + sonunda TEK sync
            let t_yaz = Instant::now();
            {
                let f = std::fs::File::create(yol).expect("create");
                let mut bw = std::io::BufWriter::new(f);
                for b in &kayitlar {
                    bw.write_all(&(b.len() as u32).to_le_bytes()).expect("len");
                    bw.write_all(b).expect("veri");
                }
                bw.flush().expect("flush");
                bw.get_ref().sync_all().expect("sync");
            }
            let yaz_sure = t_yaz.elapsed().as_secs_f64();
            // OKUMA + INGEST: tek dosyadan [uzunluk][veri]... oku, ingest et
            let t_oku = Instant::now();
            let mut buf = Vec::new();
            std::fs::File::open(yol)
                .expect("open")
                .read_to_end(&mut buf)
                .expect("read");
            let mut off = 0usize;
            for _ in 0..n {
                let len = u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
                    as usize;
                off += 4;
                let _ = node.ingest_networked(&buf[off..off + len], now + 1 + n);
                off += len;
            }
            let oku_sure = t_oku.elapsed().as_secs_f64();
            let tps = n as f64 / (yaz_sure + oku_sure);
            println!(
                "{:>8} | {:>10.3} | {:>12.3} | {:>12.0}",
                n, yaz_sure, oku_sure, tps
            );
            assert_eq!(node.vertex_count() as u64, n + 1);
        }
        let _ = std::fs::remove_file(yol);
        println!("====================================================");
        println!("NOT: BATCH (tek dosya + tek sync) - naive per-vertex fsync ile kiyasla");
    }

    #[test]
    #[ignore]
    fn buyuk_olcek() {
        use std::time::Instant;
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        println!("\n===== BUYUK OLCEK (saf ingest, imzali) =====");
        println!("{:>12} | {:>10} | {:>12}", "vertex", "sure(s)", "TPS");
        for &n in &[1_000_000u64, 2_000_000, 5_000_000, 10_000_000] {
            let now = 1_000_000u64;
            let mut node = NodeState::new_devnet(NET);
            let (gen, gid) = genesis_bytes(1, now);
            node.ingest_networked(&gen, now);
            let mut parent = gid;
            let t = Instant::now();
            for i in 0..n {
                let payload = (i as u32).to_le_bytes().to_vec();
                let v =
                    Vertex::new_signed(NET, vec![parent], payload, now + 1 + i, &sk).expect("v");
                parent = *v.id();
                let bytes = wire::encode(&v);
                let _ = node.ingest_networked(&bytes, now + 1 + i);
            }
            let sure = t.elapsed().as_secs_f64();
            let tps = n as f64 / sure;
            println!("{:>12} | {:>10.2} | {:>12.0}  [tamam]", n, sure, tps);
        }
        println!("====================================================");
    }

    // ===== HIZ OLCUMU (TPS) =====
    // Elle calistir: cargo test --release acilis_profili -- --ignored --nocapture
    // Node ACILIS profili: N vertex'i diskten yukleme simulasyonu. Zamanin
    // nereye gittigini AYRI olcer: decode / verify(imza) / integrate(DAG+GHOSTDAG).
    // Paralel-verify optimizasyonu DEGER mi? -> verify payi buyukse EVET.
    #[test]
    #[ignore]
    fn acilis_profili() {
        use std::time::Instant;
        let now = 1_000_000u64;
        let n: u64 = 20_000;
        let sk = SigningKey::from_bytes(&[7u8; 32]);

        // Once N vertex uret (lineer zincir) - bu olcum disi.
        let (gen, gid) = genesis_bytes(1, now);
        let mut tmp = NodeState::new_devnet(NET);
        tmp.ingest_networked(&gen, now);
        let mut parent = gid;
        let mut wire_bytes: Vec<Vec<u8>> = Vec::with_capacity(n as usize);
        for i in 0..n {
            let payload = (i as u32).to_le_bytes().to_vec();
            let v = Vertex::new_signed(NET, vec![parent], payload, now + 1 + i, &sk).expect("v");
            parent = *v.id();
            wire_bytes.push(wire::encode(&v));
        }

        // 1) DECODE suresi (sadece wire cozme).
        let t = Instant::now();
        let mut decoded: Vec<crate::dag::vertex::Vertex> = Vec::with_capacity(n as usize);
        for b in &wire_bytes {
            decoded.push(wire::decode(b).expect("decode"));
        }
        let decode_s = t.elapsed().as_secs_f64();

        // 2) VERIFY suresi (sadece ed25519+blake3 imza dogrulama, seri).
        let t = Instant::now();
        for v in &decoded {
            v.verify().expect("verify");
        }
        let verify_s = t.elapsed().as_secs_f64();

        // 2b) PARALEL VERIFY suresi (rayon, cok cekirdek) - karsilastirma.
        use rayon::prelude::*;
        let t = Instant::now();
        let hepsi_ok = decoded.par_iter().all(|v| v.verify().is_ok());
        let verify_par_s = t.elapsed().as_secs_f64();
        assert!(hepsi_ok, "paralel verify: hepsi gecmeli");
        println!("VERIFY(seri)  : {verify_s:.3}s");
        println!(
            "VERIFY(paralel): {verify_par_s:.3}s  ({:.1}x hizli)",
            verify_s / verify_par_s.max(0.0001)
        );

        // 3) INTEGRATE suresi (YENI yol: ingest_decoded_preverified - ZATEN decode
        //    edilmis Vertex alir, tekrar decode YOK). decoded[] paralel fazdan gelir.
        let mut node = NodeState::new_devnet(NET);
        node.ingest_networked(&gen, now);
        let t = Instant::now();
        for v in decoded.iter() {
            let _ = node.ingest_decoded_preverified(v.clone());
        }
        let integrate_s = t.elapsed().as_secs_f64();

        let toplam = decode_s + verify_s + integrate_s;
        println!("\n========== ACILIS PROFILI ({n} vertex) ==========");
        println!(
            "DECODE    : {decode_s:.3}s  (%{:.0})",
            100.0 * decode_s / toplam
        );
        println!(
            "VERIFY    : {verify_s:.3}s  (%{:.0})  <- paralel-verify bunu hedefler",
            100.0 * verify_s / toplam
        );
        println!(
            "INTEGRATE : {integrate_s:.3}s  (%{:.0})",
            100.0 * integrate_s / toplam
        );
        println!("TOPLAM    : {toplam:.3}s");
        println!("================================================");
        println!("vertex_count: {}", node.vertex_count());
    }

    // Elle calistir: cargo test --release tps_olcum -- --ignored --nocapture
    // Saf ingest hizi: vertex'ler ONCEDEN uretilir, SADECE ingest suresi olculur.
    // Tek-thread, ardisik (en muhafazakar/durust rakam; paralel kapasite daha yuksek olabilir).
    #[test]
    #[ignore]
    fn tps_olcum() {
        use std::time::Instant;
        let now = 1_000_000u64;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let n: u64 = 100;
        let sk = SigningKey::from_bytes(&[7u8; 32]);

        // 1) Vertex'leri ONCEDEN uret (her biri oncekine baglanir; zincir).
        let t_uret = Instant::now();
        let mut parent = gid;
        let mut vertices: Vec<Vec<u8>> = Vec::with_capacity(n as usize);
        for i in 0..n {
            // Basit payload: 4 bayt (i'nin LE gosterimi). Gercek imza + hash dahil.
            let payload = (i as u32).to_le_bytes().to_vec();
            let v =
                Vertex::new_signed(NET, vec![parent], payload, now + 1 + i, &sk).expect("vertex");
            parent = *v.id();
            vertices.push(wire::encode(&v));
        }
        let uret_sure = t_uret.elapsed().as_secs_f64();

        // 2) SADECE ingest'i olc.
        let t_ingest = Instant::now();
        for bytes in &vertices {
            let _ = node.ingest_networked(bytes, now + 1 + n);
        }
        let ingest_sure = t_ingest.elapsed().as_secs_f64();

        let ingest_tps = n as f64 / ingest_sure;
        let uret_tps = n as f64 / uret_sure;

        println!("\n========== AIDAG-CHAIN HIZ OLCUMU ==========");
        println!("Vertex sayisi      : {n}");
        println!("Uretim (imza+hash) : {uret_sure:.3}s  ({uret_tps:.0} vertex/s)");
        println!("INGEST (saf)       : {ingest_sure:.3}s  ({ingest_tps:.0} TPS)");
        println!("Son vertex_count   : {}", node.vertex_count());
        println!("=============================================\n");

        assert_eq!(node.vertex_count() as u64, n + 1); // genesis + n
    }

    #[test]
    fn networked_orphan_buffered_when_parent_missing() {
        let mut node = NodeState::new_devnet(NET);
        let now = 1_000_000;
        // Once bir genesis kur (graf'ta bir sey olsun).
        let (gb, gid) = genesis_bytes(1, now);
        assert!(matches!(
            node.ingest_networked(&gb, now),
            NetworkIngestOutcome::Integrated(_)
        ));

        // Olmayan bir ebeveyni bekleyen child -> Buffered, graf DEGISMEZ.
        let missing_parent = [99u8; 32];
        let (cb, cid) = child_bytes(vec![missing_parent], 2, now);
        let out = node.ingest_networked(&cb, now);
        assert!(matches!(out, NetworkIngestOutcome::Buffered(_)));
        assert_eq!(node.vertex_count(), 1); // sadece genesis
        assert_eq!(node.orphan_count(), 1); // child bekliyor
        assert!(!node.contains(&cid));
        let _ = gid;
    }

    #[test]
    fn networked_integrates_when_parent_present() {
        let mut node = NodeState::new_devnet(NET);
        let now = 1_000_000;
        let (gb, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gb, now);

        // Genesis'i parent alan child -> ebeveyn HAZIR -> Integrated.
        let (cb, cid) = child_bytes(vec![gid], 2, now);
        let out = node.ingest_networked(&cb, now);
        assert!(matches!(out, NetworkIngestOutcome::Integrated(_)));
        assert_eq!(node.vertex_count(), 2);
        assert!(node.contains(&cid));
        assert_eq!(node.orphan_count(), 0);
    }

    #[test]
    fn networked_cascade_resolves_out_of_order_chain() {
        // genesis <- B <- C. Ama SIRASIZ gelir: once C (B'yi bekler),
        // sonra B (genesis'i bekler). Genesis zaten var.
        let mut node = NodeState::new_devnet(NET);
        let now = 1_000_000;
        let (gb, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gb, now);

        let (bb, bid) = child_bytes(vec![gid], 2, now);
        let (cb, cid) = child_bytes(vec![bid], 3, now);

        // C once gelir: B yok -> Buffered.
        assert!(matches!(
            node.ingest_networked(&cb, now),
            NetworkIngestOutcome::Buffered(_)
        ));
        assert_eq!(node.vertex_count(), 1);
        assert_eq!(node.orphan_count(), 1);

        // B gelir: genesis hazir -> Integrated; cascade ile C de cozulur.
        let out = node.ingest_networked(&bb, now);
        assert!(matches!(out, NetworkIngestOutcome::Integrated(_)));
        // Hem B hem C graf'a girmis olmali (genesis + B + C = 3).
        assert_eq!(node.vertex_count(), 3);
        assert!(node.contains(&bid));
        assert!(node.contains(&cid));
        assert_eq!(node.orphan_count(), 0);
    }

    #[test]
    fn networked_duplicate_detected() {
        let mut node = NodeState::new_devnet(NET);
        let now = 1_000_000;
        let (gb, _gid) = genesis_bytes(1, now);
        assert!(matches!(
            node.ingest_networked(&gb, now),
            NetworkIngestOutcome::Integrated(_)
        ));
        // Ayni vertex tekrar -> Duplicate.
        assert!(matches!(
            node.ingest_networked(&gb, now),
            NetworkIngestOutcome::Duplicate(_)
        ));
        assert_eq!(node.vertex_count(), 1);
    }

    #[test]
    fn networked_garbage_rejected() {
        let mut node = NodeState::new_devnet(NET);
        let now = 1_000_000;
        let out = node.ingest_networked(b"not-a-vertex", now);
        assert!(matches!(out, NetworkIngestOutcome::Rejected(_)));
        assert_eq!(node.vertex_count(), 0);
        assert_eq!(node.orphan_count(), 0);
    }

    // ===== KALICILIK: export -> yeniden yukle round-trip (Adim 1) =====

    #[test]
    fn export_empty_node_is_empty() {
        let node = NodeState::new_devnet(NET);
        assert!(node.export_vertices().is_empty());
    }

    #[test]
    fn export_reimport_roundtrip_preserves_dag() {
        let now = 1_000_000;
        // 1) Kaynak node: genesis + 3 vertex'lik bir zincir kur.
        let mut src = NodeState::new_devnet(NET);
        let (gb, gid) = genesis_bytes(1, now);
        assert!(matches!(
            src.ingest_networked(&gb, now),
            NetworkIngestOutcome::Integrated(_)
        ));
        let (b1, id1) = child_bytes(vec![gid], 2, now);
        let (b2, id2) = child_bytes(vec![id1], 3, now);
        let (b3, id3) = child_bytes(vec![id2], 4, now);
        src.ingest_networked(&b1, now);
        src.ingest_networked(&b2, now);
        src.ingest_networked(&b3, now);
        assert_eq!(src.vertex_count(), 4);

        // 2) Disa aktar (kaliciligin "kaydet" adimi).
        let exported = src.export_vertices();
        assert_eq!(exported.len(), 4);

        // 3) YENI node'a yukle (kaliciligin "yukle" adimi). SIRASIZ yukle —
        //    export sirasi garantisiz; orphan+cascade sirasizligi cozmeli.
        let mut dst = NodeState::new_devnet(NET);
        for bytes in &exported {
            dst.ingest_networked(bytes, now);
        }
        // Henuz orphan'da bekleyen kalmissa, ekstra tur (sirasizlik guvencesi).
        // (Tek tur cogu durumda yeter; bu, kati round-trip kaniti.)

        // 4) Dogrulama: ayni DAG yeniden kuruldu.
        assert_eq!(dst.vertex_count(), 4, "tum vertex'ler yuklendi");
        assert_eq!(dst.orphan_count(), 0, "hicbiri orphan'da kalmadi");
        assert!(dst.contains(&gid));
        assert!(dst.contains(&id1));
        assert!(dst.contains(&id2));
        assert!(dst.contains(&id3));
        assert_eq!(src.genesis_id(), dst.genesis_id(), "genesis ayni");
    }

    #[test]
    fn ingest_synced_ignores_clock_policy_replay() {
        // REGRESYON: kalicilik bugu. Vertex'ler T1'de uretildi; cok daha sonra
        // (T2) farkli bir now ile YENIDEN yuklenir. ingest_synced saat
        // politikasini UYGULAMADIGI icin eski timestamp'ler reddedilmemeli.
        let t1 = 1_000_000u64;
        let mut src = NodeState::new_devnet(NET);
        let (gb, gid) = genesis_bytes(1, t1);
        src.ingest_networked(&gb, t1);
        let (b1, id1) = child_bytes(vec![gid], 2, t1);
        let (b2, _id2) = child_bytes(vec![id1], 3, t1);
        src.ingest_networked(&b1, t1);
        src.ingest_networked(&b2, t1);
        assert_eq!(src.vertex_count(), 3);

        let exported = src.export_vertices();

        // YENI node: ingest_synced ile yukle (now PARAMETRESI YOK -> saat
        // politikasi devre disi). Sirasiz olsa da yakinsama icin tekrar dene.
        let mut dst = NodeState::new_devnet(NET);
        let mut remaining: Vec<&Vec<u8>> = exported.iter().collect();
        loop {
            let before = dst.vertex_count();
            let mut pending = Vec::new();
            for bytes in remaining.drain(..) {
                match dst.ingest_synced(bytes) {
                    NetworkIngestOutcome::Integrated(_) | NetworkIngestOutcome::Duplicate(_) => {}
                    _ => pending.push(bytes),
                }
            }
            remaining = pending;
            if dst.vertex_count() == before || remaining.is_empty() {
                break;
            }
        }

        // Tum vertex'ler yuklendi, hicbiri orphan'da kalmadi (saat reddi YOK).
        assert_eq!(
            dst.vertex_count(),
            3,
            "synced replay tum vertex'leri yukledi"
        );
        assert_eq!(
            dst.orphan_count(),
            0,
            "saat politikasi replay'de reddetmedi"
        );
    }

    // ===== TRANSFER ingest entegrasyonu =====

    // Bir tip=4 transfer vertex'i ingest edilince, gonderen (imzalayan)
    // bakiyesinden duser, alici'ya eklenir. Gonderen = imzalayan (B modeli).
    #[test]
    fn ingest_transfer_bakiye_gunceller() {
        use crate::registry::public_key_to_adres;
        use crate::tx::TransferKaydi;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        // Gonderen = sk5'in adresi; ona test bakiyesi ver.
        let sk5 = SigningKey::from_bytes(&[5u8; 32]);
        let gonderen = public_key_to_adres(&sk5.verifying_key().to_bytes());
        let alici = [0xEE; 20];
        node.test_bakiye_ekle(gonderen, 1000);
        assert_eq!(node.bakiye(&gonderen), 1000);

        // tip=4 transfer vertex'i: sk5 imzalar (gonderen=imzalayan), alici'ya 300.
        let payload = TransferKaydi::new(alici, 300, 0).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk5).expect("transfer vertex");
        assert!(matches!(
            node.ingest_networked(&wire::encode(&v), now),
            NetworkIngestOutcome::Integrated(_)
        ));

        // KANIT: gonderenden dustu, alici'ya eklendi, TOPLAM ARZ korundu.
        assert_eq!(node.bakiye(&gonderen), 700);
        assert_eq!(node.bakiye(&alici), 300);
        assert_eq!(node.toplam_bakiye_arzi(), 1000);
    }

    // LSC TRANSFER (tip=7): LSC defteri ayri calisir. lsc bakiyesinden duser,
    // alici'ya eklenir. AIDAG transferiyle ayni mantik, ayri defter.
    #[test]
    fn ingest_lsc_transfer_bakiye_gunceller() {
        use crate::registry::public_key_to_adres;
        use crate::tx::LscTransferKaydi;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let sk5 = SigningKey::from_bytes(&[5u8; 32]);
        let gonderen = public_key_to_adres(&sk5.verifying_key().to_bytes());
        let alici = [0xEE; 20];
        node.lsc_test_bakiye_ekle(gonderen, 1000);
        assert_eq!(node.lsc_bakiye(&gonderen), 1000);

        let payload = LscTransferKaydi::new(alici, 300, 0).encode();
        let v =
            Vertex::new_signed(NET, vec![gid], payload, now, &sk5).expect("lsc transfer vertex");
        assert!(matches!(
            node.ingest_networked(&wire::encode(&v), now),
            NetworkIngestOutcome::Integrated(_)
        ));

        assert_eq!(node.lsc_bakiye(&gonderen), 700);
        assert_eq!(node.lsc_bakiye(&alici), 300);
        assert_eq!(node.lsc_toplam_arzi(), 1000);
        assert_eq!(node.bakiye(&gonderen), 0);
        assert_eq!(node.bakiye(&alici), 0);
    }

    // CIFT HARCAMA: bakiyesi olmayan/yetersiz gonderenin transfer'i bakiyeyi
    // DEGISTIRMEZ (vertex DAG'a girse de para yaratilmaz/kaybolmaz).
    #[test]
    fn ingest_transfer_yetersiz_bakiye_degistirmez() {
        use crate::registry::public_key_to_adres;
        use crate::tx::TransferKaydi;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let sk6 = SigningKey::from_bytes(&[6u8; 32]);
        let gonderen = public_key_to_adres(&sk6.verifying_key().to_bytes());
        let alici = [0xDD; 20];
        node.test_bakiye_ekle(gonderen, 100); // sadece 100 var

        // 500 gondermeye calis -> bakiye yetersiz, transfer gecersiz.
        let payload = TransferKaydi::new(alici, 500, 0).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk6).expect("transfer vertex");
        // Vertex DAG'a girer (gecerli imza/format) ama bakiye DEGISMEZ.
        node.ingest_networked(&wire::encode(&v), now);

        assert_eq!(
            node.bakiye(&gonderen),
            100,
            "yetersiz transfer bakiyeyi degistirmedi"
        );
        assert_eq!(node.bakiye(&alici), 0, "alici hicbir sey almadi");
        assert_eq!(node.toplam_bakiye_arzi(), 100, "arz korundu");
    }

    // GUVENLIK (B modeli): baskasinin parasini gonderemezsin. Imzalayan=gonderen.
    // sk7 imzalarsa, sk8'in bakiyesi ASLA harcanmaz (sk7'nin adresi gonderen olur).
    #[test]
    fn ingest_transfer_baskasinin_parasi_harcanamaz() {
        use crate::registry::public_key_to_adres;
        use crate::tx::TransferKaydi;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        // sk8'in 1000 bakiyesi var; sk7'nin 0.
        let sk7 = SigningKey::from_bytes(&[7u8; 32]);
        let sk8 = SigningKey::from_bytes(&[8u8; 32]);
        let adr7 = public_key_to_adres(&sk7.verifying_key().to_bytes());
        let adr8 = public_key_to_adres(&sk8.verifying_key().to_bytes());
        node.test_bakiye_ekle(adr8, 1000);

        // sk7 imzalar, alici kendisi (adr7), 500 ister. Gonderen=imzalayan=adr7,
        // adr7'nin bakiyesi 0 -> transfer gecersiz. sk8'in parasi DOKUNULMAZ.
        let payload = TransferKaydi::new(adr7, 500, 0).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk7).expect("transfer vertex");
        node.ingest_networked(&wire::encode(&v), now);

        assert_eq!(
            node.bakiye(&adr8),
            1000,
            "sk8'in parasi sk7 tarafindan harcanamadi"
        );
        assert_eq!(
            node.bakiye(&adr7),
            0,
            "sk7'nin bakiyesi yoktu, transfer olmadi"
        );
    }

    // ===== REPLAY KORUMASI (nonce) testleri =====

    // Ayni nonce'lu transfer iki kez yayilirsa (farkli vertex/timestamp ile),
    // IKINCISI bakiyeyi DEGISTIRMEZ. Replay etkisiz. Sirali nonce ilerler.
    #[test]
    fn ingest_transfer_replay_reddedilir() {
        use crate::registry::public_key_to_adres;
        use crate::tx::TransferKaydi;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let sk5 = SigningKey::from_bytes(&[5u8; 32]);
        let gonderen = public_key_to_adres(&sk5.verifying_key().to_bytes());
        let alici = [0xEE; 20];
        node.test_bakiye_ekle(gonderen, 1000);
        assert_eq!(node.beklenen_nonce(&gonderen), 0, "baslangic nonce 0");

        // 1) nonce=0 transfer -> basarili, bakiye duser, nonce ilerler.
        let p0 = TransferKaydi::new(alici, 300, 0).encode();
        let v0 = Vertex::new_signed(NET, vec![gid], p0, now, &sk5).expect("v0");
        node.ingest_networked(&wire::encode(&v0), now);
        assert_eq!(node.bakiye(&gonderen), 700, "ilk transfer dustu");
        assert_eq!(node.bakiye(&alici), 300);
        assert_eq!(node.beklenen_nonce(&gonderen), 1, "nonce 1'e ilerledi");

        // 2) AYNI nonce=0 ile FARKLI vertex (timestamp+1) -> REPLAY.
        //    beklenen artik 1; nonce=0 eslesmiyor -> bakiye DEGISMEZ.
        let p_replay = TransferKaydi::new(alici, 300, 0).encode();
        let v_replay =
            Vertex::new_signed(NET, vec![*v0.id()], p_replay, now + 1, &sk5).expect("vr");
        node.ingest_networked(&wire::encode(&v_replay), now + 1);
        assert_eq!(
            node.bakiye(&gonderen),
            700,
            "REPLAY etkisiz: bakiye degismedi"
        );
        assert_eq!(node.bakiye(&alici), 300, "alici ikinci kez almadi");
        assert_eq!(node.beklenen_nonce(&gonderen), 1, "nonce hala 1");

        // 3) nonce=1 ile YENI transfer -> basarili (sira ilerliyor).
        let p1 = TransferKaydi::new(alici, 200, 1).encode();
        let v1 = Vertex::new_signed(NET, vec![*v_replay.id()], p1, now + 2, &sk5).expect("v1");
        node.ingest_networked(&wire::encode(&v1), now + 2);
        assert_eq!(node.bakiye(&gonderen), 500, "ikinci gecerli transfer dustu");
        assert_eq!(node.bakiye(&alici), 500);
        assert_eq!(node.beklenen_nonce(&gonderen), 2, "nonce 2'ye ilerledi");

        // 4) Yanlis (atlamali) nonce=5 -> reddedilir, bakiye degismez.
        let p5 = TransferKaydi::new(alici, 100, 5).encode();
        let v5 = Vertex::new_signed(NET, vec![*v1.id()], p5, now + 3, &sk5).expect("v5");
        node.ingest_networked(&wire::encode(&v5), now + 3);
        assert_eq!(
            node.bakiye(&gonderen),
            500,
            "yanlis nonce bakiyeyi degistirmedi"
        );
        assert_eq!(node.beklenen_nonce(&gonderen), 2, "nonce hala 2");

        // ARZ korundu (yaratim/kayip yok).
        assert_eq!(node.toplam_bakiye_arzi(), 1000, "toplam arz sabit");
    }

    // Yetersiz bakiye + DOGRU nonce -> (A) kurali: nonce ILERLEMEZ,
    // kullanici ayni nonce ile tekrar deneyebilir.
    #[test]
    fn ingest_transfer_yetersiz_nonce_ilerletmez() {
        use crate::registry::public_key_to_adres;
        use crate::tx::TransferKaydi;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let sk6 = SigningKey::from_bytes(&[6u8; 32]);
        let gonderen = public_key_to_adres(&sk6.verifying_key().to_bytes());
        let alici = [0xDD; 20];
        node.test_bakiye_ekle(gonderen, 100);

        // nonce=0 dogru ama 500 > 100 -> transfer basarisiz -> nonce ILERLEMEZ.
        let p = TransferKaydi::new(alici, 500, 0).encode();
        let v = Vertex::new_signed(NET, vec![gid], p, now, &sk6).expect("v");
        node.ingest_networked(&wire::encode(&v), now);
        assert_eq!(node.bakiye(&gonderen), 100, "bakiye degismedi");
        assert_eq!(
            node.beklenen_nonce(&gonderen),
            0,
            "basarisiz transfer nonce ilerletmedi"
        );

        // Ayni nonce=0 ile gecerli (50) transfer -> simdi basarili.
        let p2 = TransferKaydi::new(alici, 50, 0).encode();
        let v2 = Vertex::new_signed(NET, vec![*v.id()], p2, now + 1, &sk6).expect("v2");
        node.ingest_networked(&wire::encode(&v2), now + 1);
        assert_eq!(
            node.bakiye(&gonderen),
            50,
            "ayni nonce ile tekrar denenebildi"
        );
        assert_eq!(node.beklenen_nonce(&gonderen), 1, "simdi nonce ilerledi");
    }

    // ===== KOPRU 4: AVM CAGRISI (tip=9) testleri =====

    // AVM cagrisi: deger hedefe gider, gas (%50 yak + %50 havuz) gonderenden
    // kesilir, nonce ilerler, TOPLAM LSC ARZI korunur (kayip/yaratim yok).
    #[test]
    fn ingest_avm_cagri_deger_ve_gas() {
        use crate::registry::public_key_to_adres;
        use crate::tx::AvmCagri;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let sk5 = SigningKey::from_bytes(&[5u8; 32]);
        let gonderen = public_key_to_adres(&sk5.verifying_key().to_bytes());
        let hedef = [0xEE; 20];
        node.test_bakiye_ekle(gonderen, 1_000_000_000_000_000); // AIDAG (deger)
        node.lsc_test_bakiye_ekle(gonderen, 1_000_000_000_000_000); // LSC (gas)
        let a_once = node.bakiye(&gonderen);
        let l_once = node.lsc_bakiye(&gonderen);
        let lsc_arz = node.lsc_toplam_arzi();

        let payload = AvmCagri::new(hedef, 1000, 0, vec![]).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk5).expect("avm vertex");
        node.ingest_networked(&wire::encode(&v), now);

        let yakim = [0u8; 20];
        let havuz = crate::avm::GELISTIRME_HAVUZU;
        assert_eq!(node.bakiye(&hedef), 1000, "hedef 1000 AIDAG almali");
        assert_eq!(node.lsc_bakiye(&yakim), 10_500_000_000_000, "gas yak (LSC)");
        assert_eq!(
            node.lsc_bakiye(&havuz),
            10_500_000_000_000,
            "gas havuz (LSC)"
        );
        assert_eq!(
            node.bakiye(&gonderen),
            a_once - 1000,
            "gonderen AIDAG dustu (deger)"
        );
        assert_eq!(
            node.lsc_bakiye(&gonderen),
            l_once - 21_000_000_000_000,
            "gonderen LSC gas dustu"
        );
        assert_eq!(node.beklenen_nonce(&gonderen), 1, "nonce 1'e ilerledi");
        assert_eq!(node.lsc_toplam_arzi(), lsc_arz, "LSC arzi korundu");
    }

    // KOPRU 5 (canli): node yolundan KONTRAT DEPLOY. data dolu + hedef=sifir -> CREATE.
    // Kanit: islem basarili islendi (nonce ilerledi + gas kesildi), arz korundu.
    #[test]
    fn ingest_avm_kontrat_deploy() {
        use crate::registry::public_key_to_adres;
        use crate::tx::AvmCagri;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let sk = SigningKey::from_bytes(&[11u8; 32]);
        let gonderen = public_key_to_adres(&sk.verifying_key().to_bytes());
        node.lsc_test_bakiye_ekle(gonderen, 100_000_000_000_000_000);
        let arz_basta = node.lsc_toplam_arzi();
        let bakiye_basta = node.lsc_bakiye(&gonderen);

        // BelgeDamgasi deploy bytecode'u
        let bin_hex =
            include_str!("../../avm-sozlesmeler/BelgeDamgasi_sol_BelgeDamgasi.bin").trim();
        let deploy_kod: Vec<u8> = (0..bin_hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&bin_hex[i..i + 2], 16).unwrap())
            .collect();

        // DEPLOY: hedef=sifir, deger=0, data=bytecode, nonce=0
        let sifir = [0u8; 20];
        let payload = AvmCagri::new(sifir, 0, 0, deploy_kod).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk).expect("deploy vertex");
        node.ingest_networked(&wire::encode(&v), now);

        // KANIT 1: nonce ilerledi -> handler basariyla isledi
        assert_eq!(
            node.beklenen_nonce(&gonderen),
            1,
            "deploy sonrasi nonce 1 olmali"
        );
        // KANIT 2: GERCEK gas_used kesildi (sabit 21000 DEGIL — deploy daha fazla gas).
        // Dayaniklilik: kesinti > 0, yakim+havuz = kesinti, arz korunur.
        let dusen = bakiye_basta - node.lsc_bakiye(&gonderen);
        assert!(dusen > 0, "deploy gas'i kesilmis olmali (gercek gas_used)");
        let yak = node.lsc_bakiye(&[0u8; 20]);
        let hav = node.lsc_bakiye(&crate::avm::GELISTIRME_HAVUZU);
        assert_eq!(
            yak + hav,
            dusen,
            "gas = yakim + gelistirme havuzu (kayipsiz bolusum)"
        );
        // KANIT 3: toplam arz korundu
        assert_eq!(node.lsc_toplam_arzi(), arz_basta, "toplam LSC arzi korundu");
    }

    // B1 KANIT (fon donmasi COZULDU): kontrat-tutulan native AIDAG ucuncu tarafa
    // gonderilince gercek defter (bakiye_registry) GUNCELLENIR. Eski kodda kontrat-ici
    // hareket yalniz avm_db'de kalir, alicinin gercek bakiyesi ARTMAZ -> fon donar.
    // Senaryo: Kasa deploy -> depozito(500k) -> cek(alici, 200k). Kanit: alici gercek
    // bakiyesi 200k olur, Kasa 300k'ya duser, AIDAG toplam arzi degismez.
    #[test]
    fn avm_kontrat_ici_transfer_gercek_deftere_yansir_b1() {
        use crate::registry::public_key_to_adres;
        use crate::tx::AvmCagri;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let sk = SigningKey::from_bytes(&[0x42u8; 32]);
        let gonderen = public_key_to_adres(&sk.verifying_key().to_bytes());
        let alici = [0x22u8; 20]; // ucuncu taraf (baslangicta 0 AIDAG)

        // gas (LSC) + teminat (AIDAG) bakiyesi ver.
        node.lsc_test_bakiye_ekle(gonderen, 100_000_000_000_000_000);
        node.test_bakiye_ekle(gonderen, 1_000_000); // AIDAG
        let aidag_arz_basta = node.toplam_bakiye_arzi();
        assert_eq!(aidag_arz_basta, 1_000_000, "baslangic AIDAG arzi");

        // --- 1) DEPLOY Kasa (nonce=0) ---
        let bin_hex = include_str!("../../avm-sozlesmeler/Kasa.bin").trim();
        let deploy_kod: Vec<u8> = (0..bin_hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&bin_hex[i..i + 2], 16).unwrap())
            .collect();
        let payload = AvmCagri::new([0u8; 20], 0, 0, deploy_kod).encode();
        let v_deploy = Vertex::new_signed(NET, vec![gid], payload, now, &sk).expect("deploy");
        node.ingest_networked(&wire::encode(&v_deploy), now);
        assert_eq!(node.beklenen_nonce(&gonderen), 1, "deploy sonrasi nonce 1");
        let kontratlar = node.avm_kontrat_adresleri();
        assert_eq!(kontratlar.len(), 1, "tek kontrat deploy edildi");
        let kasa = kontratlar[0];

        // --- 2) depozito() ile Kasa'ya 500k AIDAG yatir (nonce=1, deger=500k) ---
        // DAG zinciri: parent = deploy vertex (sira: deploy -> depozito -> cek).
        let depozito_data = vec![0xa8, 0x19, 0xfd, 0xf8]; // depozito()
        let payload = AvmCagri::new(kasa, 500_000, 1, depozito_data).encode();
        let v_depo =
            Vertex::new_signed(NET, vec![*v_deploy.id()], payload, now, &sk).expect("depozito");
        node.ingest_networked(&wire::encode(&v_depo), now);
        assert_eq!(
            node.beklenen_nonce(&gonderen),
            2,
            "depozito sonrasi nonce 2"
        );
        assert_eq!(node.bakiye(&kasa), 500_000, "Kasa 500k AIDAG tuttu");
        assert_eq!(node.bakiye(&gonderen), 500_000, "gonderen 500k'ya dustu");

        // --- 3) cek(alici, 200k): Kasa kontrat-ici olarak alici'ya gonderir (nonce=2) ---
        let mut cek_data = vec![0x8c, 0x7b, 0x1f, 0xb7]; // cek(address,uint256)
        cek_data.extend_from_slice(&[0u8; 12]); // address soldan 12 sifir dolgu
        cek_data.extend_from_slice(&alici); // 20 bayt adres
        let mut amt = [0u8; 32];
        amt[24..32].copy_from_slice(&200_000u64.to_be_bytes()); // uint256 big-endian
        cek_data.extend_from_slice(&amt);
        let payload = AvmCagri::new(kasa, 0, 2, cek_data).encode();
        let v_cek = Vertex::new_signed(NET, vec![*v_depo.id()], payload, now, &sk).expect("cek");
        node.ingest_networked(&wire::encode(&v_cek), now);
        assert_eq!(node.beklenen_nonce(&gonderen), 3, "cek sonrasi nonce 3");

        // KANIT (B1): kontrat-ici transfer GERCEK deftere yansidi.
        assert_eq!(
            node.bakiye(&alici),
            200_000,
            "B1: alicinin GERCEK bakiyesi kontrat-ici transferle artti (eski kodda 0 = donmus)"
        );
        assert_eq!(
            node.bakiye(&kasa),
            300_000,
            "Kasa 500k-200k = 300k'ya dustu"
        );
        assert_eq!(
            node.bakiye(&gonderen),
            500_000,
            "gonderen cek'ten etkilenmedi"
        );
        // ARZ KORUMASI: hicbir asamada AIDAG yaratilmadi/yok olmadi.
        assert_eq!(
            node.toplam_bakiye_arzi(),
            aidag_arz_basta,
            "AIDAG toplam arzi korundu (500k+300k+200k=1M)"
        );
    }

    // B3 KANIT (deploy nonce kaliciligi): AYNI hesap IKI kontrat deploy edince
    // FARKLI adres olusur (EVM nonce artar -> keccak(gonderen, nonce) degisir).
    // Eski kodda nonce hep 0 -> ayni adres -> ikinci deploy birinciyi EZER
    // (bir hesap pratikte tek kontrat deploy edebilirdi).
    #[test]
    fn avm_ayni_hesap_iki_kontrat_deploy_edebilir_b3() {
        use crate::registry::public_key_to_adres;
        use crate::tx::AvmCagri;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let sk = SigningKey::from_bytes(&[0x99u8; 32]);
        let gonderen = public_key_to_adres(&sk.verifying_key().to_bytes());
        node.lsc_test_bakiye_ekle(gonderen, 100_000_000_000_000_000);

        let bin_hex = include_str!("../../avm-sozlesmeler/Kasa.bin").trim();
        let kod: Vec<u8> = (0..bin_hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&bin_hex[i..i + 2], 16).unwrap())
            .collect();

        // DEPLOY #1 (nonce=0)
        let p1 = AvmCagri::new([0u8; 20], 0, 0, kod.clone()).encode();
        let v1 = Vertex::new_signed(NET, vec![gid], p1, now, &sk).expect("deploy1");
        node.ingest_networked(&wire::encode(&v1), now);
        assert_eq!(
            node.avm_kontrat_adresleri().len(),
            1,
            "ilk deploy: 1 kontrat"
        );

        // DEPLOY #2 (nonce=1, zincir: parent = deploy1)
        let p2 = AvmCagri::new([0u8; 20], 0, 1, kod).encode();
        let v2 = Vertex::new_signed(NET, vec![*v1.id()], p2, now, &sk).expect("deploy2");
        node.ingest_networked(&wire::encode(&v2), now);

        // KANIT (B3): iki AYRI kontrat adresi (nonce artti -> farkli CREATE adresi).
        let adresler = node.avm_kontrat_adresleri();
        assert_eq!(
            adresler.len(),
            2,
            "B3: ayni hesap IKI kontrat deploy etti (eski kodda 1 = adres cakismasi)"
        );
        assert_ne!(adresler[0], adresler[1], "iki kontrat adresi FARKLI");
        assert_eq!(node.beklenen_nonce(&gonderen), 2, "iki deploy -> nonce 2");
    }

    // B2(HAM_ETH)+B1 KANIT (raw-eth / MetaMask yolu, tip=12): imzali eth tx'lerle
    // Kasa deploy -> depozito(500k) -> cek(alici,200k). Kontrat-ici AIDAG hareketi
    // GERCEK deftere yansir (fon donmasi biter), gercek gas_used kesilir, arz korunur.
    // AVM_CAGRI ile AYNI cekirdek (avm_calistir + seed + aynala).
    #[test]
    fn ham_eth_kontrat_ici_transfer_gercek_deftere_yansir_b1_b2() {
        use alloy_consensus::{SignableTransaction, TxLegacy};
        use alloy_eips::eip2718::Encodable2718;
        use alloy_primitives::{Signature, TxKind as ATxKind, U256 as AU256};
        use alloy_signer::SignerSync;
        use alloy_signer_local::PrivateKeySigner;

        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        // Eth imzalayan (gonderen imzadan kurtarilir) + vertex imzalayan (relayer, ayri).
        let eth: PrivateKeySigner =
            "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318"
                .parse()
                .unwrap();
        let gonderen = crate::avm::evm_to_adres(&eth.address());
        let vsk = SigningKey::from_bytes(&[0x88u8; 32]);

        node.lsc_test_bakiye_ekle(gonderen, 100_000_000_000_000_000); // gas (LSC)
        node.test_bakiye_ekle(gonderen, 1_000_000); // AIDAG (deger)
        let arz_basta = node.toplam_bakiye_arzi();
        let alici = [0x22u8; 20];

        // imzali raw eth tx (EIP-2718) uret.
        let raw_eth = |nonce: u64, to: ATxKind, value: u128, input: Vec<u8>| -> Vec<u8> {
            let tx = TxLegacy {
                chain_id: Some(NET as u64),
                nonce,
                gas_price: 0,
                gas_limit: 3_000_000,
                to,
                value: AU256::from(value),
                input: input.into(),
            };
            let imza: Signature = eth.sign_hash_sync(&tx.signature_hash()).unwrap();
            let zarf: alloy_consensus::TxEnvelope = tx.into_signed(imza).into();
            zarf.encoded_2718()
        };

        let bin_hex = include_str!("../../avm-sozlesmeler/Kasa.bin").trim();
        let kod: Vec<u8> = (0..bin_hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&bin_hex[i..i + 2], 16).unwrap())
            .collect();

        // 1) DEPLOY (eth nonce=0)
        let raw1 = raw_eth(0, ATxKind::Create, 0, kod);
        let pl1 = crate::tx::ham_eth_tx_payload(&raw1);
        let v1 = Vertex::new_signed(NET, vec![gid], pl1, now, &vsk).expect("deploy");
        node.ingest_networked(&wire::encode(&v1), now);
        let kasa = node.avm_kontrat_adresleri()[0];

        // 2) depozito() (eth nonce=1, value=500k AIDAG)
        let raw2 = raw_eth(
            1,
            ATxKind::Call(crate::avm::adres_to_evm(&kasa)),
            500_000,
            vec![0xa8, 0x19, 0xfd, 0xf8],
        );
        let pl2 = crate::tx::ham_eth_tx_payload(&raw2);
        let v2 = Vertex::new_signed(NET, vec![*v1.id()], pl2, now, &vsk).expect("depozito");
        node.ingest_networked(&wire::encode(&v2), now);
        assert_eq!(
            node.bakiye(&kasa),
            500_000,
            "Kasa 500k tuttu (raw-eth depozito)"
        );

        // 3) cek(alici, 200k) (eth nonce=2)
        let mut cek = vec![0x8c, 0x7b, 0x1f, 0xb7];
        cek.extend_from_slice(&[0u8; 12]);
        cek.extend_from_slice(&alici);
        let mut amt = [0u8; 32];
        amt[24..32].copy_from_slice(&200_000u64.to_be_bytes());
        cek.extend_from_slice(&amt);
        let raw3 = raw_eth(2, ATxKind::Call(crate::avm::adres_to_evm(&kasa)), 0, cek);
        let pl3 = crate::tx::ham_eth_tx_payload(&raw3);
        let v3 = Vertex::new_signed(NET, vec![*v2.id()], pl3, now, &vsk).expect("cek");
        node.ingest_networked(&wire::encode(&v3), now);

        // KANIT (B1 raw-eth): kontrat-ici transfer GERCEK deftere yansidi.
        assert_eq!(
            node.bakiye(&alici),
            200_000,
            "B1(raw-eth): alici gercek bakiyesi kontrat-ici transferle 200k oldu"
        );
        assert_eq!(node.bakiye(&kasa), 300_000, "Kasa 300k'ya dustu");
        assert_eq!(
            node.toplam_bakiye_arzi(),
            arz_basta,
            "AIDAG toplam arzi korundu (raw-eth yolu)"
        );
    }

    // B6 KANIT (nonce modeli): native tx sonrasi deploy'un CREATE adresi BIRLESIK
    // nonce'u kullanir -> eth_getTransactionCount ile TUTARLI (MetaMask/arac adres
    // tahmini dogru). Eski kodda avm_db'nin ayri nonce'u native tx'ten ayrisir,
    // CREATE adresi eth araclarinin hesabindan FARKLI cikardi.
    #[test]
    fn b6_create_nonce_birlesik_nonce_ile_tutarli() {
        use crate::registry::public_key_to_adres;
        use crate::tx::{AvmCagri, TransferKaydi};
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let sk = SigningKey::from_bytes(&[0x66u8; 32]);
        let gonderen = public_key_to_adres(&sk.verifying_key().to_bytes());
        node.test_bakiye_ekle(gonderen, 1_000_000); // AIDAG (native transfer icin)
        node.lsc_test_bakiye_ekle(gonderen, 100_000_000_000_000_000); // LSC (deploy gas)

        // 1) NATIVE AIDAG transfer (tip=4, nonce=0) -> BIRLESIK nonce 0->1.
        let alici = [0x33u8; 20];
        let p_tr = TransferKaydi::new(alici, 1000, 0).encode();
        let v_tr = Vertex::new_signed(NET, vec![gid], p_tr, now, &sk).expect("transfer");
        node.ingest_networked(&wire::encode(&v_tr), now);
        assert_eq!(
            node.beklenen_nonce(&gonderen),
            1,
            "native transfer nonce'u 1'e ilerletti"
        );

        // 2) KONTRAT DEPLOY (nonce=1). CREATE nonce'u BIRLESIK olmali.
        let bin_hex = include_str!("../../avm-sozlesmeler/Kasa.bin").trim();
        let kod: Vec<u8> = (0..bin_hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&bin_hex[i..i + 2], 16).unwrap())
            .collect();
        let p_dep = AvmCagri::new([0u8; 20], 0, 1, kod).encode();
        let v_dep = Vertex::new_signed(NET, vec![*v_tr.id()], p_dep, now, &sk).expect("deploy");
        node.ingest_networked(&wire::encode(&v_dep), now);
        let kasa = node.avm_kontrat_adresleri()[0];

        // KANIT (B6): deploy adresi = keccak(gonderen, BIRLESIK nonce=1) = eth araclarinin
        // eth_getTransactionCount ile hesaplayacagi adres. Eski kodda avm_db nonce=0 ile
        // FARKLI (yanlis) adres cikardi.
        let beklenen_evm = crate::avm::adres_to_evm(&gonderen).create(1);
        let beklenen = crate::avm::evm_to_adres(&beklenen_evm);
        assert_eq!(
            kasa, beklenen,
            "B6: CREATE adresi birlesik nonce(1) ile tutarli"
        );
        assert_eq!(
            node.beklenen_nonce(&gonderen),
            2,
            "deploy sonrasi birlesik nonce 2"
        );
    }

    // KOPRU 5 (KALICILIK): kontrat deploy -> export -> YENI node'da replay ->
    // kontrat kodu YENI node'da da OLUSMALI. AVM state'i DAG replay'i ile kalici.
    // Bu, "dugum yeniden baslayinca sozlesme kaybolmaz" kaniti.
    #[test]
    fn avm_kontrat_replay_ile_kalici() {
        use crate::registry::public_key_to_adres;
        use crate::tx::AvmCagri;
        let now = 1_000_000;

        // 1) Kaynak node: genesis + kontrat deploy
        let mut src = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        src.ingest_networked(&gen, now);
        let sk = SigningKey::from_bytes(&[12u8; 32]);
        let gonderen = public_key_to_adres(&sk.verifying_key().to_bytes());
        src.lsc_test_bakiye_ekle(gonderen, 100_000_000_000_000_000);

        let bin_hex =
            include_str!("../../avm-sozlesmeler/BelgeDamgasi_sol_BelgeDamgasi.bin").trim();
        let deploy_kod: Vec<u8> = (0..bin_hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&bin_hex[i..i + 2], 16).unwrap())
            .collect();
        let payload = AvmCagri::new([0u8; 20], 0, 0, deploy_kod).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk).expect("deploy vertex");
        src.ingest_networked(&wire::encode(&v), now);
        assert_eq!(src.beklenen_nonce(&gonderen), 1, "src'de deploy islendi");

        // Deploy edilen kontrat adresini src'den ogren: avm_calistir adresi nonce'tan
        // turetir; src'de kod olusmus bir adres olmali. Tum vertex'leri export edip
        // YENI node'da replay edince ayni adreste kod OLUSMALI (deterministik).
        // Kontrat adresini bulmak icin: src'de hangi adreste kod var? (test yardimcisi yok,
        // bu yuzden dogrudan deterministik turetmeyi avm uzerinden dogrulariz.)

        // 2) Export + YENI node'da replay
        let exported = src.export_vertices();
        let mut dst = NodeState::new_devnet(NET);
        // LSC bakiyesi test_bakiye ile eklenmisti; replay'de gas icin gonderene LSC lazim.
        dst.lsc_test_bakiye_ekle(gonderen, 100_000_000_000_000_000);
        for bytes in &exported {
            dst.ingest_networked(bytes, now);
        }

        // 3) KANIT: dst'de de nonce ilerledi -> deploy replay'de tekrar islendi
        assert_eq!(
            dst.beklenen_nonce(&gonderen),
            1,
            "dst'de deploy replay ile islendi"
        );
        // 4) KANIT: src ve dst ayni vertex sayisi (DAG butun)
        assert_eq!(
            src.vertex_count(),
            dst.vertex_count(),
            "DAG replay ile birebir"
        );
    }

    // A4 KANIT (seffaflik): owner AIDAG'i yeterli ama LSC HEDIYESI icin bakiyesi
    // YETERSIZ. AIDAG dagitilir; ama kayit GERCEKTEN gonderilen hediyeyi (0) saklar,
    // istenen (buyuk) tutari DEGIL. "Gonderildi" yalani zincire yazilmaz.
    #[test]
    fn on_satis_lsc_hediye_yetersizse_kayit_gercegi_yansitir_a4() {
        use crate::registry::public_key_to_adres;
        use crate::tx::OnSatisDagitim;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let sk = SigningKey::from_bytes(&[0x77u8; 32]);
        let owner = public_key_to_adres(&sk.verifying_key().to_bytes());
        node.faucet_owner_ayarla(owner);
        node.test_bakiye_ekle(owner, 1_000_000); // AIDAG: bol
        node.lsc_test_bakiye_ekle(owner, 5); // LSC: YETERSIZ (hediye 10_000 istenecek)

        let alici = [0x55u8; 20];
        let odeme_ref = 4242u64;

        // Owner: aliciya 5000 AIDAG + 10_000 LSC hediye. AIDAG gider, LSC hediye GITMEZ.
        let payload = OnSatisDagitim::new(alici, 5000, 10_000, odeme_ref).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk).expect("on satis");
        node.ingest_networked(&wire::encode(&v), now);

        // KANIT 1: AIDAG dagitildi (asil urun).
        assert_eq!(node.bakiye(&alici), 5000, "alici AIDAG aldi");
        // KANIT 2: LSC hediye GITMEDI (owner bakiyesi yetersizdi).
        assert_eq!(
            node.lsc_bakiye(&alici),
            0,
            "LSC hediye gitmedi (owner yetersiz)"
        );
        // KANIT 3 (A4): kayit GERCEGI yansitir -> lsc_hediye=0, istenen 10_000 DEGIL.
        let k = node
            .on_satis_sorgula(odeme_ref)
            .expect("kayit olusmali (AIDAG gitti)");
        assert_eq!(k.aidag, 5000, "kayit: gercek AIDAG");
        assert_eq!(
            k.lsc_hediye, 0,
            "A4: kayit GERCEK hediyeyi (0) saklar, 'gonderildi' yalanini DEGIL"
        );
    }

    // FAZ2 KANIT (on-satis vesting): dagitilan AIDAG %20 TGE hemen + kalan %80 12 ay
    // kilitli; birden cok dagitim BIRIKIR; 12 ay sonra tam acik.
    #[test]
    fn on_satis_dagitim_vestingli_ve_birikimli_faz2() {
        use crate::registry::public_key_to_adres;
        use crate::tx::OnSatisDagitim;
        let tge = crate::mainnet::MAINNET_VESTING_BASLANGIC;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, tge);
        node.ingest_networked(&gen, tge);

        let sk = SigningKey::from_bytes(&[0x91u8; 32]);
        let owner = public_key_to_adres(&sk.verifying_key().to_bytes());
        node.faucet_owner_ayarla(owner);
        node.test_bakiye_ekle(owner, 1_000_000);

        let alici = [0x55u8; 20];

        // 1) Dagitim: alici 10.000 AIDAG (TGE aninda)
        let p1 = OnSatisDagitim::new(alici, 10_000, 0, 111).encode();
        let v1 = Vertex::new_signed(NET, vec![gid], p1, tge, &sk).expect("d1");
        node.ingest_networked(&wire::encode(&v1), tge);
        assert_eq!(node.bakiye(&alici), 10_000, "alici 10k AIDAG aldi");
        assert_eq!(
            node.vesting_kilitli(&alici, tge),
            8_000,
            "TGE'de %80 (8k) KILITLI -> harcanabilir 2k"
        );

        // 2) Ikinci dagitim: alici +5.000 (farkli odeme_ref) -> vesting BIRIKIR
        let p2 = OnSatisDagitim::new(alici, 5_000, 0, 222).encode();
        let v2 = Vertex::new_signed(NET, vec![*v1.id()], p2, tge, &sk).expect("d2");
        node.ingest_networked(&wire::encode(&v2), tge);
        assert_eq!(node.bakiye(&alici), 15_000, "toplam 15k AIDAG");
        assert_eq!(
            node.vesting_kilitli(&alici, tge),
            12_000,
            "kilit BIRIKTI: %80 (12k) kilitli"
        );

        // 3) 12 ay sonra tam acik
        assert_eq!(
            node.vesting_kilitli(&alici, tge + 360 * 86400),
            0,
            "12 ay sonra %0 kilitli (tam acik)"
        );
    }

    #[test]
    fn on_satis_replay_ile_kalici() {
        use crate::registry::public_key_to_adres;
        use crate::tx::OnSatisDagitim;
        let now = 1_000_000;

        // 1) Kaynak node: genesis + owner ayarla + owner'a AIDAG/LSC bakiye
        let mut src = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        src.ingest_networked(&gen, now);
        let sk = SigningKey::from_bytes(&[33u8; 32]);
        let owner = public_key_to_adres(&sk.verifying_key().to_bytes());
        src.faucet_owner_ayarla(owner);
        src.test_bakiye_ekle(owner, 1_000_000); // owner'da satilacak AIDAG
        src.lsc_test_bakiye_ekle(owner, 1_000_000); // owner'da hediye LSC

        let alici = [0x55u8; 20];
        let odeme_ref = 777u64;

        // 2) Owner on satis dagitimi yapar: aliciya 5000 AIDAG + 10 LSC, odeme_ref=777
        let payload = OnSatisDagitim::new(alici, 5000, 10, odeme_ref).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk).expect("on satis vertex");
        src.ingest_networked(&wire::encode(&v), now);

        // 3) KANIT (kaynak): alici bakiye 5000, on satis kaydi var
        assert_eq!(src.bakiye(&alici), 5000, "src: alici AIDAG aldi");
        assert_eq!(src.on_satis_sayisi(), 1, "src: on satis kaydi olustu");
        let k = src
            .on_satis_sorgula(odeme_ref)
            .expect("src: kayit bulunmali");
        assert_eq!(k.aidag, 5000);
        assert_eq!(k.alici, alici);

        // 4) Export + YENI node'da replay
        let exported = src.export_vertices();
        let mut dst = NodeState::new_devnet(NET);
        dst.faucet_owner_ayarla(owner);
        dst.test_bakiye_ekle(owner, 1_000_000);
        dst.lsc_test_bakiye_ekle(owner, 1_000_000);
        for bytes in &exported {
            dst.ingest_networked(bytes, now);
        }

        // 5) KANIT (replay): yeni node'da da alici bakiye 5000 + kayit var
        assert_eq!(dst.bakiye(&alici), 5000, "dst: alici AIDAG replay ile aldi");
        assert_eq!(
            dst.on_satis_sayisi(),
            1,
            "dst: on satis kaydi replay ile olustu"
        );
        let k2 = dst
            .on_satis_sorgula(odeme_ref)
            .expect("dst: kayit replay ile bulunmali");
        assert_eq!(k2.aidag, 5000, "dst: dogru miktar");
        assert_eq!(k2.alici, alici, "dst: dogru alici");
        assert_eq!(
            src.vertex_count(),
            dst.vertex_count(),
            "DAG replay ile birebir"
        );
    }

    #[test]
    fn on_satis_owner_disi_reddedilir() {
        use crate::registry::public_key_to_adres;
        use crate::tx::OnSatisDagitim;
        let now = 1_000_000;

        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        // GERCEK owner
        let owner_sk = SigningKey::from_bytes(&[44u8; 32]);
        let owner = public_key_to_adres(&owner_sk.verifying_key().to_bytes());
        node.faucet_owner_ayarla(owner);

        // SALDIRGAN (owner DEGIL) - kendine bakiye olsa bile on satis dagitamaz
        let saldirgan_sk = SigningKey::from_bytes(&[99u8; 32]);
        let saldirgan = public_key_to_adres(&saldirgan_sk.verifying_key().to_bytes());
        node.test_bakiye_ekle(saldirgan, 1_000_000); // saldirganin AIDAG'i olsa bile
        node.lsc_test_bakiye_ekle(saldirgan, 1_000_000);

        let alici = [0x77u8; 20];
        let alici_baslangic = node.bakiye(&alici);

        // Saldirgan tip=10 gondermeyi dener (kendini owner sanarak)
        let payload = OnSatisDagitim::new(alici, 5000, 10, 888).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &saldirgan_sk)
            .expect("saldirgan vertex");
        node.ingest_networked(&wire::encode(&v), now);

        // KANIT: aliciya HICBIR AIDAG gitmedi (owner degil = dagitim YOK)
        assert_eq!(
            node.bakiye(&alici),
            alici_baslangic,
            "owner olmayan dagitim yapamamali"
        );
        // KANIT: on satis kaydi olusmadi (dagitim reddedildi)
        assert_eq!(
            node.on_satis_sayisi(),
            0,
            "owner-disi cagri kayit olusturmamali"
        );
    }

    #[test]
    fn on_satis_yetersiz_bakiye_kayit_tutmaz() {
        use crate::registry::public_key_to_adres;
        use crate::tx::OnSatisDagitim;
        let now = 1_000_000;

        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        // Owner ayarli AMA owner'in AIDAG bakiyesi YOK (0).
        let sk = SigningKey::from_bytes(&[55u8; 32]);
        let owner = public_key_to_adres(&sk.verifying_key().to_bytes());
        node.faucet_owner_ayarla(owner);
        // Bilerek bakiye VERMIYORUZ -> transfer basarisiz olmali.

        let alici = [0x66u8; 20];
        let payload = OnSatisDagitim::new(alici, 5000, 10, 31337).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk).expect("vertex");
        node.ingest_networked(&wire::encode(&v), now);

        // KANIT: AIDAG gitmedi (owner bakiyesi yoktu)
        assert_eq!(
            node.bakiye(&alici),
            0,
            "yetersiz bakiye: alici AIDAG almamali"
        );
        // KANIT: KAYIT TUTULMADI (sahte 'dagitildi' kaydi yok)
        assert_eq!(
            node.on_satis_sayisi(),
            0,
            "transfer basarisizsa kayit TUTULMAMALI (seffafliga ihanet etmez)"
        );
        // KANIT: odeme_ref kullanilmis sayilmaz -> owner bakiye edinince TEKRAR deneyebilir
        assert!(
            node.on_satis_sorgula(31337).is_none(),
            "basarisiz dagitim kaydi olusturmaz, ref tekrar kullanilabilir"
        );
    }

    // KOPRU 5 (canli CALL): node yolundan kontrat CAGIRMA.
    // deploy -> adresi ogren -> ayni adrese kaydet(hash) CALL gonder -> islendi mi?
    #[test]
    fn ingest_avm_kontrat_call() {
        use crate::registry::public_key_to_adres;
        use crate::tx::AvmCagri;
        use revm::primitives::keccak256;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let sk = SigningKey::from_bytes(&[13u8; 32]);
        let gonderen = public_key_to_adres(&sk.verifying_key().to_bytes());
        node.lsc_test_bakiye_ekle(gonderen, 100_000_000_000_000_000);

        // 1) DEPLOY (nonce=0)
        let bin_hex =
            include_str!("../../avm-sozlesmeler/BelgeDamgasi_sol_BelgeDamgasi.bin").trim();
        let deploy_kod: Vec<u8> = (0..bin_hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&bin_hex[i..i + 2], 16).unwrap())
            .collect();
        let p_deploy = AvmCagri::new([0u8; 20], 0, 0, deploy_kod).encode();
        let v0 = Vertex::new_signed(NET, vec![gid], p_deploy, now, &sk).expect("deploy v");
        node.ingest_networked(&wire::encode(&v0), now);
        assert_eq!(node.beklenen_nonce(&gonderen), 1, "deploy islendi");

        // 2) Deploy edilen kontrat adresini ogren
        let adresler = node.avm_kontrat_adresleri();
        assert_eq!(adresler.len(), 1, "tek kontrat deploy edildi");
        let kontrat = adresler[0];

        // 3) CALL: kaydet(hash), nonce=1
        let sel = &keccak256(b"kaydet(bytes32)")[0..4];
        let belge = keccak256(b"node call testi belgesi");
        let mut calldata = Vec::new();
        calldata.extend_from_slice(sel);
        calldata.extend_from_slice(belge.as_slice());
        let bakiye_call_oncesi = node.lsc_bakiye(&gonderen);
        let p_call = AvmCagri::new(kontrat, 0, 1, calldata).encode();
        let v1 = Vertex::new_signed(NET, vec![*v0.id()], p_call, now, &sk).expect("call v");
        node.ingest_networked(&wire::encode(&v1), now);

        // 4) KANIT: call islendi -> nonce 2, gas kesildi
        assert_eq!(
            node.beklenen_nonce(&gonderen),
            2,
            "call sonrasi nonce 2 olmali"
        );
        // GERCEK gas_used kesildi (call'un gercek maliyeti). Dayaniklilik: azaldi.
        assert!(
            node.lsc_bakiye(&gonderen) < bakiye_call_oncesi,
            "call gas kesildi (gercek gas_used)"
        );
    }

    // KOPRU 5 (deger>0): AVM yolundan LSC DEGER transferi. ARZ KORUNMALI.
    // data dolu + hedef KOD-SUZ adres + deger=5000 -> EVM value transferi yapar
    // (kod olmadigi icin data onemsiz). gonderen->hedef deger gider, gas kesilir, ARZ KORUNUR.
    // (Not: payable OLMAYAN kontrata value gondermek EVM'de revert eder -> dogru davranis;
    //  bu yuzden hedef siradan adres secildi.)
    #[test]
    fn ingest_avm_deger_transferi_arz_korunur() {
        use crate::registry::public_key_to_adres;
        use crate::tx::AvmCagri;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let sk = SigningKey::from_bytes(&[14u8; 32]);
        let gonderen = public_key_to_adres(&sk.verifying_key().to_bytes());
        node.test_bakiye_ekle(gonderen, 100_000_000_000_000_000); // AIDAG (deger)
        node.lsc_test_bakiye_ekle(gonderen, 100_000_000_000_000_000); // LSC (gas)
        let a_once = node.bakiye(&gonderen);
        let l_once = node.lsc_bakiye(&gonderen);
        let lsc_arz = node.lsc_toplam_arzi();

        let hedef = [0x77u8; 20];
        let pc = AvmCagri::new(hedef, 5000, 0, vec![0x01]).encode();
        let vc = Vertex::new_signed(NET, vec![gid], pc, now, &sk).expect("avm deger");
        node.ingest_networked(&wire::encode(&vc), now);

        assert_eq!(node.beklenen_nonce(&gonderen), 1, "islendi");
        assert_eq!(node.bakiye(&hedef), 5000, "deger AIDAG hedefe gitti (5000)");
        assert_eq!(
            node.bakiye(&gonderen),
            a_once - 5000,
            "gonderen AIDAG dustu (deger)"
        );
        assert!(
            node.lsc_bakiye(&gonderen) < l_once,
            "gonderen LSC gas dustu (gercek gas_used)"
        );
        assert_eq!(
            node.bakiye(&gonderen) + node.bakiye(&hedef),
            a_once,
            "AIDAG arzi korundu"
        );
        assert_eq!(node.lsc_toplam_arzi(), lsc_arz, "LSC arzi korundu");
    }

    // AVM cagrisi: yetersiz bakiye -> hicbir sey degismez, nonce ilerlemez.
    #[test]
    fn ingest_avm_cagri_yetersiz_bakiye() {
        use crate::registry::public_key_to_adres;
        use crate::tx::AvmCagri;
        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let sk6 = SigningKey::from_bytes(&[6u8; 32]);
        let gonderen = public_key_to_adres(&sk6.verifying_key().to_bytes());
        let hedef = [0xDD; 20];
        // Sadece 5000 LSC: deger 1000 + gas 21000 = 22000 gerekli -> YETERSIZ.
        node.lsc_test_bakiye_ekle(gonderen, 5000);

        let payload = AvmCagri::new(hedef, 1000, 0, vec![]).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk6).expect("avm vertex");
        node.ingest_networked(&wire::encode(&v), now);

        assert_eq!(node.lsc_bakiye(&gonderen), 5000, "bakiye degismedi");
        assert_eq!(node.lsc_bakiye(&hedef), 0, "hedef hicbir sey almadi");
        assert_eq!(node.beklenen_nonce(&gonderen), 0, "nonce ilerlemedi");
    }

    // ===== GERCEK DUNYA: belge dogrulama ingest entegrasyonu =====

    // Bir tip=1 (Record) vertex'i ingest edilince, belge defterine islenir:
    // hash + kaydeden (imzalayan) + zaman (vertex timestamp). Sonra dogrulanir.
    #[test]
    fn ingest_belge_dogrulanir() {
        use crate::registry::public_key_to_adres;
        use crate::tx::Record;
        let now = 1_700_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        // Bir belgenin hash'i (gercekte blake3(belge_icerigi)).
        let belge_hash = [0xAB; 32];
        let sk = SigningKey::from_bytes(&[9u8; 32]);
        let kaydeden = public_key_to_adres(&sk.verifying_key().to_bytes());

        // tip=1 Record vertex'i imzala + ingest.
        let payload = Record::new(belge_hash).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk).expect("record vertex");
        assert!(matches!(
            node.ingest_networked(&wire::encode(&v), now),
            NetworkIngestOutcome::Integrated(_)
        ));

        // KANIT: belge zincirde dogrulanabilir — kim + ne zaman.
        let kayit = node
            .belge_dogrula(&belge_hash)
            .expect("belge kayitli olmali");
        assert_eq!(kayit.kaydeden, kaydeden, "kaydeden = imzalayan");
        assert_eq!(kayit.zaman, now, "zaman = vertex timestamp");
        assert_eq!(node.belge_sayisi(), 1);

        // Kayitsiz bir hash dogrulanmaz.
        assert_eq!(node.belge_dogrula(&[0x00; 32]), None);
    }

    // ILK KAYIT KAZANIR: ayni belge hash'i iki kez (farkli kisi) yazilirsa,
    // ILK kaydeden korunur (kanit bozulmaz).
    #[test]
    fn ingest_belge_ilk_kayit_kazanir() {
        use crate::registry::public_key_to_adres;
        use crate::tx::Record;
        let now = 1_700_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let belge_hash = [0xCD; 32];
        let sk_ilk = SigningKey::from_bytes(&[10u8; 32]);
        let adr_ilk = public_key_to_adres(&sk_ilk.verifying_key().to_bytes());

        // Ilk kayit (sk_ilk, now).
        let p1 = Record::new(belge_hash).encode();
        let v1 = Vertex::new_signed(NET, vec![gid], p1, now, &sk_ilk).expect("v1");
        let id1 = match node.ingest_networked(&wire::encode(&v1), now) {
            NetworkIngestOutcome::Integrated(id) => id,
            _ => panic!("ilk record entegre olmali"),
        };

        // Ayni hash, baska kisi (sk_ikinci), sonraki zaman -> kayit DEGISMEZ.
        let sk_ikinci = SigningKey::from_bytes(&[11u8; 32]);
        let p2 = Record::new(belge_hash).encode();
        let v2 = Vertex::new_signed(NET, vec![id1], p2, now + 100, &sk_ikinci).expect("v2");
        node.ingest_networked(&wire::encode(&v2), now + 100);

        // KANIT: ilk kaydeden + ilk zaman korundu.
        let kayit = node.belge_dogrula(&belge_hash).unwrap();
        assert_eq!(kayit.kaydeden, adr_ilk, "ilk kaydeden korunur");
        assert_eq!(kayit.zaman, now, "ilk zaman korunur");
        assert_eq!(node.belge_sayisi(), 1, "tek belge kaydi");
    }

    // ===== KURUM kimlik ingest entegrasyonu =====

    // tip=5 kurum vertex'i ingest edilince KurumRegistry'ye islenir:
    // kaydeden=imzalayan, kategori, ad, zaman. Sonra sorgulanir.
    #[test]
    fn ingest_kurum_kaydedilir() {
        use crate::registry::{public_key_to_adres, KurumKategori};
        use crate::tx::KurumKaydiTx;
        let now = 1_700_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        // Bir devlet kurumu kendini kaydeder (kategori=0).
        let sk = SigningKey::from_bytes(&[20u8; 32]);
        let kurum_adr = public_key_to_adres(&sk.verifying_key().to_bytes());
        let payload = KurumKaydiTx::new(0, "Tapu Mudurlugu".into()).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &sk).expect("kurum vertex");
        assert!(matches!(
            node.ingest_networked(&wire::encode(&v), now),
            NetworkIngestOutcome::Integrated(_)
        ));

        // KANIT: kurum zincirde sorgulanabilir, kaydeden=imzalayan.
        let k = node
            .kurum_sorgula(&kurum_adr)
            .expect("kurum kayitli olmali");
        assert_eq!(k.ad, "Tapu Mudurlugu");
        assert_eq!(k.kategori, KurumKategori::Devlet);
        assert_eq!(k.zaman, now);
        assert_eq!(node.kurum_sayisi(), 1);
    }

    // Devlet ve ozel firma AYNI sistemde, kategoriyle AYRILIR (karismaz).
    #[test]
    fn ingest_kurum_kategori_ayrimi() {
        use crate::registry::{public_key_to_adres, KurumKategori};
        use crate::tx::KurumKaydiTx;
        let now = 1_700_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        // Devlet kurumu (sk_d, kategori=0).
        let sk_d = SigningKey::from_bytes(&[21u8; 32]);
        let adr_d = public_key_to_adres(&sk_d.verifying_key().to_bytes());
        let pd = KurumKaydiTx::new(0, "Nufus Mudurlugu".into()).encode();
        let vd = Vertex::new_signed(NET, vec![gid], pd, now, &sk_d).expect("devlet vertex");
        let id_d = match node.ingest_networked(&wire::encode(&vd), now) {
            NetworkIngestOutcome::Integrated(id) => id,
            _ => panic!("devlet kurumu entegre olmali"),
        };

        // Ozel firma (sk_o, kategori=1).
        let sk_o = SigningKey::from_bytes(&[22u8; 32]);
        let adr_o = public_key_to_adres(&sk_o.verifying_key().to_bytes());
        let po = KurumKaydiTx::new(1, "Ahmet Insaat Ltd".into()).encode();
        let vo = Vertex::new_signed(NET, vec![id_d], po, now, &sk_o).expect("ozel vertex");
        node.ingest_networked(&wire::encode(&vo), now);

        // KANIT: devlet Devlet, ozel Ozel — karismaz.
        assert_eq!(
            node.kurum_sorgula(&adr_d).unwrap().kategori,
            KurumKategori::Devlet
        );
        assert_eq!(
            node.kurum_sorgula(&adr_o).unwrap().kategori,
            KurumKategori::Ozel
        );
        assert_eq!(node.kurum_sayisi(), 2);
    }

    #[test]
    fn ingest_faucet_cifte_damla_engellenir() {
        use crate::registry::public_key_to_adres;
        use crate::tx::FaucetKaydi;
        let now = 1_700_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let owner_sk = SigningKey::from_bytes(&[210u8; 32]);
        let owner_adr = public_key_to_adres(&owner_sk.verifying_key().to_bytes());
        node.faucet_owner_ayarla(owner_adr);

        let alici = [0x77; 20];
        let p1 = FaucetKaydi::new(alici, 1000).encode();
        let v1 = Vertex::new_signed(NET, vec![gid], p1, now, &owner_sk).expect("v1");
        node.ingest_networked(&wire::encode(&v1), now);
        let p2 = FaucetKaydi::new(alici, 1000).encode();
        let v2 = Vertex::new_signed(NET, vec![gid], p2, now + 1, &owner_sk).expect("v2");
        node.ingest_networked(&wire::encode(&v2), now + 1);

        assert_eq!(
            node.bakiye(&alici),
            1000,
            "ikinci faucet damlasi eklenmemeli"
        );
    }

    // A2 KANIT (owner-gating konsensus-deterministik): mainnet owner PINLI kurucu
    // adrese sabitlenir, env'den BAGIMSIZ. Iki mainnet node AYNI owner'i alir ->
    // node-yerel ayrisma (on-satis konsensus bolunmesi) YOK.
    #[test]
    fn a2_mainnet_owner_pinli_deterministik() {
        let n1 = NodeState::new_mainnet();
        let n2 = NodeState::new_mainnet();
        assert!(n1.faucet_owner().is_some(), "mainnet owner PINLI olmali");
        assert_eq!(
            n1.faucet_owner(),
            n2.faucet_owner(),
            "iki mainnet node AYNI owner (deterministik)"
        );
        assert_eq!(
            n1.faucet_owner(),
            Some(crate::mainnet::kurucu_adres()),
            "owner = pinli kurucu adres (env override yok)"
        );
        assert_eq!(
            NodeState::new_devnet(1).faucet_owner(),
            None,
            "devnet owner varsayilan None (testnet env ile ayarlanir)"
        );
    }

    // A2/A5 KANIT (mainnet arz guvenligi): faucet (tip=6) MINT'tir; mainnet'te
    // TAMAMEN KAPALI. Owner imzali faucet bile MINT ETMEZ -> 21M sabit arz korunur.
    #[test]
    fn a2_mainnet_faucet_mint_yapmaz() {
        use crate::registry::public_key_to_adres;
        use crate::tx::FaucetKaydi;
        let zaman = crate::mainnet::MAINNET_GENESIS_ZAMANI + 100;
        let mut node = NodeState::new_mainnet();
        // Pinli mainnet genesis'i ingest et.
        node.ingest_networked(&crate::mainnet::genesis_wire(), zaman);
        let gid = crate::mainnet::genesis_id();

        // Owner'i bir test anahtarina ayarla: mainnet guard owner'DAN BAGIMSIZ calismali
        // (owner eslesse bile mint olmamali).
        let owner_sk = SigningKey::from_bytes(&[0x44u8; 32]);
        let owner = public_key_to_adres(&owner_sk.verifying_key().to_bytes());
        node.faucet_owner_ayarla(owner);

        let alici = [0x55u8; 20];
        let arz_once = node.toplam_bakiye_arzi();
        let p = FaucetKaydi::new(alici, 1_000_000).encode();
        let v = Vertex::new_signed(
            crate::mainnet::MAINNET_NETWORK_ID,
            vec![gid],
            p,
            zaman,
            &owner_sk,
        )
        .expect("faucet vertex");
        node.ingest_networked(&wire::encode(&v), zaman);

        // KANIT: mainnet'te faucet MINT ETMEDI.
        assert_eq!(
            node.bakiye(&alici),
            0,
            "A2: mainnet faucet mint ETMEZ (21M korunur)"
        );
        assert_eq!(
            node.toplam_bakiye_arzi(),
            arz_once,
            "mainnet faucet arzi degistirmedi (mint yok)"
        );
    }

    // A1 KANIT (fon determinizmi): mainnet 7-dilim dagitimi KODA PINLI, env'siz
    // otomatik yuklenir. Iki mainnet node AYNI dagitim + vesting alir; toplam 21M.
    #[test]
    fn a1_mainnet_dagitim_pinli_deterministik() {
        let n1 = NodeState::new_mainnet();
        let n2 = NodeState::new_mainnet();
        let adr = crate::mainnet::dagitim_adresleri();
        let arz = crate::genesis::AIDAG_ARZ;

        // Toplam arz TAM 21M (kapali sistem)
        assert_eq!(n1.toplam_bakiye_arzi(), arz, "mainnet toplam AIDAG = 21M");

        // Her dilim dogru miktar (adr sirasi: eko/hazine/likidite/topluluk/kurucu/erken/onsatis)
        assert_eq!(n1.bakiye(&adr[0]), arz * 22 / 100, "ekosistem %22");
        assert_eq!(n1.bakiye(&adr[1]), arz * 25 / 100, "hazine %25");
        assert_eq!(n1.bakiye(&adr[2]), arz * 15 / 100, "likidite %15");
        assert_eq!(n1.bakiye(&adr[3]), arz * 12 / 100, "topluluk %12");
        assert_eq!(n1.bakiye(&adr[4]), arz * 13 / 100, "kurucu %13");
        assert_eq!(n1.bakiye(&adr[5]), arz * 5 / 100, "erken destekci %5");
        assert_eq!(n1.bakiye(&adr[6]), arz * 8 / 100, "on-satis %8");

        // DETERMINIZM: iki mainnet node AYNI bakiye (env'e bagli ayrisma YOK)
        for a in adr.iter() {
            assert_eq!(n1.bakiye(a), n2.bakiye(a), "iki mainnet node ayni bakiye");
        }

        // VESTING: TGE'de kurucu/ekosistem kilitli, hazine acik
        let tge = crate::mainnet::MAINNET_VESTING_BASLANGIC;
        assert_eq!(
            n1.vesting_kilitli(&adr[4], tge),
            arz * 13 / 100,
            "kurucu TGE'de tam kilitli (6ay cliff)"
        );
        assert_eq!(
            n1.vesting_kilitli(&adr[0], tge),
            arz * 22 / 100,
            "ekosistem TGE'de tam kilitli (12ay dogrusal basi)"
        );
        assert_eq!(
            n1.vesting_kilitli(&adr[1], tge),
            0,
            "hazine acik (kilit yok)"
        );

        // On-satis dilimi (idx 6) = kurucu owner adresi (operasyon/escrow)
        assert_eq!(
            crate::mainnet::kurucu_adres(),
            adr[6],
            "on-satis dilimi = kurucu (owner) adresi"
        );
        assert_eq!(
            n1.faucet_owner(),
            Some(crate::mainnet::kurucu_adres()),
            "on-satis owner pinli kurucu"
        );
    }

    // ===== FAUCET owner kontrolu (aga yayilan, guvenli) =====

    #[test]
    fn ingest_faucet_owner_basabilir() {
        use crate::registry::public_key_to_adres;
        use crate::tx::FaucetKaydi;
        let now = 1_700_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let owner_sk = SigningKey::from_bytes(&[200u8; 32]);
        let owner_adr = public_key_to_adres(&owner_sk.verifying_key().to_bytes());
        node.faucet_owner_ayarla(owner_adr);

        let alici = [0x44; 20];
        let payload = FaucetKaydi::new(alici, 1000).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &owner_sk).expect("faucet vertex");
        node.ingest_networked(&wire::encode(&v), now);

        assert_eq!(node.bakiye(&alici), 1000);
    }

    #[test]
    fn ingest_faucet_owner_olmayan_reddedilir() {
        use crate::registry::public_key_to_adres;
        use crate::tx::FaucetKaydi;
        let now = 1_700_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let owner_sk = SigningKey::from_bytes(&[200u8; 32]);
        let owner_adr = public_key_to_adres(&owner_sk.verifying_key().to_bytes());
        node.faucet_owner_ayarla(owner_adr);

        let saldirgan_sk = SigningKey::from_bytes(&[201u8; 32]);
        let alici = [0x55; 20];
        let payload = FaucetKaydi::new(alici, 1_000_000).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &saldirgan_sk).expect("v");
        node.ingest_networked(&wire::encode(&v), now);

        assert_eq!(node.bakiye(&alici), 0, "owner olmayan faucet basamaz");
    }

    #[test]
    fn ingest_faucet_owner_ayarsiz_kapali() {
        use crate::tx::FaucetKaydi;
        let now = 1_700_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        let birisi_sk = SigningKey::from_bytes(&[202u8; 32]);
        let alici = [0x66; 20];
        let payload = FaucetKaydi::new(alici, 1000).encode();
        let v = Vertex::new_signed(NET, vec![gid], payload, now, &birisi_sk).expect("v");
        node.ingest_networked(&wire::encode(&v), now);

        assert_eq!(node.bakiye(&alici), 0, "owner ayarsizsa faucet kapali");
    }

    // tip=11: EVM-UYUMLU TRANSFER node testi (gercek senaryo).
    // Bir secp256k1 (MetaMask) kullanicisi AIDAG transferi yapar. Gonderen,
    // vertex imzalayanindan DEGIL, secp256k1 imzasindan (ecrecover) cikar.
    #[test]
    fn ingest_evm_transfer_bakiye_dogru_degisir() {
        use crate::tx::{evm_transfer_mesaji, EvmTransfer};
        use k256::ecdsa::{
            signature::hazmat::PrehashSigner, RecoveryId, Signature as K256Sig,
            SigningKey as K256Sk, VerifyingKey as K256Vk,
        };
        use sha3::{Digest, Keccak256};

        let now = 1_000_000;
        let mut node = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        node.ingest_networked(&gen, now);

        // 1) secp256k1 (MetaMask) kullanicisi -> 0x gonderen adresi
        let k_sk = K256Sk::from_slice(&[7u8; 32]).expect("k256 anahtar");
        let k_vk = K256Vk::from(&k_sk);
        let nokta = k_vk.to_encoded_point(false);
        let h = Keccak256::digest(&nokta.as_bytes()[1..]);
        let mut gonderen = [0u8; 20];
        gonderen.copy_from_slice(&h[12..]);

        // 2) gonderene AIDAG bakiyesi ver
        node.test_bakiye_ekle(gonderen, 1_000_000);
        let arz_basta = node.toplam_bakiye_arzi();
        let alici = [0x99u8; 20];

        // 3) EVM transferi olustur + secp256k1 ile imzala (nonce=0)
        let miktar = 30_000u128;
        let nonce = 0u64;
        let mesaj = evm_transfer_mesaji(&alici, miktar, nonce);
        let prehash = Keccak256::digest(&mesaj);
        let (sig, recid): (K256Sig, RecoveryId) = k_sk.sign_prehash(&prehash).expect("imza");
        let evm_t = EvmTransfer {
            alici,
            miktar,
            nonce,
            recovery_id: recid.to_byte(),
            imza: sig.to_bytes().into(),
        };

        // 4) Vertex'i AYRI bir ed25519 anahtari imzalar (tasiyici/relay).
        //    Gonderen yine de secp256k1 sahibidir (ecrecover) - tasiyici degil.
        let tasiyici = SigningKey::from_bytes(&[99u8; 32]);
        let vc = Vertex::new_signed(NET, vec![gid], evm_t.encode(), now, &tasiyici)
            .expect("evm transfer vertex");
        node.ingest_networked(&wire::encode(&vc), now);

        // 5) KANIT: transfer secp256k1 sahibinden cikti
        assert_eq!(node.bakiye(&alici), miktar, "alici dogru miktari aldi");
        assert_eq!(
            node.bakiye(&gonderen),
            1_000_000 - miktar,
            "gonderen dogru dustu"
        );
        assert_eq!(node.beklenen_nonce(&gonderen), 1, "gonderen nonce ilerledi");
        assert_eq!(node.toplam_bakiye_arzi(), arz_basta, "TOPLAM ARZ KORUNDU");
    }

    // ===============================================================
    // COK-NODE ENTEGRASYON TESTLERI
    // Iki bagimsiz NodeState + AYNI genesis. Vertex'ler karsilikli
    // beslenir. Kritik soru: ayni vertex kumesi FARKLI SIRADA gelirse
    // iki node AYNI duruma mi yakinsar?
    // ===============================================================

    fn iki_node(now: u64) -> (NodeState, NodeState, VertexId) {
        let mut n1 = NodeState::new_devnet(NET);
        let mut n2 = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        n1.ingest_networked(&gen, now);
        n2.ingest_networked(&gen, now);
        (n1, n2, gid)
    }

    #[test]
    fn cok_node_senkron_ayni_duruma_yakinsar() {
        use crate::registry::public_key_to_adres;
        use crate::tx::TransferKaydi;
        let now = 1_000_000;
        let (mut n1, mut n2, gid) = iki_node(now);
        let sk = SigningKey::from_bytes(&[11u8; 32]);
        let gonderen = public_key_to_adres(&sk.verifying_key().to_bytes());
        let alici = [0xA1; 20];
        n1.test_bakiye_ekle(gonderen, 1000);
        n2.test_bakiye_ekle(gonderen, 1000);

        let p = TransferKaydi::new(alici, 300, 0).encode();
        let v = Vertex::new_signed(NET, vec![gid], p, now, &sk).expect("v");
        let bytes = wire::encode(&v);

        n1.ingest_networked(&bytes, now);
        assert_eq!(n1.bakiye(&gonderen), 700);
        assert_eq!(n2.bakiye(&gonderen), 1000, "node2 henuz gormedi");

        n2.ingest_networked(&bytes, now);
        assert_eq!(n1.bakiye(&gonderen), n2.bakiye(&gonderen), "gonderen ayni");
        assert_eq!(n1.bakiye(&alici), n2.bakiye(&alici), "alici ayni");
        assert_eq!(
            n1.beklenen_nonce(&gonderen),
            n2.beklenen_nonce(&gonderen),
            "nonce ayni"
        );
        assert_eq!(n1.toplam_bakiye_arzi(), n2.toplam_bakiye_arzi(), "arz ayni");
    }

    // EN KRITIK: ayni nonce, iki farkli transfer, iki node, TERS SIRA.
    #[test]
    fn cok_node_eszamanli_ayni_nonce_cift_harcama() {
        use crate::registry::public_key_to_adres;
        use crate::tx::TransferKaydi;
        let now = 1_000_000;
        let (mut n1, mut n2, gid) = iki_node(now);
        let sk = SigningKey::from_bytes(&[12u8; 32]);
        let gonderen = public_key_to_adres(&sk.verifying_key().to_bytes());
        let alici_a = [0xAA; 20];
        let alici_b = [0xBB; 20];
        n1.test_bakiye_ekle(gonderen, 1000);
        n2.test_bakiye_ekle(gonderen, 1000);

        let pa = TransferKaydi::new(alici_a, 800, 0).encode();
        let va = Vertex::new_signed(NET, vec![gid], pa, now, &sk).expect("va");
        let ba = wire::encode(&va);

        let pb = TransferKaydi::new(alici_b, 800, 0).encode();
        let vb = Vertex::new_signed(NET, vec![gid], pb, now + 1, &sk).expect("vb");
        let bb = wire::encode(&vb);

        // Bolunme: farkli node'lar farkli vertex'i once gorur.
        n1.ingest_networked(&ba, now);
        n2.ingest_networked(&bb, now + 1);

        // Birlesme: ikisi de digerini gorur (TERS SIRALARDA).
        n1.ingest_networked(&bb, now + 1);
        n2.ingest_networked(&ba, now);

        assert_eq!(n1.toplam_bakiye_arzi(), 1000, "node1: ARZ SABIT");
        assert_eq!(n2.toplam_bakiye_arzi(), 1000, "node2: ARZ SABIT");

        let a1 = n1.bakiye(&alici_a);
        let b1 = n1.bakiye(&alici_b);
        assert!(
            (a1 == 800 && b1 == 0) || (a1 == 0 && b1 == 800),
            "node1: TAM OLARAK BIRI uygulanmali (a={a1}, b={b1})"
        );
        assert_eq!(n1.bakiye(&gonderen), 200, "node1: gonderen 200");

        assert_eq!(
            n1.bakiye(&alici_a),
            n2.bakiye(&alici_a),
            "YAKINSAMA alici_a"
        );
        assert_eq!(
            n1.bakiye(&alici_b),
            n2.bakiye(&alici_b),
            "YAKINSAMA alici_b"
        );
        assert_eq!(
            n1.bakiye(&gonderen),
            n2.bakiye(&gonderen),
            "YAKINSAMA gonderen"
        );
        assert_eq!(n1.beklenen_nonce(&gonderen), 1, "nonce bir kez ilerledi");
        assert_eq!(n2.beklenen_nonce(&gonderen), 1, "nonce bir kez ilerledi");
    }

    // BOLUNME + BIRLESME: ayri kollar, sonra merge.
    #[test]
    fn cok_node_bolunme_birlesme_yakinsar() {
        use crate::registry::public_key_to_adres;
        use crate::tx::TransferKaydi;
        let now = 1_000_000;
        let (mut n1, mut n2, gid) = iki_node(now);
        let sk_x = SigningKey::from_bytes(&[13u8; 32]);
        let sk_y = SigningKey::from_bytes(&[14u8; 32]);
        let x = public_key_to_adres(&sk_x.verifying_key().to_bytes());
        let y = public_key_to_adres(&sk_y.verifying_key().to_bytes());
        let hedef = [0xCC; 20];

        n1.test_bakiye_ekle(x, 500);
        n1.test_bakiye_ekle(y, 500);
        n2.test_bakiye_ekle(x, 500);
        n2.test_bakiye_ekle(y, 500);

        let px = TransferKaydi::new(hedef, 100, 0).encode();
        let vx = Vertex::new_signed(NET, vec![gid], px, now, &sk_x).expect("vx");
        let bx = wire::encode(&vx);

        let py = TransferKaydi::new(hedef, 200, 0).encode();
        let vy = Vertex::new_signed(NET, vec![gid], py, now + 1, &sk_y).expect("vy");
        let by = wire::encode(&vy);

        // BOLUNME
        n1.ingest_networked(&bx, now);
        n2.ingest_networked(&by, now + 1);
        assert_eq!(n1.bakiye(&y), 500, "node1 kol B'yi gormedi");
        assert_eq!(n2.bakiye(&x), 500, "node2 kol A'yi gormedi");

        // BIRLESME: once iki kol da her iki node'a yayilir...
        n1.ingest_networked(&by, now + 1);
        n2.ingest_networked(&bx, now);

        // ...sonra IKISINI DE parent alan bir BIRLESTIRICI vertex gelir.
        // GHOSTDAG'da kardes tip'ler, onlari birlestiren bir blok gelene
        // kadar total_order'a GIRMEZ (beklenen davranis). Gercek agda bir
        // sonraki blok bunu yapar; testte biz uretiyoruz.
        let sk_m = SigningKey::from_bytes(&[15u8; 32]);
        let bos = TransferKaydi::new([0x00; 20], 0, 0).encode();
        // Parent'lar ARTAN id sirasinda olmali (protokol kurali).
        let mut ebeveynler = vec![*vx.id(), *vy.id()];
        ebeveynler.sort();
        let vm = Vertex::new_signed(NET, ebeveynler, bos, now + 2, &sk_m).expect("vm");
        let bm = wire::encode(&vm);
        n1.ingest_networked(&bm, now + 2);
        n2.ingest_networked(&bm, now + 2);

        assert_eq!(n1.bakiye(&x), n2.bakiye(&x), "YAKINSAMA X");
        assert_eq!(n1.bakiye(&y), n2.bakiye(&y), "YAKINSAMA Y");
        assert_eq!(n1.bakiye(&hedef), n2.bakiye(&hedef), "YAKINSAMA hedef");
        assert_eq!(n1.bakiye(&x), 400, "X'ten 100 dustu");
        assert_eq!(n1.bakiye(&y), 300, "Y'den 200 dustu");
        assert_eq!(n1.bakiye(&hedef), 300, "hedef 300 aldi");
        assert_eq!(n1.toplam_bakiye_arzi(), 1000, "node1 arz sabit");
        assert_eq!(n2.toplam_bakiye_arzi(), 1000, "node2 arz sabit");
    }

    // INVARIANT: artimli (append fast-path + reorg fallback) sonucu, HER ZAMAN
    // tam-yeniden-hesap ile AYNI olmali. Rastgele DAG yapilari uret; her
    // adimda iki node karsilastir: (A) artimli yol (normal ingest),
    // (B) ayni vertex'leri TAMAMEN sifirdan alan taze node. Bakiye/nonce/arz
    // birebir esit olmali. Esit degilse artimli optimizasyon BOZUK demektir.
    #[test]
    fn artimli_esittir_tam_yeniden_hesap() {
        use crate::tx::TransferKaydi;
        let net = NET;
        let now = 1_000_000u64;

        // Deterministik pseudo-random (sabit tohum -> tekrarlanabilir).
        let mut rng: u64 = 0x1234_5678_9abc_def0;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };

        let sk = SigningKey::from_bytes(&[9u8; 32]);
        let gonderen = crate::registry::public_key_to_adres(&sk.verifying_key().to_bytes());

        // ARTIMLI node (normal ingest yolu = append fast-path + reorg fallback).
        let mut a = NodeState::new_devnet(net);
        let (gen, gid) = genesis_bytes(1, now);
        a.ingest_networked(&gen, now);
        a.test_bakiye_ekle(gonderen, 1_000_000);

        // Uretilen tum vertex baytlari (B node'unu sifirdan kurmak icin).
        let mut hepsi: Vec<(Vec<u8>, u64)> = Vec::new();
        let mut tips: Vec<VertexId> = vec![gid];
        let mut nonce = 0u64;

        #[allow(clippy::explicit_counter_loop)]
        for adim in 0..60u64 {
            // Rastgele 1-2 parent sec (dallanma + birlesme uret).
            let mut parents: Vec<VertexId> = Vec::new();
            let p1 = tips[(next() as usize) % tips.len()];
            parents.push(p1);
            if tips.len() > 1 && next() % 3 == 0 {
                let p2 = tips[(next() as usize) % tips.len()];
                if p2 != p1 {
                    parents.push(p2);
                }
            }
            parents.sort();
            parents.dedup();

            let miktar = (1 + (next() % 5)) as u128;
            let payload = TransferKaydi::new([0x55; 20], miktar, nonce).encode();
            nonce += 1;
            let ts = now + adim + 1;
            let v = Vertex::new_signed(net, parents.clone(), payload, ts, &sk).expect("v");
            let bytes = wire::encode(&v);

            a.ingest_networked(&bytes, ts);
            hepsi.push((bytes, ts));

            // tips guncelle: kullanilan parent'lari cikar, yeni id ekle.
            tips.retain(|t| !parents.contains(t));
            tips.push(*v.id());

            // ---- B node: SIFIRDAN, ayni vertex'leri sirayla al ----
            let mut b = NodeState::new_devnet(net);
            b.ingest_networked(&gen, now);
            b.test_bakiye_ekle(gonderen, 1_000_000);
            for (byt, t) in &hepsi {
                b.ingest_networked(byt, *t);
            }

            // KARSILASTIR: artimli (a) == tam-hesap (b)
            assert_eq!(
                a.bakiye(&gonderen),
                b.bakiye(&gonderen),
                "adim {adim}: gonderen bakiye artimli != tam"
            );
            assert_eq!(
                a.bakiye(&[0x55; 20]),
                b.bakiye(&[0x55; 20]),
                "adim {adim}: alici bakiye artimli != tam"
            );
            assert_eq!(
                a.beklenen_nonce(&gonderen),
                b.beklenen_nonce(&gonderen),
                "adim {adim}: nonce artimli != tam"
            );
            assert_eq!(
                a.toplam_bakiye_arzi(),
                b.toplam_bakiye_arzi(),
                "adim {adim}: ARZ artimli != tam"
            );
        }
    }

    // ===============================================================
    // SYNC SAGLAMLIK: chunked/offset-tabanli sync, sync SIRASINDA
    // peer'a yeni (kucuk-id'li) vertex girerse vertex ATLAR mi?
    // export_vertices() id-sirali topolojik sira dondurur; yeni kucuk-id'li
    // vertex listenin ORTASINA girer -> offset tabanli devam istegi kayabilir.
    // Bu test ag katmanini (libp2p) DEGIL, sync MANTIGINI izole dener.
    // ===============================================================
    #[test]
    fn sync_sirasinda_eklenen_vertex_atlanmaz() {
        use crate::tx::TransferKaydi;
        let now = 1_000_000u64;
        let net = NET;
        let kucuk_chunk = 3usize; // SYNC_CHUNK'i kucultup senaryoyu tetikle

        // --- PEER node: kaynak. Genesis + bir zincir uret. ---
        let mut peer = NodeState::new_devnet(net);
        let (gen, gid) = genesis_bytes(1, now);
        peer.ingest_networked(&gen, now);

        // sk secimi: id'leri KONTROL edemeyiz (blake3), ama cok vertex uretip
        // sync ortasinda yeni ekleyerek kaymayi tetikleriz.
        let sk = SigningKey::from_bytes(&[3u8; 32]);
        let mut parent = gid;
        for i in 0..8u64 {
            let pl = TransferKaydi::new([0x11; 20], 1, i).encode();
            let v = Vertex::new_signed(net, vec![parent], pl, now + 1 + i, &sk).expect("v");
            parent = *v.id();
            peer.ingest_networked(&wire::encode(&v), now + 1 + i);
        }

        // --- CEKEN node: bos (sadece genesis). Chunked sync simule et. ---
        let mut ceken = NodeState::new_devnet(net);
        ceken.ingest_networked(&gen, now);

        // SIMULASYON: gercek ag dongusundeki offset mantiginin AYNISI.
        // 1) ilk parcayi peer.export_vertices()[offset..offset+chunk] al
        // 2) ceken'e ingest et
        // 3) offset += alinan
        // 4) SYNC ORTASINDA: peer'a YENI vertex ekle (kucuk-id olabilir)
        // 5) devam et: peer.export_vertices() YENIDEN cagirilir (gercekte de oyle)
        let mut offset = 0usize;
        let mut adim = 0;
        let mut eklendi = false;
        loop {
            let all = peer.export_vertices();
            let total = all.len();
            let parca: Vec<Vec<u8>> = all.into_iter().skip(offset).take(kucuk_chunk).collect();
            let alinan = parca.len();
            for byt in &parca {
                ceken.ingest_synced(byt);
            }
            offset += alinan;

            // Ilk parcadan sonra, sync BITMEDEN peer'a yeni vertex ekle (bir kez).
            if !eklendi && adim == 0 {
                let pl = TransferKaydi::new([0x22; 20], 1, 100).encode();
                let v = Vertex::new_signed(net, vec![parent], pl, now + 500, &sk).expect("yeni");
                parent = *v.id();
                peer.ingest_networked(&wire::encode(&v), now + 500);
                eklendi = true;
            }

            adim += 1;
            if alinan == 0 || offset >= total {
                // total, YENI ekleme sonrasi degismis olabilir; bir tur daha dene.
                let guncel_total = peer.export_vertices().len();
                if offset >= guncel_total {
                    break;
                }
            }
            if adim > 50 {
                break;
            } // sonsuz dongu guvenligi
        }

        // KANIT: ceken, peer'daki TUM vertex'lere sahip mi?
        let peer_ids: std::collections::BTreeSet<VertexId> = peer
            .export_vertices()
            .iter()
            .filter_map(|b| crate::dag::wire::decode(b).ok().map(|v| *v.id()))
            .collect();
        let ceken_ids: std::collections::BTreeSet<VertexId> = ceken
            .export_vertices()
            .iter()
            .filter_map(|b| crate::dag::wire::decode(b).ok().map(|v| *v.id()))
            .collect();

        let eksik: Vec<_> = peer_ids.difference(&ceken_ids).collect();
        eprintln!(
            "[SYNC] peer={} ceken={} eksik={}",
            peer_ids.len(),
            ceken_ids.len(),
            eksik.len()
        );
        assert!(
            eksik.is_empty(),
            "SYNC ATLADI: ceken'de {} vertex eksik (offset-kaymasi bug'i)",
            eksik.len()
        );
    }

    // ===============================================================
    // SYNC OTOMATIK TESTI (denetim raporu bloker #14).
    // Taze bir node, dolu bir peer'dan DAG'i CHUNKED/OFFSET sync ile
    // (gercek ag dongusunun AYNI mantigi: export_vertices + skip/take
    // + ingest_synced) baştan ceker. KANIT: sadece DAG yapisi degil,
    // STATE de (bakiye/nonce/arz) BIREBIR yakinsar. State ayrismasi =
    // sessiz zincir-bolunmesi; bu test onu regresyona baglar.
    // ===============================================================
    #[test]
    fn sync_taze_node_ayni_state_e_yakinsar() {
        use crate::registry::public_key_to_adres;
        use crate::tx::TransferKaydi;
        let now = 1_000_000u64;

        // --- PEER: genesis + bakiye + bir zincir transfer ---
        let mut peer = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        peer.ingest_networked(&gen, now);

        let sk = SigningKey::from_bytes(&[21u8; 32]);
        let gonderen = public_key_to_adres(&sk.verifying_key().to_bytes());
        let alici = [0x77; 20];
        peer.test_bakiye_ekle(gonderen, 10_000);

        // Zincirlenmis transfer'ler (kardes degil -> total_order'a hepsi girer)
        let mut parent = gid;
        for i in 0..6u64 {
            let p = TransferKaydi::new(alici, 100, i).encode();
            let v = Vertex::new_signed(NET, vec![parent], p, now + 1 + i, &sk).expect("v");
            parent = *v.id();
            peer.ingest_networked(&wire::encode(&v), now + 1 + i);
        }

        // Peer'in beklenen state'i (referans)
        let peer_gonderen = peer.bakiye(&gonderen);
        let peer_alici = peer.bakiye(&alici);
        let peer_nonce = peer.beklenen_nonce(&gonderen);
        let peer_arz = peer.toplam_bakiye_arzi();
        assert_eq!(peer_gonderen, 10_000 - 600, "peer: 6x100 dustu");
        assert_eq!(peer_alici, 600, "peer: alici 600 aldi");

        // --- TAZE NODE: CHUNKED sync (gercek ag mantiginin aynisi) ---
        let mut taze = NodeState::new_devnet(NET);
        // Taze node ayni baslangic dagitimini bilir (genesis dagitimi;
        // testte test_bakiye_ekle ile ayni sekilde kurulur).
        taze.test_bakiye_ekle(gonderen, 10_000);

        const CHUNK: usize = 2; // kucuk chunk -> cok turlu sync'i zorla
        let mut offset = 0usize;
        loop {
            let all = peer.export_vertices();
            let total = all.len();
            let parca: Vec<Vec<u8>> = all.into_iter().skip(offset).take(CHUNK).collect();
            if parca.is_empty() { break; }
            for byt in &parca {
                taze.ingest_synced(byt);
            }
            offset += parca.len();
            if offset >= total { break; }
        }

        // Orphan kalmissa cozdur (sirasizlik guvencesi)
        for byt in peer.export_vertices() {
            taze.ingest_synced(&byt);
        }

        // --- KANIT: STATE birebir yakinsadi ---
        assert_eq!(taze.vertex_count(), peer.vertex_count(), "vertex sayisi ayni");
        assert_eq!(taze.orphan_count(), 0, "orphan kalmadi");
        assert_eq!(taze.bakiye(&gonderen), peer_gonderen, "YAKINSAMA: gonderen bakiye");
        assert_eq!(taze.bakiye(&alici), peer_alici, "YAKINSAMA: alici bakiye");
        assert_eq!(taze.beklenen_nonce(&gonderen), peer_nonce, "YAKINSAMA: nonce");
        assert_eq!(taze.toplam_bakiye_arzi(), peer_arz, "YAKINSAMA: toplam arz");
    }

    // ===============================================================
    // SYNC DAYANIKLILIK (bloker #14, 2. varyant): vertex'ler KARISIK
    // SIRADA gelirse. Gercek agda parcalar farkli sirayla dusebilir;
    // orphan+cascade sirasizligi cozmeli ve state YINE ayni yakinsamali.
    // ===============================================================
    #[test]
    fn sync_sirasiz_gelse_de_ayni_state_e_yakinsar() {
        use crate::registry::public_key_to_adres;
        use crate::tx::TransferKaydi;
        let now = 1_000_000u64;

        let mut peer = NodeState::new_devnet(NET);
        let (gen, gid) = genesis_bytes(1, now);
        peer.ingest_networked(&gen, now);
        let sk = SigningKey::from_bytes(&[22u8; 32]);
        let gonderen = public_key_to_adres(&sk.verifying_key().to_bytes());
        let alici = [0x88; 20];
        peer.test_bakiye_ekle(gonderen, 10_000);
        let mut parent = gid;
        for i in 0..6u64 {
            let p = TransferKaydi::new(alici, 100, i).encode();
            let v = Vertex::new_signed(NET, vec![parent], p, now + 1 + i, &sk).expect("v");
            parent = *v.id();
            peer.ingest_networked(&wire::encode(&v), now + 1 + i);
        }

        let mut taze = NodeState::new_devnet(NET);
        taze.test_bakiye_ekle(gonderen, 10_000);

        // KARISIK SIRA: export'u TERS cevirerek besle (cocuklar once,
        // genesis en son) -> orphan havuzu + cascade cozmeli.
        let mut karisik = peer.export_vertices();
        karisik.reverse();

        // Cok turlu: cozulmeyenler orphan'da bekler, tekrar denenir.
        loop {
            let before = taze.vertex_count();
            for byt in &karisik {
                taze.ingest_synced(byt);
            }
            if taze.vertex_count() == before { break; }
        }

        assert_eq!(taze.vertex_count(), peer.vertex_count(), "vertex sayisi ayni");
        assert_eq!(taze.orphan_count(), 0, "orphan cozuldu");
        assert_eq!(taze.bakiye(&gonderen), peer.bakiye(&gonderen), "YAKINSAMA: gonderen");
        assert_eq!(taze.bakiye(&alici), peer.bakiye(&alici), "YAKINSAMA: alici");
        assert_eq!(taze.beklenen_nonce(&gonderen), peer.beklenen_nonce(&gonderen), "YAKINSAMA: nonce");
        assert_eq!(taze.toplam_bakiye_arzi(), peer.toplam_bakiye_arzi(), "YAKINSAMA: arz");
    }
}