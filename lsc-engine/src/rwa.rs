//! RWA defterleri: ORACLE (akis/tur/rapor) ve KYC onay kaydi.
//!
//! YETKI burada DEGIL, node.rs'te kontrol edilir (KurumRegistry rolleri + zincir
//! saati). Bu defterler yalniz durum tutar ve kurallari uygular; tum haritalar
//! BTreeMap (gezinme sirasi tum dugumlerde ayni). Zaman parametreleri HER ZAMAN
//! zincir saatidir (vertex zamani degil — imzalayan onu geriye tarihleyebilir).
//!
//! TUR MODELI (akis basina):
//! - Acik tur = son kapanan tur + 1. Kurum raporu yalniz acik tura verebilir;
//!   bir kurum bir tura bir kez rapor verir (tekrar yok sayilir).
//! - Acik turun ilk raporundan `bayat_sn` gecmisse eski raporlar dusurulur
//!   (bayat raporlarla tur kapanmaz).
//! - Rapor sayisi >= M olunca toplanir (oracle_hesap::topla). Eleme sonrasi
//!   kalan >= M ise tur KAPANIR; degilse acik kalir, yeni rapor beklenir.
//! - Kapanan tur devre kesiciden gecer:
//!     * Akis calisiyorsa ve deger onceki YAYINA gore kesici_bps'ten fazla
//!       degistiyse akis DURUR; deger "aday" olarak saklanir, yayinlanmaz.
//!     * Akis durmussa: yeni tur adaya gore kesici_bps icindeyse (iki ardisik
//!       bagimsiz tur ayni seviyeyi dogruladi) yeni deger YAYINLANIR, akis
//!       yeniden acilir; degilse aday guncellenir, akis durmaya devam eder.
//!   Yeniden acma owner'a DEGIL, raporlayicilarin ardisik turlarina baglidir.

use crate::oracle_hesap::{self, Adres};
use crate::tx::{OracleAkisTanim, OracleRapor};
use std::collections::BTreeMap;

/// Akis basina saklanan azami kapanmis tur sayisi (eski turlar budanir).
pub const ORACLE_TUR_GECMISI: usize = 1024;

/// Bir tura katilan rapor (denetim izi).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurRaporu {
    pub kurum: Adres,
    pub deger: i128,
    pub veri_hash: [u8; 32],
    /// Kurumun beyan ettigi olcum zamani (bilgi).
    pub olcum_zamani: u64,
    /// Raporun islendigi zincir saati.
    pub zaman: u64,
    /// Asiri sapma nedeniyle elendi mi?
    pub elendi: bool,
}

/// Kapanmis bir tur (Chainlink roundData karsiligi).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurKaydi {
    pub tur_no: u64,
    pub deger: i128,
    /// Turun ilk raporunun zincir saati (startedAt).
    pub baslangic: u64,
    /// Turun kapandigi zincir saati (updatedAt).
    pub guncelleme: u64,
    /// false = devre kesici nedeniyle YAYINLANMADI (aday).
    pub yayinlandi: bool,
    pub raporlar: Vec<TurRaporu>,
}

/// Akisin devre kesici durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AkisDurum {
    Calisiyor,
    /// Durduruldu; `aday` = son yayinlanmamis tur degeri.
    Durduruldu { aday: i128, aday_tur: u64 },
}

/// Bir oracle akisinin tum durumu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Akis {
    pub tanim: OracleAkisTanim,
    pub tanim_zamani: u64,
    /// Rapor kabul edilen tur numarasi.
    pub acik_tur: u64,
    /// Acik turun ilk raporunun zincir saati (rapor yoksa anlamsiz).
    pub acik_tur_baslangic: u64,
    /// Acik turun raporlari: kurum -> rapor.
    pub acik_raporlar: BTreeMap<Adres, TurRaporu>,
    /// Kapanmis turlar (son ORACLE_TUR_GECMISI).
    pub turlar: BTreeMap<u64, TurKaydi>,
    /// Son YAYINLANMIS tur numarasi.
    pub son_yayin: Option<u64>,
    pub durum: AkisDurum,
}

