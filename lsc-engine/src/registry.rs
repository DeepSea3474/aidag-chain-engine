//! Token kayit defteri (registry) — KALKAN'in kalbi.
//!
//! Siradan DEX: taklit token'i listeye alir + "uyari" gosterir; kullanici yine
//! de alabilir. BU REGISTRY: taklidi PROTOKOL SEVIYESINDE reddeder — kayitli bir
//! sembolun farkli-adresli kopyasi deftere HIC GIREMEZ. Uyari degil, ENGELLEME.
//!
//! Kimlik = kanonik kontrat ADRESI (sembol/isim DEGIL — taklit edilebilir).
//! "USDC" sembollu ama farkli adresli token = TAKLIT -> reddedilir.

/// Para birimi tipi: AIDAG/LSC tutarlari. Tek yerden yonetilir.
/// 18 ondalik + 21M arz icin u128 (u64 tasardi). Gerekirse U256 tek satir.
/// SAYAC/ZAMAN/REF para DEGIL -> u64 kalir (nonce, timestamp, odeme_ref).
pub type Tutar = u128;

use crate::tx::{StakeKaydi, TokenKaydi};
use std::collections::HashMap;

/// Bir ed25519 public key'den (32 bayt) KANONIK ADRES (20 bayt) turet.
/// Yontem: blake3(public_key)'in ilk 20 bayti. Hem STAKE hem TOKEN KAYDI ayni
/// turetmeyi kullanir -> "token kaydeden adres" ile "stake eden adres" eslesir.
/// Boylece: bir kisi kendi anahtariyla stake eder + token kaydeder; kalkan,
/// imzalayanin stake edip etmedigini bu adres uzerinden dogrular (imza sahte
/// olamaz -> kimse baskasinin stake'ini kullanamaz).
pub fn public_key_to_adres(public_key: &[u8; 32]) -> [u8; 20] {
    let hash = blake3::hash(public_key);
    let mut adres = [0u8; 20];
    adres.copy_from_slice(&hash.as_bytes()[..20]);
    adres
}

/// Bir token'i deftere kaydetme girisiminin sonucu.
#[derive(Debug, PartialEq, Eq)]
pub enum KayitSonucu {
    /// Yeni kanonik token; deftere eklendi.
    Kabul,
    /// Tam ayni token (ayni adres + sembol) zaten kayitli; tekrar eklenmedi.
    ZatenKayitli,
    /// REDDEDILDI: kayitli bir sembolun farkli-adresli TAKLIDI. Hangi gercek
    /// token'in (adresi) taklit edildigi de dondurulur (seffaflik icin).
    TaklitReddedildi { taklit_edilen_adres: [u8; 20] },
}

/// Kanonik token kayit defteri. Gercek token'lar burada tutulur; gelen her
/// yeni token TUM deftere karsi kontrol edilir (zorlayici kalkan).
#[derive(Debug, Default)]
pub struct TokenRegistry {
    kayitlar: Vec<TokenKaydi>,
}

impl TokenRegistry {
    /// Bos defter.
    pub fn yeni() -> Self {
        TokenRegistry {
            kayitlar: Vec::new(),
        }
    }

    /// Deftere kayitli token sayisi.
    /// Kayitli kanonik token'lari (adres, sembol) olarak dondurur (RPC/okuma).
    pub fn tum_tokenlar(&self) -> Vec<([u8; 20], [u8; 8])> {
        self.kayitlar
            .iter()
            .map(|k| (k.kanonik_adres, k.sembol))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.kayitlar.len()
    }

    /// Defter bos mu?
    pub fn is_empty(&self) -> bool {
        self.kayitlar.is_empty()
    }

    /// ZORLAYICI KAYIT: gelen token'i deftere karsi denetle.
    /// - Kayitli bir sembol + FARKLI adres varsa -> TaklitReddedildi (deftere GIRMEZ)
    /// - Tam ayni (adres + sembol) varsa -> ZatenKayitli
    /// - Temizse -> deftere ekle, Kabul
    pub fn kaydet(&mut self, gelen: TokenKaydi) -> KayitSonucu {
        for kayitli in &self.kayitlar {
            let ayni_sembol = kayitli.sembol == gelen.sembol;
            let ayni_adres = kayitli.kanonik_adres == gelen.kanonik_adres;
            if ayni_sembol && !ayni_adres {
                // Ayni sembol, farkli adres = TAKLIT. Reddet.
                return KayitSonucu::TaklitReddedildi {
                    taklit_edilen_adres: kayitli.kanonik_adres,
                };
            }
            if ayni_adres && ayni_sembol {
                return KayitSonucu::ZatenKayitli;
            }
        }
        // Temiz: deftere ekle.
        self.kayitlar.push(gelen);
        KayitSonucu::Kabul
    }

    /// Bir adres deftere kayitli (gercek/kanonik) mi? Kayitli token'i dondurur.
    pub fn adres_ile_bul(&self, adres: &[u8; 20]) -> Option<&TokenKaydi> {
        self.kayitlar.iter().find(|k| &k.kanonik_adres == adres)
    }

    /// Verilen token, kayitli bir sembolun TAKLIDI mi? (deftere eklemeden sorgu)
    /// Sahteyse, taklit edilen gercek adresi dondurur.
    pub fn taklit_mi(&self, gelen: &TokenKaydi) -> Option<[u8; 20]> {
        for kayitli in &self.kayitlar {
            if kayitli.sembol == gelen.sembol && kayitli.kanonik_adres != gelen.kanonik_adres {
                return Some(kayitli.kanonik_adres);
            }
        }
        None
    }
}

/// Stake (teminat) defteri: hangi adres ne kadar AIDAG kilitlemis.
/// KALKAN bagi: sadece stake etmis adresler kanonik token kaydedebilir
/// (ileride NodeState'te baglanir). Teminat = kayit hakki + durustluk tesvigi.
#[derive(Debug, Default)]
pub struct StakeRegistry {
    /// adres -> toplam kilitli miktar (ayni adres tekrar stake ederse BIRIKIR).
    stakelar: HashMap<[u8; 20], Tutar>,
}

impl StakeRegistry {
    /// Bos defter.
    pub fn yeni() -> Self {
        StakeRegistry {
            stakelar: HashMap::new(),
        }
    }

    /// Stake ekle: adresin teminatini arttir (birikimli). Donus: adresin
    /// yeni toplam stake miktari.
    pub fn stake_ekle(&mut self, kayit: StakeKaydi) -> Tutar {
        let toplam = self.stakelar.entry(kayit.staker).or_insert(0);
        *toplam = toplam.saturating_add(kayit.miktar);
        *toplam
    }

    /// Bir adresin toplam kilitli (stake) miktari. Stake yoksa 0.
    pub fn stake_miktari(&self, adres: &[u8; 20]) -> Tutar {
        self.stakelar.get(adres).copied().unwrap_or(0)
    }

    /// Adres stake etmis mi? (kalkan icin: kayit hakki var mi?)
    pub fn stake_var_mi(&self, adres: &[u8; 20]) -> bool {
        self.stakelar.get(adres).is_some_and(|&m| m > 0)
    }

    /// Defterdeki toplam kilitli AIDAG.
    pub fn toplam_stake(&self) -> Tutar {
        self.stakelar.values().copied().sum()
    }

    /// Kac farkli adres stake etmis.
    pub fn staker_sayisi(&self) -> usize {
        self.stakelar.len()
    }

    /// SLASHING (sahtecilik cezasi): bir adresin TUM stake'ini yak. Defterden
    /// tamamen silinir. Yakilan miktar dondurulur (0 = stake yoktu). Sahte/taklit
    /// token kaydetmeye KALKISMAK bile teminatin tamamini kaybettirir -> guclu
    /// caydirici. Sahteciligin bedeli agirdir.
    pub fn slash(&mut self, adres: &[u8; 20]) -> Tutar {
        self.stakelar.remove(adres).unwrap_or(0)
    }
}

