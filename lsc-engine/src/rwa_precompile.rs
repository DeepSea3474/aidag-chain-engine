//! RWA PRECOMPILE (2. asama): AVM icinden SALT-OKUNUR oracle + KYC arayuzu.
//!
//! - Oracle: Chainlink AggregatorV3 uyumlu (`decimals`, `description`, `version`,
//!   `latestRoundData`, `getRoundData(uint80)`). Her akis kendi adresinde:
//!   `oracle_adresi(akis_no)` = 12 sifir bayt || A1 DA 0C 1E || akis_no (4 bayt BE).
//! - KYC: `isApproved(address) -> bool`, tek sabit adres `KYC_ADRESI`.
//! - Ethereum standart precompile'lari 0x01..0x11 (+ 0x100 P256) araligindadir; bu
//!   adresler 0xA1DA.... ile baslayan 8 baytlik ust bolum tasir -> CAKISMA YOK
//!   (test: `ethereum_precompile_adresleriyle_cakismaz`).
//! - SALT OKUNUR: hicbir durum yazilmaz; deger (ETH/AIDAG) gonderen cagri REVERT.
//! - GAZ: her cagri sabit `RWA_PRECOMPILE_GAZ` oder (revert dahil); yetersizse OOG.
//! - ZAMAN: bayatlik ZINCIR saatiyle (`RwaGorunum::zincir_saati`); vertex / blok
//!   zamani DEGIL.
//! - MAINNET'TE KAPALI: node yalniz `rwa_aktif()` iken `Some(RwaGorunum)` verir; `None`
//!   iken bu saglayici Ethereum saglayicisiyla BIREBIR ayni davranir.

use revm::context_interface::ContextTr;
use revm::handler::{EthPrecompiles, PrecompileProvider};
use revm::interpreter::{CallInputs, CallValue, Gas, InstructionResult, InterpreterResult};
use revm::primitives::{Address, AddressSet, Bytes};

/// Her RWA precompile cagrisinin sabit gaz maliyeti (soguk SLOAD 2100 + ek yuk).
pub const RWA_PRECOMPILE_GAZ: u64 = 2_600;
/// Oracle akis adresi oneki (bayt 12..16).
pub const ORACLE_ONEK: [u8; 4] = [0xA1, 0xDA, 0x0C, 0x1E];
/// KYC precompile adresi: 12 sifir || A1 DA 4B 59 || 00 00 00 01.
pub const KYC_ADRESI: [u8; 20] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xA1, 0xDA, 0x4B, 0x59, 0, 0, 0, 1,
];
/// `version()` donusu.
pub const RWA_ORACLE_SURUM: u64 = 1;

/// Fonksiyon seciciler (keccak256(imza)[..4]; test ile dogrulanir).
pub const SEC_DECIMALS: [u8; 4] = [0x31, 0x3c, 0xe5, 0x67]; // decimals()
pub const SEC_DESCRIPTION: [u8; 4] = [0x72, 0x84, 0xe4, 0x16]; // description()
pub const SEC_VERSION: [u8; 4] = [0x54, 0xfd, 0x4d, 0x50]; // version()
pub const SEC_GET_ROUND_DATA: [u8; 4] = [0x9a, 0x6f, 0xc8, 0xf5]; // getRoundData(uint80)
pub const SEC_LATEST_ROUND_DATA: [u8; 4] = [0xfe, 0xaf, 0x96, 0x8c]; // latestRoundData()
pub const SEC_IS_APPROVED: [u8; 4] = [0x67, 0x34, 0x48, 0xdd]; // isApproved(address)

/// Akis numarasindan oracle precompile adresi. akis_no 0 gecersizdir.
pub fn oracle_adresi(akis_no: u32) -> [u8; 20] {
    let mut a = [0u8; 20];
    a[12..16].copy_from_slice(&ORACLE_ONEK);
    a[16..20].copy_from_slice(&akis_no.to_be_bytes());
    a
}

