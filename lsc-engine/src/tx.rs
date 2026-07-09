//! İşlem (transaction) tipleri — vertex payload'ında taşınan ANLAMLI veri.
//!
//! Tasarım ilkesi "açık kapı + şart": her işlem `[tip: 1 bayt][govde]` ile
//! başlar. Bugün tek tip var: Record (belge/veri kaydı). İleride yeni tipler
//! (Transfer vb.) YENİ bir tip kimliğiyle eklenir; mevcut format ve decode
//! mantığı bozulmadan genişler. Bilinmeyen tip REDDEDİLİR (şart = guard).
//!
//! Record: bir belgenin/verinin HASH'ini zincire yazar (içeriği DEĞİL —
//! gizlilik + boyut). "Kim" sorusunu vertex'in ed25519 imzası, "ne zaman"
//! sorusunu vertex'in timestamp'i zaten cevaplar; burada sadece "ne" (hash).

/// İşlem tip kimliği: belge/veri kaydı (bugün). GENİŞLEME KAPISI: ileride
/// Transfer (=2) vb. eklenir, bu sabitin yanına yeni sabit + decode dalı.
pub const TX_TYPE_RECORD: u8 = 1;

/// Record gövde uzunluğu: hash 32 bayt. Toplam kodlanmış = 1 (tip) + 32 = 33.
const HASH_LEN: usize = 32;
const RECORD_ENCODED_LEN: usize = 1 + HASH_LEN;

/// İşlem kodlama/çözme hataları.
#[derive(Debug, PartialEq, Eq)]
pub enum TxError {
    /// Tanınmayan tip baytı (genişleme kapısının şartı: bilinmeyen tip yasak).
    UnknownType(u8),
    /// Beklenen bayt uzunluğu tutmuyor.
    BadLength { expected: usize, got: usize },
    /// Hiç bayt yok (boş payload).
    Empty,
}

impl core::fmt::Display for TxError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TxError::UnknownType(t) => write!(f, "bilinmeyen islem tipi: {t}"),
            TxError::BadLength { expected, got } => {
                write!(
                    f,
                    "gecersiz islem uzunlugu: beklenen {expected}, gelen {got}"
                )
            }
            TxError::Empty => write!(f, "bos islem payload'i"),
        }
    }
}

impl std::error::Error for TxError {}

/// Belge/veri kaydı: bir içeriğin HASH'i (parmak izi). İçerik zincire girmez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Kaydedilen belgenin/verinin hash'i (örn. blake3/sha256 — 32 bayt).
    pub data_hash: [u8; HASH_LEN],
}

impl Record {
    /// Yeni bir Record.
    pub fn new(data_hash: [u8; HASH_LEN]) -> Self {
        Record { data_hash }
    }

    /// Kodla: `[TX_TYPE_RECORD][data_hash:32]` -> 33 bayt. Vertex payload'i olur.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(RECORD_ENCODED_LEN);
        out.push(TX_TYPE_RECORD);
        out.extend_from_slice(&self.data_hash);
        out
    }

    /// Çöz: tip baytini kontrol et (genişleme kapısının ŞARTI), boyutu doğrula,
    /// hash'i çıkar. Bilinmeyen tip / yanlış boyut REDDEDİLİR.
    pub fn decode(bytes: &[u8]) -> Result<Record, TxError> {
        let &first = bytes.first().ok_or(TxError::Empty)?;
        if first != TX_TYPE_RECORD {
            return Err(TxError::UnknownType(first));
        }
        if bytes.len() != RECORD_ENCODED_LEN {
            return Err(TxError::BadLength {
                expected: RECORD_ENCODED_LEN,
                got: bytes.len(),
            });
        }
        let mut data_hash = [0u8; HASH_LEN];
        data_hash.copy_from_slice(&bytes[1..RECORD_ENCODED_LEN]);
        Ok(Record { data_hash })
    }
}

// ============================================================================
// tip=2: TOKEN KIMLIK KAYDI (Kalkanli DEX cekirdegi)
// ============================================================================
// AMAC: Sahte/taklit token'lardan korumak. Bir token'in KIMLIGI = kanonik
// kontrat ADRESI (isim/sembol DEGIL — onlar taklit edilebilir). "USDC"
// gorunumlu ama farkli adresli token = TAKLIT. Kanonik adres zincire
// kaydedilir; taklitler ayni_sembol_farkli_adres ile yakalanir.

/// Token kimlik kaydi tip kimligi (genisleme kapisi: Record=1'in yaninda =2).
pub const TX_TYPE_TOKEN: u8 = 2;

/// Kanonik kontrat adresi uzunlugu (Ethereum-tarzi 20 bayt).
const ADDR_LEN: usize = 20;
/// Sembol etiketi uzunlugu (orn "USDC" — sabit 8 bayt, bos kalan sifir-dolgu).
const SYMBOL_LEN: usize = 8;
/// Kodlanmis token kaydi: 1 (tip) + 20 (adres) + 8 (sembol) = 29 bayt.
const TOKEN_ENCODED_LEN: usize = 1 + ADDR_LEN + SYMBOL_LEN;

/// Token kimlik kaydi: kanonik adres (KIMLIK) + sembol (etiket).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenKaydi {
    /// Kanonik kontrat adresi — token'in GERCEK kimligi. Taklit buradan ayrilir.
    pub kanonik_adres: [u8; ADDR_LEN],
    /// Sembol etiketi (orn "USDC"). KIMLIK DEGIL — taklit edilebilir, sadece
    /// insan-okur etiket. Kimlik karsilastirmasi ADRES uzerinden yapilir.
    pub sembol: [u8; SYMBOL_LEN],
}

impl TokenKaydi {
    /// Yeni token kaydi.
    pub fn new(kanonik_adres: [u8; ADDR_LEN], sembol: [u8; SYMBOL_LEN]) -> Self {
        TokenKaydi {
            kanonik_adres,
            sembol,
        }
    }

    /// Kodla: `[TX_TYPE_TOKEN][adres:20][sembol:8]` -> 29 bayt.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(TOKEN_ENCODED_LEN);
        out.push(TX_TYPE_TOKEN);
        out.extend_from_slice(&self.kanonik_adres);
        out.extend_from_slice(&self.sembol);
        out
    }

    /// Coz: tip + boyut dogrula (genisleme kapisinin SARTI), alanlari cikar.
    pub fn decode(bytes: &[u8]) -> Result<TokenKaydi, TxError> {
        let &first = bytes.first().ok_or(TxError::Empty)?;
        if first != TX_TYPE_TOKEN {
            return Err(TxError::UnknownType(first));
        }
        if bytes.len() != TOKEN_ENCODED_LEN {
            return Err(TxError::BadLength {
                expected: TOKEN_ENCODED_LEN,
                got: bytes.len(),
            });
        }
        let mut kanonik_adres = [0u8; ADDR_LEN];
        kanonik_adres.copy_from_slice(&bytes[1..1 + ADDR_LEN]);
        let mut sembol = [0u8; SYMBOL_LEN];
        sembol.copy_from_slice(&bytes[1 + ADDR_LEN..TOKEN_ENCODED_LEN]);
        Ok(TokenKaydi {
            kanonik_adres,
            sembol,
        })
    }
}

