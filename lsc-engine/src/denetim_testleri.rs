//! DENETIM (2026-09-24) KRITIK BULGU REGRESYON TESTLERI.
//! Her test, bagimsiz denetimdeki bir PoC'nin TERSINI dogrular: PoC'deki saldiri
//! artik ise yaramamali. Bulgu kimlikleri DENETIM_ONCESI_RAPOR.md ile ayni (K-01..K-06).

use crate::dag::vertex::{Vertex, MAX_PARENTS};
use crate::dag::wire;
use crate::node::{EvmMakbuzDurum, NetworkIngestOutcome, NodeState};
use crate::registry::{public_key_to_adres, VestingKaydi};
use crate::VertexId;
use ed25519_dalek::SigningKey;

const NET: u32 = 1;
const NOW: u64 = crate::mainnet::ON_SATIS_BASLANGIC;
const E18: u128 = 1_000_000_000_000_000_000;
const ETH_SK: &str = "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318";

fn yeni_node(net: u32) -> (NodeState, VertexId) {
    let mut node = NodeState::new_devnet(net);
    let sk = SigningKey::from_bytes(&[1u8; 32]);
    let g = Vertex::new_signed(net, vec![], vec![1, 1], NOW, &sk).unwrap();
    let gid = *g.id();
    node.ingest_networked(&wire::encode(&g), NOW);
    (node, gid)
}

fn gonder(node: &mut NodeState, parent: VertexId, payload: Vec<u8>, sk: &SigningKey) -> VertexId {
    let v = Vertex::new_signed(NET, vec![parent], payload, NOW, sk).unwrap();
    let id = *v.id();
    let r = node.ingest_networked(&wire::encode(&v), NOW);
    assert!(matches!(r, NetworkIngestOutcome::Integrated(_)), "{r:?}");
    id
}

/// Tamamen kilitli vesting (baslangic gelecekte).
fn tam_kilit(toplam: u128) -> VestingKaydi {
    let (cliff, sure) = crate::genesis::dilim_vesting(4).unwrap();
    VestingKaydi {
        toplam,
        baslangic: NOW + 365 * 86400,
        cliff_sure: cliff,
        toplam_sure: sure,
        tge_acik: 0,
    }
}

fn eth_adres() -> [u8; 20] {
    use alloy_signer_local::PrivateKeySigner;
    let eth: PrivateKeySigner = ETH_SK.parse().unwrap();
    crate::avm::evm_to_adres(&eth.address())
}

fn raw_eth(chain_id: Option<u64>, nonce: u64, to: [u8; 20], value: u128) -> Vec<u8> {
    use alloy_consensus::{SignableTransaction, TxLegacy};
    use alloy_eips::eip2718::Encodable2718;
    use alloy_primitives::{Signature, TxKind as ATxKind, U256 as AU256};
    use alloy_signer::SignerSync;
    use alloy_signer_local::PrivateKeySigner;
    let eth: PrivateKeySigner = ETH_SK.parse().unwrap();
    let tx = TxLegacy {
        chain_id,
        nonce,
        gas_price: 1_000_000_000,
        gas_limit: 21_000,
        to: ATxKind::Call(crate::avm::adres_to_evm(&to)),
        value: AU256::from(value),
        input: Default::default(),
    };
    let imza: Signature = eth.sign_hash_sync(&tx.signature_hash()).unwrap();
    let zarf: alloy_consensus::TxEnvelope = tx.into_signed(imza).into();
    zarf.encoded_2718()
}

/// tip=12 vertex'i (ham eth tx) ingest eder; vertex'i imzalayan onemsiz (gonderen imzadan).
fn eth_gonder(node: &mut NodeState, parent: VertexId, raw: &[u8]) -> VertexId {
    let tasiyici = SigningKey::from_bytes(&[0x99u8; 32]);
    gonder(node, parent, crate::tx::ham_eth_tx_payload(raw), &tasiyici)
}