/// Rapor islemenin sonucu (test/gozlem icin; state'e etkisi deterministik).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaporSonuc {
    AkisYok,
    /// `olcum_zamani` zincir saatinden ileride ya da `bayat_sn` penceresinden eski.
    OlcumZamaniGecersiz,
    YanlisTur { acik: u64 },
    Tekrar,
    /// Eklendi, tur henuz kapanmadi.
    Eklendi,
    /// Tur kapandi ve yayinlandi.
    Yayinlandi { tur: u64, deger: i128 },
    /// Tur kapandi ama devre kesici nedeniyle yayinlanmadi (akis durdu).
    KesiciDurdu { tur: u64, aday: i128 },
}

/// Okuma hatalari (latestRoundData revert sebepleri).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OkumaHatasi {
    AkisYok,
    VeriYok,
    Durduruldu,
    Bayat,
}

#[derive(Debug, Default)]
pub struct OracleRegistry {
    akislar: BTreeMap<u32, Akis>,
}

impl OracleRegistry {
    pub fn yeni() -> Self {
        Self::default()
    }

    /// Akis tanimla. ILK TANIM KAZANIR (parametreler sonradan degismez).
    pub fn tanimla(&mut self, tanim: OracleAkisTanim, zaman: u64) -> bool {
        if self.akislar.contains_key(&tanim.akis_no) {
            return false;
        }
        self.akislar.insert(
            tanim.akis_no,
            Akis {
                tanim,
                tanim_zamani: zaman,
                acik_tur: 1,
                acik_tur_baslangic: 0,
                acik_raporlar: BTreeMap::new(),
                turlar: BTreeMap::new(),
                son_yayin: None,
                durum: AkisDurum::Calisiyor,
            },
        );
        true
    }

    pub fn akis(&self, akis_no: u32) -> Option<&Akis> {
        self.akislar.get(&akis_no)
    }

    pub fn akis_var_mi(&self, akis_no: u32) -> bool {
        self.akislar.contains_key(&akis_no)
    }

    pub fn akis_sayisi(&self) -> usize {
        self.akislar.len()
    }