/// TAKLIT TESPITI (Kalkanli DEX'in kalbi): iki kayit AYNI sembolu tasiyor ama
/// FARKLI adresteyse, gelen bir TAKLITTIR. "USDC gorunumlu ama sahte adresli"
/// dolandiriciligi tam olarak budur. Ayni adres (gercek esit) -> taklit DEGIL.
pub fn ayni_sembol_farkli_adres(kayitli: &TokenKaydi, gelen: &TokenKaydi) -> bool {
    kayitli.sembol == gelen.sembol && kayitli.kanonik_adres != gelen.kanonik_adres
}

/// Stake (teminat) kaydi tip kimligi (Record=1, Token=2'nin yaninda =3).
pub const TX_TYPE_STAKE: u8 = 3;

/// Transfer (odeme): gonderen=imzalayan, payload=alici+miktar.
pub const TX_TYPE_TRANSFER: u8 = 4;

/// Kurum/firma kimlik kaydi: kaydeden=imzalayan, payload=kategori+ad.
pub const TX_TYPE_KURUM: u8 = 5;

/// Faucet (TESTNET test AIDAG basimi): SADECE owner imzalarsa gecerli.
/// Owner kontrolu ingest'te (imzalayan == faucet owner mi). payload=alici+miktar.
pub const TX_TYPE_FAUCET: u8 = 6;

/// LSC transfer (yakit/gas coini odemesi). AIDAG transferiyle AYNI veri
/// (alici+miktar) ama AYRI defter (lsc_registry). tip=7. AIDAG transferine dokunmaz.
pub const TX_TYPE_LSC_TRANSFER: u8 = 7;

/// Testnet eslestirme: test -> gercek odul adresi. Zincire yazilir (kalici).
pub const TX_TYPE_ESLESTIRME: u8 = 8;
/// tip=9: AVM cagrisi (EVM uzerinden LSC deger transferi). Kopru 4.
pub const TX_TYPE_AVM_CAGRI: u8 = 9;
pub const TX_TYPE_ON_SATIS: u8 = 10;
pub const ESLESTIRME_ENCODED_LEN: usize = 1 + ADDR_LEN + ADDR_LEN;

/// Kurum adi azami uzunluk (spam/DoS korumasi).
const KURUM_AD_MAX: usize = 64;

/// Kodlanmis stake kaydi: 1 (tip) + 20 (staker adresi) + 8 (miktar u64) = 29 bayt.
const STAKE_ENCODED_LEN: usize = 1 + ADDR_LEN + 16; // u128 = 16 bayt

/// Stake (teminat) kaydi: bir adres, belirli miktar AIDAG'i KILITLER (teminat).
/// KALKAN bagi: sadece stake etmis adresler kanonik token kaydedebilir ->
/// "kanonik adresi kim belirliyor?" sorusunun cevabi = TEMINAT yatiranlar.
/// Bedavaya kayit YOK; once AIDAG kilitlenir (durustluk tesvigi).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StakeKaydi {
    /// Stake eden adres (teminati yatiran).
    pub staker: [u8; ADDR_LEN],
    /// Kilitlenen AIDAG miktari (teminat). Buyuk-endian u64 kodlanir.
    pub miktar: crate::registry::Tutar,
}

impl StakeKaydi {
    /// Yeni stake kaydi.
    pub fn new(staker: [u8; ADDR_LEN], miktar: crate::registry::Tutar) -> Self {
        StakeKaydi { staker, miktar }
    }

    /// Kodla: `[TX_TYPE_STAKE][staker:20][miktar:8]` -> 29 bayt.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(STAKE_ENCODED_LEN);
        out.push(TX_TYPE_STAKE);
        out.extend_from_slice(&self.staker);
        out.extend_from_slice(&self.miktar.to_be_bytes());
        out
    }

    /// Coz: tip + boyut dogrula, alanlari cikar (TokenKaydi ile ayni sertlik).
    pub fn decode(bytes: &[u8]) -> Result<StakeKaydi, TxError> {
        let &first = bytes.first().ok_or(TxError::Empty)?;
        if first != TX_TYPE_STAKE {
            return Err(TxError::UnknownType(first));
        }
        if bytes.len() != STAKE_ENCODED_LEN {
            return Err(TxError::BadLength {
                expected: STAKE_ENCODED_LEN,
                got: bytes.len(),
            });
        }
        let mut staker = [0u8; ADDR_LEN];
        staker.copy_from_slice(&bytes[1..1 + ADDR_LEN]);
        let mut miktar_bytes = [0u8; 16];
        miktar_bytes.copy_from_slice(&bytes[1 + ADDR_LEN..STAKE_ENCODED_LEN]);
        let miktar = u128::from_be_bytes(miktar_bytes);
        Ok(StakeKaydi { staker, miktar })
    }
}

/// Kodlanmis transfer kaydi: 1 (tip) + 20 (alici) + 8 (miktar) = 29 bayt.
const TRANSFER_ENCODED_LEN: usize = 1 + ADDR_LEN + 16 + 8; // miktar u128(16) + nonce u64(8)

/// Transfer (odeme) kaydi: GONDEREN = vertex'i imzalayan (payload'da YOK;
/// public_key'den turetilir -> "baskasi adina transfer" IMKANSIZ, imza sahte
/// olamaz). Payload sadece ALICI + MIKTAR tasir. (Kalkan'daki "kaydeden=imzalayan"
/// deseniyle ayni guvenlik mantigi.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferKaydi {
    /// Alici adres (parayi alan).
    pub alici: [u8; ADDR_LEN],
    /// Transfer edilen AIDAG miktari. Buyuk-endian u64 (StakeKaydi ile tutarli).
    pub miktar: crate::registry::Tutar,
    /// Replay korumasi: gonderenin BEKLENEN nonce'u. Payload'a dahil -> id'ye
    /// -> imzaya dahil (kurcalaninca vertex verify() ile reddedilir).
    pub nonce: u64,
}

impl TransferKaydi {
    pub fn new(alici: [u8; ADDR_LEN], miktar: crate::registry::Tutar, nonce: u64) -> Self {
        TransferKaydi {
            alici,
            miktar,
            nonce,
        }
    }