// ---------------------------------------------------------------- K-01
#[test]
fn k01_tip_sisirme_uretimi_durdurmaz() {
    let mut node = NodeState::new_mainnet();
    let simdi = crate::mainnet::ON_SATIS_BASLANGIC;
    let gid = node.ingest(&crate::mainnet::genesis_wire(), simdi).unwrap();
    let net = crate::mainnet::MAINNET_NETWORK_ID;
    let durust = SigningKey::from_bytes(&[0x10u8; 32]);
    let v = Vertex::new_signed(net, vec![gid], b"durust-1".to_vec(), simdi, &durust).unwrap();
    let vid = *v.id();
    assert!(matches!(
        node.ingest_networked(&wire::encode(&v), simdi),
        NetworkIngestOutcome::Integrated(_)
    ));
    // Saldirgan 12 paralel uc uretir (MAX_PARENTS=8'den fazla).
    let s = SigningKey::from_bytes(&[0xE1u8; 32]);
    for i in 0..12u8 {
        let a = Vertex::new_signed(net, vec![vid], vec![0xF0, i], simdi, &s).unwrap();
        node.ingest_networked(&wire::encode(&a), simdi);
    }
    assert_eq!(node.tips().len(), 12);
    // Durust uretici her turda en fazla 8 uc secer ve vertex uretebilir; uclar birlesir.
    for tur in 0..3u64 {
        let p = node.uretim_ebeveynleri();
        assert!(p.len() <= MAX_PARENTS, "secim MAX_PARENTS'i asmamali");
        assert!(p.windows(2).all(|w| w[0] < w[1]), "canonical (artan) sira");
        let d = Vertex::new_signed(
            net,
            p,
            format!("durust-{tur}").into_bytes(),
            simdi + 1 + tur,
            &durust,
        )
        .expect("durust vertex uretilebilmeli");
        let r = node.ingest_networked(&wire::encode(&d), simdi + 1 + tur);
        assert!(matches!(r, NetworkIngestOutcome::Integrated(_)), "{r:?}");
    }
    assert_eq!(node.tips().len(), 1, "fazla uclar kademeli birlestirildi");
}

// ---------------------------------------------------------------- K-02
#[test]
fn k02_gelecek_tarihli_vertex_orphan_havuzuna_giremez() {
    let simdi = crate::mainnet::ON_SATIS_BASLANGIC;
    let net = crate::mainnet::MAINNET_NETWORK_ID;
    let gelecek: u64 = 4_200_000_000; // 2103
    let s = SigningKey::from_bytes(&[0xE2u8; 32]);
    let gid = crate::mainnet::genesis_id();
    let p = Vertex::new_signed(net, vec![gid], b"P".to_vec(), simdi, &s).unwrap();
    let c = Vertex::new_signed(net, vec![*p.id()], b"C".to_vec(), gelecek, &s).unwrap();
    let (pb, cb) = (wire::encode(&p), wire::encode(&c));

    let mut n = NodeState::new_mainnet();
    n.ingest(&crate::mainnet::genesis_wire(), simdi).unwrap();
    // Eskiden: Buffered (saat kontrolsuz havuz + disk). Simdi: havuza girmeden red.
    assert!(matches!(
        n.ingest_networked(&cb, simdi),
        NetworkIngestOutcome::Rejected(_)
    ));
    assert_eq!(n.orphan_count(), 0);
    // Ebeveyn pull-sync ile gelse bile C DAG'a girmez.
    assert!(matches!(
        n.ingest_synced_es(&pb, simdi),
        NetworkIngestOutcome::Integrated(_)
    ));
    assert!(!n.contains(c.id()));
    // Durust uretim devam eder.
    let d = SigningKey::from_bytes(&[0x10u8; 32]);
    let yeni = Vertex::new_signed(
        net,
        n.uretim_ebeveynleri(),
        b"durust".to_vec(),
        simdi + 60,
        &d,
    )
    .unwrap();
    assert!(matches!(
        n.ingest_networked(&wire::encode(&yeni), simdi + 60),
        NetworkIngestOutcome::Integrated(_)
    ));
    // Disk yukleyicisinin kullandigi kontrol (lsc-net): C gercek saate gore reddedilir,
    // durust P kabul edilir.
    assert!(n.saat_politikasi(&c, simdi).is_err());
    assert!(n.saat_politikasi(&p, simdi).is_ok());
}