    /// Yetkisi node'da dogrulanmis bir kurumun raporunu isle.
    /// `simdi` = zincir saati. `hala_yetkili`: tur kapanirken acik turdaki
    /// raporlarin sahiplerinin rolu hala aktif mi (iptal edilenin raporu sayilmaz).
    pub fn rapor_isle(
        &mut self,
        kurum: Adres,
        rapor: &OracleRapor,
        simdi: u64,
        hala_yetkili: impl Fn(&Adres) -> bool,
    ) -> RaporSonuc {
        let Some(akis) = self.akislar.get_mut(&rapor.akis_no) else {
            return RaporSonuc::AkisYok;
        };
        // OLCUM ZAMANI PENCERESI (zincir saatine gore): ileri tarihli ya da
        // `bayat_sn`'den eski olcum reddedilir. NOT: bu, kurumun beyanidir; yalan
        // soyleyen kurumu DEGIL, gecikmeli aktarimi (bayat olcumun "taze" gibi
        // islenmesini) yakalar. Asil koruma M-of-N rol yonetimi + medyan/elemedir.
        if rapor.olcum_zamani > simdi
            || rapor.olcum_zamani.saturating_add(u64::from(akis.tanim.bayat_sn)) < simdi
        {
            return RaporSonuc::OlcumZamaniGecersiz;
        }
        if rapor.tur_no != akis.acik_tur {
            return RaporSonuc::YanlisTur { acik: akis.acik_tur };
        }
        // Bayat acik tur: ilk rapordan bayat_sn gecmisse eski raporlari dusur.
        if !akis.acik_raporlar.is_empty()
            && oracle_hesap::bayat_mi(akis.acik_tur_baslangic, simdi, akis.tanim.bayat_sn)
        {
            akis.acik_raporlar.clear();
        }
        if akis.acik_raporlar.contains_key(&kurum) {
            return RaporSonuc::Tekrar;
        }
        if akis.acik_raporlar.is_empty() {
            akis.acik_tur_baslangic = simdi;
        }
        akis.acik_raporlar.insert(
            kurum,
            TurRaporu {
                kurum,
                deger: rapor.deger,
                veri_hash: rapor.veri_hash,
                olcum_zamani: rapor.olcum_zamani,
                zaman: simdi,
                elendi: false,
            },
        );

        // Rolu sonradan iptal edilen kurumlarin raporlari toplamaya girmez.
        let gecerli: Vec<(Adres, i128)> = akis
            .acik_raporlar
            .values()
            .filter(|r| hala_yetkili(&r.kurum))
            .map(|r| (r.kurum, r.deger))
            .collect();
        let Some(t) = oracle_hesap::topla(&gecerli, akis.tanim.esik_m, akis.tanim.sapma_bps)
        else {
            return RaporSonuc::Eklendi;
        };

        // TUR KAPANIR.
        let tur_no = akis.acik_tur;
        let onceki_yayin = akis
            .son_yayin
            .and_then(|n| akis.turlar.get(&n))
            .map(|k| k.deger);
        let kesici = akis.tanim.kesici_bps;
        let (yayinla, yeni_durum) = match akis.durum {
            AkisDurum::Calisiyor => {
                if oracle_hesap::kesici_tetiklenir(onceki_yayin, t.deger, kesici) {
                    (false, AkisDurum::Durduruldu { aday: t.deger, aday_tur: tur_no })
                } else {
                    (true, AkisDurum::Calisiyor)
                }
            }
            AkisDurum::Durduruldu { aday, .. } => {
                if oracle_hesap::sapma_asiyor(t.deger, aday, kesici) {
                    (false, AkisDurum::Durduruldu { aday: t.deger, aday_tur: tur_no })
                } else {
                    (true, AkisDurum::Calisiyor)
                }
            }
        };

        let raporlar: Vec<TurRaporu> = gecerli
            .iter()
            .filter_map(|(a, _)| akis.acik_raporlar.get(a).cloned())
            .map(|mut r| {
                r.elendi = t.elenen.contains(&r.kurum);
                r
            })
            .collect();
        akis.turlar.insert(
            tur_no,
            TurKaydi {
                tur_no,
                deger: t.deger,
                baslangic: akis.acik_tur_baslangic,
                guncelleme: simdi,
                yayinlandi: yayinla,
                raporlar,
            },
        );
        while akis.turlar.len() > ORACLE_TUR_GECMISI {
            akis.turlar.pop_first();
        }
        if yayinla {
            akis.son_yayin = Some(tur_no);
        }
        akis.durum = yeni_durum;
        akis.acik_tur = tur_no.saturating_add(1);
        akis.acik_raporlar.clear();
        if yayinla {
            RaporSonuc::Yayinlandi { tur: tur_no, deger: t.deger }
        } else {
            RaporSonuc::KesiciDurdu { tur: tur_no, aday: t.deger }
        }
    }

    /// latestRoundData: son YAYINLANMIS tur. Akis durmussa ya da veri zincir
    /// saatine gore bayatsa HATA (tuketici eski/supheli veriyle islem yapmasin).
    pub fn son_veri(&self, akis_no: u32, simdi: u64) -> Result<&TurKaydi, OkumaHatasi> {
        let akis = self.akislar.get(&akis_no).ok_or(OkumaHatasi::AkisYok)?;
        if matches!(akis.durum, AkisDurum::Durduruldu { .. }) {
            return Err(OkumaHatasi::Durduruldu);
        }
        let tur = akis
            .son_yayin
            .and_then(|n| akis.turlar.get(&n))
            .ok_or(OkumaHatasi::VeriYok)?;
        if oracle_hesap::bayat_mi(tur.guncelleme, simdi, akis.tanim.bayat_sn) {
            return Err(OkumaHatasi::Bayat);
        }
        Ok(tur)
    }

    /// getRoundData: belirli bir YAYINLANMIS tur (gecmis; bayatlik kontrolu yok).
    pub fn tur_verisi(&self, akis_no: u32, tur_no: u64) -> Option<&TurKaydi> {
        self.akislar
            .get(&akis_no)?
            .turlar
            .get(&tur_no)
            .filter(|t| t.yayinlandi)
    }
}

/// Bir kurumun bir adres icin KYC durumu. Kisisel veri YOK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KycDurumu {
    pub onay: bool,
    /// Son degisikligin zincir saati.
    pub zaman: u64,
    /// Kurumun kendi KYC dosyasinin hash'i.
    pub kanit_hash: [u8; 32],
}

/// KYC onay kaydi: (adres, onaylayan kurum) -> durum. Her kurum yalniz KENDI
/// onayini verir/iptal eder; baska kurumun kaydina dokunamaz.
#[derive(Debug, Default)]
pub struct KycRegistry {
    kayitlar: BTreeMap<(Adres, Adres), KycDurumu>,
}