    /// Kodla: `[TX_TYPE_TRANSFER][alici:20][miktar:8 BE][nonce:8 BE]` -> 37 bayt.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(TRANSFER_ENCODED_LEN);
        out.push(TX_TYPE_TRANSFER);
        out.extend_from_slice(&self.alici);
        out.extend_from_slice(&self.miktar.to_be_bytes());
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }

    /// Coz: tip + boyut dogrula, alanlari cikar (StakeKaydi ile ayni sertlik).
    pub fn decode(bytes: &[u8]) -> Result<TransferKaydi, TxError> {
        let &first = bytes.first().ok_or(TxError::Empty)?;
        if first != TX_TYPE_TRANSFER {
            return Err(TxError::UnknownType(first));
        }
        if bytes.len() != TRANSFER_ENCODED_LEN {
            return Err(TxError::BadLength {
                expected: TRANSFER_ENCODED_LEN,
                got: bytes.len(),
            });
        }
        let mut alici = [0u8; ADDR_LEN];
        alici.copy_from_slice(&bytes[1..1 + ADDR_LEN]);
        let mut miktar_bytes = [0u8; 16];
        miktar_bytes.copy_from_slice(&bytes[1 + ADDR_LEN..1 + ADDR_LEN + 16]);
        let miktar = u128::from_be_bytes(miktar_bytes);
        let mut nonce_bytes = [0u8; 8];
        nonce_bytes.copy_from_slice(&bytes[1 + ADDR_LEN + 16..TRANSFER_ENCODED_LEN]);
        let nonce = u64::from_be_bytes(nonce_bytes);
        Ok(TransferKaydi {
            alici,
            miktar,
            nonce,
        })
    }
}

/// LSC transfer kaydi. AIDAG'in TransferKaydi'siyle AYNI veri (alici+miktar)
/// ama tip=7 ile kodlanir ve LSC defterine (lsc_registry) islenir.
/// AIDAG transferine (tip=4) HIC dokunmaz; ayri/paralel yapi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LscTransferKaydi {
    pub alici: [u8; ADDR_LEN],
    pub miktar: crate::registry::Tutar,
    pub nonce: u64,
}

impl LscTransferKaydi {
    pub fn new(alici: [u8; ADDR_LEN], miktar: crate::registry::Tutar, nonce: u64) -> Self {
        LscTransferKaydi {
            alici,
            miktar,
            nonce,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(TRANSFER_ENCODED_LEN);
        out.push(TX_TYPE_LSC_TRANSFER);
        out.extend_from_slice(&self.alici);
        out.extend_from_slice(&self.miktar.to_be_bytes());
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<LscTransferKaydi, TxError> {
        let &first = bytes.first().ok_or(TxError::Empty)?;
        if first != TX_TYPE_LSC_TRANSFER {
            return Err(TxError::UnknownType(first));
        }
        if bytes.len() != TRANSFER_ENCODED_LEN {
            return Err(TxError::BadLength {
                expected: TRANSFER_ENCODED_LEN,
                got: bytes.len(),
            });
        }
        let mut alici = [0u8; ADDR_LEN];
        alici.copy_from_slice(&bytes[1..1 + ADDR_LEN]);
        let mut miktar_bytes = [0u8; 16];
        miktar_bytes.copy_from_slice(&bytes[1 + ADDR_LEN..1 + ADDR_LEN + 16]);
        let miktar = u128::from_be_bytes(miktar_bytes);
        let mut nonce_bytes = [0u8; 8];
        nonce_bytes.copy_from_slice(&bytes[1 + ADDR_LEN + 16..TRANSFER_ENCODED_LEN]);
        let nonce = u64::from_be_bytes(nonce_bytes);
        Ok(LscTransferKaydi {
            alici,
            miktar,
            nonce,
        })
    }
}

/// Kurum/firma kimlik kaydi (zincire yazilan islem). KAYDEDEN = imzalayan
/// (payload'da YOK; vertex public_key'den turetilir -> baskasi adina kurum
/// kaydi IMKANSIZ). Payload: [5][kategori:1][ad:degisken].
///   kategori bayti: 0 = Devlet, 1 = Ozel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KurumKaydiTx {
    /// 0 = Devlet, 1 = Ozel.
    pub kategori: u8,
    /// Kurum/firma adi (UTF-8, azami KURUM_AD_MAX bayt).
    pub ad: String,
}

impl KurumKaydiTx {
    pub fn new(kategori: u8, ad: String) -> Self {
        KurumKaydiTx { kategori, ad }
    }

    /// Kodla: [TX_TYPE_KURUM][kategori:1][ad baytlari].
    pub fn encode(&self) -> Vec<u8> {
        let ad_bytes = self.ad.as_bytes();
        let mut out = Vec::with_capacity(2 + ad_bytes.len());
        out.push(TX_TYPE_KURUM);
        out.push(self.kategori);
        out.extend_from_slice(ad_bytes);
        out
    }

    /// Coz: tip + kategori + ad. Bilinmeyen tip / asiri uzun ad / gecersiz
    /// kategori / bozuk UTF-8 REDDEDILIR (tavizsiz saglamlik).
    pub fn decode(bytes: &[u8]) -> Result<KurumKaydiTx, TxError> {
        let &first = bytes.first().ok_or(TxError::Empty)?;
        if first != TX_TYPE_KURUM {
            return Err(TxError::UnknownType(first));
        }
        // En az tip(1)+kategori(1) = 2 bayt; ad bos olabilir ama makul degil,
        // yine de >=2 sart. Ust sinir: 2 + KURUM_AD_MAX.
        if bytes.len() < 2 || bytes.len() > 2 + KURUM_AD_MAX {
            return Err(TxError::BadLength {
                expected: 2 + KURUM_AD_MAX,
                got: bytes.len(),
            });
        }
        let kategori = bytes[1];
        if kategori > 1 {
            // Sadece 0 (Devlet) / 1 (Ozel) gecerli.
            return Err(TxError::UnknownType(kategori));
        }
        let ad = match std::str::from_utf8(&bytes[2..]) {
            Ok(s) => s.to_string(),
            Err(_) => return Err(TxError::UnknownType(0xFE)), // bozuk UTF-8
        };
        Ok(KurumKaydiTx { kategori, ad })
    }
}

/// Faucet kaydi (TESTNET): owner'in bir adrese test AIDAG basmasi.
/// Payload: [6][alici:20][miktar:8 BE]. GUVENLIK: bu payload tek basina yetki
/// VERMEZ; ingest, vertex'i imzalayanin FAUCET OWNER olup olmadigini dogrular
/// (owner degilse REDDEDILIR). Owner ayarli degilse faucet tamamen kapali.
/// Test AIDAG'in gercek degeri yoktur; mainnet'te owner kaldirilarak kapatilir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaucetKaydi {
    pub alici: [u8; 20],
    pub miktar: crate::registry::Tutar,
}

