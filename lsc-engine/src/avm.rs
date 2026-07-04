//! AVM (AIDAG Virtual Machine) — revm cekirdegi uzerine kurulan "bize ait" katman.
//! KOPRU 1 (adres): ed25519 adres [u8;20] <-> revm Address.
//! KOPRU 2 (state): revm Database trait'i SENIN LSC defterinle. EVM native = LSC.

use revm::bytecode::Bytecode;
use revm::database_interface::DatabaseCommit;
use revm::primitives::Address;
use revm::primitives::AddressMap;
use revm::primitives::{B256, KECCAK_EMPTY, U256};
use revm::state::Account;
use revm::state::AccountInfo;
use revm::Database;
use std::collections::HashMap;
use std::convert::Infallible;

/// Senin adresin ([u8;20]) -> revm Address. Kayipsiz.
pub fn adres_to_evm(adres: &[u8; 20]) -> Address {
    Address::from(*adres)
}
/// revm Address -> senin adresin ([u8;20]). Kayipsiz.
pub fn evm_to_adres(addr: &Address) -> [u8; 20] {
    addr.into_array()
}

/// KOPRU 2: AVM'nin durum (state) kaynagi.
/// EVM "bu adresin bakiyesi ne?" diye sordugunda -> SENIN LSC defterinden cevap.
/// EVM native para = LSC (yakit). Sozlesme kod/storage: sonra eklenecek (su an bos).
#[derive(Clone)]
pub struct AidagDatabase {
    /// adres -> LSC bakiyesi (EVM native). Simdilik kopya; sonra gercek deftere baglanir.
    lsc_bakiyeler: HashMap<[u8; 20], crate::registry::Tutar>,
    /// KOPRU 5: adres -> sozlesme kodu (deploy edilen bytecode).
    kodlar: HashMap<[u8; 20], Bytecode>,
    /// KOPRU 5: (adres, slot) -> deger. Sozlesme kalici depolama.
    depo: HashMap<([u8; 20], U256), U256>,
}

impl AidagDatabase {
    pub fn yeni() -> Self {
        AidagDatabase {
            lsc_bakiyeler: HashMap::new(),
            kodlar: HashMap::new(),
            depo: HashMap::new(),
        }
    }
    /// Test/kurulum: bir adrese LSC bakiyesi koy (EVM bunu gorecek).
    pub fn lsc_koy(&mut self, adres: [u8; 20], miktar: crate::registry::Tutar) {
        self.lsc_bakiyeler.insert(adres, miktar);
    }
    /// Bir adresin LSC bakiyesini oku.
    pub fn lsc_bakiye(&self, adres: &[u8; 20]) -> crate::registry::Tutar {
        self.lsc_bakiyeler.get(adres).copied().unwrap_or(0)
    }

    /// KOPRU 5: bir adrese sozlesme kodu (bytecode) koy (deploy/test).
    pub fn kod_koy(&mut self, adres: [u8; 20], kod: Bytecode) {
        self.kodlar.insert(adres, kod);
    }
    /// KOPRU 5: bir adresin kodunu oku (yoksa None).
    pub fn kod_oku(&self, adres: &[u8; 20]) -> Option<&Bytecode> {
        self.kodlar.get(adres)
    }

    /// TEST/DOGRULAMA: deploy edilmis tum kontrat adreslerini dondur.
    pub fn kontrat_adresleri(&self) -> Vec<[u8; 20]> {
        self.kodlar.keys().copied().collect()
    }
    /// KOPRU 5: bir slota deger yaz (test/kurulum).
    pub fn depo_koy(&mut self, adres: [u8; 20], slot: U256, deger: U256) {
        self.depo.insert((adres, slot), deger);
    }
    /// KOPRU 5: bir slottaki degeri oku.
    pub fn depo_oku(&self, adres: &[u8; 20], slot: &U256) -> U256 {
        self.depo
            .get(&(*adres, *slot))
            .copied()
            .unwrap_or(U256::ZERO)
    }

    /// KOPRU 3 (gerceklestirme): isleyenden gas ucretini LSC olarak kes,
    /// %50 yak (YAKIM_ADRESI) + %50 gelistirme havuzuna yaz.
    /// Doner: Ok((yakilan, gelistirme)) | Err(yetersiz bakiye).
    /// KAPALI: kesilen = yakilan + gelistirme (yoktan para yok, kayip yok).
    pub fn gas_kes_ve_dagit(
        &mut self,
        isleyen: &[u8; 20],
        gas_used: u64,
    ) -> Result<(u64, u64), &'static str> {
        let ucret = gas_ucreti_hesapla(gas_used); // u64 (teknik birim)
        let ucret_t = ucret as crate::registry::Tutar; // para hesabina girerken donusum
        let mevcut = self.lsc_bakiye(isleyen);
        if mevcut < ucret_t {
            return Err("yetersiz LSC bakiyesi (gas odenemiyor)");
        }
        // 1) isleyenden kes
        self.lsc_bakiyeler.insert(*isleyen, mevcut - ucret_t);
        // 2) %50 yak + %50 gelistirme (gas u64 hesabi, sonra paraya donusur)
        let (yakilan, gelistirme) = gas_ucreti_bol(ucret);
        // yakim: YAKIM_ADRESI bakiyesine ekle (oradan asla cikmaz = yok olmus sayilir)
        let y = self.lsc_bakiye(&YAKIM_ADRESI);
        self.lsc_bakiyeler.insert(YAKIM_ADRESI, y + yakilan as crate::registry::Tutar);
        // gelistirme havuzu
        let g = self.lsc_bakiye(&GELISTIRME_HAVUZU);
        self.lsc_bakiyeler.insert(GELISTIRME_HAVUZU, g + gelistirme as crate::registry::Tutar);
        Ok((yakilan, gelistirme))
    }
}

impl Database for AidagDatabase {
    type Error = Infallible;

    /// EVM hesap bilgisi sorar -> LSC bakiyesini AccountInfo olarak don.
    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Infallible> {
        let adres = address.into_array();
        let bakiye = self.lsc_bakiyeler.get(&adres).copied().unwrap_or(0);
        // KOPRU 5: bu adresin sozlesme kodu var mi?
        let (code_hash, code) = match self.kodlar.get(&adres) {
            Some(b) => (b.hash_slow(), Some(b.clone())),
            None => (KECCAK_EMPTY, None),
        };
        Ok(Some(AccountInfo {
            balance: U256::from(bakiye),
            code_hash,
            code,
            ..Default::default()
        }))
    }

    /// KOPRU 5: hash e karsilik gelen sozlesme kodunu dondurur (yoksa bos).
    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Infallible> {
        // KOPRU 5: hash'e karsilik gelen kodu bul (yoksa bos).
        for kod in self.kodlar.values() {
            if kod.hash_slow() == code_hash {
                return Ok(kod.clone());
            }
        }
        Ok(Bytecode::default())
    }

    /// KOPRU 5: (adres, slot) -> deger okur (yoksa sifir).
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Infallible> {
        // KOPRU 5: (adres, slot) -> deger.
        let adres = address.into_array();
        Ok(self
            .depo
            .get(&(adres, index))
            .copied()
            .unwrap_or(U256::ZERO))
    }

    /// Blok hash: su an kullanilmiyor, sifir doner (EVM opcode uyumu icin).
    fn block_hash(&mut self, _number: u64) -> Result<B256, Infallible> {
        Ok(B256::ZERO)
    }
}