/// Bakiye defteri: hangi adres ne kadar AIDAG'a sahip (serbest, kilitsiz).
/// Stake'ten FARKLI: stake KILITLI teminat; bakiye SERBEST, transfer edilebilir.
/// TRANSFER cekirdegi: gonderen bakiyesinden DUSER, alici bakiyesine EKLENIR.
/// Cift harcama korumasi: bakiye yetmezse transfer REDDEDILIR (asla negatif/overflow).
#[derive(Debug, Default)]
pub struct BakiyeRegistry {
    /// adres -> serbest AIDAG bakiyesi.
    bakiyeler: HashMap<[u8; 20], Tutar>,
    /// VESTING: adres -> vesting plani. Kilitli kisim transfer EDILEMEZ.
    vesting: HashMap<[u8; 20], VestingKaydi>,
    /// Su anki zincir zamani (transfer'de vesting kontrolu icin).
    simdi_zaman: u64,
}

/// Vesting plani: (TGE-acik) + cliff + dogrusal acilim (blok/zaman bazli).
#[derive(Clone, Debug)]
pub struct VestingKaydi {
    pub toplam: Tutar,
    pub baslangic: u64,
    pub cliff_sure: u64,
    pub toplam_sure: u64,
    /// TGE'de (baslangic aninda) HEMEN acilan miktar (on-satis %20 gibi). Kalan
    /// (toplam - tge_acik) cliff+dogrusal vest'e tabi. 0 = klasik davranis (geri uyumlu).
    pub tge_acik: Tutar,
}

impl VestingKaydi {
    pub fn acilmis(&self, simdi: u64) -> Tutar {
        // TGE'den once hicbir sey acik degil.
        if simdi < self.baslangic {
            return 0;
        }
        // Vest'e tabi kisim = toplam - tge_acik (tge_acik zaten hemen serbest).
        let vest_toplam = self.toplam.saturating_sub(self.tge_acik);
        let vested = if simdi < self.baslangic + self.cliff_sure {
            0
        } else {
            let gecen = simdi.saturating_sub(self.baslangic);
            if self.toplam_sure == 0 || gecen >= self.toplam_sure {
                vest_toplam
            } else {
                vest_toplam * (gecen as u128) / (self.toplam_sure as u128)
            }
        };
        self.tge_acik.saturating_add(vested)
    }
    pub fn kilitli(&self, simdi: u64) -> Tutar {
        self.toplam.saturating_sub(self.acilmis(simdi))
    }
}

impl BakiyeRegistry {
    pub fn yeni() -> Self {
        BakiyeRegistry {
            bakiyeler: HashMap::new(),
            vesting: HashMap::new(),
            simdi_zaman: 0,
        }
    }

    /// Bir adresin serbest bakiyesi. Yoksa 0.
    pub fn bakiye(&self, adres: &[u8; 20]) -> Tutar {
        self.bakiyeler.get(adres).copied().unwrap_or(0)
    }

    /// TEST/DEVNET: bir adrese bakiye basla (gercek arz DEGIL; mekanik testi icin).
    /// Gercek arz/dagitim modeli sonra (audit+hukuk asamasi). Birikimli.
    pub fn test_bakiye_ekle(&mut self, adres: [u8; 20], miktar: Tutar) -> Tutar {
        let b = self.bakiyeler.entry(adres).or_insert(0);
        *b = b.saturating_add(miktar);
        *b
    }

    /// VESTING: bir adrese vesting plani ekle.
    pub fn vesting_ekle(&mut self, adres: [u8; 20], kayit: VestingKaydi) {
        self.vesting.insert(adres, kayit);
    }
    /// Bir adresin su an KILITLI miktari.
    pub fn vesting_kilitli(&self, adres: &[u8; 20], simdi: u64) -> Tutar {
        self.vesting
            .get(adres)
            .map(|v| v.kilitli(simdi))
            .unwrap_or(0)
    }
    /// Zincir zamanini ayarla (transfer'de vesting kontrolu icin).
    pub fn zaman_ayarla(&mut self, simdi: u64) {
        self.simdi_zaman = simdi;
    }

    /// TRANSFER: gonderenden alici'ya `miktar` AIDAG aktar.
    /// KURALLAR (cift harcama + butunluk korumasi):
    ///   - gonderen bakiyesi >= miktar olmali (yoksa TransferSonuc::YetersizBakiye)
    ///   - miktar > 0 olmali (0 transfer anlamsiz -> GecersizMiktar)
    ///   - gonderen != alici (kendine transfer anlamsiz -> GecersizMiktar)
    ///   - overflow korumasi: alici bakiyesi + miktar tasmamali
    /// Basarili olursa gonderenden DUSER, alici'ya EKLER, yeni gonderen bakiyesini doner.
    pub fn transfer(
        &mut self,
        gonderen: &[u8; 20],
        alici: &[u8; 20],
        miktar: Tutar,
    ) -> TransferSonuc {
        if miktar == 0 || gonderen == alici {
            return TransferSonuc::GecersizMiktar;
        }
        let gonderen_bakiye = self.bakiye(gonderen);
        // VESTING KILIT: kilitli kisim harcanamaz.
        let kilitli = self.vesting_kilitli(gonderen, self.simdi_zaman);
        let harcanabilir = gonderen_bakiye.saturating_sub(kilitli);
        if harcanabilir < miktar {
            return TransferSonuc::YetersizBakiye {
                mevcut: harcanabilir,
                istenen: miktar,
            };
        }
        let alici_bakiye = self.bakiye(alici);
        // overflow korumasi (teorik; u64 tavani cok yuksek ama tavizsiz sağlamlik).
        let alici_yeni = match alici_bakiye.checked_add(miktar) {
            Some(v) => v,
            None => return TransferSonuc::GecersizMiktar,
        };
        // Uygula: once dus, sonra ekle ( ara durumda toplam arz korunur).
        self.bakiyeler.insert(*gonderen, gonderen_bakiye - miktar);
        self.bakiyeler.insert(*alici, alici_yeni);
        TransferSonuc::Basarili {
            gonderen_yeni_bakiye: gonderen_bakiye - miktar,
        }
    }

    /// KOPRU 2 (AVM): TUM AIDAG bakiyelerini (adres->tutar) dondur.
    /// EVM'e calistirmadan ONCE tam gorunum vermek icin (avm_db seed).
    pub fn tum_bakiyeler(&self) -> &HashMap<[u8; 20], Tutar> {
        &self.bakiyeler
    }

    /// KOPRU 2 (AVM geri-yansitma / B1): EVM sonrasi AIDAG bakiyelerini deftere
    /// KALICI aynala. Kontrat-ici tum native hareketler (payable/withdraw/split)
    /// boylece gercek deftere gecer -> fon donmasi biter.
    /// ARZ KORUMASI: cagiran, avm_db'yi calistirmadan ONCE bu defterin TAM
    /// kopyasiyla seed etmis olmali; EVM value-korumalidir (gas_price=0, native
    /// yaratmaz/yakmaz) -> sum(kaynak) == onceki toplam_arz. Vesting kilit-katmani
    /// AYRIDIR (vesting map'e dokunulmaz; yalnizca ham bakiyeler guncellenir).
    pub fn aidag_aynala(&mut self, kaynak: &HashMap<[u8; 20], Tutar>) {
        self.bakiyeler = kaynak.clone();
    }

    /// Defterdeki toplam serbest AIDAG (arz denetimi/test icin).
    pub fn toplam_arz(&self) -> Tutar {
        self.bakiyeler.values().copied().sum()
    }

    /// Kac farkli adresin bakiyesi var.
    pub fn hesap_sayisi(&self) -> usize {
        self.bakiyeler.len()
    }
}

/// Transfer girisiminin sonucu (cift harcama + butunluk).
#[derive(Debug, PartialEq, Eq)]
pub enum TransferSonuc {
    /// Basarili: gonderenden dusuldu, alici'ya eklendi.
    Basarili { gonderen_yeni_bakiye: Tutar },
    /// Gonderen bakiyesi yetersiz (cift harcama engellendi).
    YetersizBakiye { mevcut: Tutar, istenen: Tutar },
    /// Miktar 0, gonderen==alici, ya da overflow.
    GecersizMiktar,
}