// ---------------------------------------------------------------- K-03
#[test]
fn k03_basarisiz_avm_cagrisi_kontrat_durumunu_silmez() {
    let (mut node, gid) = yeni_node(NET);
    let sk = SigningKey::from_bytes(&[11u8; 32]);
    let g = public_key_to_adres(&sk.verifying_key().to_bytes());
    node.lsc_test_bakiye_ekle(g, E18);
    node.test_bakiye_ekle(g, 1_000_000);
    let bin_hex = include_str!("../../avm-sozlesmeler/Kasa.bin").trim();
    let kod: Vec<u8> = (0..bin_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&bin_hex[i..i + 2], 16).unwrap())
        .collect();
    let v1 = gonder(
        &mut node,
        gid,
        crate::tx::AvmCagri::new([0u8; 20], 0, 0, kod).encode(),
        &sk,
    );
    let kasa = node.avm_kontrat_adresleri()[0];
    let v2 = gonder(
        &mut node,
        v1,
        crate::tx::AvmCagri::new(kasa, 500_000, 1, vec![0xa8, 0x19, 0xfd, 0xf8]).encode(),
        &sk,
    );
    assert_eq!(node.bakiye(&kasa), 500_000);

    // Saldiri: initcode sinirini asan deploy (revm dogrulama hatasi).
    let s = SigningKey::from_bytes(&[0x70u8; 32]);
    let sa = public_key_to_adres(&s.verifying_key().to_bytes());
    node.lsc_test_bakiye_ekle(sa, E18 / 100);
    gonder(
        &mut node,
        v2,
        crate::tx::AvmCagri::new([0u8; 20], 0, 0, vec![0x5B; 50_000]).encode(),
        &s,
    );
    assert!(node.avm_kod_var_mi(&kasa), "kontrat kodu korunmali");
    assert_eq!(node.avm_kontrat_adresleri().len(), 1);
    assert_eq!(node.bakiye(&kasa), 500_000);
}

// ---------------------------------------------------------------- K-04
#[test]
fn k04_vesting_kilidi_tip9_ile_asilamaz() {
    let (mut node, gid) = yeni_node(NET);
    let sk = SigningKey::from_bytes(&[0x42u8; 32]);
    let kurucu = public_key_to_adres(&sk.verifying_key().to_bytes());
    let kilitli = 2_730_000 * E18;
    let serbest = 1_000 * E18;
    node.test_bakiye_ekle(kurucu, kilitli + serbest);
    node.vesting_ekle(kurucu, tam_kilit(kilitli));
    node.lsc_test_bakiye_ekle(kurucu, E18 / 10);
    let dis = [0x77u8; 20];

    // Kilitli miktarin tamami: on kontrol reddeder, hicbir sey degismez.
    let v1 = gonder(
        &mut node,
        gid,
        crate::tx::AvmCagri::new(dis, kilitli, 0, vec![0x00]).encode(),
        &sk,
    );
    assert_eq!(node.bakiye(&dis), 0);
    assert_eq!(node.bakiye(&kurucu), kilitli + serbest);

    // Serbest kisim gonderilebilir; kilit sonrasinda da yerinde kalir.
    gonder(
        &mut node,
        v1,
        crate::tx::AvmCagri::new(dis, serbest, 0, vec![0x00]).encode(),
        &sk,
    );
    assert_eq!(node.bakiye(&dis), serbest);
    assert_eq!(
        node.bakiye(&kurucu),
        kilitli,
        "kilitli kisim hesapta kalmali"
    );
}

