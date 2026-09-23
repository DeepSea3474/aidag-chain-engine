//! DENETIM REGRESYON TESTLERI (2026-09-24 ic denetim).
//! Her test, denetimde KANITLANMIS bir saldiriyi yeniden dener ve artik
//! BASARISIZ oldugunu gosterir. Saldiri senaryosu her testin basinda yazili.

use super::*;
use crate::dag::wire;
use crate::registry::{public_key_to_adres, VestingKaydi};
use ed25519_dalek::SigningKey;

const NET: u32 = 7;

fn genesis(node: &mut NodeState, ts: u64) -> VertexId {
    let sk = SigningKey::from_bytes(&[1u8; 32]);
    let v = Vertex::new_signed(NET, vec![], vec![1, 1], ts, &sk).unwrap();
    let id = *v.id();
    node.ingest_networked(&wire::encode(&v), ts);
    id
}

/// Vertex'i imzala + ag yolundan ingest et; (id, sonuc) doner.
fn gonder(node: &mut NodeState, sk: &SigningKey, parents: Vec<VertexId>, payload: Vec<u8>, ts: u64) -> (VertexId, NetworkIngestOutcome) {
    let v = Vertex::new_signed(node.graph.network_id(), parents, payload, ts, sk).unwrap();
    let id = *v.id();
    let s = node.ingest_networked(&wire::encode(&v), ts);
    (id, s)
}

fn kasa_kodu() -> Vec<u8> {
    let bin_hex = include_str!("../../avm-sozlesmeler/Kasa.bin").trim();
    (0..bin_hex.len()).step_by(2).map(|i| u8::from_str_radix(&bin_hex[i..i + 2], 16).unwrap()).collect()
}

// ---------------------------------------------------------------------------
// ham ETH (tip=12) yardimcisi
// ---------------------------------------------------------------------------
fn eth_imzali(chain_id: Option<u64>, nonce: u64, to: [u8; 20], value: u128, yuksek_s: bool) -> Vec<u8> {
    use alloy_consensus::{SignableTransaction, TxLegacy};
    use alloy_eips::eip2718::Encodable2718;
    use alloy_primitives::{Address, Signature, TxKind, U256};
    use alloy_signer::SignerSync;
    use alloy_signer_local::PrivateKeySigner;
    let eth: PrivateKeySigner =
        "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318".parse().unwrap();
    let tx = TxLegacy {
        chain_id,
        nonce,
        gas_price: 0,
        gas_limit: 3_000_000,
        to: TxKind::Call(Address::from(to)),
        value: U256::from(value),
        input: Default::default(),
    };
    let mut imza: Signature = eth.sign_hash_sync(&tx.signature_hash()).unwrap();
    if yuksek_s {
        // Ayni gonderene cozulen "esnek" imza: s' = n - s, parite ters.
        let n = U256::from_be_slice(&[
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE,
            0xBA, 0xAE, 0xDC, 0xE6, 0xAF, 0x48, 0xA0, 0x3B, 0xBF, 0xD2, 0x5E, 0x8C, 0xD0, 0x36, 0x41, 0x41,
        ]);
        imza = Signature::new(imza.r(), n - imza.s(), !imza.v());
    }
    let zarf: alloy_consensus::TxEnvelope = tx.into_signed(imza).into();
    zarf.encoded_2718()
}
fn eth_gonderen() -> [u8; 20] {
    use alloy_signer_local::PrivateKeySigner;
    let eth: PrivateKeySigner =
        "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318".parse().unwrap();
    crate::avm::evm_to_adres(&eth.address())
}

