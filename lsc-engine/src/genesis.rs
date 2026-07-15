//! Genesis kurucusu — DEVNET/TEST için arz + dağıtım mekaniği.
//!
//! DÜRÜSTLÜK NOTU: Bu, dağıtım MEKANİĞİNİ kurar ve test eder. Gerçek mainnet
//! arzı/dağıtımı ancak audit + hukuki uyum + çok-node testnet sonrası, pinli
//! genesis ile ilan edilir. Buradaki adresler örnek/placeholder'dır.
//!
//! İLKE: Arz KAPALIDIR (kayıpsız). 6 dilimin toplamı TAM olarak toplam arza
//! eşit olmalıdır; değilse genesis reddedilir (`kapali_mi` == false).

use crate::registry::Tutar;

/// 18 ondalık (EVM/wei uyumu için).
pub const ONDALIK: u128 = 1_000_000_000_000_000_000;

/// AIDAG toplam arzı: 21.000.000 (×10^18). Sabit, madencilik yok.
pub const AIDAG_ARZ: u128 = 21_000_000 * ONDALIK;

/// LSC toplam arzı: 2.100.000.000 (×10^18). Ağ yakıtı.
pub const LSC_ARZ: u128 = 2_100_000_000 * ONDALIK;

/// Genesis dağıtım dilimleri (AIDAG). Her dilim bir adrese gider.
/// Yüzdeler: Ekosistem %30, Hazine %25, Likidite %15, Topluluk %12,
/// Kurucu %13, Erken Destekçi %5 = %100 (kapalı).
#[derive(Debug, Clone)]
pub struct GenesisDagitim {
    pub ekosistem: (GenAdres, Tutar),
    pub hazine: (GenAdres, Tutar),
    pub likidite: (GenAdres, Tutar),
    pub topluluk: (GenAdres, Tutar),
    pub kurucu: (GenAdres, Tutar),
    pub erken_destekci: (GenAdres, Tutar),
}

type GenAdres = [u8; 20];

impl GenesisDagitim {
    /// Toplam arzdan 6 dilimi yüzdelere göre hesaplar.
    /// Yuvarlama artığı (varsa) ekosisteme eklenir — böylece toplam TAM arz olur.
    pub fn planla(adresler: [GenAdres; 6]) -> Self {
        let arz = AIDAG_ARZ;
        let hazine = arz * 25 / 100;
        let likidite = arz * 15 / 100;
        let topluluk = arz * 12 / 100;
        let kurucu = arz * 13 / 100;
        let erken = arz * 5 / 100;
        // Ekosistem = kalan (yuvarlama artığını da alır -> kapalılık garantisi)
        let ekosistem = arz - hazine - likidite - topluluk - kurucu - erken;
        GenesisDagitim {
            ekosistem: (adresler[0], ekosistem),
            hazine: (adresler[1], hazine),
            likidite: (adresler[2], likidite),
            topluluk: (adresler[3], topluluk),
            kurucu: (adresler[4], kurucu),
            erken_destekci: (adresler[5], erken),
        }
    }

    /// Tüm dilimlerin toplamı.
    pub fn toplam(&self) -> Tutar {
        self.ekosistem.1
            + self.hazine.1
            + self.likidite.1
            + self.topluluk.1
            + self.kurucu.1
            + self.erken_destekci.1
    }

    /// KAPALILIK KONTROLÜ: toplam TAM olarak AIDAG_ARZ mı?
    /// false dönerse genesis GEÇERSİZ (yaratım/kayıp var demektir).
    pub fn kapali_mi(&self) -> bool {
        self.toplam() == AIDAG_ARZ
    }

    /// Dilimleri (adres, miktar) listesi olarak döner — bakiye yüklemek için.
    pub fn dilimler(&self) -> [(GenAdres, Tutar); 6] {
        [
            self.ekosistem,
            self.hazine,
            self.likidite,
            self.topluluk,
            self.kurucu,
            self.erken_destekci,
        ]
    }
}

/// VESTING TAKVIMI — MÜHÜRLÜ plan (MAINNET_TEKONOMIK.md). Vesting parametreleri
/// TEK KAYNAKTAN gelir: hem genesis 6-dilim dağıtımı hem de mainnet interim
/// kurucu-dilimi bu sabitleri kullanır. Env'de gevşek/serbest değer YOK →
/// yanlış konfigürasyon imkânsız, denetim (audit) net.
pub const CLIFF_6AY: u64 = 180 * 86400;
/// Doğrusal açılım toplam süresi: 2 yıl.
pub const VESTING_2YIL: u64 = 730 * 86400;