#[test]
fn k04_vesting_kilidi_tip12_ile_asilamaz_ve_arz_korunur() {
    let (mut node, gid) = yeni_node(NET);
    let a = eth_adres();
    let toplam = 1_000 * E18;
    node.test_bakiye_ekle(a, toplam);
    node.vesting_ekle(a, tam_kilit(toplam));
    node.lsc_test_bakiye_ekle(a, E18 / 10);
    let dis = [0x7Bu8; 20];
    let chain = crate::avm::evm_chain_id(NET);
    eth_gonder(&mut node, gid, &raw_eth(Some(chain), 0, dis, 400 * E18));
    assert_eq!(node.bakiye(&dis), 0, "kilitli AIDAG tip=12 ile tasinamaz");
    assert_eq!(node.bakiye(&a), toplam);
    assert_eq!(node.bakiye(&a) + node.bakiye(&dis), toplam, "arz korunmali");
}

// ---------------------------------------------------------------- K-05
#[test]
fn k05_makbuz_gercek_sonucu_yansitir() {
    let (mut node, gid) = yeni_node(NET);
    let chain = crate::avm::evm_chain_id(NET);
    let a = eth_adres();
    let dis = [0x55u8; 20];
    // Bilinmeyen hash -> makbuz YOK (RPC null doner).
    assert!(node.evm_makbuz(&[0xde; 32]).is_none());

    // Bakiyesiz gonderici: DAG'a girse bile makbuz "uygulanmadi", status 0.
    let raw0 = raw_eth(Some(chain), 0, dis, E18);
    let v1 = eth_gonder(&mut node, gid, &raw0);
    let m0 = node
        .evm_makbuz(&crate::avm::eth_tx_hash(&raw0))
        .expect("makbuz kaydi");
    assert!(
        matches!(m0.durum, EvmMakbuzDurum::Uygulanmadi(_)),
        "{:?}",
        m0.durum
    );
    assert_eq!(node.bakiye(&dis), 0);

    // Bakiye + gas verilince ayni nonce ile yeni tx basarili olur.
    node.test_bakiye_ekle(a, 5 * E18);
    node.lsc_test_bakiye_ekle(a, E18 / 10);
    let raw1 = raw_eth(Some(chain), 0, dis, E18);
    eth_gonder(&mut node, v1, &raw1);
    let m1 = node
        .evm_makbuz(&crate::avm::eth_tx_hash(&raw1))
        .expect("makbuz");
    assert!(
        matches!(m1.durum, EvmMakbuzDurum::Basarili { .. }),
        "{:?}",
        m1.durum
    );
    assert_eq!(m1.gonderen, a);
    assert_eq!(m1.hedef, Some(dis));
    assert_eq!(node.bakiye(&dis), E18);
}

// ---------------------------------------------------------------- K-06
#[test]
fn k06_chain_id_zorunlu() {
    let dis = [0x66u8; 20];
    let mainnet_chain = crate::avm::evm_chain_id(crate::mainnet::MAINNET_NETWORK_ID);
    assert_eq!(mainnet_chain, 3474, "mainnet MetaMask chain id degismemeli");
    assert_ne!(
        crate::avm::evm_chain_id(1),
        mainnet_chain,
        "testnet mainnet'ten farkli"
    );
    assert_ne!(
        crate::avm::evm_chain_id(1),
        1,
        "testnet Ethereum mainnet ile cakismamali"
    );

    // Ethereum (chain 1) imzali ve chain id'siz legacy tx reddedilir; dogru chain kabul.
    assert!(crate::avm::ham_eth_tx_coz(&raw_eth(Some(1), 0, dis, 1), mainnet_chain).is_err());
    assert!(crate::avm::ham_eth_tx_coz(&raw_eth(None, 0, dis, 1), mainnet_chain).is_err());
    assert!(
        crate::avm::ham_eth_tx_coz(&raw_eth(Some(mainnet_chain), 0, dis, 1), mainnet_chain).is_ok()
    );

    // Durum seviyesinde: baska zincire imzali tx DAG'da olsa da uygulanmaz.
    let (mut node, gid) = yeni_node(NET);
    let a = eth_adres();
    node.test_bakiye_ekle(a, 1_000 * E18);
    node.lsc_test_bakiye_ekle(a, E18 / 10);
    eth_gonder(&mut node, gid, &raw_eth(Some(1), 0, dis, 400 * E18));
    assert_eq!(node.bakiye(&dis), 0);
    assert_eq!(node.beklenen_nonce(&a), 0, "nonce tuketilmemeli");
}