impl KycRegistry {
    pub fn yeni() -> Self {
        Self::default()
    }

    /// Kurumun (yetkisi node'da dogrulanmis) onay/iptal kaydini isle.
    pub fn isle(&mut self, adres: Adres, kurum: Adres, onay: bool, kanit_hash: [u8; 32], zaman: u64) {
        self.kayitlar
            .insert((adres, kurum), KycDurumu { onay, zaman, kanit_hash });
    }

    /// isApproved(address): adres, rolu HALA AKTIF en az bir kurumca onayli mi?
    /// Rolu iptal edilen kurumun onaylari otomatik gecersizdir.
    pub fn onayli_mi(&self, adres: &Adres, kurum_aktif: impl Fn(&Adres) -> bool) -> bool {
        self.adres_araligi(adres)
            .any(|((_, kurum), d)| d.onay && kurum_aktif(kurum))
    }

    /// Belirli bir kurumun bu adres icin kaydi.
    pub fn kurum_kaydi(&self, adres: &Adres, kurum: &Adres) -> Option<KycDurumu> {
        self.kayitlar.get(&(*adres, *kurum)).copied()
    }

    /// Adresin tum kurum kayitlari (kurum sirasina gore).
    pub fn adres_kayitlari(&self, adres: &Adres) -> Vec<(Adres, KycDurumu)> {
        self.adres_araligi(adres).map(|((_, k), d)| (*k, *d)).collect()
    }

    pub fn len(&self) -> usize {
        self.kayitlar.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kayitlar.is_empty()
    }

    fn adres_araligi<'a>(
        &'a self,
        adres: &Adres,
    ) -> impl Iterator<Item = (&'a (Adres, Adres), &'a KycDurumu)> + 'a {
        self.kayitlar.range((*adres, [0u8; 20])..=(*adres, [0xFFu8; 20]))
    }
}

/// Yonetim esigi alt siniri: tek anahtarla yonetim YOK.
pub const RWA_YONETIM_ASGARI_ESIK: u8 = 2;

/// Yonetim islemi red sebepleri (deterministik; state'e dokunulmaz).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YonetimHatasi {
    /// Yonetim kurulmamis (mainnet'te imzaci listesi henuz pinlenmedi).
    Kurulmamis,
    /// Nonce zincirdeki yonetim sayacina esit degil (replay / eski imza).
    YanlisNonce { beklenen: u64 },
    /// Zincir saati `son_gecerlilik`'i gecti.
    SuresiDolmus,
    /// Benzersiz, gecerli imzaci sayisi esigin altinda.
    YetersizImza { gecerli: usize, esik: u8 },
    /// Imzaci sayisi esigin altina duserdi (kendini kilitleme).
    EsikAltinaDusurme,
    /// Esik [ASGARI, imzaci sayisi] disinda.
    GecersizEsik,
    ImzaciZatenVar,
    ImzaciYok,
    AzamiImzaci,
    /// Gecersiz ed25519 acik anahtari.
    GecersizAnahtar,
}

/// RWA yonetimi: M-of-N imzaci kumesi + replay sayaci. Imzaci ANAHTARLARI
/// sunucuda DEGIL; burada yalniz ACIK anahtarlar tutulur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YonetimRegistry {
    imzacilar: std::collections::BTreeSet<[u8; 32]>,
    esik: u8,
    /// Sonraki gecerli yonetim isleminin nonce'u. Her YETKILENDIRILMIS islemde +1.
    nonce: u64,
}

fn anahtar_gecerli(pk: &[u8; 32]) -> bool {
    ed25519_dalek::VerifyingKey::from_bytes(pk).is_ok()
}

impl YonetimRegistry {
    /// Baslangic kurulumu (genesis disi, DAG disi baslangic durumu).
    pub fn kur(imzacilar: &[[u8; 32]], esik: u8) -> Result<Self, YonetimHatasi> {
        let kume: std::collections::BTreeSet<[u8; 32]> = imzacilar.iter().copied().collect();
        if kume.len() != imzacilar.len() {
            return Err(YonetimHatasi::ImzaciZatenVar);
        }
        if kume.len() > crate::tx::RWA_YONETIM_AZAMI_IMZA {
            return Err(YonetimHatasi::AzamiImzaci);
        }
        if !kume.iter().all(anahtar_gecerli) {
            return Err(YonetimHatasi::GecersizAnahtar);
        }
        if esik < RWA_YONETIM_ASGARI_ESIK || usize::from(esik) > kume.len() {
            return Err(YonetimHatasi::GecersizEsik);
        }
        Ok(YonetimRegistry { imzacilar: kume, esik, nonce: 0 })
    }