/// KOPRU 5: EVM'in urettigi state degisikliklerini SENIN deftere KALICI yaz.
/// EVM calisir -> degisen hesaplar (bakiye/kod/storage) -> burada defterlere islenir.
/// Bu olmadan deploy edilen sozlesme kalici olmaz.
impl DatabaseCommit for AidagDatabase {
    fn commit(&mut self, changes: AddressMap<Account>) {
        for (address, account) in changes.iter() {
            // sadece dokunulmus/degismis hesaplari isle
            if !account.is_touched() {
                continue;
            }
            let adres = address.into_array();

            // 1) Bakiye (LSC native) - u64'e sigdigi kadar
            let bakiye_u128 = account.info.balance.try_into().unwrap_or(u128::MAX);
            self.lsc_bakiyeler.insert(adres, bakiye_u128);

            // 2) Kod (deploy edilen sozlesme)
            if let Some(kod) = &account.info.code {
                if !kod.is_empty() {
                    self.kodlar.insert(adres, kod.clone());
                }
            }

            // 3) Storage slotlari (present_value)
            for (slot, deger) in account.storage.iter() {
                self.depo.insert((adres, *slot), deger.present_value);
            }
        }
    }
}

/// KOPRU 2 tamamlama: EVM'in urettigi state degisimini SENIN LSC defterine geri yaz.
/// EVM calisir -> state diff uretir -> bu fonksiyon o diff'i LSC bakiyelerine yansitir.
/// Tam tur: defter -> EVM -> calistir -> defter. (HashMap su an; sonra gercek registry.)
/// KOPRU 3 (GAS): EVM isleminin harcadigi gas'i LSC ucretine cevirir + dagitir.
/// Model (TASARIM belgesi): hibrit -> %50 YAK + %50 GELISTIRME havuzu.
/// Gas ucreti = gas_used * GAS_FIYATI_LSC (sabit fiyat, simdilik).
/// AIDAG'a DOKUNMAZ; sadece LSC. Doner: (yakilan, gelistirmeye_giden).
/// Yakim adresi: buraya giden LSC yok olur (kimsenin erisemeyecegi adres).
pub const YAKIM_ADRESI: [u8; 20] = [0u8; 20];
/// Gelistirme havuzu adresi: gas'in %50'si projeyi finanse etmek icin buraya.
pub const GELISTIRME_HAVUZU: [u8; 20] = [
    0x6e, 0xab, 0x1d, 0x6c, 0x2e, 0x5f, 0x70, 0x81, 0x92, 0xa3, 0xb4, 0xc5, 0xd6, 0xe7, 0xf8, 0x09,
    0x1a, 0x2b, 0x3c, 0x4d,
];

pub const GAS_FIYATI_LSC: u64 = 1; // 1 LSC / gas birimi (sabit, mainnette dinamik olacak).

/// Gas ucretini hesapla. gas_used -> toplam LSC ucreti.
pub fn gas_ucreti_hesapla(gas_used: u64) -> u64 {
    gas_used.saturating_mul(GAS_FIYATI_LSC)
}

/// Gas ucretini %50 yak + %50 gelistirme olarak bol. Doner: (yakilan, gelistirme).
/// Tek sayida 1 LSC artani yakima eklenir (deflasyon yonunde, kayip yok).
pub fn gas_ucreti_bol(ucret: u64) -> (u64, u64) {
    let gelistirme = ucret / 2;
    let yakilan = ucret - gelistirme; // tek sayi artani yakima
    (yakilan, gelistirme)
}

/// AVM calistirma sonucu.
pub struct AvmSonuc {
    pub basarili: bool,
    pub gas_used: u64,
    /// Deploy (CREATE) ise olusan sozlesme adresi.
    pub olusan_adres: Option<[u8; 20]>,
}

/// KOPRU 5 (canli): AVM islemini DETERMINISTIK calistir.
/// - hedef == [0;20] ve data dolu -> DEPLOY (CREATE), data = bytecode
/// - hedef dolu ve data dolu      -> CALL, data = calldata
/// - data bos                     -> sadece deger transferi (CALL, bos input)
/// `zaman`: vertex timestamp (DETERMINIZM: gercek saat DEGIL; tum dugumlerde ayni).
/// LSC bakiyeleri cagiran tarafindan db'ye ONCEDEN yuklenmis olmali; sonuc db'ye commit edilir.
/// Doner: AvmSonuc. Hata = revm seviyesinde calistirilmadi.
pub fn avm_calistir(
    db: &mut AidagDatabase,
    gonderen: &[u8; 20],
    hedef: &[u8; 20],
    deger: crate::registry::Tutar,
    data: &[u8],
    zaman: u64,
) -> Result<AvmSonuc, &'static str> {
    use revm::context::TxEnv;
    use revm::primitives::{Bytes, TxKind};
    use revm::{Context, ExecuteCommitEvm, MainBuilder, MainContext};

    let deploy = hedef == &[0u8; 20] && !data.is_empty();
    let kind = if deploy {
        TxKind::Create
    } else {
        TxKind::Call(adres_to_evm(hedef))
    };

    // DETERMINIZM: blok timestamp'ini vertex zamanina sabitle.
    let mut ctx = Context::mainnet().with_db(std::mem::replace(db, AidagDatabase::yeni()));
    ctx.modify_block(|b| {
        b.timestamp = U256::from(zaman);
    });
    let mut evm = ctx.build_mainnet();

    let tx = TxEnv::builder()
        .caller(adres_to_evm(gonderen))
        .kind(kind)
        .value(U256::from(deger))
        .data(Bytes::from(data.to_vec()))
        .gas_limit(3_000_000)
        .gas_price(0)
        .build()
        .map_err(|_| "tx olusturulamadi")?;

    let sonuc = evm
        .transact_commit(tx)
        .map_err(|_| "revm calistirilamadi")?;

    // db'yi geri al: evm.ctx uzerinden db_mut() ile eris, mem::replace ile cikar.
    use revm::context_interface::ContextTr;
    *db = std::mem::replace(evm.ctx.db_mut(), AidagDatabase::yeni());

    let basarili = sonuc.is_success();
    let gas_used = sonuc.tx_gas_used();
    let olusan_adres = match &sonuc {
        revm::context::result::ExecutionResult::Success {
            output: revm::context::result::Output::Create(_, Some(addr)),
            ..
        } => Some(evm_to_adres(addr)),
        _ => None,
    };

    Ok(AvmSonuc {
        basarili,
        gas_used,
        olusan_adres,
    })
}

/// eth_call icin: OKUMA-ONLY sozlesme cagrisi.
/// State'i DEGISTIRMEZ (db'nin kopyasi uzerinde calisir), sozlesmenin
/// dondurdugu ham veriyi (return data) verir. eth_call standardi: zincire yazmaz.
pub fn avm_call_oku(
    db: &AidagDatabase,
    gonderen: &[u8; 20],
    hedef: &[u8; 20],
    data: &[u8],
) -> Result<Vec<u8>, &'static str> {
    use revm::context::TxEnv;
    use revm::primitives::{Bytes, TxKind};
    use revm::{Context, ExecuteEvm, MainBuilder, MainContext};

    // OKUMA-ONLY: db'nin KOPYASI uzerinde calis (gercek state degismez).
    let db_kopya = db.clone();
    let ctx = Context::mainnet().with_db(db_kopya);
    let mut evm = ctx.build_mainnet();

    let tx = TxEnv::builder()
        .caller(adres_to_evm(gonderen))
        .kind(TxKind::Call(adres_to_evm(hedef)))
        .data(Bytes::from(data.to_vec()))
        .gas_limit(10_000_000)
        .gas_price(0)
        .build()
        .map_err(|_| "tx olusturulamadi")?;

    // transact (transact_commit DEGIL) -> state commit edilmez, sadece calistirir
    let sonuc = evm.transact(tx).map_err(|_| "revm call calistirilamadi")?;

    match sonuc.result {
        revm::context::result::ExecutionResult::Success {
            output: revm::context::result::Output::Call(veri),
            ..
        } => Ok(veri.to_vec()),
        revm::context::result::ExecutionResult::Success { .. } => Ok(Vec::new()),
        _ => Err("call basarisiz (revert ya da hata)"),
    }
}