// ---------------------------------------------------------------- genesis vesting tarihi
/// KARARLAR.md K-16: genesis vesting baslangici = TGE_BELIRSIZ (2100).
mod vesting_tarihi {
    use crate::dag::vertex::Vertex;
    use crate::dag::wire;
    use crate::node::{NetworkIngestOutcome, NodeState};
    use crate::registry::{public_key_to_adres, VestingKaydi};
    use crate::VertexId;
    use ed25519_dalek::SigningKey;

    const NET: u32 = 1;
    const E18: u128 = 1_000_000_000_000_000_000;

    const ESKI_VESTING_BASLANGIC: u64 = 1_790_467_200;
    /// Kontrol anlari: eski baslangic ve sonrasi, 2100'e kadar.
    const ANLAR: [u64; 8] = [
        ESKI_VESTING_BASLANGIC,
        ESKI_VESTING_BASLANGIC + 1,
        1_790_553_600, // 2026-09-28
        1_806_019_200, // 2027-03-26 (eski 6 ay cliff sonu)
        1_822_003_200, // 2027-09-27
        1_853_625_600, // 2028-09-27
        1_893_456_000, // 2030-01-01
        4_102_444_799, // 2099-12-31 23:59:59
    ];

    fn yeni_node(now: u64) -> (NodeState, VertexId) {
        let mut node = NodeState::new_devnet(NET);
        let sk = SigningKey::from_bytes(&[1u8; 32]);
        let g = Vertex::new_signed(NET, vec![], vec![1, 1], now, &sk).unwrap();
        let gid = *g.id();
        node.ingest_networked(&wire::encode(&g), now);
        (node, gid)
    }

    fn gonder(
        node: &mut NodeState,
        parent: VertexId,
        payload: Vec<u8>,
        sk: &SigningKey,
        now: u64,
    ) -> VertexId {
        let v = Vertex::new_signed(NET, vec![parent], payload, now, sk).unwrap();
        let id = *v.id();
        let r = node.ingest_networked(&wire::encode(&v), now);
        assert!(matches!(r, NetworkIngestOutcome::Integrated(_)), "{r:?}");
        id
    }

    /// Mainnet kurucu dilimi takvimi, GERCEK sabitle (MAINNET_VESTING_BASLANGIC).
    fn kurucu_takvimi(toplam: u128) -> VestingKaydi {
        let (cliff, sure) = crate::genesis::dilim_vesting(4).unwrap();
        VestingKaydi {
            toplam,
            baslangic: crate::mainnet::MAINNET_VESTING_BASLANGIC,
            cliff_sure: cliff,
            toplam_sure: sure,
            tge_acik: 0,
        }
    }

    // ------------------------------------------------ genesis ve tarih
    #[test]
    fn genesis_id_degismedi() {
        let id = crate::mainnet::genesis_id();
        let hex: String = id.iter().map(|b| format!("{b:02x}")).collect();
        assert!(hex.starts_with("b82345008ae109d8"), "genesis id: {hex}");
        let v = wire::decode(&crate::mainnet::genesis_wire()).unwrap();
        assert_eq!(*v.id(), id, "pinli genesis baytlari ayni id'yi uretmeli");
        let mut n = NodeState::new_mainnet();
        let gid = n
            .ingest(&crate::mainnet::genesis_wire(), 1_785_024_000)
            .unwrap();
        assert_eq!(gid, id);
    }