/// Adres bir oracle akis adresi mi? (akis_no >= 1)
pub fn oracle_akis_no(adres: &[u8; 20]) -> Option<u32> {
    if adres[..12] != [0u8; 12] || adres[12..16] != ORACLE_ONEK {
        return None;
    }
    let n = u32::from_be_bytes([adres[16], adres[17], adres[18], adres[19]]);
    (n != 0).then_some(n)
}

/// Adres RWA precompile adresi mi?
pub fn rwa_adresi_mi(adres: &[u8; 20]) -> bool {
    *adres == KYC_ADRESI || oracle_akis_no(adres).is_some()
}

/// Precompile'in okudugu RWA durumu (salt-okunur odunc). Tek islem boyunca sabit.
#[derive(Clone, Copy)]
pub struct RwaGorunum<'a> {
    pub oracle: &'a crate::rwa::OracleRegistry,
    pub kurumlar: &'a crate::registry::KurumRegistry,
    pub kyc: &'a crate::rwa::KycRegistry,
    /// Bayatlik/rol kontrolu icin ZINCIR saati.
    pub zincir_saati: u64,
}

impl RwaGorunum<'_> {
    fn kyc_onayli(&self, adres: &[u8; 20]) -> bool {
        self.kyc.onayli_mi(adres, |k| {
            self.kurumlar
                .rol_aktif_mi(k, crate::tx::ROL_KYC_ONAYLAYICI, 0, self.zincir_saati)
        })
    }
}

/// Ethereum precompile'lari + (etkinse) RWA precompile'lari.
pub struct AidagPrecompiles<'a> {
    eth: EthPrecompiles,
    rwa: Option<RwaGorunum<'a>>,
}

impl<'a> AidagPrecompiles<'a> {
    pub fn new(eth: EthPrecompiles, rwa: Option<RwaGorunum<'a>>) -> Self {
        AidagPrecompiles { eth, rwa }
    }
}

impl<CTX: ContextTr> PrecompileProvider<CTX> for AidagPrecompiles<'_> {
    type Output = InterpreterResult;

    fn set_spec(&mut self, spec: <CTX::Cfg as revm::context::Cfg>::Spec) -> bool {
        <EthPrecompiles as PrecompileProvider<CTX>>::set_spec(&mut self.eth, spec)
    }

    fn run(&mut self, context: &mut CTX, inputs: &CallInputs) -> Result<Option<InterpreterResult>, String> {
        if let Some(rwa) = self.rwa {
            let adres: [u8; 20] = inputs.bytecode_address.into_array();
            if rwa_adresi_mi(&adres) {
                let girdi = inputs.input.as_bytes(context).to_vec();
                return Ok(Some(rwa_calistir(&rwa, &adres, &girdi, inputs)));
            }
        }
        <EthPrecompiles as PrecompileProvider<CTX>>::run(&mut self.eth, context, inputs)
    }

    /// Sicak adresler YALNIZ Ethereum'unkiler (RWA adresleri soguk erisim oder;
    /// RWA kapaliyken davranis Ethereum saglayicisiyla birebir ayni).
    fn warm_addresses(&self) -> &AddressSet {
        self.eth.warm_addresses()
    }

    fn contains(&self, address: &Address) -> bool {
        self.eth.contains(address) || (self.rwa.is_some() && rwa_adresi_mi(&address.into_array()))
    }
}

/// Precompile cikti yardimcisi: sabit gaz; yetersizse OOG (cikti yok).
fn sonuc(inputs: &CallInputs, basari: bool, cikti: Vec<u8>) -> InterpreterResult {
    let mut gas = Gas::new_with_regular_gas_and_reservoir(inputs.gas_limit, inputs.reservoir);
    if !gas.record_regular_cost(RWA_PRECOMPILE_GAZ) {
        gas.spend_all();
        return InterpreterResult {
            result: InstructionResult::PrecompileOOG,
            output: Bytes::new(),
            gas,
        };
    }
    InterpreterResult {
        result: if basari { InstructionResult::Return } else { InstructionResult::Revert },
        output: Bytes::from(cikti),
        gas,
    }
}

