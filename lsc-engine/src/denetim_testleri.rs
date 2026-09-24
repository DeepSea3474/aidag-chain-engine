//! DENETIM (2026-09-24) REGRESYON TESTLERI — K-04 + genesis vesting tarihi.
//! Bulgu kimlikleri DENETIM_ONCESI_RAPOR.md ve KARARLAR.md ile ayni.

use crate::dag::vertex::Vertex;
use crate::dag::wire;
use crate::node::{NetworkIngestOutcome, NodeState};
use crate::registry::{public_key_to_adres, VestingKaydi};
use crate::VertexId;
use ed25519_dalek::SigningKey;

const NET: u32 = 1;
const E18: u128 = 1_000_000_000_000_000_000;
const ETH_SK: &str = "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318";
/// Eski genesis vesting baslangici (2026-09-27 00:00 UTC).
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

fn raw_eth(nonce: u64, to: [u8; 20], value: u128) -> Vec<u8> {
    use alloy_consensus::{SignableTransaction, TxLegacy};
    use alloy_eips::eip2718::Encodable2718;
    use alloy_primitives::{Signature, TxKind as ATxKind, U256 as AU256};
    use alloy_signer::SignerSync;
    use alloy_signer_local::PrivateKeySigner;
    let eth: PrivateKeySigner = ETH_SK.parse().unwrap();
    let tx = TxLegacy {
        chain_id: Some(3474),
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

// ------------------------------------------------ K-04
fn tam_kilit(toplam: u128, now: u64) -> VestingKaydi {
    let (cliff, sure) = crate::genesis::dilim_vesting(4).unwrap();
    VestingKaydi {
        toplam,
        baslangic: now + 365 * 86400,
        cliff_sure: cliff,
        toplam_sure: sure,
        tge_acik: 0,
    }
}

#[test]
fn k04_vesting_kilidi_tip9_ile_asilamaz() {
    let now = crate::mainnet::ON_SATIS_BASLANGIC;
    let (mut node, gid) = yeni_node(now);
    let sk = SigningKey::from_bytes(&[0x42u8; 32]);
    let kurucu = public_key_to_adres(&sk.verifying_key().to_bytes());
    let kilitli = 2_730_000 * E18;
    let serbest = 1_000 * E18;
    node.test_bakiye_ekle(kurucu, kilitli + serbest);
    node.vesting_ekle(kurucu, tam_kilit(kilitli, now));
    node.lsc_test_bakiye_ekle(kurucu, E18 / 10);
    let dis = [0x77u8; 20];
    let v1 = gonder(
        &mut node,
        gid,
        crate::tx::AvmCagri::new(dis, kilitli, 0, vec![0x00]).encode(),
        &sk,
        now,
    );
    assert_eq!(node.bakiye(&dis), 0);
    assert_eq!(node.bakiye(&kurucu), kilitli + serbest);
    gonder(
        &mut node,
        v1,
        crate::tx::AvmCagri::new(dis, serbest, 0, vec![0x00]).encode(),
        &sk,
        now,
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
    use alloy_signer_local::PrivateKeySigner;
    let now = crate::mainnet::ON_SATIS_BASLANGIC;
    let (mut node, gid) = yeni_node(now);
    let eth: PrivateKeySigner = ETH_SK.parse().unwrap();
    let a = crate::avm::evm_to_adres(&eth.address());
    let toplam = 1_000 * E18;
    node.test_bakiye_ekle(a, toplam);
    node.vesting_ekle(a, tam_kilit(toplam, now));
    node.lsc_test_bakiye_ekle(a, E18 / 10);
    let dis = [0x7Bu8; 20];
    let tasiyici = SigningKey::from_bytes(&[0x99u8; 32]);
    let raw = raw_eth(0, dis, 400 * E18);
    gonder(
        &mut node,
        gid,
        crate::tx::ham_eth_tx_payload(&raw),
        &tasiyici,
        now,
    );
    assert_eq!(node.bakiye(&dis), 0, "kilitli AIDAG tip=12 ile tasinamaz");
    assert_eq!(node.bakiye(&a) + node.bakiye(&dis), toplam, "arz korunmali");
}