// ===========================================================================
// 1) KRITIK: gecersiz AVM tx tum kontratlari/storage'i SILIYORDU.
// Saldiri: tip=9 ile 50 KB initcode (EIP-3860 siniri 49152) -> revm dogrulama
// hatasi -> bos db yerinde kalirdi. Beklenen: kontrat durur, nonce ilerler,
// taban gas kesilir (bedava tekrar yok).
// ===========================================================================
#[test]
fn d1_gecersiz_avm_tx_kontratlari_silmez() {
    use crate::tx::AvmCagri;
    let now = crate::mainnet::ON_SATIS_BASLANGIC;
    let mut node = NodeState::new_devnet(NET);
    let g = genesis(&mut node, now);
    let sk = SigningKey::from_bytes(&[0x42; 32]);
    let a = public_key_to_adres(&sk.verifying_key().to_bytes());
    node.lsc_test_bakiye_ekle(a, 100_000_000_000_000_000);
    let (v1, _) = gonder(&mut node, &sk, vec![g], AvmCagri::new([0; 20], 0, 0, kasa_kodu()).encode(), now);
    assert_eq!(node.avm_kontrat_adresleri().len(), 1);
    let lsc_once = node.lsc_bakiye(&a);
    let (_v2, _) = gonder(&mut node, &sk, vec![v1], AvmCagri::new([0; 20], 0, 1, vec![0u8; 50_000]).encode(), now + 1);
    assert_eq!(node.avm_kontrat_adresleri().len(), 1, "gecersiz tx kontrati SILMEMELI");
    assert_eq!(node.beklenen_nonce(&a), 2, "gecersiz tx nonce'u ilerletir (tekrar oynatilamaz)");
    assert!(node.lsc_bakiye(&a) < lsc_once, "gecersiz tx bedava degil (taban gas kesildi)");
}

// ===========================================================================
// 2) KRITIK: EVM yolu (tip=12 MetaMask) vesting kilidini atliyordu.
// Saldiri: kilitli AIDAG'in tamamini tek eth_sendRawTransaction ile tasimak.
// ===========================================================================
#[test]
fn d2_evm_yolu_vesting_kilidini_asamaz() {
    let now = crate::mainnet::ON_SATIS_BASLANGIC;
    let mut node = NodeState::new_devnet(NET);
    let g = genesis(&mut node, now);
    let gonderen = eth_gonderen();
    let alici = [0x22; 20];
    node.lsc_test_bakiye_ekle(gonderen, 100_000_000_000_000_000);
    node.test_bakiye_ekle(gonderen, 1_000_500);
    node.vesting_ekle(gonderen, VestingKaydi { toplam: 1_000_000, baslangic: now + 10_000_000, cliff_sure: 0, toplam_sure: 1, tge_acik: 0 });
    let chain = crate::avm::evm_chain_id(NET);
    let tasiyici = SigningKey::from_bytes(&[0x88; 32]);
    // kilitli miktari gondermeye calis
    let (v1, _) = gonder(&mut node, &tasiyici, vec![g], crate::tx::ham_eth_tx_payload(&eth_imzali(Some(chain), 0, alici, 1_000_000, false)), now);
    assert_eq!(node.bakiye(&alici), 0, "kilitli AIDAG EVM yoluyla tasinamaz");
    assert_eq!(node.bakiye(&gonderen), 1_000_500);
    // yalniz harcanabilir kisim (500) gonderilebilir
    let n = node.beklenen_nonce(&gonderen);
    let (_v2, _) = gonder(&mut node, &tasiyici, vec![v1], crate::tx::ham_eth_tx_payload(&eth_imzali(Some(chain), n, alici, 500, false)), now + 1);
    assert_eq!(node.bakiye(&alici), 500, "serbest kisim gonderilebilir");
    assert_eq!(node.bakiye(&gonderen), 1_000_000, "kilitli kisim yerinde");
}

// ===========================================================================
// 3) KRITIK/YUKSEK: tip=12 chainId kontrolu yoktu -> BSC/Ethereum tx replay.
// Saldiri: kurbanin BSC (56), Ethereum (1), chainId'siz veya mainnet (3474)
// imzasini bu agda yeniden oynatmak. Ek: yuksek-S esnek imza.
// ===========================================================================
#[test]
fn d3_tip12_yabanci_chainid_ve_yuksek_s_reddedilir() {
    let now = crate::mainnet::ON_SATIS_BASLANGIC;
    let mut node = NodeState::new_devnet(NET);
    let mut ebeveyn = genesis(&mut node, now);
    let gonderen = eth_gonderen();
    let alici = [0x33; 20];
    node.lsc_test_bakiye_ekle(gonderen, 100_000_000_000_000_000);
    node.test_bakiye_ekle(gonderen, 1_000);
    let tasiyici = SigningKey::from_bytes(&[0x88; 32]);
    for (i, chain) in [Some(56u64), Some(1), None, Some(3474)].into_iter().enumerate() {
        let (id, _) = gonder(&mut node, &tasiyici, vec![ebeveyn], crate::tx::ham_eth_tx_payload(&eth_imzali(chain, 0, alici, 100, false)), now + i as u64);
        ebeveyn = id;
        assert_eq!(node.bakiye(&alici), 0, "chainId {chain:?} bu agda gecersiz olmali");
        assert_eq!(node.beklenen_nonce(&gonderen), 0, "yabanci tx nonce tuketemez");
    }
    // yuksek-S (dogru chainId ile bile) reddedilir
    let dogru = crate::avm::evm_chain_id(NET);
    let raw_s = eth_imzali(Some(dogru), 0, alici, 100, true);
    assert!(!crate::avm::ham_eth_tx_coz(&raw_s).unwrap().s_dusuk);
    let (id, _) = gonder(&mut node, &tasiyici, vec![ebeveyn], crate::tx::ham_eth_tx_payload(&raw_s), now + 10);
    assert_eq!(node.bakiye(&alici), 0, "yuksek-S imza reddedilir");
    // dogru chainId + dusuk-S -> gecerli
    let (_id, _) = gonder(&mut node, &tasiyici, vec![id], crate::tx::ham_eth_tx_payload(&eth_imzali(Some(dogru), 0, alici, 100, false)), now + 11);
    assert_eq!(node.bakiye(&alici), 100, "kendi chainId'siyle gecerli");
}