    pub fn imzacilar(&self) -> Vec<[u8; 32]> {
        self.imzacilar.iter().copied().collect()
    }

    pub fn esik(&self) -> u8 {
        self.esik
    }

    pub fn nonce(&self) -> u64 {
        self.nonce
    }

    pub fn imzaci_mi(&self, pk: &[u8; 32]) -> bool {
        self.imzacilar.contains(pk)
    }

    /// Islemi yetkilendir: nonce == sayac, sure dolmamis, imza mesajini gecerli
    /// imzalayan BENZERSIZ imzaci sayisi >= esik. Liste-disi anahtar, gecersiz
    /// imza ve AYNI imzacinin tekrari SAYILMAZ.
    pub fn yetkilendir(
        &self,
        islem: &crate::tx::YonetimIslemi,
        network_id: u32,
        simdi: u64,
    ) -> Result<(), YonetimHatasi> {
        if islem.nonce != self.nonce {
            return Err(YonetimHatasi::YanlisNonce { beklenen: self.nonce });
        }
        if simdi > islem.son_gecerlilik {
            return Err(YonetimHatasi::SuresiDolmus);
        }
        let mesaj = islem.imza_mesaji(network_id);
        let mut sayilan = std::collections::BTreeSet::new();
        for (pk, imza) in &islem.imzalar {
            if !self.imzacilar.contains(pk) || sayilan.contains(pk) {
                continue;
            }
            let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(pk) else {
                continue;
            };
            let sig = ed25519_dalek::Signature::from_bytes(imza);
            if vk.verify_strict(&mesaj, &sig).is_ok() {
                sayilan.insert(*pk);
            }
        }
        if sayilan.len() < usize::from(self.esik) {
            return Err(YonetimHatasi::YetersizImza { gecerli: sayilan.len(), esik: self.esik });
        }
        Ok(())
    }

    /// Yetkilendirilmis islem tuketildi: sayac +1 (eski imzalar artik gecersiz).
    pub fn nonce_ilerlet(&mut self) {
        self.nonce = self.nonce.saturating_add(1);
    }