/// Bir belge kaydinin kanitlanabilir detayi: KIM (imzalayan adres) ve NE ZAMAN.
/// Belgenin KENDISI zincirde DEGIL (sadece hash'i); gizlilik + boyut korunur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BelgeKaydi {
    /// Belgeyi zincire yazan (imzalayan vertex'in adresi). KIM sorusu.
    pub kaydeden: [u8; 20],
    /// Kaydin zaman damgasi (vertex timestamp'i). NE ZAMAN sorusu.
    pub zaman: u64,
}

/// Belge/veri dogrulama defteri: hash -> (kaydeden, zaman).
/// GERCEK DUNYA KULLANIMI: bir belgenin/verinin parmak izi (hash) zincire
/// yazilir; sonra "bu belge su tarihte vardi, su adres imzaladi, DEGISMEDI"
/// diye dogrulanir. Sahteciligin panzehiri: icerik degisirse hash degisir,
/// kayitla eslesmez. ILK KAYIT KAZANIR: ayni hash daha sonra baska biri
/// tarafindan yazilirsa, ILK kaydeden korunur (oncelik = kanit).
#[derive(Debug, Default)]
pub struct RecordRegistry {
    kayitlar: HashMap<[u8; 32], BelgeKaydi>,
}

impl RecordRegistry {
    pub fn yeni() -> Self {
        RecordRegistry {
            kayitlar: HashMap::new(),
        }
    }

    /// Belge kaydet: hash -> (kaydeden, zaman). ILK KAYIT KAZANIR; ayni hash
    /// zaten varsa DOKUNULMAZ (ilk kaydeden + ilk zaman korunur, kanit bozulmaz).
    /// Donus: true = yeni kayit eklendi, false = zaten kayitliydi (degismedi).
    pub fn kaydet(&mut self, data_hash: [u8; 32], kaydeden: [u8; 20], zaman: u64) -> bool {
        if self.kayitlar.contains_key(&data_hash) {
            return false; // ilk kayit korunur
        }
        self.kayitlar
            .insert(data_hash, BelgeKaydi { kaydeden, zaman });
        true
    }

    /// Bir belge hash'i zincirde kayitli mi? Detayi (kim, ne zaman) dondurur.
    pub fn dogrula(&self, data_hash: &[u8; 32]) -> Option<BelgeKaydi> {
        self.kayitlar.get(data_hash).copied()
    }

    /// Kayitli belge sayisi.
    pub fn len(&self) -> usize {
        self.kayitlar.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kayitlar.is_empty()
    }
}

/// Bir kurumun/firmanin kategorisi. Devlet ve ozel sektor KESIN AYRILIR
/// (karistirilmaz). Belge dogrulamada "bu belge hangi tur kurumdan" netlesir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KurumKategori {
    /// Resmi devlet kurum/kurulusu (onay/yetki katmani anlasma ile baglanir).
    Devlet,
    /// Ozel sektor firmasi / sahsi isyeri.
    Ozel,
}

/// Bir kurum kimlik kaydinin detayi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KurumKaydi {
    /// Kurum/firma adi (orn "Tapu Mudurlugu" ya da "Ahmet Insaat Ltd").
    pub ad: String,
    /// Kategori: Devlet mi Ozel mi (KESIN ayrim, karistirilmaz).
    pub kategori: KurumKategori,
    /// Kayit zamani (vertex timestamp).
    pub zaman: u64,
}

/// Kurum/firma kimlik defteri: adres -> KurumKaydi.
/// AMAC: "bu adres hangi kurum/firma, hangi kategori (devlet/ozel)" diye
/// dogrulanabilir kimlik. Belge dogrulama ile birleserek: "bu belge su KURUMDAN
/// geldi" denebilir (sadece "su adresten" degil). ILK KAYIT KAZANIR (adres bir
/// kez kaydedilir; kimlik bozulmaz). Resmi onay/yetki katmani (devlet kurumlari
/// icin) ANLASMA yapildiginda API/baglanti ile eklenir — su an altyapi HAZIR.
#[derive(Debug, Default)]
pub struct KurumRegistry {
    kayitlar: HashMap<[u8; 20], KurumKaydi>,
}

impl KurumRegistry {
    pub fn yeni() -> Self {
        KurumRegistry {
            kayitlar: HashMap::new(),
        }
    }

    /// Kurum/firma kaydet: adres -> (ad, kategori, zaman). ILK KAYIT KAZANIR;
    /// adres zaten kayitliysa DOKUNULMAZ (kimlik bozulmaz).
    /// Donus: true = yeni kayit, false = zaten kayitliydi.
    pub fn kaydet(
        &mut self,
        adres: [u8; 20],
        ad: String,
        kategori: KurumKategori,
        zaman: u64,
    ) -> bool {
        if self.kayitlar.contains_key(&adres) {
            return false;
        }
        self.kayitlar.insert(
            adres,
            KurumKaydi {
                ad,
                kategori,
                zaman,
            },
        );
        true
    }

    /// Bir adres hangi kurum/firma? (ad, kategori, zaman). Kayitli degilse None.
    pub fn sorgula(&self, adres: &[u8; 20]) -> Option<&KurumKaydi> {
        self.kayitlar.get(adres)
    }

    /// Kayitli kurum/firma sayisi.
    pub fn len(&self) -> usize {
        self.kayitlar.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kayitlar.is_empty()
    }
}

/// Nonce defteri: adres -> bir sonraki BEKLENEN islem sayaci.
/// REPLAY KORUMASI: her hesabin islemleri 0,1,2,... sirayla numaralanir.
/// Ayni islem (ayni nonce) iki kez islenemez; eski/tekrarli islem reddedilir.
/// Yeni hesap nonce=0'dan baslar. Basarili islem sonrasi nonce 1 artar.
#[derive(Debug, Default)]
pub struct NonceRegistry {
    /// adres -> bir sonraki beklenen nonce (kayit yoksa 0 beklenir).
    nonce: HashMap<[u8; 20], u64>,
}

impl NonceRegistry {
    pub fn yeni() -> Self {
        NonceRegistry {
            nonce: HashMap::new(),
        }
    }

    /// Bir adresin bir sonraki beklenen nonce'u (kayit yoksa 0).
    pub fn beklenen(&self, adres: &[u8; 20]) -> u64 {
        self.nonce.get(adres).copied().unwrap_or(0)
    }

    /// Gelen nonce dogru mu? (beklenen ile esit mi). REPLAY korumasinin kalbi.
    pub fn dogru_mu(&self, adres: &[u8; 20], gelen_nonce: u64) -> bool {
        self.beklenen(adres) == gelen_nonce
    }

    /// Basarili islem sonrasi nonce'u 1 ilerlet (yeni beklenen = eski+1).
    /// SADECE dogru_mu() true dondukten ve islem uygulandiktan sonra cagrilmali.
    pub fn ilerlet(&mut self, adres: &[u8; 20]) {
        let b = self.nonce.entry(*adres).or_insert(0);
        *b = b.saturating_add(1);
    }

    /// Kac adresin nonce kaydi var (en az bir islem yapmis adres sayisi).
    pub fn len(&self) -> usize {
        self.nonce.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nonce.is_empty()
    }
}

#[cfg(test)]
mod tests {