    #[test]
    fn vesting_baslangici_tge_belirsiz_ile_ayni() {
        assert_eq!(
            crate::mainnet::MAINNET_VESTING_BASLANGIC,
            crate::mainnet::TGE_BELIRSIZ
        );
        assert_eq!(crate::mainnet::MAINNET_VESTING_BASLANGIC, 4_102_444_800);
        // On satis TGE'si zincirde hic ayarlanmamissa da ayni tarih kullanilir.
        assert_eq!(
            NodeState::new_mainnet().on_satis_tge(),
            crate::mainnet::TGE_BELIRSIZ
        );
    }

    #[test]
    fn eski_tarihten_2100e_kadar_hicbir_genesis_dilimi_acilmaz() {
        let n = NodeState::new_mainnet();
        let dagitim = crate::genesis::GenesisDagitim::planla(crate::mainnet::dagitim_adresleri());
        for (idx, (adres, miktar)) in dagitim.dilimler().iter().enumerate() {
            assert_eq!(n.bakiye(adres), *miktar, "dilim {idx} genesis bakiyesi");
            for t in ANLAR {
                let kilitli = n.vesting_kilitli(adres, t);
                match crate::genesis::dilim_vesting(idx) {
                    // Vestingli dilimler (ekosistem, likidite, topluluk, kurucu, destekci):
                    // 2100'den once TAMAMI kilitli.
                    Some(_) => assert_eq!(kilitli, *miktar, "dilim {idx} t={t} acildi"),
                    // Hazine (1) ve on satis emaneti (6): tasarim geregi kilitsiz (degismedi).
                    None => assert_eq!(kilitli, 0, "dilim {idx} kilitsiz olmali"),
                }
            }
        }
    }

    #[test]
    fn eski_tarihten_sonra_kilitli_dilim_ne_transferle_ne_avm_ile_tasinamaz() {
        let simdi = 1_790_553_600; // 2026-09-28: eski takvimde acilma baslamisti
        let (mut node, gid) = yeni_node(simdi);
        let sk = SigningKey::from_bytes(&[0x42u8; 32]);
        let sahip = public_key_to_adres(&sk.verifying_key().to_bytes());
        let toplam = 2_730_000 * E18;
        node.test_bakiye_ekle(sahip, toplam);
        node.vesting_ekle(sahip, kurucu_takvimi(toplam));
        node.lsc_test_bakiye_ekle(sahip, E18 / 10);
        assert_eq!(node.vesting_kilitli(&sahip, simdi), toplam);
        let dis = [0x77u8; 20];
        // tip=4 yerel transfer
        let v1 = gonder(
            &mut node,
            gid,
            crate::tx::TransferKaydi::new(dis, 1, 0).encode(),
            &sk,
            simdi,
        );
        // tip=9 AVM (K-04)
        gonder(
            &mut node,
            v1,
            crate::tx::AvmCagri::new(dis, 1, 0, vec![0x00]).encode(),
            &sk,
            simdi,
        );
        assert_eq!(node.bakiye(&dis), 0);
        assert_eq!(node.bakiye(&sahip), toplam);
    }
}