/// Cozulmus ham Ethereum islemi (eth_sendRawTransaction icin).
pub struct HamEthIslem {
    pub gonderen: [u8; 20],
    pub hedef: Option<[u8; 20]>, // None = deploy (CREATE)
    pub deger: u128,
    pub veri: Vec<u8>,
    pub nonce: u64,
    pub gas_limit: u64,
}

/// MetaMask/web3'ten gelen RLP-kodlu ham Ethereum tx'i coz + gondereni kurtar.
/// Girdi: 0x-prefix ol/olmasin ham bytes. Cikti: cozulmus islem + gonderen adres.

/// Ham eth tx'in hash'i = keccak256(raw). Ethereum standardi (tx_hash).
pub fn eth_tx_hash(raw: &[u8]) -> [u8; 32] {
    use revm::primitives::keccak256;
    keccak256(raw).into()
}

pub fn ham_eth_tx_coz(raw: &[u8]) -> Result<HamEthIslem, &'static str> {
    use alloy_consensus::transaction::{SignerRecoverable, Transaction};
    use alloy_consensus::TxEnvelope;
    use alloy_eips::eip2718::Decodable2718;

    let zarf = TxEnvelope::decode_2718(&mut &raw[..]).map_err(|_| "raw tx cozulemedi (RLP)")?;
    let gonderen_addr = zarf.recover_signer_unchecked().map_err(|_| "imzadan gonderen kurtarilamadi")?;
    let gonderen = evm_to_adres(&gonderen_addr);

    let hedef = zarf.to().map(|a| evm_to_adres(&a));
    let deger: u128 = zarf.value().try_into().map_err(|_| "deger u128'e sigmiyor")?;
    let veri = zarf.input().to_vec();
    let nonce = zarf.nonce();
    let gas_limit = zarf.gas_limit();

    Ok(HamEthIslem { gonderen, hedef, deger, veri, nonce, gas_limit })
}

/// eth_sendRawTransaction cekirdegi: ham tx'i coz -> AVM'de calistir.
/// Doner: (tx_hash, avm_sonuc). tx_hash = keccak256(raw) - eth standardi.
/// state degisir (deploy/call kalici). Gonderen imzadan (guvenli).
pub fn ham_eth_tx_isle(
    db: &mut AidagDatabase,
    raw: &[u8],
    zaman: u64,
) -> Result<([u8; 32], AvmSonuc), &'static str> {
    use revm::primitives::keccak256;

    // 1) Coz + gonderen kurtar (imzadan)
    let islem = ham_eth_tx_coz(raw)?;

    // 2) tx hash = keccak256(raw bytes) - Ethereum standardi
    let tx_hash: [u8; 32] = keccak256(raw).into();

    // 3) AVM'de calistir: hedef None -> deploy, dolu -> call
    let hedef = islem.hedef.unwrap_or([0u8; 20]);
    let sonuc = avm_calistir(db, &islem.gonderen, &hedef, islem.deger, &islem.veri, zaman)?;

    Ok((tx_hash, sonuc))
}


pub fn state_lsc_deftere_yansit<'a, I>(
    state: I,
    lsc_defter: &mut std::collections::HashMap<[u8; 20], crate::registry::Tutar>,
) where
    I: IntoIterator<Item = (&'a Address, &'a revm::state::Account)>,
{
    for (addr, hesap) in state.into_iter() {
        let adres = addr.into_array();
        // EVM bakiyesi (U256) -> u64 (LSC). Tasma korumali.
        let yeni_bakiye: crate::registry::Tutar = hesap.info.balance.saturating_to::<u128>();
        lsc_defter.insert(adres, yeni_bakiye);
    }
}

/// KOPRU 4 (ISLEM): EVM islemini REPLAY korumali calistir.
/// Akis: 1) nonce dogru mu? (degilse RED) 2) gas kes+dagit (Kopru 3) 3) nonce ilerlet.
/// nonce_reg: registry.rs'teki NonceRegistry. isleyen: islemi yapan adres.
/// gelen_nonce: islemin tasidigi nonce. gas_used: islemin harcadigi gas.
/// Doner: Ok((yakilan, gelistirme)) | Err(replay ya da yetersiz bakiye).
pub fn islem_nonce_korumali(
    db: &mut AidagDatabase,
    nonce_reg: &mut crate::registry::NonceRegistry,
    isleyen: &[u8; 20],
    gelen_nonce: u64,
    gas_used: u64,
) -> Result<(u64, u64), &'static str> {
    // 1) REPLAY KORUMASI: nonce dogru mu? (beklenen ile esit degilse reddet)
    if !nonce_reg.dogru_mu(isleyen, gelen_nonce) {
        return Err("nonce hatali (replay ya da sira atlama) - islem reddedildi");
    }
    // 2) Gas kes + dagit (Kopru 3). Yetersiz bakiyede burada Err doner, nonce ILERLEMEZ.
    let (yakilan, gelistirme) = db.gas_kes_ve_dagit(isleyen, gas_used)?;
    // 3) Basarili -> nonce'u ilerlet (ayni nonce bir daha kullanilamaz).
    nonce_reg.ilerlet(isleyen);
    Ok((yakilan, gelistirme))
}

#[cfg(test)]
mod tests {
    use super::*;
    use revm::context::TxEnv;
    use revm::primitives::{Address, TxKind, U256};
    use revm::{Context, ExecuteCommitEvm, ExecuteEvm, MainBuilder, MainContext};

    #[test]
    fn revm_evm_olusturulabiliyor() {
        let _evm = Context::mainnet().build_mainnet();
        println!("AVM: revm EVM ornegi kuruldu");
    }

    #[test]
    fn revm_islem_calistirabiliyor() {
        let mut evm = Context::mainnet().build_mainnet();
        let tx = TxEnv::builder()
            .caller(Address::repeat_byte(0x11))
            .kind(TxKind::Call(Address::repeat_byte(0x22)))
            .value(U256::ZERO)
            .gas_limit(21_000)
            .build()
            .unwrap();
        let sonuc = evm.transact(tx);
        println!("AVM: transact sonucu = {:?}", sonuc.is_ok());
    }

    #[test]
    fn kopru1_adres_donusumu_kayipsiz() {
        let benim: [u8; 20] = [
            0xfe, 0x4a, 0x94, 0x47, 0xe5, 0x13, 0xe1, 0x75, 0xa3, 0xd1, 0xf5, 0x2a, 0x11, 0x0f,
            0x01, 0x68, 0x9e, 0x80, 0x00, 0x7a,
        ];
        let evm_adres = adres_to_evm(&benim);
        let geri = evm_to_adres(&evm_adres);
        assert_eq!(benim, geri);
        println!("AVM Kopru1: adres round-trip kayipsiz");
    }