// ===========================================================================
// 4) ORTA: EVM CHAINID opcode'u 1 donduruyordu (Ethereum mainnet).
// ===========================================================================
#[test]
fn d4_chainid_opcode_ag_kimligi() {
    use revm::bytecode::Bytecode;
    let mut db = crate::avm::AidagDatabase::yeni();
    let adres = [0x77u8; 20];
    // CHAINID PUSH0 MSTORE PUSH1 0x20 PUSH0 RETURN
    db.kod_koy(adres, Bytecode::new_raw(vec![0x46, 0x5f, 0x52, 0x60, 0x20, 0x5f, 0xf3].into()));
    let oku = |c: u64| {
        let out = crate::avm::avm_call_oku_zincir(&db, &[0; 20], &adres, &[], c).unwrap();
        u64::from_be_bytes(out[24..32].try_into().unwrap())
    };
    assert_eq!(oku(3474), 3474);
    assert_eq!(oku(crate::avm::evm_chain_id(1)), 3_474_000_001);
    assert_ne!(crate::avm::evm_chain_id(1), 1, "testnet chainId Ethereum (1) ile cakismamali");
}

// ===========================================================================
// 5) YUKSEK: tip=11 imzasi ag kimligi icermiyordu -> testnet imzasi mainnet'te gecerli.
// ===========================================================================
#[test]
fn d5_tip11_imzasi_baska_agda_gecersiz() {
    use crate::tx::{evm_transfer_mesaji, EvmTransfer};
    use k256::ecdsa::{signature::hazmat::PrehashSigner, RecoveryId, Signature as KS, SigningKey as KSk};
    use sha3::{Digest, Keccak256};
    let sk = KSk::from_slice(&[7u8; 32]).unwrap();
    let alici = [0x99; 20];
    let mesaj = evm_transfer_mesaji(crate::avm::evm_chain_id(1), &alici, 500, 0);
    let (sig, recid): (KS, RecoveryId) = sk.sign_prehash(&Keccak256::digest(&mesaj)).unwrap();
    let t = EvmTransfer { alici, miktar: 500, nonce: 0, recovery_id: recid.to_byte(), imza: sig.to_bytes().into() };
    let testnet = t.gonderen_adres(crate::avm::evm_chain_id(1)).unwrap();
    let mainnet = t.gonderen_adres(3474);
    assert_ne!(mainnet, Some(testnet), "testnet imzasi mainnet'te AYNI gondereni vermemeli");
}

// ===========================================================================
// 6) YUKSEK: stake bedava ve baskasi adina yapilabiliyordu (token gaspi + slash).
// ===========================================================================
#[test]
fn d6_stake_bedava_ve_baskasi_adina_olmaz() {
    use crate::tx::StakeKaydi;
    let now = crate::mainnet::ON_SATIS_BASLANGIC;
    let mut node = NodeState::new_devnet(NET);
    let g = genesis(&mut node, now);
    let saldirgan = SigningKey::from_bytes(&[0x51; 32]);
    let s_adr = public_key_to_adres(&saldirgan.verifying_key().to_bytes());
    let kurban = [0xAB; 20];
    // bakiyesiz, kendi adina: RED
    let (v1, _) = gonder(&mut node, &saldirgan, vec![g], StakeKaydi::new(s_adr, 1_000_000).encode(), now);
    assert_eq!(node.stake_miktari(&s_adr), 0, "bakiyesiz stake olmaz");
    // baskasi adina: RED (bakiyesi olsa bile)
    node.test_bakiye_ekle(s_adr, 1_000_000);
    let (_v2, _) = gonder(&mut node, &saldirgan, vec![v1], StakeKaydi::new(kurban, 1_000).encode(), now + 1);
    assert_eq!(node.stake_miktari(&kurban), 0, "baskasi adina stake olmaz");
    assert_eq!(node.bakiye(&s_adr), 1_000_000, "reddedilen stake bakiye dusurmez");
}