impl FaucetKaydi {
    pub fn new(alici: [u8; 20], miktar: crate::registry::Tutar) -> Self {
        FaucetKaydi { alici, miktar }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(29);
        out.push(TX_TYPE_FAUCET);
        out.extend_from_slice(&self.alici);
        out.extend_from_slice(&self.miktar.to_be_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<FaucetKaydi, TxError> {
        let &first = bytes.first().ok_or(TxError::Empty)?;
        if first != TX_TYPE_FAUCET {
            return Err(TxError::UnknownType(first));
        }
        if bytes.len() != 37 {
            return Err(TxError::BadLength {
                expected: 29,
                got: bytes.len(),
            });
        }
        let mut alici = [0u8; 20];
        alici.copy_from_slice(&bytes[1..21]);
        let mut miktar_bytes = [0u8; 16];
        miktar_bytes.copy_from_slice(&bytes[21..37]);
        Ok(FaucetKaydi {
            alici,
            miktar: u128::from_be_bytes(miktar_bytes),
        })
    }
}

/// AVM cagrisi (Kopru 4). EVM uzerinden LSC deger transferi.
/// GONDEREN = vertex imzalayani (signer'dan turetilir); payload'da YOK.
/// Format: [TX_TYPE_AVM_CAGRI][hedef:20][deger:8 BE][nonce:8 BE] -> 37 bayt.
/// deger = LSC (EVM native). nonce = replay korumasi (transfer ile ayni).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvmCagri {
    pub hedef: [u8; ADDR_LEN],
    pub deger: crate::registry::Tutar,
    pub nonce: u64,
    /// AVM verisi: hedef=sifir adres ise CREATE bytecode; hedef dolu ise CALL calldata.
    /// Bos ise basit LSC deger transferi (geriye uyumlu davranis).
    pub data: Vec<u8>,
}

/// Sabit kisim (data haric): [tip][hedef][deger][nonce][data_len:4] = 41 bayt.
const AVM_CAGRI_SABIT_LEN: usize = 1 + ADDR_LEN + 16 + 8 + 4; // deger u128(16)+nonce(8)+len(4) = 49

impl AvmCagri {
    pub fn new(
        hedef: [u8; ADDR_LEN],
        deger: crate::registry::Tutar,
        nonce: u64,
        data: Vec<u8>,
    ) -> Self {
        AvmCagri {
            hedef,
            deger,
            nonce,
            data,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(AVM_CAGRI_SABIT_LEN + self.data.len());
        out.push(TX_TYPE_AVM_CAGRI);
        out.extend_from_slice(&self.hedef);
        out.extend_from_slice(&self.deger.to_be_bytes());
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&(self.data.len() as u32).to_be_bytes());
        out.extend_from_slice(&self.data);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<AvmCagri, TxError> {
        let &first = bytes.first().ok_or(TxError::Empty)?;
        if first != TX_TYPE_AVM_CAGRI {
            return Err(TxError::UnknownType(first));
        }
        if bytes.len() < AVM_CAGRI_SABIT_LEN {
            return Err(TxError::BadLength {
                expected: AVM_CAGRI_SABIT_LEN,
                got: bytes.len(),
            });
        }
        let mut hedef = [0u8; ADDR_LEN];
        hedef.copy_from_slice(&bytes[1..1 + ADDR_LEN]);
        let mut deger_bytes = [0u8; 16];
        deger_bytes.copy_from_slice(&bytes[1 + ADDR_LEN..1 + ADDR_LEN + 16]);
        let deger = u128::from_be_bytes(deger_bytes);
        let mut nonce_bytes = [0u8; 8];
        nonce_bytes.copy_from_slice(&bytes[1 + ADDR_LEN + 16..1 + ADDR_LEN + 16 + 8]);
        let nonce = u64::from_be_bytes(nonce_bytes);
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&bytes[1 + ADDR_LEN + 16 + 8..AVM_CAGRI_SABIT_LEN]);
        let data_len = u32::from_be_bytes(len_bytes) as usize;
        if bytes.len() != AVM_CAGRI_SABIT_LEN + data_len {
            return Err(TxError::BadLength {
                expected: AVM_CAGRI_SABIT_LEN + data_len,
                got: bytes.len(),
            });
        }
        let data = bytes[AVM_CAGRI_SABIT_LEN..].to_vec();
        Ok(AvmCagri {
            hedef,
            deger,
            nonce,
            data,
        })
    }
}

/// ON SATIS DAGITIM (tip=10). SADECE owner cagirir. Hazineden AIDAG + LSC hediye.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnSatisDagitim {
    pub alici: [u8; ADDR_LEN],
    pub aidag: crate::registry::Tutar,
    pub lsc_hediye: crate::registry::Tutar,
    pub odeme_ref: u64,
}
const ON_SATIS_LEN: usize = 1 + ADDR_LEN + 16 + 16 + 8; // aidag(16)+lsc_hediye(16)+odeme_ref(8)