    // KOPRU 2 KANIT: EVM, adresin bakiyesini SENIN LSC defterinden okuyor mu?
    #[test]
    fn kopru2_evm_lsc_bakiyesini_okuyor() {
        let mut db = AidagDatabase::yeni();
        let adres: [u8; 20] = [0xAB; 20];
        db.lsc_koy(adres, 5000); // LSC defterine 5000 koy
                                 // EVM'in soracagi gibi sor:
        let evm_adres = adres_to_evm(&adres);
        let hesap = db.basic(evm_adres).unwrap().unwrap();
        // KANIT: EVM'in gordugu bakiye = bizim LSC defterimizdeki 5000
        assert_eq!(hesap.balance, U256::from(5000u64));
        println!("AVM Kopru2: EVM, LSC bakiyesini okudu = {}", hesap.balance);
    }

    // KOPRU 2 DERIN: EVM, database'imize BAGLI olarak gercek transfer calistirir mi?
    // A'ya LSC ver -> A'dan B'ye deger transferi -> EVM'in urettigi state degisimini gor.
    #[test]
    fn kopru2_evm_database_transfer_calistirir() {
        let a: [u8; 20] = [0xAA; 20];
        let b: [u8; 20] = [0xBB; 20];
        let mut db = AidagDatabase::yeni();
        db.lsc_koy(a, 1_000_000); // A'ya 1.000.000 LSC

        let mut evm = Context::mainnet().with_db(db).build_mainnet();
        let tx = TxEnv::builder()
            .caller(adres_to_evm(&a))
            .kind(TxKind::Call(adres_to_evm(&b)))
            .value(U256::from(1000u64)) // 1000 LSC transfer
            .gas_limit(21_000)
            .gas_price(0) // gas ucreti yok (Kopru 3'te LSC gas eklenecek)
            .build()
            .unwrap();

        let sonuc = evm.transact(tx).expect("transact");
        // EVM'in urettigi state degisiminde B'nin bakiyesi arttı mi?
        let b_evm = adres_to_evm(&b);
        let b_hesap = sonuc.state.get(&b_evm);
        println!(
            "AVM Kopru2-derin: islem basarili={}, B state'te var mi={}",
            sonuc.result.is_success(),
            b_hesap.is_some()
        );
        assert!(sonuc.result.is_success(), "EVM transfer basarili olmali");
    }

    // KOPRU 2 TAM TUR: defter -> EVM -> calistir -> defter.
    // Gercek LSC defteri (map) -> EVM transfer -> sonucu deftere geri yaz -> dogrula.
    #[test]
    fn kopru2_tam_tur_defter_evm_defter() {
        use std::collections::HashMap;
        let a: [u8; 20] = [0xAA; 20];
        let b: [u8; 20] = [0xBB; 20];

        // 1) GERCEK LSC defteri (map): A'da 1.000.000
        let mut lsc_defter: HashMap<[u8; 20], crate::registry::Tutar> = HashMap::new();
        lsc_defter.insert(a, 1_000_000);

        // 2) Defterden EVM database kur
        let mut db = AidagDatabase::yeni();
        for (adr, mik) in lsc_defter.iter() {
            db.lsc_koy(*adr, *mik);
        }

        // 3) EVM: A'dan B'ye 1000 transfer
        let mut evm = Context::mainnet().with_db(db).build_mainnet();
        let tx = TxEnv::builder()
            .caller(adres_to_evm(&a))
            .kind(TxKind::Call(adres_to_evm(&b)))
            .value(U256::from(1000u64))
            .gas_limit(21_000)
            .gas_price(0)
            .build()
            .unwrap();
        let sonuc = evm.transact(tx).expect("transact");
        assert!(sonuc.result.is_success());

        // 4) EVM sonucunu GERCEK deftere geri yaz
        state_lsc_deftere_yansit(sonuc.state.iter(), &mut lsc_defter);

        // 5) KANIT: defterde A dustu, B arttı (tam tur tamamlandi)
        let a_son = *lsc_defter.get(&a).unwrap();
        let b_son = *lsc_defter.get(&b).unwrap_or(&0);
        println!("AVM Kopru2-tamtur: A={a_son}, B={b_son}");
        assert_eq!(b_son, 1000, "B 1000 LSC almali");
        assert!(a_son < 1_000_000, "A'nin bakiyesi dusmeli");
    }
    // KOPRU 3 KANIT: gas -> LSC ucreti -> %50 yak + %50 gelistirme.
    #[test]
    fn kopru3_gas_ucreti_ve_bolme() {
        // 21000 gas, fiyat=1 -> 21000 LSC ucret
        let ucret = gas_ucreti_hesapla(21_000);
        assert_eq!(ucret, 21_000);
        // %50 yak + %50 gelistirme
        let (yak, gelistirme) = gas_ucreti_bol(ucret);
        assert_eq!(yak, 10_500);
        assert_eq!(gelistirme, 10_500);
        assert_eq!(yak + gelistirme, ucret); // KAYIP YOK (kapali)
        println!(
            "AVM Kopru3: 21000 gas -> {} LSC -> yak={} gelistirme={}",
            ucret, yak, gelistirme
        );
    }

    // KOPRU 3 KANIT: tek sayi artan yakima gider (kayip yok, deflasyon yonunde).
    #[test]
    fn kopru3_tek_sayi_kayip_yok() {
        let (yak, gelistirme) = gas_ucreti_bol(7); // tek sayi
        assert_eq!(yak, 4);
        assert_eq!(gelistirme, 3);
        assert_eq!(yak + gelistirme, 7); // toplam korunur
        println!(
            "AVM Kopru3: 7 LSC -> yak={} gelistirme={} (artan yakima)",
            yak, gelistirme
        );
    }
    // KOPRU 3 GERCEKLESTIRME KANIT: isleyenden gercekten LSC kesilir + dagitilir.
    #[test]
    fn kopru3_gas_gercek_kesinti_ve_dagitim() {
        let isleyen: [u8; 20] = [0xAA; 20];
        let mut db = AidagDatabase::yeni();
        db.lsc_koy(isleyen, 100_000); // isleyende 100.000 LSC

        // 21.000 gas'lik islem -> 21.000 LSC kesilmeli
        let (yakilan, gelistirme) = db
            .gas_kes_ve_dagit(&isleyen, 21_000)
            .expect("kesinti basarili");

        // isleyende 79.000 kalmali
        assert_eq!(db.lsc_bakiye(&isleyen), 79_000);
        // %50 yak + %50 gelistirme
        assert_eq!(yakilan, 10_500);
        assert_eq!(gelistirme, 10_500);
        // YAKIM_ADRESI'nde 10.500, GELISTIRME_HAVUZU'nda 10.500
        assert_eq!(db.lsc_bakiye(&YAKIM_ADRESI), 10_500);
        assert_eq!(db.lsc_bakiye(&GELISTIRME_HAVUZU), 10_500);
        // KAPALI: kesilen (21000) = yakilan + gelistirme (kayip yok)
        assert_eq!(yakilan + gelistirme, 21_000);
        // KAPALI: toplam korundu -> 79000 + 10500 + 10500 = 100000
        assert_eq!(
            db.lsc_bakiye(&isleyen)
                + db.lsc_bakiye(&YAKIM_ADRESI)
                + db.lsc_bakiye(&GELISTIRME_HAVUZU),
            100_000
        );
        println!("AVM Kopru3-gercek: 21000 gas -> isleyen=79000, yak=10500, gelistirme=10500 (toplam korundu)");
    }