// ===========================================================================
// 7) ORTA: tip=8 eslestirme imzalayana bagli degildi (odul gaspi).
// ===========================================================================
#[test]
fn d7_eslestirme_yalniz_sahibi_yapar() {
    use crate::tx::EslestirmeKaydi;
    let now = crate::mainnet::ON_SATIS_BASLANGIC;
    let mut node = NodeState::new_devnet(NET);
    let g = genesis(&mut node, now);
    let saldirgan = SigningKey::from_bytes(&[0x61; 32]);
    let kurban_test = [0x5A; 20];
    let (v1, _) = gonder(&mut node, &saldirgan, vec![g], EslestirmeKaydi::new(kurban_test, [0x66; 20]).encode(), now);
    assert_eq!(node.eslesme_sorgula(&kurban_test), None, "baskasinin test adresi eslestirilemez");
    let sahibi = SigningKey::from_bytes(&[0x62; 32]);
    let test_adr = public_key_to_adres(&sahibi.verifying_key().to_bytes());
    let (_v2, _) = gonder(&mut node, &sahibi, vec![v1], EslestirmeKaydi::new(test_adr, [0x67; 20]).encode(), now + 1);
    assert_eq!(node.eslesme_sorgula(&test_adr), Some([0x67; 20]), "sahibi eslestirebilir");
}

// ===========================================================================
// 8) ORTA: on-satis tahsisi escrow'da rezerve degildi (owner bosaltabiliyordu).
// ===========================================================================
#[test]
fn d8_escrow_rezervi_owner_harcayamaz_claim_calisir() {
    use crate::tx::{ClaimTalebi, OnSatisDagitim, TransferKaydi};
    let now = crate::mainnet::ON_SATIS_BASLANGIC;
    let mut node = NodeState::new_devnet(NET);
    let g = genesis(&mut node, now);
    let owner_sk = SigningKey::from_bytes(&[0x71; 32]);
    let owner = public_key_to_adres(&owner_sk.verifying_key().to_bytes());
    node.faucet_owner_ayarla(owner);
    node.test_bakiye_ekle(owner, 1_000);
    let alici_sk = SigningKey::from_bytes(&[0x72; 32]);
    let alici = public_key_to_adres(&alici_sk.verifying_key().to_bytes());
    let (v1, _) = gonder(&mut node, &owner_sk, vec![g], OnSatisDagitim::new(alici, alici, 600, 0, 42).encode(), now);
    assert_eq!(node.harcanabilir(&owner), 400, "600 AIDAG aliciya rezerve");
    // owner rezervi harcamaya calisir
    let (v2, _) = gonder(&mut node, &owner_sk, vec![v1], TransferKaydi::new([0x09; 20], 500, 0).encode(), now + 1);
    assert_eq!(node.bakiye(&[0x09; 20]), 0, "owner rezerve AIDAG'i harcayamaz");
    // serbest kisim harcanabilir
    let (v3, _) = gonder(&mut node, &owner_sk, vec![v2], TransferKaydi::new([0x09; 20], 400, 0).encode(), now + 2);
    assert_eq!(node.bakiye(&[0x09; 20]), 400);
    // TGE sonrasi alici claim eder (rezervden)
    let tge = node.on_satis_tge();
    let (_v4, _) = gonder(&mut node, &alici_sk, vec![v3], ClaimTalebi::new(42).encode(), tge + 1);
    assert_eq!(node.bakiye(&alici), 120, "TGE'de %20 claim rezervden odenir");
}