    // FAZ2 KANIT (TGE-acilis vesting): on-satis modeli = %20 TGE hemen + kalan %80
    // 12 ay dogrusal. tge_acik alani dogru calisir; kilitli kisim transfer edilemez.
    #[test]
    fn vesting_tge_acilis_on_satis_modeli() {
        let gun = 86400u64;
        let bas = 1_000_000u64;
        let v = VestingKaydi {
            toplam: 1000,
            baslangic: bas,
            cliff_sure: 0,
            toplam_sure: 360 * gun, // 12 ay
            tge_acik: 200,          // %20 TGE hemen
        };
        // Acilim egrisi
        assert_eq!(v.acilmis(bas - 1), 0, "TGE oncesi 0");
        assert_eq!(v.acilmis(bas), 200, "TGE'de %20 (200) acik");
        assert_eq!(v.kilitli(bas), 800, "TGE'de %80 (800) kilitli");
        assert_eq!(
            v.acilmis(bas + 180 * gun),
            600,
            "6. ay %60 (200 + 80%'in yarisi)"
        );
        assert_eq!(v.acilmis(bas + 360 * gun), 1000, "12. ay %100");

        // Transfer zorlamasi: TGE'de yalniz %20 harcanabilir
        let mut reg = BakiyeRegistry::yeni();
        let sahip = [0x33u8; 20];
        let alici = [0x44u8; 20];
        reg.test_bakiye_ekle(sahip, 1000);
        reg.vesting_ekle(sahip, v);
        reg.zaman_ayarla(bas); // TGE
        assert!(
            matches!(
                reg.transfer(&sahip, &alici, 200),
                TransferSonuc::Basarili { .. }
            ),
            "TGE'de 200 (=%20) transfer OK"
        );
        assert!(
            matches!(
                reg.transfer(&sahip, &alici, 1),
                TransferSonuc::YetersizBakiye { .. }
            ),
            "kalan %80 kilitli -> ek transfer RED"
        );
    }

    #[test]
    fn vesting_kilitli_transfer_edilemez() {
        let mut reg = BakiyeRegistry::yeni();
        let kurucu = [0x11u8; 20];
        let alici = [0x22u8; 20];
        reg.test_bakiye_ekle(kurucu, 1000);
        let baslangic = 1_000_000u64;
        reg.vesting_ekle(
            kurucu,
            VestingKaydi {
                toplam: 1000,
                baslangic,
                cliff_sure: 180 * 86400,
                toplam_sure: 730 * 86400,
                tge_acik: 0,
            },
        );
        // Cliff icinde (3. ay) -> kilitli, transfer REDDEDILMELI
        reg.zaman_ayarla(baslangic + 90 * 86400);
        let s1 = reg.transfer(&kurucu, &alici, 100);
        assert!(
            matches!(s1, TransferSonuc::YetersizBakiye { .. }),
            "cliff icinde -> transfer reddedilmeli"
        );
        // 2 yil sonra -> acildi, transfer BASARILI
        reg.zaman_ayarla(baslangic + 730 * 86400);
        let s2 = reg.transfer(&kurucu, &alici, 100);
        assert!(
            matches!(s2, TransferSonuc::Basarili { .. }),
            "vesting bitti -> transfer basarili"
        );
        // Yari yolda kismen kilitli
        let k = reg.vesting_kilitli(&kurucu, baslangic + 365 * 86400);
        assert!(k > 0, "yari yolda kismen kilitli olmali");
    }
    use super::*;

    fn sym(s: &str) -> [u8; 8] {
        let mut out = [0u8; 8];
        let b = s.as_bytes();
        out[..b.len()].copy_from_slice(b);
        out
    }