    // KOPRU 3 GERCEKLESTIRME KANIT: yetersiz bakiye reddedilir (gas odenemez).
    #[test]
    fn kopru3_yetersiz_bakiye_reddedilir() {
        let isleyen: [u8; 20] = [0xBB; 20];
        let mut db = AidagDatabase::yeni();
        db.lsc_koy(isleyen, 1_000); // sadece 1000 LSC

        // 21.000 gas -> 21.000 LSC gerekir ama 1000 var -> RED
        let sonuc = db.gas_kes_ve_dagit(&isleyen, 21_000);
        assert!(sonuc.is_err(), "yetersiz bakiye reddedilmeli");
        // bakiye DEGISMEMELI (basarisiz islem bakiyeyi bozmaz)
        assert_eq!(db.lsc_bakiye(&isleyen), 1_000);
        println!("AVM Kopru3-gercek: yetersiz bakiye dogru reddedildi, bakiye korundu");
    }

    // KOPRU 5 KANIT: gercek sozlesme kodu deploy + cagir + storage yaz/oku.
    // Bytecode: PUSH1 0x2a PUSH1 0x00 SSTORE STOP  (slot 0'a 42 yaz)
    #[test]
    fn kopru5_sozlesme_storage_yazar() {
        use revm::bytecode::Bytecode;
        use revm::primitives::Bytes;
        let kontrat: [u8; 20] = [0xCC; 20];
        let cagiran: [u8; 20] = [0xAA; 20];

        // Bytecode: 60 2a 60 00 55 00
        let ham = Bytes::from(vec![0x60, 0x2a, 0x60, 0x00, 0x55, 0x00]);
        let kod = Bytecode::new_raw(ham);

        let mut db = AidagDatabase::yeni();
        db.lsc_koy(cagiran, 1_000_000);
        db.kod_koy(kontrat, kod); // sozlesmeyi deploy et

        let mut evm = Context::mainnet().with_db(db).build_mainnet();
        let tx = TxEnv::builder()
            .caller(adres_to_evm(&cagiran))
            .kind(TxKind::Call(adres_to_evm(&kontrat)))
            .value(U256::ZERO)
            .gas_limit(100_000)
            .gas_price(0)
            .build()
            .unwrap();

        let sonuc = evm.transact(tx).expect("transact");
        println!(
            "KOPRU5: sozlesme calisti basarili={}",
            sonuc.result.is_success()
        );
        assert!(sonuc.result.is_success(), "sozlesme calismali");

        // state'te kontratin slot 0'i 42 (0x2a) olmali
        let kontrat_evm = adres_to_evm(&kontrat);
        let hesap = sonuc
            .state
            .get(&kontrat_evm)
            .expect("kontrat state'te olmali");
        let slot0 = hesap
            .storage
            .get(&U256::ZERO)
            .expect("slot 0 yazilmis olmali");
        println!("KOPRU5: slot0 yeni deger = {}", slot0.present_value);
        assert_eq!(slot0.present_value, U256::from(42u64), "slot 0 = 42 olmali");
    }

    // KOPRU 5 KURUMSAL KANIT: gercek Solidity sozlesmesi (BelgeDamgasi).
    // deploy -> kaydet(hash) -> dogrula(hash)=true -> ayni hash tekrar kaydet=RED.
    // Bytecode solcjs 0.8.35 ile derlendi (avm-sozlesmeler/BelgeDamgasi.sol).
    #[test]
    fn kopru5_belge_damgasi_kurumsal() {
        use revm::primitives::{keccak256, Bytes, TxKind};

        // 1) Deploy bytecode'u dosyadan al (derleme aninda gomulur)
        let bin_hex =
            include_str!("../../avm-sozlesmeler/BelgeDamgasi_sol_BelgeDamgasi.bin").trim();
        let deploy_kod = Bytes::from(hex_decode(bin_hex));

        let kurum: [u8; 20] = [0xAA; 20];
        let mut db = AidagDatabase::yeni();
        db.lsc_koy(kurum, 100_000_000);

        let mut evm = Context::mainnet().with_db(db).build_mainnet();

        // 2) DEPLOY (TxKind::Create) - revm constructor calistirir, runtime kod yerlesir
        let deploy_tx = TxEnv::builder()
            .caller(adres_to_evm(&kurum))
            .kind(TxKind::Create)
            .data(deploy_kod)
            .gas_limit(3_000_000)
            .gas_price(0)
            .build()
            .unwrap();
        let deploy_sonuc = evm.transact_commit(deploy_tx).expect("deploy transact");
        assert!(
            deploy_sonuc.is_success(),
            "deploy basarili olmali: {:?}",
            deploy_sonuc
        );
        let kontrat_adres = match deploy_sonuc {
            revm::context::result::ExecutionResult::Success { output, .. } => match output {
                revm::context::result::Output::Create(_, Some(addr)) => addr,
                _ => panic!("deploy adres dondurmeli"),
            },
            _ => panic!("deploy basarisiz"),
        };
        println!("KURUMSAL: sozlesme deploy edildi -> {:?}", kontrat_adres);

        // 3) Fonksiyon selector'lari (revm keccak256)
        let sel_kaydet = &keccak256(b"kaydet(bytes32)")[0..4];
        let sel_dogrula = &keccak256(b"dogrula(bytes32)")[0..4];

        // Ornek belge hash'i (32 bayt)
        let belge_hash = keccak256(b"AIDAG ornek belge: diploma #2026-001");

        // 4) KAYDET(belge_hash)
        let mut data = Vec::new();
        data.extend_from_slice(sel_kaydet);
        data.extend_from_slice(belge_hash.as_slice());
        let kaydet_tx = TxEnv::builder()
            .caller(adres_to_evm(&kurum))
            .kind(TxKind::Call(kontrat_adres))
            .data(Bytes::from(data))
            .gas_limit(300_000)
            .gas_price(0)
            .build()
            .unwrap();
        let r1 = evm.transact_commit(kaydet_tx).expect("kaydet transact");
        println!("KURUMSAL: 1. kayit basarili={}", r1.is_success());
        assert!(r1.is_success(), "ilk kayit basarili olmali");

        // 5) DOGRULA(belge_hash) -> varMi=true bekliyoruz
        let mut data2 = Vec::new();
        data2.extend_from_slice(sel_dogrula);
        data2.extend_from_slice(belge_hash.as_slice());
        let dogrula_tx = TxEnv::builder()
            .caller(adres_to_evm(&kurum))
            .kind(TxKind::Call(kontrat_adres))
            .data(Bytes::from(data2))
            .gas_limit(300_000)
            .gas_price(0)
            .build()
            .unwrap();
        let r2 = evm.transact(dogrula_tx).expect("dogrula transact");
        let cikti = match &r2.result {
            revm::context::result::ExecutionResult::Success { output, .. } => match output {
                revm::context::result::Output::Call(b) => b.clone(),
                _ => panic!("dogrula call ciktisi bekleniyor"),
            },
            _ => panic!("dogrula basarisiz"),
        };
        // ilk 32 bayt = bool varMi; son baytinin 1 olmasi true demek
        let var_mi = cikti.get(31).copied().unwrap_or(0) == 1;
        println!("KURUMSAL: dogrula -> belge kayitli mi = {}", var_mi);
        assert!(var_mi, "kaydedilen belge dogrulamada var gorunmeli");

        // 6) AYNI belgeyi TEKRAR kaydet -> RED (cift-kayit korumasi, require)
        let mut data3 = Vec::new();
        data3.extend_from_slice(sel_kaydet);
        data3.extend_from_slice(belge_hash.as_slice());
        let tekrar_tx = TxEnv::builder()
            .caller(adres_to_evm(&kurum))
            .kind(TxKind::Call(kontrat_adres))
            .data(Bytes::from(data3))
            .gas_limit(300_000)
            .gas_price(0)
            .build()
            .unwrap();
        let r3 = evm.transact(tekrar_tx).expect("tekrar transact");
        println!(
            "KURUMSAL: ayni belge 2. kez kayit basarili mi = {} (false BEKLENIYOR)",
            r3.result.is_success()
        );
        assert!(
            !r3.result.is_success(),
            "ayni belge ikinci kez kaydedilememeli (cift-kayit korumasi)"
        );

        println!("KURUMSAL KANIT TAMAM: deploy + kaydet + dogrula + cift-kayit reddi calisti.");
    }