/// `dilimler()` sırasında KURUCU dilim index'i.
pub const DILIM_KURUCU: usize = 4;

/// `dilimler()` index'ine göre vesting planı. `Some((cliff_sure, toplam_sure))`
/// → dilim KİLİTLİ (cliff + doğrusal açılım). `None` → AÇIK (vesting yok).
///
/// Plan (MÜHÜRLÜ):
///   0 ekosistem      : AÇIK
///   1 hazine         : AÇIK bakiye (harcama Payhawk-kilit ile ayrıca sınırlı — Faz D)
///   2 likidite       : 2 yıl doğrusal, cliff YOK (DEX likidite esnekliği)
///   3 topluluk       : AÇIK
///   4 kurucu         : 6 ay cliff + 2 yıl doğrusal (dump koruması)
///   5 erken destekçi : 6 ay cliff + 2 yıl doğrusal
pub fn dilim_vesting(idx: usize) -> Option<(u64, u64)> {
    match idx {
        2 => Some((0, VESTING_2YIL)),
        4 | 5 => Some((CLIFF_6AY, VESTING_2YIL)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_kapali_toplam_tam_arz() {
        let adresler = [
            [1u8; 20], [2u8; 20], [3u8; 20], [4u8; 20], [5u8; 20], [6u8; 20],
        ];
        let g = GenesisDagitim::planla(adresler);
        // Kapalılık: toplam TAM olarak 21M ×10^18
        assert!(g.kapali_mi(), "genesis kapali degil - arz korunmuyor!");
        assert_eq!(g.toplam(), AIDAG_ARZ);
    }

    #[test]
    fn genesis_dilimler_dogru_oran() {
        let adresler = [
            [1u8; 20], [2u8; 20], [3u8; 20], [4u8; 20], [5u8; 20], [6u8; 20],
        ];
        let g = GenesisDagitim::planla(adresler);
        // Hazine %25
        assert_eq!(g.hazine.1, AIDAG_ARZ * 25 / 100);
        // Kurucu %13
        assert_eq!(g.kurucu.1, AIDAG_ARZ * 13 / 100);
        // Erken destekçi %5
        assert_eq!(g.erken_destekci.1, AIDAG_ARZ * 5 / 100);
        // Ekosistem, kalanı alır (>= %30, yuvarlama artığıyla)
        assert!(g.ekosistem.1 >= AIDAG_ARZ * 30 / 100);
    }

    #[test]
    fn vesting_plani_muhurlu_takvime_uyar() {
        // MÜHÜRLÜ plan: ekosistem/hazine/topluluk AÇIK; likidite 2yıl cliffsiz;
        // kurucu+destekçi 6ay cliff+2yıl. Regresyon kilidi (plan değişirse test kırılır).
        assert_eq!(dilim_vesting(0), None, "ekosistem açık olmalı");
        assert_eq!(dilim_vesting(1), None, "hazine (genesis) açık olmalı");
        assert_eq!(
            dilim_vesting(2),
            Some((0, VESTING_2YIL)),
            "likidite 2yıl cliffsiz"
        );
        assert_eq!(dilim_vesting(3), None, "topluluk açık olmalı");
        assert_eq!(
            dilim_vesting(4),
            Some((CLIFF_6AY, VESTING_2YIL)),
            "kurucu 6ay+2yıl"
        );
        assert_eq!(
            dilim_vesting(5),
            Some((CLIFF_6AY, VESTING_2YIL)),
            "destekçi 6ay+2yıl"
        );
        assert_eq!(
            DILIM_KURUCU, 4,
            "kurucu index'i dilimler() sırasıyla tutarlı"
        );
    }

    #[test]
    fn genesis_dilimler_bakiyeye_yuklenebilir() {
        use crate::registry::BakiyeRegistry;
        let adresler = [
            [1u8; 20], [2u8; 20], [3u8; 20], [4u8; 20], [5u8; 20], [6u8; 20],
        ];
        let g = GenesisDagitim::planla(adresler);
        let mut reg = BakiyeRegistry::yeni();
        for (adres, miktar) in g.dilimler() {
            reg.test_bakiye_ekle(adres, miktar);
        }
        // Yüklenen toplam == arz
        let mut toplam = 0u128;
        for (adres, _) in g.dilimler() {
            toplam += reg.bakiye(&adres);
        }
        assert_eq!(toplam, AIDAG_ARZ, "yuklenen toplam arza esit degil");
    }
}