fn kelime_u64(v: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&v.to_be_bytes());
    w
}

fn kelime_i128(v: i128) -> [u8; 32] {
    let mut w = if v < 0 { [0xFFu8; 32] } else { [0u8; 32] }; // isaret genisletme (int256)
    w[16..].copy_from_slice(&v.to_be_bytes());
    w
}

fn abi_string(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = kelime_u64(0x20).to_vec();
    out.extend_from_slice(&kelime_u64(b.len() as u64));
    out.extend_from_slice(b);
    out.resize(64 + b.len().div_ceil(32) * 32, 0);
    out
}

/// Revert verisi: Solidity `Error(string)`.
fn hata(mesaj: &str) -> Vec<u8> {
    let mut out = vec![0x08, 0xc3, 0x79, 0xa0];
    out.extend_from_slice(&abi_string(mesaj));
    out
}

fn tur_tuple(t: &crate::rwa::TurKaydi) -> Vec<u8> {
    let mut out = Vec::with_capacity(160);
    out.extend_from_slice(&kelime_u64(t.tur_no)); // roundId (uint80)
    out.extend_from_slice(&kelime_i128(t.deger)); // answer (int256)
    out.extend_from_slice(&kelime_u64(t.baslangic)); // startedAt
    out.extend_from_slice(&kelime_u64(t.guncelleme)); // updatedAt
    out.extend_from_slice(&kelime_u64(t.tur_no)); // answeredInRound
    out
}

/// Girdiden 32 baytlik ABI kelimesi (yoksa None).
fn arguman(girdi: &[u8], i: usize) -> Option<&[u8]> {
    girdi.get(4 + 32 * i..4 + 32 * (i + 1))
}