impl OnSatisDagitim {
    pub fn new(
        alici: [u8; ADDR_LEN],
        aidag: crate::registry::Tutar,
        lsc_hediye: crate::registry::Tutar,
        odeme_ref: u64,
    ) -> Self {
        OnSatisDagitim {
            alici,
            aidag,
            lsc_hediye,
            odeme_ref,
        }
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(ON_SATIS_LEN);
        out.push(TX_TYPE_ON_SATIS);
        out.extend_from_slice(&self.alici);
        out.extend_from_slice(&self.aidag.to_be_bytes());
        out.extend_from_slice(&self.lsc_hediye.to_be_bytes());
        out.extend_from_slice(&self.odeme_ref.to_be_bytes());
        out
    }
    pub fn decode(bytes: &[u8]) -> Result<OnSatisDagitim, TxError> {
        let &first = bytes.first().ok_or(TxError::Empty)?;
        if first != TX_TYPE_ON_SATIS {
            return Err(TxError::UnknownType(first));
        }
        if bytes.len() != ON_SATIS_LEN {
            return Err(TxError::BadLength {
                expected: ON_SATIS_LEN,
                got: bytes.len(),
            });
        }
        let mut alici = [0u8; ADDR_LEN];
        alici.copy_from_slice(&bytes[1..1 + ADDR_LEN]);
        let mut b16 = [0u8; 16];
        b16.copy_from_slice(&bytes[1 + ADDR_LEN..1 + ADDR_LEN + 16]);
        let aidag = u128::from_be_bytes(b16);
        b16.copy_from_slice(&bytes[1 + ADDR_LEN + 16..1 + ADDR_LEN + 32]);
        let lsc_hediye = u128::from_be_bytes(b16);
        let mut b8 = [0u8; 8];
        b8.copy_from_slice(&bytes[1 + ADDR_LEN + 32..ON_SATIS_LEN]);
        let odeme_ref = u64::from_be_bytes(b8);
        Ok(OnSatisDagitim {
            alici,
            aidag,
            lsc_hediye,
            odeme_ref,
        })
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn avm_cagri_encode_decode_roundtrip() {
        // data'li ve data'siz iki durumu da test et
        let c = AvmCagri::new([0x7A; 20], 4242, 7, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let bytes = c.encode();
        assert_eq!(bytes.len(), AVM_CAGRI_SABIT_LEN + 4, "41 + data uzunlugu");
        assert_eq!(bytes[0], TX_TYPE_AVM_CAGRI, "ilk bayt tip=9");
        let geri = AvmCagri::decode(&bytes).expect("decode basarili");
        assert_eq!(geri, c, "round-trip ayni olmali");
        assert_eq!(geri.hedef, [0x7A; 20]);
        assert_eq!(geri.deger, 4242);
        assert_eq!(geri.nonce, 7);
        assert_eq!(geri.data, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        // bos data da calismali
        let bos = AvmCagri::new([1; 20], 5, 0, vec![]);
        assert_eq!(
            bos.encode().len(),
            AVM_CAGRI_SABIT_LEN,
            "data bos -> 41 bayt"
        );
        assert_eq!(AvmCagri::decode(&bos.encode()).unwrap(), bos);
    }

    #[test]
    fn avm_cagri_yanlis_uzunluk_reddedilir() {
        // data_len=0 diyen ama fazladan bayt iceren -> bozuk
        let mut bytes = AvmCagri::new([1; 20], 100, 0, vec![]).encode();
        bytes.push(0xFF); // data_len=0 ama 1 fazla bayt -> uzunluk uyusmaz
        assert!(
            AvmCagri::decode(&bytes).is_err(),
            "yanlis uzunluk reddedilmeli"
        );
        // cok kisa (sabit kisimdan kucuk) -> bozuk
        assert!(
            AvmCagri::decode(&[9u8; 10]).is_err(),
            "cok kisa reddedilmeli"
        );
    }
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let r = Record::new([7u8; 32]);
        let bytes = r.encode();
        assert_eq!(bytes.len(), RECORD_ENCODED_LEN);
        assert_eq!(bytes[0], TX_TYPE_RECORD);
        let back = Record::decode(&bytes).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn decode_rejects_unknown_type() {
        let mut bytes = Record::new([1u8; 32]).encode();
        bytes[0] = 99; // bilinmeyen tip
        assert_eq!(Record::decode(&bytes), Err(TxError::UnknownType(99)));
    }

    #[test]
    fn decode_rejects_bad_length() {
        let mut bytes = Record::new([1u8; 32]).encode();
        bytes.push(0); // fazladan bayt -> 34
        assert!(matches!(
            Record::decode(&bytes),
            Err(TxError::BadLength { .. })
        ));
    }

    #[test]
    fn decode_rejects_short() {
        let bytes = vec![TX_TYPE_RECORD, 1, 2, 3]; // tip dogru ama kisa
        assert!(matches!(
            Record::decode(&bytes),
            Err(TxError::BadLength {
                expected: 33,
                got: 4
            })
        ));
    }

    #[test]
    fn decode_rejects_empty() {
        assert_eq!(Record::decode(&[]), Err(TxError::Empty));
    }

    #[test]
    fn different_hash_different_encoding() {
        let a = Record::new([1u8; 32]).encode();
        let b = Record::new([2u8; 32]).encode();
        assert_ne!(a, b);
    }

    // ---- tip=2: TokenKaydi + taklit tespiti testleri ----

    fn sym(s: &str) -> [u8; 8] {
        let mut out = [0u8; 8];
        let b = s.as_bytes();
        out[..b.len()].copy_from_slice(b);
        out
    }

    #[test]
    fn token_encode_decode_roundtrip() {
        let t = TokenKaydi::new([0x11; 20], sym("USDC"));
        let bytes = t.encode();
        assert_eq!(bytes.len(), TOKEN_ENCODED_LEN); // 29
        assert_eq!(bytes[0], TX_TYPE_TOKEN);
        let back = TokenKaydi::decode(&bytes).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn token_decode_rejects_unknown_type() {
        let mut bytes = TokenKaydi::new([1; 20], sym("USDC")).encode();
        bytes[0] = 99;
        assert_eq!(TokenKaydi::decode(&bytes), Err(TxError::UnknownType(99)));
    }

    #[test]
    fn token_decode_rejects_bad_length() {
        let mut bytes = TokenKaydi::new([1; 20], sym("USDC")).encode();
        bytes.push(0); // 30 -> gecersiz
        assert!(matches!(
            TokenKaydi::decode(&bytes),
            Err(TxError::BadLength { expected: 29, .. })
        ));
    }

    #[test]
    fn token_record_tipleri_karismaz() {
        // Record (tip=1) baytlari TokenKaydi::decode'a verilince reddedilmeli
        let record_bytes = Record::new([5u8; 32]).encode();
        assert_eq!(
            TokenKaydi::decode(&record_bytes),
            Err(TxError::UnknownType(TX_TYPE_RECORD))
        );
    }

    // --- TAKLIT TESPITI (Kalkanli DEX kalbi) ---

    #[test]
    fn taklit_yakalanir_ayni_sembol_farkli_adres() {
        // Gercek USDC: adres 0xAA...
        let gercek = TokenKaydi::new([0xAA; 20], sym("USDC"));
        // Sahte USDC: AYNI sembol "USDC" AMA farkli adres 0xBB...
        let sahte = TokenKaydi::new([0xBB; 20], sym("USDC"));
        assert!(
            ayni_sembol_farkli_adres(&gercek, &sahte),
            "ayni sembol + farkli adres TAKLIT olarak yakalanmali"
        );
    }

    #[test]
    fn gercek_token_taklit_sayilmaz() {
        // Ayni sembol + AYNI adres = gercek esit, taklit DEGIL
        let a = TokenKaydi::new([0xAA; 20], sym("USDC"));
        let b = TokenKaydi::new([0xAA; 20], sym("USDC"));
        assert!(!ayni_sembol_farkli_adres(&a, &b));
    }

    #[test]
    fn farkli_sembol_taklit_sayilmaz() {
        // Farkli sembol (USDC vs DAI) -> ayni-isim taklidi DEGIL
        let usdc = TokenKaydi::new([0xAA; 20], sym("USDC"));
        let dai = TokenKaydi::new([0xBB; 20], sym("DAI"));
        assert!(!ayni_sembol_farkli_adres(&usdc, &dai));
    }

    // ===== tip=3: StakeKaydi testleri (TokenKaydi ile ayni sertlik) =====

    #[test]
    fn stake_encode_decode_roundtrip() {
        let s = StakeKaydi::new([0x11; 20], 1000);
        let bytes = s.encode();
        assert_eq!(bytes.len(), STAKE_ENCODED_LEN); // 1+20+16 = 37 (u128 miktar)
        assert_eq!(bytes[0], TX_TYPE_STAKE);
        let back = StakeKaydi::decode(&bytes).unwrap();
        assert_eq!(back, s);
        assert_eq!(back.miktar, 1000);
    }

    #[test]
    fn stake_yanlis_tip_reddedilir() {
        let mut bytes = StakeKaydi::new([1; 20], 50).encode();
        bytes[0] = 99;
        assert_eq!(StakeKaydi::decode(&bytes), Err(TxError::UnknownType(99)));
    }

    #[test]
    fn stake_kisa_bayt_reddedilir() {
        let mut bytes = StakeKaydi::new([1; 20], 50).encode();
        bytes.pop();
        assert!(matches!(
            StakeKaydi::decode(&bytes),
            Err(TxError::BadLength { .. })
        ));
    }

    #[test]
    fn stake_token_baytlarini_reddeder() {
        // tip=2 (Token) baytlari StakeKaydi::decode'a verilince reddedilmeli
        let token_bytes = TokenKaydi::new([1; 20], [b'X'; 8]).encode();
        assert_eq!(
            StakeKaydi::decode(&token_bytes),
            Err(TxError::UnknownType(TX_TYPE_TOKEN))
        );
    }

    #[test]
    fn stake_miktar_buyuk_deger_korunur() {
        // u128: 18 ondalik + buyuk arz icin gereken buyuk degerler korunur.
        let s = StakeKaydi::new([0xAB; 20], u128::MAX);
        let back = StakeKaydi::decode(&s.encode()).unwrap();
        assert_eq!(back.miktar, u128::MAX);
    }

    // ===== TransferKaydi testleri =====

    #[test]
    fn transfer_encode_decode_roundtrip() {
        let t = TransferKaydi::new([0xCD; 20], 12345, 0);
        let bytes = t.encode();
        assert_eq!(bytes.len(), TRANSFER_ENCODED_LEN);
        assert_eq!(bytes[0], TX_TYPE_TRANSFER);
        let back = TransferKaydi::decode(&bytes).unwrap();
        assert_eq!(back, t);
        assert_eq!(back.alici, [0xCD; 20]);
        assert_eq!(back.miktar, 12345);
    }

    #[test]
    fn transfer_miktar_buyuk_deger_korunur() {
        let t = TransferKaydi::new([0x11; 20], u128::MAX, 0);
        let back = TransferKaydi::decode(&t.encode()).unwrap();
        assert_eq!(back.miktar, u128::MAX);
    }

    #[test]
    fn transfer_yanlis_tip_reddedilir() {
        // stake (tip=3) baytlari TransferKaydi::decode'a verilince reddedilmeli.
        let stake_bytes = StakeKaydi::new([1; 20], 100).encode();
        assert_eq!(
            TransferKaydi::decode(&stake_bytes),
            Err(TxError::UnknownType(TX_TYPE_STAKE))
        );
    }

    #[test]
    fn transfer_bozuk_uzunluk_reddedilir() {
        let mut bytes = TransferKaydi::new([1; 20], 100, 0).encode();
        bytes.push(0xFF); // fazladan bayt
        assert!(matches!(
            TransferKaydi::decode(&bytes),
            Err(TxError::BadLength { .. })
        ));
    }

    // ===== KurumKaydiTx testleri =====

    #[test]
    fn kurum_encode_decode_roundtrip() {
        let k = KurumKaydiTx::new(0, "Tapu Mudurlugu".into());
        let bytes = k.encode();
        assert_eq!(bytes[0], TX_TYPE_KURUM);
        assert_eq!(bytes[1], 0); // Devlet
        let back = KurumKaydiTx::decode(&bytes).unwrap();
        assert_eq!(back, k);
        assert_eq!(back.kategori, 0);
        assert_eq!(back.ad, "Tapu Mudurlugu");
    }

    #[test]
    fn kurum_ozel_kategori() {
        let k = KurumKaydiTx::new(1, "Ahmet Insaat Ltd".into());
        let back = KurumKaydiTx::decode(&k.encode()).unwrap();
        assert_eq!(back.kategori, 1); // Ozel
        assert_eq!(back.ad, "Ahmet Insaat Ltd");
    }

    #[test]
    fn kurum_gecersiz_kategori_reddedilir() {
        // kategori > 1 gecersiz (sadece 0=Devlet, 1=Ozel).
        let mut bytes = KurumKaydiTx::new(0, "X".into()).encode();
        bytes[1] = 5; // gecersiz kategori
        assert!(KurumKaydiTx::decode(&bytes).is_err());
    }

    #[test]
    fn kurum_asiri_uzun_ad_reddedilir() {
        // 64 bayttan uzun ad -> reddedilmeli (spam/DoS korumasi).
        let uzun = "A".repeat(100);
        let bytes = KurumKaydiTx::new(0, uzun).encode();
        assert!(matches!(
            KurumKaydiTx::decode(&bytes),
            Err(TxError::BadLength { .. })
        ));
    }

    #[test]
    fn kurum_yanlis_tip_reddedilir() {
        let stake_bytes = StakeKaydi::new([1; 20], 100).encode();
        assert_eq!(
            KurumKaydiTx::decode(&stake_bytes),
            Err(TxError::UnknownType(TX_TYPE_STAKE))
        );
    }

    // ===== FaucetKaydi testleri =====

    #[test]
    fn faucet_encode_decode_roundtrip() {
        let f = FaucetKaydi::new([0x33; 20], 1000);
        let bytes = f.encode();
        assert_eq!(bytes.len(), 37); // 1+20+16 (u128 miktar)
        assert_eq!(bytes[0], TX_TYPE_FAUCET);
        let back = FaucetKaydi::decode(&bytes).unwrap();
        assert_eq!(back, f);
        assert_eq!(back.alici, [0x33; 20]);
        assert_eq!(back.miktar, 1000);
    }

    #[test]
    fn faucet_yanlis_tip_reddedilir() {
        let transfer_bytes = TransferKaydi::new([1; 20], 100, 0).encode();
        assert_eq!(
            FaucetKaydi::decode(&transfer_bytes),
            Err(TxError::UnknownType(TX_TYPE_TRANSFER))
        );
    }

    #[test]
    fn faucet_bozuk_uzunluk_reddedilir() {
        let mut bytes = FaucetKaydi::new([1; 20], 100).encode();
        bytes.push(0xFF);
        assert!(matches!(
            FaucetKaydi::decode(&bytes),
            Err(TxError::BadLength { .. })
        ));
    }
}

/// Eslestirme kaydi: test adresini gercek odul adresine baglar (zincire yazilir).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EslestirmeKaydi {
    pub test_adresi: [u8; ADDR_LEN],
    pub gercek_adres: [u8; ADDR_LEN],
}

impl EslestirmeKaydi {
    pub fn new(test_adresi: [u8; ADDR_LEN], gercek_adres: [u8; ADDR_LEN]) -> Self {
        EslestirmeKaydi {
            test_adresi,
            gercek_adres,
        }
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(ESLESTIRME_ENCODED_LEN);
        out.push(TX_TYPE_ESLESTIRME);
        out.extend_from_slice(&self.test_adresi);
        out.extend_from_slice(&self.gercek_adres);
        out
    }
    pub fn decode(bytes: &[u8]) -> Result<EslestirmeKaydi, TxError> {
        let &first = bytes.first().ok_or(TxError::Empty)?;
        if first != TX_TYPE_ESLESTIRME {
            return Err(TxError::UnknownType(first));
        }
        if bytes.len() != ESLESTIRME_ENCODED_LEN {
            return Err(TxError::BadLength {
                expected: ESLESTIRME_ENCODED_LEN,
                got: bytes.len(),
            });
        }
        let mut test_adresi = [0u8; ADDR_LEN];
        test_adresi.copy_from_slice(&bytes[1..1 + ADDR_LEN]);
        let mut gercek_adres = [0u8; ADDR_LEN];
        gercek_adres.copy_from_slice(&bytes[1 + ADDR_LEN..ESLESTIRME_ENCODED_LEN]);
        Ok(EslestirmeKaydi {
            test_adresi,
            gercek_adres,
        })
    }
}

// ============================================================================
// tip=11: EVM-UYUMLU TRANSFER (secp256k1 imzali — MetaMask/Trust/Ledger vb.)
// ----------------------------------------------------------------------------
// Mevcut Transfer (tip=4) ed25519 imzalayani kullanir. Bu kardes tip, secp256k1
// (Ethereum standardi) ile imzalanmis transferi tasir. GONDEREN PAYLOAD'DA
// TASINMAZ — ecrecover ile imzadan kurtarilir (Secenek B, POC 4 ile kanitli).
//
// Imzalanan mesaj = [alici:20][miktar:8] (transferin ozu, keccak256 ile).
// Bu DILIM yalniz COZME + gonderen adres cikarma yapar; bakiye/transfer YOK.
// ============================================================================

/// EVM-uyumlu transfer tip kimligi (Record=1..Eslestirme=8, AVM=9, OnSatis=10'un yaninda =11).
pub const TX_TYPE_EVM_TRANSFER: u8 = 11;
/// tip=12: HAM ETHEREUM TX (eth_sendRawTransaction icin). Payload = RLP-kodlu
/// imzali eth tx. GONDEREN vertex imzasindan DEGIL, eth tx'in secp256k1
/// imzasindan gelir (ham_eth_tx_coz ile). Vertex imzasi sadece paketleme/tasima.
/// Format: [12][ham_tx_bytes...]. Node bunu ham_eth_tx_isle ile calistirir.
pub const TX_TYPE_HAM_ETH_TX: u8 = 12;

/// tip=12 payload olustur: [12] + ham eth tx bytes.
pub fn ham_eth_tx_payload(raw_eth_tx: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + raw_eth_tx.len());
    out.push(TX_TYPE_HAM_ETH_TX);
    out.extend_from_slice(raw_eth_tx);
    out
}

/// tip=12 payload'dan ham eth tx bytes cikar (ilk bayti at).
pub fn ham_eth_tx_coz_payload(payload: &[u8]) -> Option<&[u8]> {
    if payload.first() == Some(&TX_TYPE_HAM_ETH_TX) && payload.len() > 1 {
        Some(&payload[1..])
    } else {
        None
    }
}

/// Kodlanmis EVM transfer:
/// [tip:1][alici:20][miktar:8][recovery_id:1][imza:64] = 94 bayt.
const EVM_TRANSFER_ENCODED_LEN: usize = 1 + ADDR_LEN + 16 + 8 + 1 + 64; // miktar u128(16) + nonce(8) + recid(1) + imza(64)

/// secp256k1 imzali transfer islemi (cozulmus hali).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmTransfer {
    /// Alici adresi (20 bayt, Ethereum 0x adres formati).
    pub alici: [u8; ADDR_LEN],
    /// Transfer miktari.
    pub miktar: crate::registry::Tutar,
    /// Replay korumasi: gonderenin beklenen nonce'u (kendi nonce sistemimiz).
    pub nonce: u64,
    /// ecrecover recovery id (0 veya 1).
    pub recovery_id: u8,
    /// secp256k1 imza (64 bayt, r||s).
    pub imza: [u8; 64],
}

/// EVM transferinde IMZALANAN mesaj: [alici:20][miktar:8] (transferin ozu).
/// Gonderen bu mesaji secp256k1 ile imzalar; biz ecrecover ile gondereni buluruz.
pub fn evm_transfer_mesaji(
    alici: &[u8; ADDR_LEN],
    miktar: crate::registry::Tutar,
    nonce: u64,
) -> Vec<u8> {
    let mut m = Vec::with_capacity(ADDR_LEN + 16 + 8);
    m.extend_from_slice(alici);
    m.extend_from_slice(&miktar.to_be_bytes());
    m.extend_from_slice(&nonce.to_be_bytes());
    m
}

impl EvmTransfer {
    /// Kodla: [TX_TYPE_EVM_TRANSFER][alici:20][miktar:8][recovery_id:1][imza:64].
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(EVM_TRANSFER_ENCODED_LEN);
        out.push(TX_TYPE_EVM_TRANSFER);
        out.extend_from_slice(&self.alici);
        out.extend_from_slice(&self.miktar.to_be_bytes());
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.push(self.recovery_id);
        out.extend_from_slice(&self.imza);
        out
    }