// ===========================================================================
// 9) KURUCU TGE KARARI: TGE yalniz kurucunun zincir karariyla belirlenir;
// karar yoksa (mainnet) hicbir genesis dilimi acilmaz, tarih kendiliginden
// kesinlesmez. Karar verilince vesting o tarihten baslar.
// ===========================================================================
#[test]
fn d9_mainnet_kurucu_karari_yoksa_tge_belirsiz_ve_genesis_kilitli() {
    let mut node = NodeState::new_mainnet();
    assert!(!node.tge_karari_var_mi());
    assert_eq!(node.on_satis_tge(), u64::MAX, "karar yoksa TGE belirsiz");
    // 27 Eylul'den (eski sabit) cok sonra bile genesis dilimleri kilitli
    let t = crate::mainnet::MAINNET_VESTING_BASLANGIC + 400 * 86_400;
    let gid = node.ingest(&crate::mainnet::genesis_wire(), t).unwrap();
    let sk = SigningKey::from_bytes(&[0x33; 32]);
    let (_id, s) = gonder(&mut node, &sk, vec![gid], crate::tx::Record::new([0x44; 32]).encode(), t);
    assert!(matches!(s, NetworkIngestOutcome::Integrated(_)));
    for (i, a) in crate::mainnet::dagitim_adresleri().iter().enumerate() {
        if crate::genesis::dilim_vesting(i).is_some() {
            assert_eq!(node.harcanabilir(a), 0, "dilim {i}: kurucu TGE karari olmadan acilmaz");
        }
    }
}

#[test]
fn d9b_kurucu_tge_karari_vestingi_baslatir() {
    use crate::tx::TgeAyarla;
    let now = crate::mainnet::ON_SATIS_BASLANGIC;
    let mut node = NodeState::new_devnet(NET);
    let g = genesis(&mut node, now);
    let owner_sk = SigningKey::from_bytes(&[0x81; 32]);
    let owner = public_key_to_adres(&owner_sk.verifying_key().to_bytes());
    node.faucet_owner_ayarla(owner);
    let ekip = [0x55; 20];
    node.test_bakiye_ekle(ekip, 1_000);
    // plan: `now`dan itibaren 100 gunde dogrusal (cliff yok)
    node.vesting_ekle(ekip, VestingKaydi { toplam: 1_000, baslangic: now, cliff_sure: 0, toplam_sure: 100 * 86_400, tge_acik: 0 });
    // kurucu TGE'yi 200 gun sonraya karar verir (bildirim >= 3 gun)
    let tge = now + 200 * 86_400;
    let (v1, _) = gonder(&mut node, &owner_sk, vec![g], TgeAyarla::new(tge).encode(), now);
    assert_eq!(node.on_satis_tge(), tge);
    // plan baslangicindan 150 gun sonra (TGE oncesi): hala tamamen kilitli
    let (v2, _) = gonder(&mut node, &owner_sk, vec![v1], crate::tx::Record::new([1; 32]).encode(), now + 150 * 86_400);
    assert_eq!(node.harcanabilir(&ekip), 0, "TGE kararindan once vesting acilmaz");
    // TGE + 50 gun: yarisi acik
    let (_v3, _) = gonder(&mut node, &owner_sk, vec![v2], crate::tx::Record::new([2; 32]).encode(), tge + 50 * 86_400);
    assert_eq!(node.harcanabilir(&ekip), 500, "vesting TGE kararindan baslar");
}

// ===========================================================================
// 10) YUKSEK: tek owner anahtariyla tek islemde 210M LSC basilabiliyordu.
// ===========================================================================
#[test]
fn d10_odul_gunluk_tavani() {
    use crate::tx::ComputeReward;
    let now = crate::mainnet::ON_SATIS_BASLANGIC;
    let mut node = NodeState::new_devnet(NET);
    let g = genesis(&mut node, now);
    let owner_sk = SigningKey::from_bytes(&[0x91; 32]);
    let owner = public_key_to_adres(&owner_sk.verifying_key().to_bytes());
    node.faucet_owner_ayarla(owner);
    let w = [0x0A; 20];
    let e = crate::genesis::ONDALIK;
    let (v1, _) = gonder(&mut node, &owner_sk, vec![g], ComputeReward::new(w, 210_000_000 * e, 1).encode(), now);
    assert_eq!(node.lsc_bakiye(&w), 0, "tek islemde tum emisyon basilamaz");
    let (v2, _) = gonder(&mut node, &owner_sk, vec![v1], ComputeReward::new(w, 6_000 * e, 2).encode(), now + 1);
    let (v3, _) = gonder(&mut node, &owner_sk, vec![v2], ComputeReward::new(w, 5_000 * e, 3).encode(), now + 2);
    assert_eq!(node.lsc_bakiye(&w), 6_000 * e, "gunluk tavan (10k) asilamaz");
    let (_v4, _) = gonder(&mut node, &owner_sk, vec![v3], ComputeReward::new(w, 5_000 * e, 4).encode(), now + 86_400);
    assert_eq!(node.lsc_bakiye(&w), 11_000 * e, "ertesi gun yeni kota");
}