    /// Imzaci/esik eylemini uygula. KENDINI KILITLEME KORUMASI: imzaci sayisi
    /// esigin altina dusurulemez; esik [ASGARI, imzaci sayisi] disina cikamaz.
    /// Rol ve kurum dogrulama eylemleri burada degil (node.rs).
    pub fn yapi_degistir(&mut self, eylem: &crate::tx::YonetimEylemi) -> Result<(), YonetimHatasi> {
        use crate::tx::YonetimEylemi as E;
        match eylem {
            E::ImzaciEkle(pk) => {
                if self.imzacilar.contains(pk) {
                    return Err(YonetimHatasi::ImzaciZatenVar);
                }
                if self.imzacilar.len() >= crate::tx::RWA_YONETIM_AZAMI_IMZA {
                    return Err(YonetimHatasi::AzamiImzaci);
                }
                if !anahtar_gecerli(pk) {
                    return Err(YonetimHatasi::GecersizAnahtar);
                }
                self.imzacilar.insert(*pk);
            }
            E::ImzaciCikar(pk) => {
                if !self.imzacilar.contains(pk) {
                    return Err(YonetimHatasi::ImzaciYok);
                }
                if self.imzacilar.len() - 1 < usize::from(self.esik) {
                    return Err(YonetimHatasi::EsikAltinaDusurme);
                }
                self.imzacilar.remove(pk);
            }
            E::Esik(m) => {
                if *m < RWA_YONETIM_ASGARI_ESIK || usize::from(*m) > self.imzacilar.len() {
                    return Err(YonetimHatasi::GecersizEsik);
                }
                self.esik = *m;
            }
            E::Rol(_) | E::KurumDogrula { .. } => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tanim(m: u8) -> OracleAkisTanim {
        OracleAkisTanim {
            akis_no: 1,
            ondalik: 8,
            esik_m: m,
            sapma_bps: 200,      // %2 eleme
            kesici_bps: 1_000,   // %10 devre kesici
            bayat_sn: 3_600,
            aciklama: "XAU/USD".into(),
        }
    }

    /// Olcum zamani = islem ani (gecerli pencere icinde).
    fn rapor(tur: u64, deger: i128, olcum: u64) -> OracleRapor {
        OracleRapor { akis_no: 1, tur_no: tur, deger, olcum_zamani: olcum, veri_hash: [deger as u8; 32] }
    }

    fn k(n: u8) -> Adres {
        [n; 20]
    }

    fn herkes(_: &Adres) -> bool {
        true
    }

    /// M kurumla bir turu kapat (k(1)..k(n) sirayla).
    fn tur_kapat(o: &mut OracleRegistry, tur: u64, degerler: &[i128], t: u64) -> RaporSonuc {
        let mut son = RaporSonuc::Eklendi;
        for (i, d) in degerler.iter().enumerate() {
            son = o.rapor_isle(k(i as u8 + 1), &rapor(tur, *d, t), t, herkes);
        }
        son
    }

    #[test]
    fn akis_ilk_tanim_kazanir() {
        let mut o = OracleRegistry::yeni();
        assert!(o.tanimla(tanim(3), 10));
        assert!(!o.tanimla(OracleAkisTanim { esik_m: 1, ..tanim(3) }, 20));
        assert_eq!(o.akis(1).unwrap().tanim.esik_m, 3, "esik sonradan dusurulemez");
    }

    #[test]
    fn m_rapor_gelince_medyan_yayinlanir() {
        let mut o = OracleRegistry::yeni();
        o.tanimla(tanim(3), 0);
        assert_eq!(o.rapor_isle(k(1), &rapor(1, 1000, 100), 100, herkes), RaporSonuc::Eklendi);
        assert_eq!(o.rapor_isle(k(2), &rapor(1, 1010, 101), 101, herkes), RaporSonuc::Eklendi);
        assert_eq!(o.son_veri(1, 101), Err(OkumaHatasi::VeriYok));
        assert_eq!(
            o.rapor_isle(k(3), &rapor(1, 1005, 102), 102, herkes),
            RaporSonuc::Yayinlandi { tur: 1, deger: 1005 }
        );
        let t = o.son_veri(1, 102).unwrap();
        assert_eq!((t.tur_no, t.deger, t.baslangic, t.guncelleme), (1, 1005, 100, 102));
        assert_eq!(o.akis(1).unwrap().acik_tur, 2);
    }

    #[test]
    fn ayni_kurum_ayni_tura_iki_kez_sayilmaz() {
        let mut o = OracleRegistry::yeni();
        o.tanimla(tanim(2), 0);
        o.rapor_isle(k(1), &rapor(1, 1000, 1), 1, herkes);
        assert_eq!(o.rapor_isle(k(1), &rapor(1, 1000, 2), 2, herkes), RaporSonuc::Tekrar);
        assert_eq!(o.akis(1).unwrap().acik_raporlar.len(), 1);
    }

    #[test]
    fn yanlis_tur_ve_tanimsiz_akis_reddedilir() {
        let mut o = OracleRegistry::yeni();
        assert_eq!(o.rapor_isle(k(1), &rapor(1, 1, 1), 1, herkes), RaporSonuc::AkisYok);
        o.tanimla(tanim(1), 0);
        assert_eq!(o.rapor_isle(k(1), &rapor(2, 1, 1), 1, herkes), RaporSonuc::YanlisTur { acik: 1 });
        assert_eq!(o.rapor_isle(k(1), &rapor(0, 1, 1), 1, herkes), RaporSonuc::YanlisTur { acik: 1 });
    }

    #[test]
    fn asiri_sapan_elenir_ve_iz_kalir() {
        let mut o = OracleRegistry::yeni();
        o.tanimla(tanim(3), 0);
        tur_kapat(&mut o, 1, &[1000, 1001, 999], 10);
        // tur 2: 4 rapor, biri %2'den fazla sapiyor; 4. gelene kadar 3'te kapanabilir
        // mi? 1000,1002,1500 -> medyan 1002, 1500 elenir, 2 kalir < 3 -> ACIK kalir.
        assert_eq!(o.rapor_isle(k(1), &rapor(2, 1000, 20), 20, herkes), RaporSonuc::Eklendi);
        assert_eq!(o.rapor_isle(k(2), &rapor(2, 1500, 20), 20, herkes), RaporSonuc::Eklendi);
        assert_eq!(o.rapor_isle(k(3), &rapor(2, 1002, 20), 20, herkes), RaporSonuc::Eklendi);
        assert_eq!(
            o.rapor_isle(k(4), &rapor(2, 1001, 21), 21, herkes),
            RaporSonuc::Yayinlandi { tur: 2, deger: 1001 }
        );
        let t = o.tur_verisi(1, 2).unwrap();
        let elenen: Vec<Adres> = t.raporlar.iter().filter(|r| r.elendi).map(|r| r.kurum).collect();
        assert_eq!(elenen, vec![k(2)], "elenen rapor denetim izinde isaretli");
    }

    #[test]
    fn bayat_acik_tur_raporlari_dusurulur() {
        let mut o = OracleRegistry::yeni();
        o.tanimla(tanim(2), 0);
        o.rapor_isle(k(1), &rapor(1, 1000, 100), 100, herkes);
        // 3600 sn'den fazla sonra ikinci rapor: ilk rapor bayat -> tur kapanmaz.
        assert_eq!(o.rapor_isle(k(2), &rapor(1, 1000, 100 + 3_601), 100 + 3_601, herkes), RaporSonuc::Eklendi);
        assert_eq!(o.akis(1).unwrap().acik_raporlar.len(), 1);
        assert_eq!(o.akis(1).unwrap().acik_tur_baslangic, 3_701);
    }

    #[test]
    fn son_veri_bayatlasir() {
        let mut o = OracleRegistry::yeni();
        o.tanimla(tanim(1), 0);
        o.rapor_isle(k(1), &rapor(1, 1000, 500), 500, herkes);
        assert!(o.son_veri(1, 500 + 3_600).is_ok());
        assert_eq!(o.son_veri(1, 500 + 3_601), Err(OkumaHatasi::Bayat));
        assert_eq!(o.son_veri(9, 500), Err(OkumaHatasi::AkisYok));
    }

    #[test]
    fn devre_kesici_durdurur_ve_ardisik_turla_acilir() {
        let mut o = OracleRegistry::yeni();
        o.tanimla(tanim(1), 0);
        assert!(matches!(o.rapor_isle(k(1), &rapor(1, 1000, 1), 1, herkes), RaporSonuc::Yayinlandi { .. }));
        // %50 sicrama -> DURUR, yayinlanmaz, okuma hata verir.
        assert_eq!(
            o.rapor_isle(k(1), &rapor(2, 1500, 2), 2, herkes),
            RaporSonuc::KesiciDurdu { tur: 2, aday: 1500 }
        );
        assert_eq!(o.son_veri(1, 2), Err(OkumaHatasi::Durduruldu));
        assert!(o.tur_verisi(1, 2).is_none(), "aday tur yayinlanmis sayilmaz");
        // Ardisik tur adaydan yine cok farkli (900) -> hala durgun, aday guncellenir.
        assert_eq!(
            o.rapor_isle(k(1), &rapor(3, 900, 3), 3, herkes),
            RaporSonuc::KesiciDurdu { tur: 3, aday: 900 }
        );
        // Ardisik tur adayi dogruluyor (%10 icinde) -> yayin + akis acilir.
        assert_eq!(
            o.rapor_isle(k(1), &rapor(4, 950, 4), 4, herkes),
            RaporSonuc::Yayinlandi { tur: 4, deger: 950 }
        );
        assert_eq!(o.son_veri(1, 4).unwrap().deger, 950);
        assert_eq!(o.akis(1).unwrap().durum, AkisDurum::Calisiyor);
    }

    #[test]
    fn rolu_iptal_edilenin_acik_raporu_sayilmaz() {
        let mut o = OracleRegistry::yeni();
        o.tanimla(tanim(2), 0);
        o.rapor_isle(k(1), &rapor(1, 1000, 1), 1, herkes);
        // k(1)'in rolu iptal edildi; k(2) raporlayinca yalniz 1 gecerli -> kapanmaz.
        let k1_haric = |a: &Adres| *a != k(1);
        assert_eq!(o.rapor_isle(k(2), &rapor(1, 1000, 2), 2, k1_haric), RaporSonuc::Eklendi);
        assert_eq!(
            o.rapor_isle(k(3), &rapor(1, 1002, 3), 3, k1_haric),
            RaporSonuc::Yayinlandi { tur: 1, deger: 1000 }
        );
        let t = o.tur_verisi(1, 1).unwrap();
        assert!(t.raporlar.iter().all(|r| r.kurum != k(1)));
    }

    #[test]
    fn tur_gecmisi_budanir() {
        let mut o = OracleRegistry::yeni();
        o.tanimla(tanim(1), 0);
        for tur in 1..=(ORACLE_TUR_GECMISI as u64 + 5) {
            o.rapor_isle(k(1), &rapor(tur, 1000, tur), tur, herkes);
        }
        let a = o.akis(1).unwrap();
        assert_eq!(a.turlar.len(), ORACLE_TUR_GECMISI);
        assert!(o.tur_verisi(1, 5).is_none());
        assert!(o.tur_verisi(1, 6).is_some());
        assert!(o.son_veri(1, ORACLE_TUR_GECMISI as u64 + 5).is_ok());
    }

    #[test]
    fn olcum_zamani_ileride_veya_pencereden_eskiyse_reddedilir() {
        let mut o = OracleRegistry::yeni();
        o.tanimla(tanim(1), 0); // bayat_sn = 3600
        let simdi = 10_000;
        let r = |olcum| OracleRapor { akis_no: 1, tur_no: 1, deger: 1000, olcum_zamani: olcum, veri_hash: [0; 32] };
        assert_eq!(o.rapor_isle(k(1), &r(simdi + 1), simdi, herkes), RaporSonuc::OlcumZamaniGecersiz, "ileri tarihli");
        assert_eq!(o.rapor_isle(k(1), &r(simdi - 3_601), simdi, herkes), RaporSonuc::OlcumZamaniGecersiz, "pencereden eski");
        assert_eq!(o.rapor_isle(k(1), &r(u64::MAX), simdi, herkes), RaporSonuc::OlcumZamaniGecersiz);
        assert!(o.akis(1).unwrap().acik_raporlar.is_empty(), "reddedilen rapor tura girmez");
        // Sinirlar dahil: tam pencere basi ve tam zincir saati kabul.
        assert_eq!(
            o.rapor_isle(k(1), &r(simdi - 3_600), simdi, herkes),
            RaporSonuc::Yayinlandi { tur: 1, deger: 1000 }
        );
        let r2 = OracleRapor { tur_no: 2, ..r(simdi) };
        assert!(matches!(o.rapor_isle(k(1), &r2, simdi, herkes), RaporSonuc::Yayinlandi { tur: 2, .. }));
    }

    #[test]
    fn kyc_onay_iptal_ve_kurum_rolu() {
        let mut y = KycRegistry::yeni();
        let adres = [0xAB; 20];
        assert!(!y.onayli_mi(&adres, herkes));
        y.isle(adres, k(1), true, [1; 32], 10);
        assert!(y.onayli_mi(&adres, herkes));
        // Onaylayan kurumun rolu iptal -> onay gecersiz.
        assert!(!y.onayli_mi(&adres, |a| *a != k(1)));
        // Ikinci kurum da onaylar; birincisi iptal eder -> ikinci onay yeterli.
        y.isle(adres, k(2), true, [2; 32], 11);
        y.isle(adres, k(1), false, [3; 32], 12);
        assert!(y.onayli_mi(&adres, herkes));
        assert!(!y.kurum_kaydi(&adres, &k(1)).unwrap().onay);
        y.isle(adres, k(2), false, [4; 32], 13);
        assert!(!y.onayli_mi(&adres, herkes));
        // Yeniden onay
        y.isle(adres, k(2), true, [5; 32], 14);
        assert!(y.onayli_mi(&adres, herkes));
        assert_eq!(y.adres_kayitlari(&adres).len(), 2);
        // Baska adresin kayitlari karismaz
        assert!(!y.onayli_mi(&[0xAC; 20], herkes));
        assert!(y.adres_kayitlari(&[0xAA; 20]).is_empty());
    }
}