    /// Coz: tip + boyut dogrula (genisleme kapisinin SARTI), alanlari cikar.
    pub fn decode(bytes: &[u8]) -> Result<EvmTransfer, TxError> {
        let &first = bytes.first().ok_or(TxError::Empty)?;
        if first != TX_TYPE_EVM_TRANSFER {
            return Err(TxError::UnknownType(first));
        }
        if bytes.len() != EVM_TRANSFER_ENCODED_LEN {
            return Err(TxError::BadLength {
                expected: EVM_TRANSFER_ENCODED_LEN,
                got: bytes.len(),
            });
        }
        let mut alici = [0u8; ADDR_LEN];
        alici.copy_from_slice(&bytes[1..1 + ADDR_LEN]);
        let mut miktar_bytes = [0u8; 16];
        miktar_bytes.copy_from_slice(&bytes[1 + ADDR_LEN..1 + ADDR_LEN + 16]);
        let miktar = u128::from_be_bytes(miktar_bytes);
        let mut nonce_bytes = [0u8; 8];
        nonce_bytes.copy_from_slice(&bytes[1 + ADDR_LEN + 16..1 + ADDR_LEN + 16 + 8]);
        let nonce = u64::from_be_bytes(nonce_bytes);
        let recovery_id = bytes[1 + ADDR_LEN + 16 + 8];
        let mut imza = [0u8; 64];
        imza.copy_from_slice(&bytes[1 + ADDR_LEN + 16 + 8 + 1..EVM_TRANSFER_ENCODED_LEN]);
        Ok(EvmTransfer {
            alici,
            miktar,
            nonce,
            recovery_id,
            imza,
        })
    }