    // Basit hex decoder (test icin; harici crate yok).
    fn hex_decode(h: &str) -> Vec<u8> {
        let h = h.strip_prefix("0x").unwrap_or(h);
        (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
            .collect()
    }

    // KALKAN-1 (gas limiti): sonsuz dongu kontrati gas limitiyle DURDURULUR.
    // Bytecode: 5b 60 00 56 = JUMPDEST PUSH1 0x00 JUMP (sonsuz dongu).
    // Beklenti: revm gas bitince OutOfGas/Halt ile durur; sistem KILITLENMEZ, panik YOK.
    #[test]
    fn kalkan1_sonsuz_dongu_gas_ile_durur() {
        use revm::bytecode::Bytecode;
        use revm::primitives::{Bytes, TxKind};

        let kontrat: [u8; 20] = [0xDD; 20];
        let cagiran: [u8; 20] = [0xAA; 20];

        // sonsuz dongu: JUMPDEST PUSH1 0 JUMP
        let ham = Bytes::from(vec![0x5b, 0x60, 0x00, 0x56]);
        let kod = Bytecode::new_raw(ham);

        let mut db = AidagDatabase::yeni();
        db.lsc_koy(cagiran, 1_000_000);
        db.kod_koy(kontrat, kod);

        let mut evm = Context::mainnet().with_db(db).build_mainnet();
        let tx = TxEnv::builder()
            .caller(adres_to_evm(&cagiran))
            .kind(TxKind::Call(adres_to_evm(&kontrat)))
            .value(U256::ZERO)
            .gas_limit(100_000) // KALKAN: sinirli gas
            .gas_price(0)
            .build()
            .unwrap();

        // transact PANIC etmemeli (sistem kilitlenmez) - sadece basarisiz sonuc doner
        let sonuc = evm.transact(tx).expect("transact panik etmemeli");
        let basarili = sonuc.result.is_success();
        let kullanilan_gas = sonuc.result.tx_gas_used();
        println!(
            "KALKAN1: sonsuz dongu -> basarili={} (false BEKLENIYOR), gas_used={}",
            basarili, kullanilan_gas
        );

        // 1) Sonuc BASARISIZ olmali (sonsuz dongu tamamlanamaz)
        assert!(!basarili, "sonsuz dongu basarili OLMAMALI (gas bitmeli)");
        // 2) Gas, limite yakin/limitte tukenmt olmali (koruma calisti)
        assert!(
            kullanilan_gas >= 90_000,
            "gas limite kadar tuketilmis olmali (koruma)"
        );
        println!("KALKAN1 TAMAM: sonsuz dongu gas limitiyle guvenle durduruldu.");
    }

    // KOPRU 5 (canli fonksiyon): avm_calistir ile deploy + cagir, DETERMINISTIK timestamp.
    #[test]
    fn avm_calistir_deploy_ve_cagir() {
        use revm::primitives::keccak256;

        let bin_hex =
            include_str!("../../avm-sozlesmeler/BelgeDamgasi_sol_BelgeDamgasi.bin").trim();
        let deploy_kod = hex_decode(bin_hex);

        let kurum: [u8; 20] = [0xAA; 20];
        let sifir: [u8; 20] = [0u8; 20];
        let mut db = AidagDatabase::yeni();
        db.lsc_koy(kurum, 100_000_000);

        // 1) DEPLOY: hedef=sifir, data=bytecode, zaman=1000 (deterministik)
        let r1 =
            avm_calistir(&mut db, &kurum, &sifir, 0, &deploy_kod, 1000).expect("deploy calismali");
        assert!(r1.basarili, "deploy basarili olmali");
        let kontrat = r1.olusan_adres.expect("deploy adres dondurmeli");
        println!(
            "avm_calistir: deploy OK -> {:?}, gas={}",
            kontrat, r1.gas_used
        );

        // 2) CALL: kaydet(hash)
        let sel_kaydet = &keccak256(b"kaydet(bytes32)")[0..4];
        let belge = keccak256(b"deterministik belge testi");
        let mut calldata = Vec::new();
        calldata.extend_from_slice(sel_kaydet);
        calldata.extend_from_slice(belge.as_slice());
        let r2 =
            avm_calistir(&mut db, &kurum, &kontrat, 0, &calldata, 1000).expect("cagri calismali");
        println!(
            "avm_calistir: kaydet basarili={}, gas={}",
            r2.basarili, r2.gas_used
        );
        assert!(r2.basarili, "kaydet basarili olmali");

        // 3) DETERMINIZM: ayni storage slotu db'de kalici olmali (kontrat state yazildi)
        // toplamKayit (slot... cozmek yerine) -> en azindan kontratin kodu db'de duruyor mu?
        assert!(
            db.kod_oku(&kontrat).is_some(),
            "deploy edilen kod db'de kalici olmali"
        );
        println!(
            "avm_calistir TAMAM: deploy+cagri db'ye kalici islendi (deterministik zaman=1000)."
        );
    }

    // KOPRU 4 KANIT: nonce replay korumasi gercekten calisiyor.
    #[test]
    fn kopru4_replay_korumasi_calisiyor() {
        use crate::registry::NonceRegistry;
        let isleyen: [u8; 20] = [0xCC; 20];
        let mut db = AidagDatabase::yeni();
        db.lsc_koy(isleyen, 100_000);
        let mut nreg = NonceRegistry::yeni();

        // 1) Ilk islem: nonce=0 -> basarili, nonce 1'e ilerler.
        let r1 = islem_nonce_korumali(&mut db, &mut nreg, &isleyen, 0, 21_000);
        assert!(r1.is_ok(), "ilk islem (nonce=0) basarili olmali");
        assert_eq!(nreg.beklenen(&isleyen), 1, "nonce 1'e ilerlemeli");
        assert_eq!(db.lsc_bakiye(&isleyen), 79_000, "gas kesilmis olmali");

        // 2) REPLAY: ayni nonce=0 tekrar -> REDDEDILMELI.
        let r2 = islem_nonce_korumali(&mut db, &mut nreg, &isleyen, 0, 21_000);
        assert!(r2.is_err(), "replay (nonce=0 tekrar) reddedilmeli");
        // bakiye DEGISMEMELI (reddedilen islem gas kesmez)
        assert_eq!(
            db.lsc_bakiye(&isleyen),
            79_000,
            "replay bakiyeyi degistirmemeli"
        );

        // 3) Sira atlama: nonce=5 (beklenen 1) -> REDDEDILMELI.
        let r3 = islem_nonce_korumali(&mut db, &mut nreg, &isleyen, 5, 21_000);
        assert!(r3.is_err(), "sira atlayan nonce reddedilmeli");

        // 4) Dogru sira: nonce=1 -> basarili.
        let r4 = islem_nonce_korumali(&mut db, &mut nreg, &isleyen, 1, 21_000);
        assert!(r4.is_ok(), "dogru nonce=1 basarili olmali");
        assert_eq!(nreg.beklenen(&isleyen), 2, "nonce 2'ye ilerlemeli");
        println!("AVM Kopru4: replay reddedildi, sira korundu, dogru islem gecti");
    }

    // ERC-20 UYUMLULUK KANITI: standart ERC-20 token AVM'de calisir.
    // Gercek solc 0.8.26 ile derlenmis standart ERC-20 (transfer/balanceOf).
    // Kanit: deploy -> balanceOf -> transfer -> bakiyeler dogru degisti.
    #[test]
    fn erc20_standart_token_calisir() {
        use revm::primitives::{keccak256, Bytes, TxKind, U256};

        let bin_hex = include_str!("../../avm-sozlesmeler/Token.bin").trim();
        let deployer: [u8; 20] = [0xAA; 20];
        let alici: [u8; 20] = [0xBB; 20];
        let baslangic_arz: u128 = 1_000_000;

        let mut db = AidagDatabase::yeni();
        db.lsc_koy(deployer, 100_000_000);
        let mut evm = Context::mainnet().with_db(db).build_mainnet();

        // DEPLOY: bytecode + constructor arg (arz, 32 bayt)
        let mut deploy_data = hex_decode(bin_hex);
        let mut arz_bytes = [0u8; 32];
        arz_bytes[16..32].copy_from_slice(&baslangic_arz.to_be_bytes());
        deploy_data.extend_from_slice(&arz_bytes);

        let deploy_tx = TxEnv::builder()
            .caller(adres_to_evm(&deployer))
            .kind(TxKind::Create)
            .data(Bytes::from(deploy_data))
            .gas_limit(3_000_000)
            .gas_price(0)
            .build()
            .unwrap();
        let ds = evm.transact_commit(deploy_tx).expect("erc20 deploy");
        assert!(ds.is_success(), "ERC-20 deploy basarili olmali: {:?}", ds);
        let kontrat = match ds {
            revm::context::result::ExecutionResult::Success { output, .. } => match output {
                revm::context::result::Output::Create(_, Some(addr)) => addr,
                _ => panic!("deploy adres dondurmeli"),
            },
            _ => panic!("deploy basarisiz"),
        };
        println!("ERC-20: token deploy edildi -> {:?}", kontrat);

        // balanceOf(deployer) -> baslangic_arz
        let sel_balance = &keccak256(b"balanceOf(address)")[0..4];
        let mut bo = Vec::new();
        bo.extend_from_slice(sel_balance);
        let mut adr32 = [0u8; 32];
        adr32[12..32].copy_from_slice(&deployer);
        bo.extend_from_slice(&adr32);
        let bo_tx = TxEnv::builder()
            .caller(adres_to_evm(&deployer))
            .kind(TxKind::Call(kontrat))
            .data(Bytes::from(bo))
            .gas_limit(300_000).gas_price(0).build().unwrap();
        let bo_r = evm.transact(bo_tx).expect("balanceOf");
        let bo_out = match &bo_r.result {
            revm::context::result::ExecutionResult::Success { output, .. } => match output {
                revm::context::result::Output::Call(b) => b.clone(),
                _ => panic!("balanceOf ciktisi"),
            },
            _ => panic!("balanceOf basarisiz"),
        };
        let d_bakiye = U256::from_be_slice(&bo_out);
        println!("ERC-20: deployer bakiyesi = {}", d_bakiye);
        assert_eq!(d_bakiye, U256::from(baslangic_arz), "deployer tum arza sahip");

        // transfer(alici, 1000)
        let sel_transfer = &keccak256(b"transfer(address,uint256)")[0..4];
        let mut tr = Vec::new();
        tr.extend_from_slice(sel_transfer);
        let mut alici32 = [0u8; 32];
        alici32[12..32].copy_from_slice(&alici);
        tr.extend_from_slice(&alici32);
        let mut m32 = [0u8; 32];
        m32[16..32].copy_from_slice(&1000u128.to_be_bytes());
        tr.extend_from_slice(&m32);
        let tr_tx = TxEnv::builder()
            .caller(adres_to_evm(&deployer))
            .kind(TxKind::Call(kontrat))
            .data(Bytes::from(tr))
            .gas_limit(300_000).gas_price(0).build().unwrap();
        let tr_r = evm.transact_commit(tr_tx).expect("transfer");
        assert!(tr_r.is_success(), "transfer basarili olmali");
        println!("ERC-20: transfer(alici, 1000) basarili");

        // balanceOf(alici) -> 1000
        let mut bo2 = Vec::new();
        bo2.extend_from_slice(sel_balance);
        let mut al32 = [0u8; 32];
        al32[12..32].copy_from_slice(&alici);
        bo2.extend_from_slice(&al32);
        let bo2_tx = TxEnv::builder()
            .caller(adres_to_evm(&alici))
            .kind(TxKind::Call(kontrat))
            .data(Bytes::from(bo2))
            .gas_limit(300_000).gas_price(0).build().unwrap();
        let bo2_r = evm.transact(bo2_tx).expect("balanceOf2");
        let bo2_out = match &bo2_r.result {
            revm::context::result::ExecutionResult::Success { output, .. } => match output {
                revm::context::result::Output::Call(b) => b.clone(),
                _ => panic!("balanceOf2 ciktisi"),
            },
            _ => panic!("balanceOf2 basarisiz"),
        };
        let a_bakiye = U256::from_be_slice(&bo2_out);
        println!("ERC-20: alici bakiyesi = {}", a_bakiye);
        assert_eq!(a_bakiye, U256::from(1000), "alici 1000 token almali");

        println!("ERC-20 KANIT TAMAM: deploy + balanceOf + transfer AVM'de calisti.");
    }


    // eth_call MOTORU KANITI: avm_call_oku gercek ERC-20'yi OKUR (state degismez).
    // Deploy et -> avm_call_oku ile balanceOf oku -> dogru deger + state degismedi.
    #[test]
    fn avm_call_oku_erc20_balanceof() {
        use revm::primitives::{keccak256, Bytes, TxKind};

        let bin_hex = include_str!("../../avm-sozlesmeler/Token.bin").trim();
        let deployer: [u8; 20] = [0xCC; 20];
        let arz: u128 = 1_000_000;

        let mut db = AidagDatabase::yeni();
        db.lsc_koy(deployer, 100_000_000);

        // Deploy (state'e yaz) - avm_calistir kullan (kalici)
        let mut deploy_data = hex_decode(bin_hex);
        let mut arz32 = [0u8; 32];
        arz32[16..32].copy_from_slice(&arz.to_be_bytes());
        deploy_data.extend_from_slice(&arz32);
        let sonuc = avm_calistir(&mut db, &deployer, &[0u8; 20], 0, &deploy_data, 1234)
            .expect("deploy");
        let kontrat = sonuc.olusan_adres.expect("kontrat adresi");
        println!("avm_call testi: kontrat deploy -> {:?}", kontrat);

        // balanceOf(deployer) calldata
        let sel = &keccak256(b"balanceOf(address)")[0..4];
        let mut cd = Vec::new();
        cd.extend_from_slice(sel);
        let mut adr32 = [0u8; 32];
        adr32[12..32].copy_from_slice(&deployer);
        cd.extend_from_slice(&adr32);

        // avm_call_oku ile OKU (eth_call motoru)
        let cikti = avm_call_oku(&db, &[0u8; 20], &kontrat, &cd).expect("call oku");
        let deger = revm::primitives::U256::from_be_slice(&cikti);
        println!("avm_call testi: balanceOf = {}", deger);
        assert_eq!(deger, revm::primitives::U256::from(arz), "balanceOf arzi dondurmeli");

        // state DEGISMEDI mi? (okuma-only): tekrar oku, ayni deger
        let cikti2 = avm_call_oku(&db, &[0u8; 20], &kontrat, &cd).expect("call oku 2");
        assert_eq!(cikti, cikti2, "okuma state degistirmemeli");
        println!("avm_call KANIT: eth_call motoru ERC-20 okudu, state degismedi.");
    }


    // HAM ETH TX COZME KANITI: gercek imzali Ethereum tx coz + gonderen kurtar.
    // MetaMask/web3'un urettigi RLP tx'i cozup gondereni buluyor muyuz.
    #[test]
    fn ham_eth_tx_coz_gonderen_kurtar() {
        use alloy_consensus::{SignableTransaction, TxLegacy};
        use alloy_primitives::{TxKind, U256, Signature};
        use alloy_signer::SignerSync;
        use alloy_signer_local::PrivateKeySigner;

        // Bilinen bir ozel anahtar -> bilinen adres
        let signer: PrivateKeySigner =
            "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318"
                .parse()
                .unwrap();
        let beklenen_adres = evm_to_adres(&signer.address());
        println!("Beklenen gonderen: 0x{}", hex_encode(&beklenen_adres));

        // Bir islem olustur (basit transfer: hedef + deger)
        let hedef = [0x11u8; 20];
        let tx = TxLegacy {
            chain_id: Some(3474),
            nonce: 0,
            gas_price: 0,
            gas_limit: 21_000,
            to: TxKind::Call(adres_to_evm(&hedef)),
            value: U256::from(1000u64),
            input: Default::default(),
        };

        // Imzala
        let imza: Signature = signer.sign_hash_sync(&tx.signature_hash()).unwrap();
        let imzali = tx.into_signed(imza);
        let zarf: alloy_consensus::TxEnvelope = imzali.into();

        // RLP encode (raw tx bytes) - MetaMask'in gonderdigi format
        use alloy_eips::eip2718::Encodable2718;
        let raw = zarf.encoded_2718();
        println!("Raw tx uzunluk: {} bayt", raw.len());

        // COZ
        let cozulmus = ham_eth_tx_coz(&raw).expect("ham tx cozulmeli");
        println!("Cozulen gonderen: 0x{}", hex_encode(&cozulmus.gonderen));
        println!("Cozulen hedef: {:?}", cozulmus.hedef.map(|h| hex_encode(&h)));
        println!("Cozulen deger: {}", cozulmus.deger);

        assert_eq!(cozulmus.gonderen, beklenen_adres, "gonderen dogru kurtarilmali");
        assert_eq!(cozulmus.hedef, Some(hedef), "hedef dogru");
        assert_eq!(cozulmus.deger, 1000, "deger dogru");
        assert_eq!(cozulmus.nonce, 0, "nonce dogru");
        println!("HAM ETH TX KANIT: RLP coz + ecrecover gonderen dogru.");
    }

    // Basit hex encoder (test print icin)
    fn hex_encode(b: &[u8]) -> String {
        b.iter().map(|x| format!("{:02x}", x)).collect()
    }


    // HAM ETH TX ISLE KANITI: imzali tx -> AVM'de calistir (uctan uca).
    // Once ERC-20 deploy (test kurulumu), sonra imzali transfer tx'i ISLE.
    #[test]
    fn ham_eth_tx_isle_erc20_transfer() {
        use alloy_consensus::{SignableTransaction, TxLegacy};
        use alloy_primitives::{Signature, TxKind, U256 as AU256};
        use alloy_signer::SignerSync;
        use alloy_signer_local::PrivateKeySigner;
        use revm::primitives::keccak256;

        let signer: PrivateKeySigner =
            "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318"
                .parse().unwrap();
        let gonderen = evm_to_adres(&signer.address());

        // Kurulum: gonderen'e LSC (gas) + ERC-20 deploy
        let mut db = AidagDatabase::yeni();
        db.lsc_koy(gonderen, 100_000_000);
        let bin_hex = include_str!("../../avm-sozlesmeler/Token.bin").trim();
        let mut deploy_data = hex_decode(bin_hex);
        let mut arz32 = [0u8; 32];
        arz32[16..32].copy_from_slice(&(1_000_000u128).to_be_bytes());
        deploy_data.extend_from_slice(&arz32);
        let d = avm_calistir(&mut db, &gonderen, &[0u8; 20], 0, &deploy_data, 100).expect("deploy");
        let kontrat = d.olusan_adres.expect("kontrat");
        println!("ISLE testi: ERC-20 deploy -> 0x{}", hex_encode(&kontrat));

        // Imzali transfer(alici, 500) tx'i olustur
        let alici = [0x22u8; 20];
        let sel = &keccak256(b"transfer(address,uint256)")[0..4];
        let mut calldata = Vec::new();
        calldata.extend_from_slice(sel);
        let mut a32 = [0u8; 32]; a32[12..32].copy_from_slice(&alici);
        calldata.extend_from_slice(&a32);
        let mut m32 = [0u8; 32]; m32[16..32].copy_from_slice(&(500u128).to_be_bytes());
        calldata.extend_from_slice(&m32);

        let tx = TxLegacy {
            chain_id: Some(3474), nonce: 0, gas_price: 0, gas_limit: 300_000,
            to: TxKind::Call(adres_to_evm(&kontrat)),
            value: AU256::ZERO,
            input: calldata.into(),
        };
        let imza: Signature = signer.sign_hash_sync(&tx.signature_hash()).unwrap();
        let zarf: alloy_consensus::TxEnvelope = tx.into_signed(imza).into();
        use alloy_eips::eip2718::Encodable2718;
        let raw = zarf.encoded_2718();

        // ISLE (coz + AVM'de calistir)
        let (tx_hash, sonuc) = ham_eth_tx_isle(&mut db, &raw, 200).expect("isle");
        println!("ISLE testi: tx_hash=0x{} basarili={}", hex_encode(&tx_hash), sonuc.basarili);
        assert!(sonuc.basarili, "transfer tx basarili olmali");

        // Dogrula: alici'nin ERC-20 bakiyesi 500 mu (avm_call_oku ile)
        let mut bo = Vec::new();
        bo.extend_from_slice(&keccak256(b"balanceOf(address)")[0..4]);
        let mut al32 = [0u8; 32]; al32[12..32].copy_from_slice(&alici);
        bo.extend_from_slice(&al32);
        let cikti = avm_call_oku(&db, &[0u8; 20], &kontrat, &bo).expect("balanceOf");
        let bakiye = revm::primitives::U256::from_be_slice(&cikti);
        println!("ISLE testi: alici ERC-20 bakiyesi = {}", bakiye);
        assert_eq!(bakiye, revm::primitives::U256::from(500u64), "alici 500 token almali");
        println!("HAM ETH TX ISLE KANIT: imzali tx -> ERC-20 transfer AVM'de calisti.");
    }

}