    #[test]
    fn gercek_token_kabul_edilir() {
        let mut reg = TokenRegistry::yeni();
        let usdc = TokenKaydi::new([0xAA; 20], sym("USDC"));
        assert_eq!(reg.kaydet(usdc), KayitSonucu::Kabul);
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn sahte_token_protokol_seviyesinde_reddedilir() {
        let mut reg = TokenRegistry::yeni();
        // Gercek USDC kaydedilir
        reg.kaydet(TokenKaydi::new([0xAA; 20], sym("USDC")));
        // Sahte USDC: ayni sembol, farkli adres -> REDDEDILMELI
        let sahte = TokenKaydi::new([0xBB; 20], sym("USDC"));
        assert_eq!(
            reg.kaydet(sahte),
            KayitSonucu::TaklitReddedildi {
                taklit_edilen_adres: [0xAA; 20]
            }
        );
        // KRITIK: sahte deftere GIRMEDI (hala 1 kayit)
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn ayni_token_tekrar_eklenmez() {
        let mut reg = TokenRegistry::yeni();
        reg.kaydet(TokenKaydi::new([0xAA; 20], sym("USDC")));
        let ayni = TokenKaydi::new([0xAA; 20], sym("USDC"));
        assert_eq!(reg.kaydet(ayni), KayitSonucu::ZatenKayitli);
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn farkli_tokenler_birlikte_yasar() {
        let mut reg = TokenRegistry::yeni();
        assert_eq!(
            reg.kaydet(TokenKaydi::new([0xAA; 20], sym("USDC"))),
            KayitSonucu::Kabul
        );
        assert_eq!(
            reg.kaydet(TokenKaydi::new([0xBB; 20], sym("DAI"))),
            KayitSonucu::Kabul
        );
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn adres_ile_bul_calisir() {
        let mut reg = TokenRegistry::yeni();
        reg.kaydet(TokenKaydi::new([0xAA; 20], sym("USDC")));
        assert!(reg.adres_ile_bul(&[0xAA; 20]).is_some());
        assert!(reg.adres_ile_bul(&[0xCC; 20]).is_none());
    }

    #[test]
    fn taklit_mi_sorgusu_eklemeden_calisir() {
        let mut reg = TokenRegistry::yeni();
        reg.kaydet(TokenKaydi::new([0xAA; 20], sym("USDC")));
        // Sahte USDC sorgusu -> taklit edilen gercek adresi doner
        let sahte = TokenKaydi::new([0xBB; 20], sym("USDC"));
        assert_eq!(reg.taklit_mi(&sahte), Some([0xAA; 20]));
        // Temiz token -> taklit degil
        let temiz = TokenKaydi::new([0xCC; 20], sym("DAI"));
        assert_eq!(reg.taklit_mi(&temiz), None);
        // Sorgu deftere eklemedi (hala 1)
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn stake_ekle_ve_oku() {
        let mut reg = StakeRegistry::yeni();
        let yeni = reg.stake_ekle(StakeKaydi::new([0xAA; 20], 1000));
        assert_eq!(yeni, 1000);
        assert_eq!(reg.stake_miktari(&[0xAA; 20]), 1000);
        assert!(reg.stake_var_mi(&[0xAA; 20]));
        assert_eq!(reg.staker_sayisi(), 1);
    }

    #[test]
    fn stake_birikir_ayni_adres() {
        let mut reg = StakeRegistry::yeni();
        reg.stake_ekle(StakeKaydi::new([0xAA; 20], 1000));
        let toplam = reg.stake_ekle(StakeKaydi::new([0xAA; 20], 500));
        assert_eq!(toplam, 1500); // birikti
        assert_eq!(reg.stake_miktari(&[0xAA; 20]), 1500);
        assert_eq!(reg.staker_sayisi(), 1); // hala tek adres
    }

    #[test]
    fn stake_etmeyen_adres_hak_yok() {
        let reg = StakeRegistry::yeni();
        assert!(!reg.stake_var_mi(&[0xBB; 20]));
        assert_eq!(reg.stake_miktari(&[0xBB; 20]), 0);
    }

    #[test]
    fn stake_toplam_ve_coklu_adres() {
        let mut reg = StakeRegistry::yeni();
        reg.stake_ekle(StakeKaydi::new([0xAA; 20], 1000));
        reg.stake_ekle(StakeKaydi::new([0xBB; 20], 2000));
        assert_eq!(reg.toplam_stake(), 3000);
        assert_eq!(reg.staker_sayisi(), 2);
        assert!(reg.stake_var_mi(&[0xAA; 20]));
        assert!(reg.stake_var_mi(&[0xBB; 20]));
    }

    // ===== BakiyeRegistry / transfer testleri =====

    #[test]
    #[ignore]
    fn fuzz_kalkan_bakiye() {
        // ADVERSARIAL FUZZ: bakiye/transfer kalkani. EN KRITIK: toplam arz SABIT.
        use super::{BakiyeRegistry, TransferSonuc};
        let turlar: u64 = std::env::var("BAKIYE_TUR")
            .ok()
            .and_then(|x| x.parse().ok())
            .unwrap_or(2000);
        let mut lcg: u64 = 0x8CB92BA72F3D8DD7;
        let mut rng = || {
            lcg = lcg
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            lcg
        };
        for tur in 0..turlar {
            if tur % 1000 == 0 {
                eprintln!("[bakiye] {}/{} tur", tur, turlar);
            }
            let mut reg = BakiyeRegistry::yeni();
            let adres_sayisi = 2 + (rng() % 5) as usize;
            let mut adresler: Vec<[u8; 20]> = Vec::new();
            for i in 0..adres_sayisi {
                let mut a = [0u8; 20];
                a[0] = i as u8;
                for x in a.iter_mut().skip(1) {
                    *x = (rng() & 0xff) as u8;
                }
                adresler.push(a);
                reg.test_bakiye_ekle(a, (rng() % 1_000_000) as u128);
            }
            let baslangic_arz = reg.toplam_arz();
            let islem_sayisi = 10 + (rng() % 40) as usize;
            for _ in 0..islem_sayisi {
                let gonderen = adresler[(rng() % adres_sayisi as u64) as usize];
                let alici = adresler[(rng() % adres_sayisi as u64) as usize];
                let miktar = (rng() % 2_000_000) as u128;
                let onceki_g = reg.bakiye(&gonderen);
                let onceki_a = reg.bakiye(&alici);
                match reg.transfer(&gonderen, &alici, miktar) {
                    TransferSonuc::Basarili { .. } => {
                        if gonderen != alici {
                            if reg.bakiye(&gonderen) != onceki_g - miktar {
                                panic!("KALKAN DELINDI tur={}: gonderenden yanlis dustu", tur);
                            }
                            if reg.bakiye(&alici) != onceki_a + miktar {
                                panic!("KALKAN DELINDI tur={}: aliciya yanlis eklendi", tur);
                            }
                        }
                    }
                    TransferSonuc::YetersizBakiye { .. } => {
                        if reg.bakiye(&gonderen) != onceki_g {
                            panic!(
                                "KALKAN DELINDI tur={}: yetersiz reddedildi ama bakiye degisti",
                                tur
                            );
                        }
                        if onceki_g >= miktar && miktar > 0 && gonderen != alici {
                            panic!("KALKAN HATASI tur={}: yeterliydi ama yetersiz dendi", tur);
                        }
                    }
                    TransferSonuc::GecersizMiktar => {
                        if miktar > 0
                            && gonderen != alici
                            && reg.bakiye(&alici).checked_add(miktar).is_some()
                        {
                            panic!(
                                "KALKAN HATASI tur={}: gecerli transfer GecersizMiktar dendi",
                                tur
                            );
                        }
                    }
                }
                if reg.toplam_arz() != baslangic_arz {
                    panic!(
                        "KALKAN DELINDI tur={}: ARZ DEGISTI! bas={} simdi={}",
                        tur,
                        baslangic_arz,
                        reg.toplam_arz()
                    );
                }
            }
        }
        eprintln!(
            "BAKIYE OK: {} tur, arz korundu, yetersiz bakiye reddedildi",
            turlar
        );
    }

    #[test]
    fn bakiye_basit_transfer() {
        let mut reg = BakiyeRegistry::yeni();
        reg.test_bakiye_ekle([0xAA; 20], 1000);
        let sonuc = reg.transfer(&[0xAA; 20], &[0xBB; 20], 300);
        assert_eq!(
            sonuc,
            TransferSonuc::Basarili {
                gonderen_yeni_bakiye: 700
            }
        );
        assert_eq!(reg.bakiye(&[0xAA; 20]), 700);
        assert_eq!(reg.bakiye(&[0xBB; 20]), 300);
        // TOPLAM ARZ KORUNUR (para yaratilmadi/kaybolmadi).
        assert_eq!(reg.toplam_arz(), 1000);
    }

    #[test]
    fn transfer_cift_harcama_engellenir() {
        let mut reg = BakiyeRegistry::yeni();
        reg.test_bakiye_ekle([0xAA; 20], 100);
        // Sahip olmadigi 500'u gondermeye calis -> REDDEDILMELI.
        let sonuc = reg.transfer(&[0xAA; 20], &[0xBB; 20], 500);
        assert_eq!(
            sonuc,
            TransferSonuc::YetersizBakiye {
                mevcut: 100,
                istenen: 500
            }
        );
        // Bakiyeler DEGISMEDI (cift harcama olmadi).
        assert_eq!(reg.bakiye(&[0xAA; 20]), 100);
        assert_eq!(reg.bakiye(&[0xBB; 20]), 0);
        assert_eq!(reg.toplam_arz(), 100);
    }

    #[test]
    fn transfer_tam_bakiye_gonderilebilir() {
        let mut reg = BakiyeRegistry::yeni();
        reg.test_bakiye_ekle([0xAA; 20], 100);
        let sonuc = reg.transfer(&[0xAA; 20], &[0xBB; 20], 100);
        assert_eq!(
            sonuc,
            TransferSonuc::Basarili {
                gonderen_yeni_bakiye: 0
            }
        );
        assert_eq!(reg.bakiye(&[0xAA; 20]), 0);
        assert_eq!(reg.bakiye(&[0xBB; 20]), 100);
    }

    #[test]
    fn transfer_sifir_miktar_reddedilir() {
        let mut reg = BakiyeRegistry::yeni();
        reg.test_bakiye_ekle([0xAA; 20], 100);
        assert_eq!(
            reg.transfer(&[0xAA; 20], &[0xBB; 20], 0),
            TransferSonuc::GecersizMiktar
        );
    }

    #[test]
    fn transfer_kendine_reddedilir() {
        let mut reg = BakiyeRegistry::yeni();
        reg.test_bakiye_ekle([0xAA; 20], 100);
        assert_eq!(
            reg.transfer(&[0xAA; 20], &[0xAA; 20], 50),
            TransferSonuc::GecersizMiktar
        );
        assert_eq!(reg.bakiye(&[0xAA; 20]), 100);
    }

    #[test]
    fn transfer_overflow_korunur() {
        let mut reg = BakiyeRegistry::yeni();
        reg.test_bakiye_ekle([0xAA; 20], 10);
        reg.test_bakiye_ekle([0xBB; 20], u128::MAX);
        // alici zaten u64::MAX; +10 tasma -> reddedilmeli, bakiyeler korunur.
        assert_eq!(
            reg.transfer(&[0xAA; 20], &[0xBB; 20], 10),
            TransferSonuc::GecersizMiktar
        );
        assert_eq!(reg.bakiye(&[0xAA; 20]), 10);
        assert_eq!(reg.bakiye(&[0xBB; 20]), u128::MAX);
    }

    #[test]
    fn bakiye_18_ondalik_21m_arz_tasmaz() {
        // u128 GECISININ ASIL AMACI: 18 ondalik (BSC/Ethereum) + 21M arz.
        // 21_000_000 * 10^18 = 2.1e25 -> u64'e SIGMAZDI (u64 max ~1.8e19).
        // u128 ile tasmadan islenir; bu, 18 ondalik uyumunun kanitidir.
        let mut reg = BakiyeRegistry::yeni();
        let yirmi_bir_milyon: Tutar = 21_000_000u128 * 1_000_000_000_000_000_000u128; // 21M * 10^18
        reg.test_bakiye_ekle([0xA1; 20], yirmi_bir_milyon);
        assert_eq!(reg.bakiye(&[0xA1; 20]), yirmi_bir_milyon);
        assert_eq!(reg.toplam_arz(), yirmi_bir_milyon);
        // Transfer de bu buyuklukte calisir: yarisini gonder.
        let yari = yirmi_bir_milyon / 2;
        let sonuc = reg.transfer(&[0xA1; 20], &[0xB2; 20], yari);
        assert!(matches!(sonuc, TransferSonuc::Basarili { .. }));
        assert_eq!(reg.bakiye(&[0xB2; 20]), yari);
        // Arz korundu (yaratim/yok olma yok).
        assert_eq!(reg.toplam_arz(), yirmi_bir_milyon);
    }

    #[test]
    fn transfer_zincirleme_arz_korunur() {
        let mut reg = BakiyeRegistry::yeni();
        reg.test_bakiye_ekle([0xAA; 20], 1000);
        reg.transfer(&[0xAA; 20], &[0xBB; 20], 400);
        reg.transfer(&[0xBB; 20], &[0xCC; 20], 150);
        reg.transfer(&[0xCC; 20], &[0xAA; 20], 50);
        // Her transfer sonrasi toplam arz HEP 1000 (degismez).
        assert_eq!(reg.toplam_arz(), 1000);
        assert_eq!(reg.bakiye(&[0xAA; 20]), 650);
        assert_eq!(reg.bakiye(&[0xBB; 20]), 250);
        assert_eq!(reg.bakiye(&[0xCC; 20]), 100);
    }

    // ===== RecordRegistry / belge dogrulama testleri =====

    #[test]
    fn belge_kaydet_dogrula() {
        let mut reg = RecordRegistry::yeni();
        let h = [0x11u8; 32];
        // Baslangicta kayitli degil.
        assert_eq!(reg.dogrula(&h), None);
        // Kaydet -> yeni kayit (true).
        assert!(reg.kaydet(h, [0xAA; 20], 1_000_000));
        // Dogrula -> kim + ne zaman.
        let kayit = reg.dogrula(&h).expect("kayitli olmali");
        assert_eq!(kayit.kaydeden, [0xAA; 20]);
        assert_eq!(kayit.zaman, 1_000_000);
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn belge_ilk_kayit_kazanir() {
        let mut reg = RecordRegistry::yeni();
        let h = [0x22u8; 32];
        // Ilk kaydeden: AA, zaman 100.
        assert!(reg.kaydet(h, [0xAA; 20], 100));
        // Ayni hash, baska biri (BB), sonraki zaman -> REDDEDILIR (false), ilk korunur.
        assert!(!reg.kaydet(h, [0xBB; 20], 200));
        let kayit = reg.dogrula(&h).unwrap();
        assert_eq!(kayit.kaydeden, [0xAA; 20], "ilk kaydeden korunur");
        assert_eq!(kayit.zaman, 100, "ilk zaman korunur");
        assert_eq!(reg.len(), 1, "tek kayit");
    }

    #[test]
    fn belge_farkli_hash_farkli_kayit() {
        let mut reg = RecordRegistry::yeni();
        assert!(reg.kaydet([0x01; 32], [0xAA; 20], 100));
        assert!(reg.kaydet([0x02; 32], [0xBB; 20], 200));
        assert_eq!(reg.len(), 2);
        assert_eq!(reg.dogrula(&[0x01; 32]).unwrap().kaydeden, [0xAA; 20]);
        assert_eq!(reg.dogrula(&[0x02; 32]).unwrap().kaydeden, [0xBB; 20]);
    }

    #[test]
    fn belge_kayitsiz_hash_dogrulanmaz() {
        let reg = RecordRegistry::yeni();
        assert_eq!(reg.dogrula(&[0xFF; 32]), None);
        assert!(reg.is_empty());
    }

    // ===== KurumRegistry / kurum kimlik testleri =====

    #[test]
    fn kurum_kaydet_sorgula() {
        let mut reg = KurumRegistry::yeni();
        assert!(reg.sorgula(&[0xAA; 20]).is_none());
        assert!(reg.kaydet(
            [0xAA; 20],
            "Tapu Mudurlugu".into(),
            KurumKategori::Devlet,
            1000
        ));
        let k = reg.sorgula(&[0xAA; 20]).expect("kayitli olmali");
        assert_eq!(k.ad, "Tapu Mudurlugu");
        assert_eq!(k.kategori, KurumKategori::Devlet);
        assert_eq!(k.zaman, 1000);
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn kurum_kategori_karismaz() {
        let mut reg = KurumRegistry::yeni();
        reg.kaydet(
            [0xAA; 20],
            "Tapu Mudurlugu".into(),
            KurumKategori::Devlet,
            100,
        );
        reg.kaydet(
            [0xBB; 20],
            "Ahmet Insaat Ltd".into(),
            KurumKategori::Ozel,
            200,
        );
        // KESIN AYRIM: devlet kurumu Devlet, ozel firma Ozel — karismaz.
        assert_eq!(
            reg.sorgula(&[0xAA; 20]).unwrap().kategori,
            KurumKategori::Devlet
        );
        assert_eq!(
            reg.sorgula(&[0xBB; 20]).unwrap().kategori,
            KurumKategori::Ozel
        );
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn kurum_ilk_kayit_kazanir() {
        let mut reg = KurumRegistry::yeni();
        // Ayni adres: ilk kayit (Devlet, "Gercek Kurum").
        assert!(reg.kaydet(
            [0xCC; 20],
            "Gercek Kurum".into(),
            KurumKategori::Devlet,
            100
        ));
        // Ayni adres tekrar (sahte deneme: farkli ad/kategori) -> REDDEDILIR (false).
        assert!(!reg.kaydet([0xCC; 20], "Sahte Kurum".into(), KurumKategori::Ozel, 200));
        // Ilk kayit korunur (kimlik bozulmaz).
        let k = reg.sorgula(&[0xCC; 20]).unwrap();
        assert_eq!(k.ad, "Gercek Kurum");
        assert_eq!(k.kategori, KurumKategori::Devlet);
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn kurum_kayitsiz_adres_sorgulanmaz() {
        let reg = KurumRegistry::yeni();
        assert!(reg.sorgula(&[0xFF; 20]).is_none());
        assert!(reg.is_empty());
    }

    // ===== NonceRegistry / replay korumasi testleri =====

    #[test]
    fn nonce_baslangic_sifir() {
        let reg = NonceRegistry::yeni();
        // Yeni hesap nonce=0 bekler.
        assert_eq!(reg.beklenen(&[0xAA; 20]), 0);
        assert!(reg.dogru_mu(&[0xAA; 20], 0));
        assert!(!reg.dogru_mu(&[0xAA; 20], 1)); // 1 erken
    }

    #[test]
    fn nonce_ilerler() {
        let mut reg = NonceRegistry::yeni();
        // 0 dogru -> uygula -> ilerlet -> simdi 1 beklenir.
        assert!(reg.dogru_mu(&[0xAA; 20], 0));
        reg.ilerlet(&[0xAA; 20]);
        assert_eq!(reg.beklenen(&[0xAA; 20]), 1);
        assert!(reg.dogru_mu(&[0xAA; 20], 1));
        assert!(!reg.dogru_mu(&[0xAA; 20], 0)); // 0 artik eski (replay)
    }

    #[test]
    fn nonce_replay_reddedilir() {
        let mut reg = NonceRegistry::yeni();
        // Ilk islem: nonce=0, uygula, ilerlet.
        assert!(reg.dogru_mu(&[0xAA; 20], 0));
        reg.ilerlet(&[0xAA; 20]);
        // AYNI islemi tekrar oyna (nonce=0) -> REDDEDILMELI (replay).
        assert!(
            !reg.dogru_mu(&[0xAA; 20], 0),
            "eski nonce replay reddedilir"
        );
        // Gelecekteki nonce (2) de reddedilir (sira atlanamaz).
        assert!(!reg.dogru_mu(&[0xAA; 20], 2), "sira atlanamaz");
        // Sadece dogru sira (1) kabul.
        assert!(reg.dogru_mu(&[0xAA; 20], 1));
    }

    #[test]
    fn nonce_adresler_bagimsiz() {
        let mut reg = NonceRegistry::yeni();
        reg.ilerlet(&[0xAA; 20]); // AA: 0->1
                                  // BB hala 0 bekler (her hesap kendi sayacini tutar).
        assert_eq!(reg.beklenen(&[0xAA; 20]), 1);
        assert_eq!(reg.beklenen(&[0xBB; 20]), 0);
    }

    #[test]
    #[ignore]
    fn fuzz_kalkan_replay() {
        // ADVERSARIAL FUZZ: replay/cift-harcama kalkani (NonceRegistry).
        use super::NonceRegistry;
        let turlar: u64 = std::env::var("NONCE_TUR")
            .ok()
            .and_then(|x| x.parse().ok())
            .unwrap_or(2000);
        let mut lcg: u64 = 0xC2B2AE3D27D4EB4F;
        let mut rng = || {
            lcg = lcg
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            lcg
        };
        for tur in 0..turlar {
            if tur % 1000 == 0 {
                eprintln!("[nonce] {}/{} tur", tur, turlar);
            }
            let mut reg = NonceRegistry::yeni();
            let adres_sayisi = 1 + (rng() % 4) as usize;
            let mut adresler: Vec<[u8; 20]> = Vec::new();
            let mut beklenen: Vec<u64> = Vec::new();
            for _ in 0..adres_sayisi {
                let mut a = [0u8; 20];
                for x in a.iter_mut() {
                    *x = (rng() & 0xff) as u8;
                }
                adresler.push(a);
                beklenen.push(0);
            }
            let islem_sayisi = 5 + (rng() % 20) as usize;
            for _ in 0..islem_sayisi {
                let idx = (rng() % adres_sayisi as u64) as usize;
                let adres = adresler[idx];
                let exp = beklenen[idx];
                let senaryo = rng() % 3;
                let gelen = match senaryo {
                    0 => exp,
                    1 => {
                        if exp > 0 {
                            exp - 1
                        } else {
                            exp + 1
                        }
                    }
                    _ => exp + 1 + (rng() % 5),
                };
                let kabul = reg.dogru_mu(&adres, gelen);
                let olmali = gelen == exp;
                if kabul != olmali {
                    panic!(
                        "KALKAN DELINDI tur={}: nonce={} beklenen={} kalkan={} olmali={}",
                        tur, gelen, exp, kabul, olmali
                    );
                }
                if kabul {
                    reg.ilerlet(&adres);
                    beklenen[idx] += 1;
                }
                if exp > 0 && reg.dogru_mu(&adres, exp - 1) {
                    panic!("KALKAN DELINDI tur={}: kullanilmis nonce {} tekrar kabul edildi (replay gecti!)", tur, exp - 1);
                }
            }
        }
        eprintln!(
            "NONCE OK: {} tur, replay ve atlama reddedildi, dogru nonce ilerledi",
            turlar
        );
    }
}

/// Testnet eslestirme defteri: test_adresi -> gercek (mainnet odul) adresi.
/// BIR KERELIK: bir test adresi bir kez eslestirilir, SONRADAN DEGISTIRILEMEZ
/// (guvenlik: eslesme sabit kalir, odul calinamaz). Testnet katilimcisinin
/// odulu, mainnette bu eslesmis gercek adrese gider.
#[derive(Debug, Default)]
pub struct EslestirmeRegistry {
    /// test_adresi -> gercek_adres
    eslesmeler: std::collections::HashMap<[u8; 20], [u8; 20]>,
    /// gercek_adres -> test_adresi (TERS INDEKS: bir gercek adres sadece BIR test
    /// adresine baglanabilir; ayni gercek adres ikinci kez kullanilamaz = anti-Sybil).
    kullanilan_gercek: std::collections::HashMap<[u8; 20], [u8; 20]>,
}

impl EslestirmeRegistry {
    pub fn yeni() -> Self {
        EslestirmeRegistry {
            eslesmeler: std::collections::HashMap::new(),
            kullanilan_gercek: std::collections::HashMap::new(),
        }
    }

    /// Eslestir. BIR KERELIK: test adresi zaten eslesmisse DEGISTIRMEZ,
    /// mevcut eslesmeyi dondurur (false = yeni eklenmedi). Yeni ise ekler (true).
    pub fn eslestir(&mut self, test_adresi: [u8; 20], gercek_adres: [u8; 20]) -> bool {
        // KURAL 1: test adresi zaten eslesmisse degistirme.
        if self.eslesmeler.contains_key(&test_adresi) {
            return false;
        }
        // KURAL 2 (anti-Sybil): bu GERCEK adres baska bir test adresine bagliysa REDDET.
        // Bir gercek adres = bir test cuzdani. Ayni gercek adres tekrar kullanilamaz.
        if self.kullanilan_gercek.contains_key(&gercek_adres) {
            return false;
        }
        self.eslesmeler.insert(test_adresi, gercek_adres);
        self.kullanilan_gercek.insert(gercek_adres, test_adresi);
        true
    }

    /// Bir gercek adres zaten kullanilmis mi (baska test'e bagli mi)?
    pub fn gercek_kullanilmis(&self, gercek_adres: &[u8; 20]) -> bool {
        self.kullanilan_gercek.contains_key(gercek_adres)
    }

    /// Bir test adresinin eslesmis gercek adresini dondurur (yoksa None).
    pub fn sorgula(&self, test_adresi: &[u8; 20]) -> Option<[u8; 20]> {
        self.eslesmeler.get(test_adresi).copied()
    }

    /// Toplam eslesme sayisi.
    pub fn sayisi(&self) -> usize {
        self.eslesmeler.len()
    }

    /// Tum eslesmeler (diske yazma / mainnet gecisi icin).
    pub fn tum_eslesmeler(&self) -> Vec<([u8; 20], [u8; 20])> {
        self.eslesmeler.iter().map(|(k, v)| (*k, *v)).collect()
    }
}

/// Ön satış dağıtım defteri: odeme_ref -> dağıtım kaydı (kalıcı, şeffaf).
/// AMAÇ: her ön satış dağıtımını (kime, ne kadar, hangi ödeme karşılığı, ne zaman)
/// kalıcı tutmak. ÇİFTE DAĞITIM ENGELİ: aynı odeme_ref ikinci kez kullanılamaz
/// (bir ödeme = bir dağıtım; owner yanlışlıkla iki kez gönderse bile çifte AIDAG gitmez).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnSatisKaydi {
    pub alici: [u8; 20],
    pub aidag: Tutar,
    pub lsc_hediye: Tutar,
    pub zaman: u64,
}

pub struct OnSatisRegistry {
    /// odeme_ref -> kayit. Bir odeme_ref bir kez kullanilir.
    kayitlar: std::collections::HashMap<u64, OnSatisKaydi>,
}

impl OnSatisRegistry {
    pub fn yeni() -> Self {
        OnSatisRegistry {
            kayitlar: std::collections::HashMap::new(),
        }
    }

    /// Bu odeme_ref daha once kullanildi mi? (cifte dagitim kontrolu)
    pub fn kullanilmis(&self, odeme_ref: u64) -> bool {
        self.kayitlar.contains_key(&odeme_ref)
    }

    /// Dagitimi kaydet. odeme_ref zaten varsa REDDET (false = kaydedilmedi, cifte engel).
    /// Yeni ise ekler (true). Owner, kaydet=true donerse dagitimi yapmali.
    pub fn kaydet(
        &mut self,
        odeme_ref: u64,
        alici: [u8; 20],
        aidag: Tutar,
        lsc_hediye: Tutar,
        zaman: u64,
    ) -> bool {
        if self.kayitlar.contains_key(&odeme_ref) {
            return false;
        }
        self.kayitlar.insert(
            odeme_ref,
            OnSatisKaydi {
                alici,
                aidag,
                lsc_hediye,
                zaman,
            },
        );
        true
    }

    /// Bir odeme_ref'in kaydini dondurur (yoksa None). Sorgu/itiraz icin.
    pub fn sorgula(&self, odeme_ref: u64) -> Option<&OnSatisKaydi> {
        self.kayitlar.get(&odeme_ref)
    }

    /// Toplam dagitim sayisi.
    pub fn sayisi(&self) -> usize {
        self.kayitlar.len()
    }

    /// Toplam dagitilan AIDAG (denetim/seffaflik icin).
    pub fn toplam_aidag(&self) -> u128 {
        self.kayitlar.values().map(|k| k.aidag).sum()
    }

    /// Tum kayitlar (diske yazma / mainnet gecisi icin).
    pub fn tum_kayitlar(&self) -> Vec<(u64, OnSatisKaydi)> {
        self.kayitlar.iter().map(|(k, v)| (*k, v.clone())).collect()
    }

    /// Bir alicinin TUM alimlari (kisisel gorunum: "benim alimlarim").
    /// Zamana gore sirali (eskiden yeniye). odeme_ref + kayit dondurur.
    pub fn adrese_gore(&self, alici: &[u8; 20]) -> Vec<(u64, OnSatisKaydi)> {
        let mut v: Vec<(u64, OnSatisKaydi)> = self
            .kayitlar
            .iter()
            .filter(|(_, k)| &k.alici == alici)
            .map(|(r, k)| (*r, k.clone()))
            .collect();
        v.sort_by_key(|(_, k)| k.zaman);
        v
    }

    /// TUM kayitlar, zamana gore sirali (genel seffaf liste + hareket cizelgesi icin).
    pub fn tum_kayitlar_sirali(&self) -> Vec<(u64, OnSatisKaydi)> {
        let mut v = self.tum_kayitlar();
        v.sort_by_key(|(_, k)| k.zaman);
        v
    }

    /// Bir alicinin toplam aldigi AIDAG (kisisel ozet).
    pub fn adres_toplam_aidag(&self, alici: &[u8; 20]) -> u128 {
        self.kayitlar
            .values()
            .filter(|k| &k.alici == alici)
            .map(|k| k.aidag)
            .sum()
    }
}

#[cfg(test)]
mod on_satis_testleri {
    use super::OnSatisRegistry;

    #[test]
    fn cifte_dagitim_engellenir() {
        let mut r = OnSatisRegistry::yeni();
        let alici = [1u8; 20];
        // ilk dagitim: odeme_ref=100 -> kabul (true)
        assert!(
            r.kaydet(100, alici, 5000, 10, 1234),
            "ilk dagitim kabul edilmeli"
        );
        assert_eq!(r.sayisi(), 1);
        assert_eq!(r.toplam_aidag(), 5000);
        // ayni odeme_ref=100 tekrar -> RED (false), cifte dagitim yok
        assert!(
            !r.kaydet(100, alici, 5000, 10, 1235),
            "ayni odeme_ref reddedilmeli"
        );
        assert_eq!(r.sayisi(), 1, "kayit sayisi degismemeli");
        assert_eq!(
            r.toplam_aidag(),
            5000,
            "toplam AIDAG degismemeli (cifte yok)"
        );
        // farkli odeme_ref=200 -> kabul
        assert!(
            r.kaydet(200, alici, 3000, 0, 1236),
            "yeni odeme_ref kabul edilmeli"
        );
        assert_eq!(r.sayisi(), 2);
        assert_eq!(r.toplam_aidag(), 8000);
        // sorgula calisiyor mu
        let kayit = r.sorgula(100).expect("kayit bulunmali");
        assert_eq!(kayit.aidag, 5000);
        assert_eq!(kayit.alici, alici);
    }

    #[test]
    fn kullanilmis_kontrolu() {
        let mut r = OnSatisRegistry::yeni();
        assert!(!r.kullanilmis(42), "baslangicta kullanilmamis olmali");
        r.kaydet(42, [2u8; 20], 1000, 0, 999);
        assert!(r.kullanilmis(42), "kayittan sonra kullanilmis olmali");
    }

    #[test]
    fn adrese_gore_kisisel_gorunum() {
        let mut r = OnSatisRegistry::yeni();
        let ali = [0xAAu8; 20];
        let veli = [0xBBu8; 20];
        // Ali iki alim yapar (farkli zaman), Veli bir alim
        r.kaydet(1, ali, 50, 0, 100); // Ali: 50 AIDAG, zaman 100
        r.kaydet(2, veli, 30, 0, 150); // Veli: 30 AIDAG
        r.kaydet(3, ali, 20, 0, 200); // Ali: 20 AIDAG, zaman 200

        // Ali kendi alimlarini sorgular -> 2 alim, zamana sirali
        let ali_alimlar = r.adrese_gore(&ali);
        assert_eq!(ali_alimlar.len(), 2, "Ali'nin 2 alimi olmali");
        assert_eq!(ali_alimlar[0].1.zaman, 100, "ilk alim eski (zaman 100)");
        assert_eq!(ali_alimlar[1].1.zaman, 200, "ikinci alim yeni (zaman 200)");
        assert_eq!(r.adres_toplam_aidag(&ali), 70, "Ali toplam 70 AIDAG aldi");

        // Veli kendi alimlari -> 1 alim
        assert_eq!(r.adrese_gore(&veli).len(), 1);
        assert_eq!(r.adres_toplam_aidag(&veli), 30);

        // Genel sirali liste -> 3 alim, zamana gore
        let hepsi = r.tum_kayitlar_sirali();
        assert_eq!(hepsi.len(), 3);
        assert_eq!(hepsi[0].1.zaman, 100);
        assert_eq!(hepsi[2].1.zaman, 200);
        // Genel toplam
        assert_eq!(r.toplam_aidag(), 100, "toplam satilan 50+30+20=100");
    }

    #[test]
    #[ignore]
    fn fuzz_kalkan_sahte_belge() {
        // ADVERSARIAL FUZZ: belge/diploma kalkani (RecordRegistry).
        use super::RecordRegistry;
        // hash = ham 32-bayt (mevcut testlerle tutarli; keccak katmani ayri).
        let turlar: u64 = std::env::var("BELGE_TUR")
            .ok()
            .and_then(|x| x.parse().ok())
            .unwrap_or(2000);
        let mut lcg: u64 = 0x9E6C63D0676A9A99;
        let mut rng = || {
            lcg = lcg
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            lcg
        };
        let h32 = |rng: &mut dyn FnMut() -> u64| {
            let mut a = [0u8; 32];
            for x in a.iter_mut() {
                *x = (rng() & 0xff) as u8;
            }
            a
        };
        let a20 = |rng: &mut dyn FnMut() -> u64| {
            let mut a = [0u8; 20];
            for x in a.iter_mut() {
                *x = (rng() & 0xff) as u8;
            }
            a
        };
        for tur in 0..turlar {
            if tur % 1000 == 0 {
                eprintln!("[belge] {}/{} tur", tur, turlar);
            }
            let mut reg = RecordRegistry::yeni();
            let gercek_hash = h32(&mut rng);
            let kaydeden = a20(&mut rng);
            reg.kaydet(gercek_hash, kaydeden, 1000 + tur);
            // 1: kayitli belge dogrulanmali
            if reg.dogrula(&gercek_hash).is_none() {
                panic!("KALKAN DELINDI tur={}: kayitli belge dogrulanamadi", tur);
            }
            // 2: sahte (kayitsiz) hash reddedilmeli
            let mut sahte = h32(&mut rng);
            while sahte == gercek_hash {
                sahte = h32(&mut rng);
            }
            if reg.dogrula(&sahte).is_some() {
                panic!("KALKAN DELINDI tur={}: sahte belge dogrulandi!", tur);
            }
            // 3: tahrif — gercek hash'in 1 byte'i degismis -> reddedilmeli
            let mut tahrif = gercek_hash;
            let idx = (rng() % 32) as usize;
            tahrif[idx] = tahrif[idx].wrapping_add(1);
            if tahrif != gercek_hash && reg.dogrula(&tahrif).is_some() {
                panic!(
                    "KALKAN DELINDI tur={}: tahrif edilmis belge dogrulandi!",
                    tur
                );
            }
            // 4: ilk kayit kazanir — ayni hash farkli kaydeden ile ezilmemeli
            let baska = a20(&mut rng);
            if reg.kaydet(gercek_hash, baska, 9999) {
                panic!("KALKAN DELINDI tur={}: kayitli belge ezildi!", tur);
            }
            if reg.dogrula(&gercek_hash).unwrap().kaydeden != kaydeden {
                panic!("KALKAN DELINDI tur={}: ilk kaydeden degisti", tur);
            }
        }
        eprintln!(
            "BELGE OK: {} tur, sahte/tahrif reddedildi, ilk kayit korundu",
            turlar
        );
    }
}