// ===========================================================================
// 11) YUKSEK: belge/kurum kayit zamani geriye tarihlenebiliyordu.
// Etkinlesmeden sonra kayit zamani = zincir saati (monoton).
// ===========================================================================
#[test]
fn d11_belge_zamani_geriye_tarihlenemez() {
    let akt = crate::mainnet::GUVENLIK_V2_AKTIVASYON;
    let mut node = NodeState::new_devnet(NET);
    let g = genesis(&mut node, akt + 1000);
    let sk = SigningKey::from_bytes(&[0xA1; 32]);
    let (v1, _) = gonder(&mut node, &sk, vec![g], crate::tx::Record::new([0x01; 32]).encode(), akt + 1000);
    // 200 sn geri tarihli (nedensellik payi icinde) kayit
    let (_v2, _) = gonder(&mut node, &sk, vec![v1], crate::tx::Record::new([0x02; 32]).encode(), akt + 800);
    assert_eq!(node.belge_dogrula(&[0x02; 32]).unwrap().zaman, akt + 1000, "kayit zamani zincir saatinden geri gidemez");
}

#[test]
fn d11b_etkinlesme_oncesi_eski_davranis() {
    let t0 = crate::mainnet::ON_SATIS_BASLANGIC;
    let mut node = NodeState::new_devnet(NET);
    let g = genesis(&mut node, t0 + 1000);
    let sk = SigningKey::from_bytes(&[0xA2; 32]);
    let (v1, _) = gonder(&mut node, &sk, vec![g], crate::tx::Record::new([0x03; 32]).encode(), t0 + 1000);
    let (_v2, _) = gonder(&mut node, &sk, vec![v1], crate::tx::Record::new([0x04; 32]).encode(), t0 + 800);
    assert_eq!(node.belge_dogrula(&[0x04; 32]).unwrap().zaman, t0 + 800, "gecmis kayitlar degismez");
}

// ===========================================================================
// 12) YUKSEK: cop/bilinmeyen/asiri buyuk payload DAG'a kalici giriyordu.
// ===========================================================================
#[test]
fn d12_mainnet_yapisal_islem_kurali() {
    use crate::tx::AvmCagri;
    let t = crate::mainnet::ON_SATIS_BASLANGIC;
    let mut node = NodeState::new_mainnet();
    let gid = node.ingest(&crate::mainnet::genesis_wire(), t).unwrap();
    let sk = SigningKey::from_bytes(&[0x33; 32]);
    let vc = node.vertex_count();
    let (_, s) = gonder(&mut node, &sk, vec![gid], vec![0xEE; 1000], t);
    assert!(matches!(s, NetworkIngestOutcome::Rejected(_)), "bilinmeyen tip reddedilir");
    let (_, s) = gonder(&mut node, &sk, vec![gid], vec![1, 2, 3], t);
    assert!(matches!(s, NetworkIngestOutcome::Rejected(_)), "cozulemeyen payload reddedilir");
    let (_, s) = gonder(&mut node, &sk, vec![gid], AvmCagri::new([0; 20], 0, 0, vec![0; 70_000]).encode(), t);
    assert!(matches!(s, NetworkIngestOutcome::Rejected(_)), "asiri buyuk payload reddedilir");
    assert_eq!(node.vertex_count(), vc, "reddedilenler DAG'a girmedi");
    let (_, s) = gonder(&mut node, &sk, vec![gid], crate::tx::Record::new([9; 32]).encode(), t);
    assert!(matches!(s, NetworkIngestOutcome::Integrated(_)), "gecerli islem kabul edilir");
}

// ===========================================================================
// 13) YUKSEK (DoS): toplam_stake u128 tasmasi /status'u kalici cokertiyordu.
// ===========================================================================
#[test]
fn d13_toplam_stake_tasmaz() {
    use crate::tx::StakeKaydi;
    let mut r = crate::registry::StakeRegistry::yeni();
    r.stake_ekle(StakeKaydi::new([1; 20], u128::MAX - 1));
    r.stake_ekle(StakeKaydi::new([2; 20], u128::MAX - 1));
    assert_eq!(r.toplam_stake(), u128::MAX, "tasma panik yerine doyar");
}