// ---------------------------------------------------------------- RWA ana agda kapali
/// DENETIM (2026-09-25, main birlestirmesi): RWA modulleri (tip=17..20 + precompile)
/// mainnet'te KAPALI. Ayni senaryo devnet'te calisir (kontrol), mainnet'te -- imzalar
/// dogru ag kimligiyle (3474) atilmis ve yetkiler kurulmus olsa bile -- hicbir RWA
/// durumu olusmaz. Boylece "kapali" sonucu yanlis imza gibi baska bir nedene dayanmaz.
mod rwa_ana_ag {
    use crate::dag::vertex::Vertex;
    use crate::dag::wire;
    use crate::node::{NetworkIngestOutcome, NodeState};
    use crate::registry::public_key_to_adres;
    use crate::tx::{
        KurumKaydiTx, KurumYetki, KycKayit, OracleAkisTanim, OracleRapor, YonetimEylemi,
        YonetimIslemi, ROL_KYC_ONAYLAYICI, ROL_ORACLE_RAPORLAYICI,
    };
    use crate::VertexId;
    use ed25519_dalek::{Signer, SigningKey};

    const T0: u64 = 1_800_000_000;
    const AKIS: u32 = 1;
    const MUSTERI: [u8; 20] = [9; 20];

    fn sk(n: u8) -> SigningKey {
        SigningKey::from_bytes(&[n; 32])
    }
    fn adr(k: &SigningKey) -> [u8; 20] {
        public_key_to_adres(&k.verifying_key().to_bytes())
    }

    struct Ag {
        node: NodeState,
        net: u32,
        son: VertexId,
        t: u64,
        n: u32,
    }

    impl Ag {
        fn yeni(mainnet: bool) -> Self {
            if mainnet {
                let mut node = NodeState::new_mainnet();
                let gid = node.ingest(&crate::mainnet::genesis_wire(), T0).unwrap();
                Ag {
                    node,
                    net: crate::mainnet::MAINNET_NETWORK_ID,
                    son: gid,
                    t: T0,
                    n: 0,
                }
            } else {
                let mut node = NodeState::new_devnet(1);
                let g = Vertex::new_signed(1, vec![], vec![1, 1], T0, &sk(1)).unwrap();
                let gid = *g.id();
                node.ingest_networked(&wire::encode(&g), T0);
                Ag {
                    node,
                    net: 1,
                    son: gid,
                    t: T0,
                    n: 0,
                }
            }
        }
        fn gonder(&mut self, k: &SigningKey, payload: Vec<u8>) {
            let v = Vertex::new_signed(self.net, vec![self.son], payload, self.t, k).unwrap();
            let r = self.node.ingest_networked(&wire::encode(&v), self.t);
            assert!(matches!(r, NetworkIngestOutcome::Integrated(_)), "{r:?}");
            self.son = *v.id();
        }
        fn ilerle(&mut self, sn: u64) {
            self.t += sn;
            self.n += 1;
            let p = [b"dolgu".to_vec(), self.n.to_be_bytes().to_vec()].concat();
            self.gonder(&sk(0x5A), p);
        }
        /// 2-of-3 yonetim imzasiyla rol ver (imza mesaji BU AGIN kimligiyle).
        fn rol_ver(&mut self, ys: &[SigningKey; 3], kurum: [u8; 20], rol: u8, kapsam: u32) {
            let mut y = YonetimIslemi {
                nonce: self.node.rwa_yonetim().unwrap().nonce(),
                son_gecerlilik: self.t + 86_400,
                eylem: YonetimEylemi::Rol(KurumYetki::new(kurum, rol, kapsam, true)),
                imzalar: vec![],
            };
            let m = y.imza_mesaji(self.net);
            y.imzalar = ys[..2]
                .iter()
                .map(|k| (k.verifying_key().to_bytes(), k.sign(&m).to_bytes()))
                .collect();
            self.gonder(&sk(0x77), y.encode());
        }
    }