fn rwa_calistir(rwa: &RwaGorunum<'_>, adres: &[u8; 20], girdi: &[u8], inputs: &CallInputs) -> InterpreterResult {
    // SALT OKUNUR: deger aktarimi reddedilir (frame, revert'te aktarimi geri alir).
    if matches!(inputs.value, CallValue::Transfer(v) if !v.is_zero()) {
        return sonuc(inputs, false, hata("RWA: deger kabul edilmez"));
    }
    let Some(sec) = girdi.get(..4) else {
        return sonuc(inputs, false, hata("RWA: secici yok"));
    };
    let sec: [u8; 4] = sec.try_into().expect("4 bayt");

    if *adres == KYC_ADRESI {
        if sec != SEC_IS_APPROVED {
            return sonuc(inputs, false, hata("RWA: bilinmeyen fonksiyon"));
        }
        let Some(w) = arguman(girdi, 0) else {
            return sonuc(inputs, false, hata("RWA: arguman eksik"));
        };
        if w[..12] != [0u8; 12] {
            return sonuc(inputs, false, hata("RWA: gecersiz adres"));
        }
        let mut a = [0u8; 20];
        a.copy_from_slice(&w[12..]);
        let onay = rwa.kyc_onayli(&a);
        return sonuc(inputs, true, kelime_u64(u64::from(onay)).to_vec());
    }

    let akis_no = oracle_akis_no(adres).expect("rwa_adresi_mi dogruladi");
    let Some(akis) = rwa.oracle.akis(akis_no) else {
        return sonuc(inputs, false, hata("RWA: akis yok"));
    };
    match sec {
        SEC_DECIMALS => sonuc(inputs, true, kelime_u64(u64::from(akis.tanim.ondalik)).to_vec()),
        SEC_DESCRIPTION => sonuc(inputs, true, abi_string(&akis.tanim.aciklama)),
        SEC_VERSION => sonuc(inputs, true, kelime_u64(RWA_ORACLE_SURUM).to_vec()),
        SEC_LATEST_ROUND_DATA => match rwa.oracle.son_veri(akis_no, rwa.zincir_saati) {
            Ok(t) => sonuc(inputs, true, tur_tuple(t)),
            Err(e) => {
                let m = match e {
                    crate::rwa::OkumaHatasi::AkisYok => "RWA: akis yok",
                    crate::rwa::OkumaHatasi::VeriYok => "RWA: veri yok",
                    crate::rwa::OkumaHatasi::Durduruldu => "RWA: akis durduruldu",
                    crate::rwa::OkumaHatasi::Bayat => "RWA: veri bayat",
                };
                sonuc(inputs, false, hata(m))
            }
        },
        SEC_GET_ROUND_DATA => {
            let Some(w) = arguman(girdi, 0) else {
                return sonuc(inputs, false, hata("RWA: arguman eksik"));
            };
            // uint80 ABI: ust 22 bayt sifir; tur numarasi u64'e sigmali.
            if w[..24] != [0u8; 24] {
                return sonuc(inputs, false, hata("RWA: tur yok"));
            }
            let tur = u64::from_be_bytes(w[24..].try_into().expect("8 bayt"));
            match rwa.oracle.tur_verisi(akis_no, tur) {
                Some(t) => sonuc(inputs, true, tur_tuple(t)),
                None => sonuc(inputs, false, hata("RWA: tur yok")),
            }
        }
        _ => sonuc(inputs, false, hata("RWA: bilinmeyen fonksiyon")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avm::{avm_call_oku_rwa, avm_calistir_rwa, AidagDatabase};
    use crate::registry::{KurumKategori, KurumRegistry};
    use crate::rwa::{KycRegistry, OracleRegistry};
    use crate::tx::{OracleAkisTanim, OracleRapor, ROL_KYC_ONAYLAYICI};
    use revm::precompile::{PrecompileSpecId, Precompiles};
    use revm::primitives::keccak256;

    const T: u64 = 1_800_000_000;
    const KURUM: [u8; 20] = [0x42; 20];
    const MUSTERI: [u8; 20] = [0xC1; 20];

    struct Durum {
        oracle: OracleRegistry,
        kurumlar: KurumRegistry,
        kyc: KycRegistry,
    }

    /// Akis 1 (8 ondalik, M=1): tur 1 = 2_500.12345678 (T'de), KYC: MUSTERI onayli.
    fn durum() -> Durum {
        let mut oracle = OracleRegistry::yeni();
        oracle.tanimla(
            OracleAkisTanim {
                akis_no: 1,
                ondalik: 8,
                esik_m: 1,
                sapma_bps: 200,
                kesici_bps: 1_000,
                bayat_sn: 3_600,
                aciklama: "XAU / USD".into(),
            },
            T,
        );
        let r = OracleRapor { akis_no: 1, tur_no: 1, deger: 250_012_345_678, olcum_zamani: T, veri_hash: [1; 32] };
        oracle.rapor_isle(KURUM, &r, T, |_| true);
        let mut kurumlar = KurumRegistry::yeni();
        kurumlar.kaydet(KURUM, "Banka".into(), KurumKategori::Ozel, 0);
        kurumlar.rol_ver(KURUM, ROL_KYC_ONAYLAYICI, 0, 0);
        let mut kyc = KycRegistry::yeni();
        kyc.isle(MUSTERI, KURUM, true, [2; 32], T);
        Durum { oracle, kurumlar, kyc }
    }

    fn gorunum(d: &Durum, saat: u64) -> RwaGorunum<'_> {
        RwaGorunum { oracle: &d.oracle, kurumlar: &d.kurumlar, kyc: &d.kyc, zincir_saati: saat }
    }

    fn cagri(sec: [u8; 4], arg: Option<[u8; 32]>) -> Vec<u8> {
        let mut v = sec.to_vec();
        if let Some(a) = arg {
            v.extend_from_slice(&a);
        }
        v
    }

    fn adres_arg(a: [u8; 20]) -> [u8; 32] {
        let mut w = [0u8; 32];
        w[12..].copy_from_slice(&a);
        w
    }

    fn oku(d: &Durum, saat: u64, hedef: [u8; 20], data: &[u8]) -> Result<Vec<u8>, &'static str> {
        avm_call_oku_rwa(&AidagDatabase::yeni(), &[0; 20], &hedef, data, Some(gorunum(d, saat)))
    }

    #[test]
    fn secicileri_keccak_ile_dogru() {
        for (imza, sec) in [
            ("decimals()", SEC_DECIMALS),
            ("description()", SEC_DESCRIPTION),
            ("version()", SEC_VERSION),
            ("getRoundData(uint80)", SEC_GET_ROUND_DATA),
            ("latestRoundData()", SEC_LATEST_ROUND_DATA),
            ("isApproved(address)", SEC_IS_APPROVED),
        ] {
            assert_eq!(&keccak256(imza.as_bytes())[..4], &sec, "{imza}");
        }
    }

    #[test]
    fn ethereum_precompile_adresleriyle_cakismaz() {
        let specler = [
            PrecompileSpecId::HOMESTEAD,
            PrecompileSpecId::BYZANTIUM,
            PrecompileSpecId::ISTANBUL,
            PrecompileSpecId::BERLIN,
            PrecompileSpecId::CANCUN,
            PrecompileSpecId::PRAGUE,
            PrecompileSpecId::OSAKA,
        ];
        let bizim: Vec<[u8; 20]> = [KYC_ADRESI, oracle_adresi(1), oracle_adresi(2), oracle_adresi(u32::MAX)].to_vec();
        for sp in specler {
            let eth = Precompiles::new(sp);
            assert!(eth.addresses().count() > 0);
            for a in eth.addresses() {
                let a: [u8; 20] = a.into_array();
                assert!(!rwa_adresi_mi(&a), "{sp:?}: Ethereum adresi RWA sayildi: {a:?}");
                // Ethereum precompile'lari dusuk aralikta (<= 0x100); bizimkiler 2^63 ustu.
                assert!(a[..18] == [0u8; 18], "{sp:?}: beklenmeyen Ethereum precompile adresi");
            }
            for b in &bizim {
                assert!(!eth.contains(&Address::from(*b)), "{sp:?}: cakisma");
            }
        }
        assert!(!rwa_adresi_mi(&oracle_adresi(0)), "akis 0 gecersiz");
        assert_eq!(oracle_akis_no(&oracle_adresi(7)), Some(7));
        assert_eq!(oracle_akis_no(&KYC_ADRESI), None);
    }

    #[test]
    fn latest_round_data_chainlink_abi() {
        let d = durum();
        let o = oku(&d, T + 60, oracle_adresi(1), &cagri(SEC_LATEST_ROUND_DATA, None)).unwrap();
        assert_eq!(o.len(), 160);
        let w = |i: usize| &o[32 * i..32 * (i + 1)];
        assert_eq!(w(0), &kelime_u64(1)); // roundId
        assert_eq!(w(1), &kelime_i128(250_012_345_678)); // answer
        assert_eq!(w(2), &kelime_u64(T)); // startedAt
        assert_eq!(w(3), &kelime_u64(T)); // updatedAt
        assert_eq!(w(4), &kelime_u64(1)); // answeredInRound
        let dec = oku(&d, T, oracle_adresi(1), &cagri(SEC_DECIMALS, None)).unwrap();
        assert_eq!(dec, kelime_u64(8));
        let ver = oku(&d, T, oracle_adresi(1), &cagri(SEC_VERSION, None)).unwrap();
        assert_eq!(ver, kelime_u64(RWA_ORACLE_SURUM));
        let desc = oku(&d, T, oracle_adresi(1), &cagri(SEC_DESCRIPTION, None)).unwrap();
        assert_eq!(desc, abi_string("XAU / USD"));
        let mut tur = [0u8; 32];
        tur[31] = 1;
        let g = oku(&d, T + 10 * 86_400, oracle_adresi(1), &cagri(SEC_GET_ROUND_DATA, Some(tur))).unwrap();
        assert_eq!(g, o, "gecmis tur bayatlik kontrolu olmadan okunur");
    }

    #[test]
    fn negatif_deger_int256_isaret_genisletir() {
        assert_eq!(kelime_i128(-1), [0xFF; 32]);
        let w = kelime_i128(-2);
        assert_eq!(w[31], 0xFE);
        assert!(w[..31].iter().all(|b| *b == 0xFF));
        assert_eq!(kelime_i128(i128::MIN)[..16], [0xFF; 16]);
    }

    #[test]
    fn bayat_durmus_tanimsiz_ve_hatali_cagri_revert() {
        let d = durum();
        let latest = cagri(SEC_LATEST_ROUND_DATA, None);
        assert!(oku(&d, T + 3_600, oracle_adresi(1), &latest).is_ok(), "sinir: bayat degil");
        assert!(oku(&d, T + 3_601, oracle_adresi(1), &latest).is_err(), "ZINCIR saatine gore bayat");
        assert!(oku(&d, T, oracle_adresi(2), &latest).is_err(), "tanimsiz akis");
        assert!(oku(&d, T, oracle_adresi(1), &[0xde, 0xad]).is_err(), "secici yok");
        assert!(oku(&d, T, oracle_adresi(1), &cagri([1, 2, 3, 4], None)).is_err(), "bilinmeyen fonksiyon");
        assert!(oku(&d, T, oracle_adresi(1), &cagri(SEC_GET_ROUND_DATA, None)).is_err(), "arguman eksik");
        let mut buyuk = [0u8; 32];
        buyuk[0] = 1;
        assert!(oku(&d, T, oracle_adresi(1), &cagri(SEC_GET_ROUND_DATA, Some(buyuk))).is_err());
        let mut tur9 = [0u8; 32];
        tur9[31] = 9;
        assert!(oku(&d, T, oracle_adresi(1), &cagri(SEC_GET_ROUND_DATA, Some(tur9))).is_err());
        // Devre kesici: %50 sicrama -> akis durur -> latestRoundData revert.
        let mut d2 = durum();
        let r = OracleRapor { akis_no: 1, tur_no: 2, deger: 375_000_000_000, olcum_zamani: T + 1, veri_hash: [3; 32] };
        d2.oracle.rapor_isle(KURUM, &r, T + 1, |_| true);
        assert!(oku(&d2, T + 1, oracle_adresi(1), &latest).is_err(), "durmus akis okunmaz");
    }

    #[test]
    fn kyc_is_approved() {
        let d = durum();
        let sor = |a| oku(&d, T, KYC_ADRESI, &cagri(SEC_IS_APPROVED, Some(adres_arg(a)))).unwrap();
        assert_eq!(sor(MUSTERI), kelime_u64(1));
        assert_eq!(sor([0xC2; 20]), kelime_u64(0));
        // Kurum rolu iptal edilince onay gecersiz.
        let mut d2 = durum();
        d2.kurumlar.rol_al(KURUM, ROL_KYC_ONAYLAYICI, 0, T);
        let o = oku(&d2, T, KYC_ADRESI, &cagri(SEC_IS_APPROVED, Some(adres_arg(MUSTERI)))).unwrap();
        assert_eq!(o, kelime_u64(0));
        // Ust 12 bayti dolu (gecersiz ABI adres) ve eksik arguman -> revert.
        let mut kotu = adres_arg(MUSTERI);
        kotu[0] = 1;
        assert!(oku(&d, T, KYC_ADRESI, &cagri(SEC_IS_APPROVED, Some(kotu))).is_err());
        assert!(oku(&d, T, KYC_ADRESI, &cagri(SEC_IS_APPROVED, None)).is_err());
        assert!(oku(&d, T, KYC_ADRESI, &cagri(SEC_LATEST_ROUND_DATA, None)).is_err());
    }

    #[test]
    fn rwa_kapaliyken_ethereum_ile_birebir_ayni() {
        // None: RWA adresi siradan bos hesap -> cagri basarili, cikti BOS (veri yok).
        let db = AidagDatabase::yeni();
        let latest = cagri(SEC_LATEST_ROUND_DATA, None);
        assert_eq!(avm_call_oku_rwa(&db, &[0; 20], &oracle_adresi(1), &latest, None), Ok(vec![]));
        assert_eq!(
            avm_call_oku_rwa(&db, &[0; 20], &KYC_ADRESI, &cagri(SEC_IS_APPROVED, Some(adres_arg(MUSTERI))), None),
            Ok(vec![])
        );
        // Ethereum precompile (0x02 SHA-256) her iki durumda ayni sonucu verir.
        let mut sha = [0u8; 20];
        sha[19] = 2;
        let d = durum();
        let a = avm_call_oku_rwa(&db, &[0; 20], &sha, b"abc", None).unwrap();
        let b = avm_call_oku_rwa(&db, &[0; 20], &sha, b"abc", Some(gorunum(&d, T))).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
    }

    /// Calldata'yi `hedef`e STATICCALL ile ileten (gaz = `gaz` ya da tum gaz) ve
    /// donus verisini aynen donduren/revert eden minimal kontrat (init kodu).
    fn vekil_kontrat(hedef: [u8; 20], gaz: Option<u16>) -> Vec<u8> {
        let mut r = vec![0x36, 0x60, 0x00, 0x60, 0x00, 0x37]; // calldatacopy(0,0,cds)
        r.extend_from_slice(&[0x60, 0x00, 0x60, 0x00, 0x36, 0x60, 0x00, 0x73]);
        r.extend_from_slice(&hedef);
        match gaz {
            Some(g) => r.extend_from_slice(&[0x61, (g >> 8) as u8, g as u8]),
            None => r.push(0x5a), // GAS
        }
        r.push(0xfa); // STATICCALL
        r.extend_from_slice(&[0x3d, 0x60, 0x00, 0x60, 0x00, 0x3e]); // returndatacopy
        let hedef_ofs = r.len() + 3 + 4;
        r.extend_from_slice(&[0x60, hedef_ofs as u8, 0x57]); // jumpi(basari)
        r.extend_from_slice(&[0x3d, 0x60, 0x00, 0xfd]); // revert(0, rds)
        r.extend_from_slice(&[0x5b, 0x3d, 0x60, 0x00, 0xf3]); // jumpdest; return(0, rds)
        let mut init = vec![0x60, r.len() as u8, 0x60, 0x0c, 0x60, 0x00, 0x39, 0x60, r.len() as u8, 0x60, 0x00, 0xf3];
        assert_eq!(init.len(), 0x0c);
        init.extend_from_slice(&r);
        init
    }

    fn vekil_kur(db: &mut AidagDatabase, hedef: [u8; 20], gaz: Option<u16>, zaman: u64) -> [u8; 20] {
        let s = avm_calistir_rwa(db, &[0x11; 20], &[0; 20], 0, &vekil_kontrat(hedef, gaz), zaman, None)
            .expect("deploy");
        assert!(s.basarili);
        s.olusan_adres.expect("adres")
    }

    #[test]
    fn kontrat_staticcall_ile_okur() {
        let d = durum();
        let mut db = AidagDatabase::yeni();
        let v = vekil_kur(&mut db, oracle_adresi(1), None, T);
        let o = avm_call_oku_rwa(&db, &[0; 20], &v, &cagri(SEC_LATEST_ROUND_DATA, None), Some(gorunum(&d, T))).unwrap();
        assert_eq!(&o[32..64], &kelime_i128(250_012_345_678));
        let k = vekil_kur(&mut db, KYC_ADRESI, None, T);
        let o = avm_call_oku_rwa(&db, &[0; 20], &k, &cagri(SEC_IS_APPROVED, Some(adres_arg(MUSTERI))), Some(gorunum(&d, T))).unwrap();
        assert_eq!(o, kelime_u64(1));
        // Bayat veri kontrattan da okunamaz (ZINCIR saati; vekil revert eder).
        assert!(avm_call_oku_rwa(&db, &[0; 20], &v, &cagri(SEC_LATEST_ROUND_DATA, None), Some(gorunum(&d, T + 3_601))).is_err());
    }

    #[test]
    fn her_cagri_sabit_gaz_oder() {
        let d = durum();
        let mut db = AidagDatabase::yeni();
        // Tam RWA_PRECOMPILE_GAZ ile basarili, bir eksigiyle OOG.
        let yeter = vekil_kur(&mut db, oracle_adresi(1), Some(RWA_PRECOMPILE_GAZ as u16), T);
        let eksik = vekil_kur(&mut db, oracle_adresi(1), Some(RWA_PRECOMPILE_GAZ as u16 - 1), T);
        let data = cagri(SEC_DECIMALS, None);
        assert_eq!(avm_call_oku_rwa(&db, &[0; 20], &yeter, &data, Some(gorunum(&d, T))), Ok(kelime_u64(8).to_vec()));
        assert!(avm_call_oku_rwa(&db, &[0; 20], &eksik, &data, Some(gorunum(&d, T))).is_err(), "2599 gaz: OOG");
        // Revert eden cagri da ayni gazi oder: bilinmeyen fonksiyon 2600 ile revert (OOG degil)
        // -> vekil revert verisini (Error(string)) aynen tasir.
        let kyc_yeter = vekil_kur(&mut db, KYC_ADRESI, Some(RWA_PRECOMPILE_GAZ as u16), T);
        assert!(avm_call_oku_rwa(&db, &[0; 20], &kyc_yeter, &data, Some(gorunum(&d, T))).is_err());
        // Islem duzeyinde (dogrudan cagri): gas_used = 21000 taban + calldata (4 sifir-disi
        // bayt x 16) + RWA_PRECOMPILE_GAZ. Revert eden cagri (bilinmeyen secici) AYNI gazi oder.
        let mut db2 = AidagDatabase::yeni();
        let beklenen = 21_000 + 4 * 16 + RWA_PRECOMPILE_GAZ;
        let a = avm_calistir_rwa(&mut db2, &[0x11; 20], &oracle_adresi(1), 0, &data, T, Some(gorunum(&d, T))).unwrap();
        assert!(a.basarili);
        assert_eq!(a.gas_used, beklenen);
        let r = avm_calistir_rwa(&mut db2, &[0x12; 20], &oracle_adresi(1), 0, &[1, 2, 3, 4], T, Some(gorunum(&d, T))).unwrap();
        assert!(!r.basarili);
        assert_eq!(r.gas_used, beklenen, "revert de ayni sabit gazi oder");
    }

    #[test]
    fn deger_gonderen_cagri_reddedilir_ve_bakiye_degismez() {
        let d = durum();
        let mut db = AidagDatabase::yeni();
        let gonderen = [0x21; 20];
        let mut b = std::collections::HashMap::new();
        b.insert(gonderen, 1_000_000u128);
        db.aidag_yukle_hepsi(&b);
        let s = avm_calistir_rwa(&mut db, &gonderen, &KYC_ADRESI, 5, &cagri(SEC_IS_APPROVED, Some(adres_arg(MUSTERI))), T, Some(gorunum(&d, T)))
            .unwrap();
        assert!(!s.basarili, "deger aktarimi REVERT");
        assert_eq!(db.aidag_bakiye(&gonderen), 1_000_000);
        assert_eq!(db.aidag_bakiye(&KYC_ADRESI), 0);
        // Degersiz ayni cagri basarili.
        let s = avm_calistir_rwa(&mut db, &gonderen, &KYC_ADRESI, 0, &cagri(SEC_IS_APPROVED, Some(adres_arg(MUSTERI))), T, Some(gorunum(&d, T)))
            .unwrap();
        assert!(s.basarili);
    }
}