    /// ecrecover: imzadan GONDERENIN 0x adresini kurtar (Secenek B, POC 4).
    /// Imza gecersizse None. Bu DILIM bakiyeye/transfer'e DOKUNMAZ.
    pub fn gonderen_adres(&self) -> Option<[u8; ADDR_LEN]> {
        use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
        use sha3::{Digest, Keccak256};

        let mesaj = evm_transfer_mesaji(&self.alici, self.miktar, self.nonce);
        // Ethereum standardi: mesaj keccak256'lanir, o hash imzalanir/kurtarilir.
        // Imzalama (sign_prehash) ile birebir ayni olmali (yoksa gonderen yanlis cikar).
        let prehash = Keccak256::digest(&mesaj);
        let sig = Signature::from_slice(&self.imza).ok()?;
        let recid = RecoveryId::from_byte(self.recovery_id)?;
        let vk = VerifyingKey::recover_from_prehash(&prehash, &sig, recid).ok()?;
        let nokta = vk.to_encoded_point(false);
        let hash = Keccak256::digest(&nokta.as_bytes()[1..]);
        let mut adres = [0u8; ADDR_LEN];
        adres.copy_from_slice(&hash[12..]);
        Some(adres)
    }
}

#[cfg(test)]
mod evm_transfer_tests {
    use super::*;
    use k256::ecdsa::{signature::hazmat::PrehashSigner, RecoveryId, Signature, SigningKey};
    use sha3::{Digest, Keccak256};