    /// Tam RWA senaryosu: sahip akis tanimlar, yonetim (2-of-3) rolleri verir, bildirim
    /// suresi beklenir, oracle kurumu rapor yazar, KYC kurumu musteri onaylar.
    /// Doner: (akis tanimli, oracle verisi var, KYC onayli, verilen rol sayisi, precompile yaniti)
    fn senaryo(mainnet: bool) -> (bool, bool, bool, usize, Vec<u8>) {
        let mut a = Ag::yeni(mainnet);
        let sahip = sk(0x91);
        a.node.faucet_owner_ayarla(adr(&sahip));
        let ys = [sk(0xE1), sk(0xE2), sk(0xE3)];
        let pks: Vec<[u8; 32]> = ys.iter().map(|k| k.verifying_key().to_bytes()).collect();
        a.node.rwa_yonetim_kur(&pks, 2).expect("2-of-3");
        let tanim = OracleAkisTanim {
            akis_no: AKIS,
            ondalik: 8,
            esik_m: 1,
            sapma_bps: 200,
            kesici_bps: 1_000,
            bayat_sn: 3_600,
            aciklama: "XAU/USD".into(),
        };
        a.gonder(&sahip, tanim.encode());
        let (oracle_k, kyc_k) = (sk(0x21), sk(0x22));
        a.gonder(&oracle_k, KurumKaydiTx::new(1, "Rafineri".into()).encode());
        a.gonder(&kyc_k, KurumKaydiTx::new(1, "Banka".into()).encode());
        a.rol_ver(&ys, adr(&oracle_k), ROL_ORACLE_RAPORLAYICI, AKIS);
        a.rol_ver(&ys, adr(&kyc_k), ROL_KYC_ONAYLAYICI, 0);
        a.ilerle(crate::mainnet::RWA_ROL_BILDIRIM_SURESI);
        let rapor = OracleRapor {
            akis_no: AKIS,
            tur_no: 1,
            deger: 2_500,
            olcum_zamani: a.t,
            veri_hash: [7; 32],
        };
        a.gonder(&oracle_k, rapor.encode());
        a.gonder(
            &kyc_k,
            KycKayit {
                adres: MUSTERI,
                onay: true,
                kanit_hash: [0; 32],
            }
            .encode(),
        );
        let precompile = a
            .node
            .avm_call(
                &[0; 20],
                &crate::rwa_precompile::oracle_adresi(AKIS),
                &crate::rwa_precompile::SEC_LATEST_ROUND_DATA,
            )
            .unwrap_or_default();
        (
            a.node.oracle_akis(AKIS).is_some(),
            a.node.oracle_son_veri(AKIS).is_ok(),
            a.node.kyc_onayli_mi(&MUSTERI),
            a.node.kurum_rolleri(&adr(&oracle_k)).len() + a.node.kurum_rolleri(&adr(&kyc_k)).len(),
            precompile,
        )
    }

    #[test]
    fn rwa_aktivasyon_karari_verilmedi() {
        assert_eq!(crate::mainnet::RWA_MAINNET_AKTIVASYON, None);
    }

    #[test]
    fn ayni_rwa_senaryosu_devnette_calisir_mainnette_etkisiz() {
        // Kontrol: devnet'te senaryo gercekten RWA durumu olusturuyor.
        let (akis, oracle, kyc, roller, pc) = senaryo(false);
        assert!(
            akis && oracle && kyc,
            "devnet: akis={akis} oracle={oracle} kyc={kyc}"
        );
        assert_eq!(roller, 2);
        assert_eq!(pc.len(), 160, "devnet: oracle precompile veri dondurur");
        // Mainnet: ayni senaryo, dogru ag imzasiyla -> hicbir RWA durumu yok.
        let (akis, oracle, kyc, roller, pc) = senaryo(true);
        assert!(!akis, "mainnet: oracle akisi tanimlanmamali");
        assert!(!oracle, "mainnet: oracle verisi olmamali");
        assert!(!kyc, "mainnet: KYC onayi olmamali");
        assert_eq!(roller, 0, "mainnet: kurum rolu verilmemeli");
        assert!(
            pc.is_empty(),
            "mainnet: precompile adresi bos hesap gibi davranmali"
        );
    }
}