    // Yardimci: secp256k1 ile bir EVM transferi olustur (imzala).
    fn imzali_transfer(
        sk: &SigningKey,
        alici: [u8; ADDR_LEN],
        miktar: crate::registry::Tutar,
        nonce: u64,
    ) -> EvmTransfer {
        let mesaj = evm_transfer_mesaji(&alici, miktar, nonce);
        let prehash = Keccak256::digest(&mesaj);
        let (sig, recid): (Signature, RecoveryId) = sk.sign_prehash(&prehash).expect("imza");
        EvmTransfer {
            alici,
            miktar,
            nonce,
            recovery_id: recid.to_byte(),
            imza: sig.to_bytes().into(),
        }
    }

    #[test]
    fn evm_transfer_roundtrip_encode_decode() {
        let sk = SigningKey::random(&mut rand::rngs::OsRng);
        let alici = [0x33u8; ADDR_LEN];
        let t = imzali_transfer(&sk, alici, 5000, 0);
        // encode -> decode ayni nesneyi vermeli
        let kodlu = t.encode();
        assert_eq!(kodlu.len(), EVM_TRANSFER_ENCODED_LEN);
        let cozulen = EvmTransfer::decode(&kodlu).expect("decode");
        assert_eq!(cozulen, t, "encode->decode round-trip ayni olmali");
    }

    #[test]
    fn evm_transfer_gonderen_dogru_kurtarilir() {
        let sk = SigningKey::random(&mut rand::rngs::OsRng);
        // gercek gonderen adresi (keccak yontemi)
        use k256::ecdsa::VerifyingKey;
        let vk = VerifyingKey::from(&sk);
        let nokta = vk.to_encoded_point(false);
        let h = Keccak256::digest(&nokta.as_bytes()[1..]);
        let mut gercek = [0u8; ADDR_LEN];
        gercek.copy_from_slice(&h[12..]);

        let t = imzali_transfer(&sk, [0x44u8; ADDR_LEN], 123, 0);
        // ecrecover ile gonderen, gercek adresle ayni olmali
        assert_eq!(
            t.gonderen_adres(),
            Some(gercek),
            "ecrecover gondereni dogru bulmali"
        );
    }

    #[test]
    fn evm_transfer_tahrif_edilmis_imza_reddedilir_veya_farkli_adres() {
        let sk = SigningKey::random(&mut rand::rngs::OsRng);
        let mut t = imzali_transfer(&sk, [0x55u8; ADDR_LEN], 999, 0);
        let dogru = t.gonderen_adres();
        // miktari degistir (imza eski miktara aitti) -> gonderen ya None ya farkli
        t.miktar = 1;
        let bozuk = t.gonderen_adres();
        assert_ne!(
            bozuk, dogru,
            "tahrif edilen transfer ayni gondereni vermemeli"
        );
    }

    #[test]
    #[ignore]
    fn fuzz_kalkan_sahte_token() {
        // ADVERSARIAL FUZZ: sahte token kalkani (ayni_sembol_farkli_adres).
        let turlar: u64 = std::env::var("KALKAN_TUR").ok().and_then(|x| x.parse().ok()).unwrap_or(2000);
        let mut lcg: u64 = 0x14057B7EF767814F;
        let mut rng = || { lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); lcg };
        for tur in 0..turlar {
            if tur % 1000 == 0 { eprintln!("[kalkan] {}/{} tur", tur, turlar); }
            let adr = |rng: &mut dyn FnMut() -> u64| { let mut a = [0u8; ADDR_LEN]; for x in a.iter_mut() { *x = (rng() & 0xff) as u8; } a };
            let smb = |rng: &mut dyn FnMut() -> u64| { let mut s = [0u8; SYMBOL_LEN]; for x in s.iter_mut() { *x = (rng() & 0xff) as u8; } s };
            let adres_a = adr(&mut rng);
            let mut adres_b = adr(&mut rng);
            while adres_b == adres_a { adres_b = adr(&mut rng); }
            let sembol = smb(&mut rng);
            let mut sembol2 = smb(&mut rng);
            while sembol2 == sembol { sembol2 = smb(&mut rng); }
            let gercek = TokenKaydi::new(adres_a, sembol);
            // 1: ayni sembol, farkli adres -> TAKLIT (true)
            let taklit = TokenKaydi::new(adres_b, sembol);
            if !ayni_sembol_farkli_adres(&gercek, &taklit) {
                panic!("KALKAN DELINDI tur={}: sahte token (ayni sembol farkli adres) gecti!", tur);
            }
            // 2: ayni adres -> TAKLIT DEGIL (false)
            if ayni_sembol_farkli_adres(&gercek, &TokenKaydi::new(adres_a, sembol)) {
                panic!("KALKAN YANLIS POZITIF tur={}: gercek token taklit sayildi", tur);
            }
            // 3: farkli sembol + farkli adres -> TAKLIT DEGIL (false)
            if ayni_sembol_farkli_adres(&gercek, &TokenKaydi::new(adres_b, sembol2)) {
                panic!("KALKAN YANLIS POZITIF tur={}: ayri token taklit sayildi", tur);
            }
            // 4: encode/decode round-trip kimligi bozmamali
            let dec = TokenKaydi::decode(&taklit.encode()).expect("decode basarisiz");
            if dec.kanonik_adres != taklit.kanonik_adres || dec.sembol != taklit.sembol {
                panic!("KALKAN CODEC BOZUK tur={}", tur);
            }
        }
        eprintln!("KALKAN OK: {} tur, sahte token korumasi tuttu", turlar);
    }
}
