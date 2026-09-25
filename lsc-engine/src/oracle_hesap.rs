//! Oracle toplama hesabi — SAF, yan etkisiz, float YOK.
//!
//! DETERMINIZM: tum dugumler ayni rapor kumesinden birebir ayni sonucu uretmeli.
//! - Degerler i128 tamsayi (akisin `ondalik`'ina gore olceklenmis).
//! - Siralama TAM: (deger, kurum_adresi) -> esit degerlerde bile sira tek.
//! - Medyan = ALT medyan `sirali[(n-1)/2]`: ortalama/bolme yok, yuvarlama yok.
//! - Sapma karsilastirmasi `|x-r| * 10_000 > |r| * bps` 256-bit tamsayiyla
//!   yapilir -> i128 sinir degerlerinde bile tasma/doygunluk yok.

/// Kurum adresi.
pub type Adres = [u8; 20];

/// (a * b) sonucunu 256-bit (yuksek, dusuk) olarak dondurur. b < 2^16.
fn carp_256(a: u128, b: u16) -> (u128, u128) {
    const M64: u128 = u64::MAX as u128;
    let b = b as u128;
    let dusuk = (a & M64) * b; // < 2^80
    let yuksek = (a >> 64) * b; // < 2^80
    // sonuc = yuksek * 2^64 + dusuk
    // (yuksek << 64) u128'de yalniz DUSUK 128 biti tutar; ust bitler (yuksek >> 64).
    let (toplam, tasma) = dusuk.overflowing_add(yuksek << 64);
    ((yuksek >> 64) + u128::from(tasma), toplam) // < 2^16 + 1: tasmaz
}

/// `x`, referans `r`'den `bps` baz puandan FAZLA mi sapiyor?
/// Kural: `|x - r| * 10_000 > |r| * bps`. r = 0 ise her sifir-disi x sapar.
pub fn sapma_asiyor(x: i128, r: i128, bps: u16) -> bool {
    let fark = carp_256(x.abs_diff(r), crate::tx::BPS);
    let sinir = carp_256(r.unsigned_abs(), bps);
    fark > sinir
}

/// Alt medyan. Girdi SIRALI olmali. Bos ise None.
pub fn alt_medyan(sirali: &[i128]) -> Option<i128> {
    if sirali.is_empty() {
        return None;
    }
    Some(sirali[(sirali.len() - 1) / 2])
}

/// Bir turun toplama sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toplama {
    /// Yayinlanacak deger (elemeden sonra kalanlarin alt medyani).
    pub deger: i128,
    /// Ilk (elemesiz) medyan — eleme referansi.
    pub ilk_medyan: i128,
    /// Hesaba katilan kurumlar (deger sirasina gore).
    pub kullanilan: Vec<Adres>,
    /// Asiri saptigi icin elenen kurumlar (deger sirasina gore).
    pub elenen: Vec<Adres>,
}

/// Raporlari topla: medyan -> asiri sapanlari ele -> kalan >= M ise kalanlarin
/// medyani. Kalan < M ise None (tur kapanmaz, daha fazla rapor beklenir).
/// Girdi sirasi sonucu ETKILEMEZ (icte tam siralanir).
pub fn topla(raporlar: &[(Adres, i128)], esik_m: u8, sapma_bps: u16) -> Option<Toplama> {
    let m = usize::from(esik_m.max(1));
    if raporlar.len() < m {
        return None;
    }
    let mut sirali: Vec<(i128, Adres)> = raporlar.iter().map(|(a, d)| (*d, *a)).collect();
    sirali.sort_unstable(); // tam siralama: (deger, adres) -> tek sonuc
    let degerler: Vec<i128> = sirali.iter().map(|(d, _)| *d).collect();
    let ilk_medyan = alt_medyan(&degerler)?;

    let mut kullanilan = Vec::new();
    let mut kalan_degerler = Vec::new();
    let mut elenen = Vec::new();
    for (d, a) in &sirali {
        if sapma_asiyor(*d, ilk_medyan, sapma_bps) {
            elenen.push(*a);
        } else {
            kullanilan.push(*a);
            kalan_degerler.push(*d);
        }
    }
    if kalan_degerler.len() < m {
        return None;
    }
    Some(Toplama {
        deger: alt_medyan(&kalan_degerler)?,
        ilk_medyan,
        kullanilan,
        elenen,
    })
}

/// Devre kesici: onceki yayinlanmis degere gore ani asiri degisim mi?
/// Onceki yoksa (ilk tur) tetiklenmez.
pub fn kesici_tetiklenir(onceki: Option<i128>, yeni: i128, kesici_bps: u16) -> bool {
    match onceki {
        Some(o) => sapma_asiyor(yeni, o, kesici_bps),
        None => false,
    }
}

/// Veri bayat mi? `simdi - guncelleme > bayat_sn` (zincir saatiyle cagrilir).
pub fn bayat_mi(guncelleme: u64, simdi: u64, bayat_sn: u32) -> bool {
    simdi.saturating_sub(guncelleme) > u64::from(bayat_sn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(n: u8) -> Adres {
        [n; 20]
    }

    #[test]
    fn alt_medyan_tek_ve_cift() {
        assert_eq!(alt_medyan(&[]), None);
        assert_eq!(alt_medyan(&[5]), Some(5));
        assert_eq!(alt_medyan(&[1, 2, 3]), Some(2));
        assert_eq!(alt_medyan(&[1, 2, 3, 4]), Some(2), "cift N: alt medyan, ortalama yok");
        assert_eq!(alt_medyan(&[-9, -3, 0, 7]), Some(-3));
    }

    #[test]
    fn sapma_sinirda_dogru() {
        // r=1000, %2 (200 bps): 1020 sinirda (esit) -> sapmaz; 1021 sapar.
        assert!(!sapma_asiyor(1020, 1000, 200));
        assert!(sapma_asiyor(1021, 1000, 200));
        assert!(!sapma_asiyor(980, 1000, 200));
        assert!(sapma_asiyor(979, 1000, 200));
        // negatif referans: |r| kullanilir
        assert!(!sapma_asiyor(-1020, -1000, 200));
        assert!(sapma_asiyor(-1021, -1000, 200));
        // r = 0: sifir-disi her deger sapar, 0 sapmaz
        assert!(!sapma_asiyor(0, 0, 200));
        assert!(sapma_asiyor(1, 0, 200));
    }

    #[test]
    fn sapma_i128_sinirlarinda_tasmaz() {
        // |MIN - MAX| = 2^128 - 1: tasma yerine dogru 256-bit karsilastirma.
        assert!(sapma_asiyor(i128::MIN, i128::MAX, 10_000));
        assert!(sapma_asiyor(i128::MAX, i128::MIN, 10_000));
        assert!(!sapma_asiyor(i128::MAX, i128::MAX, 1));
        assert!(!sapma_asiyor(i128::MIN, i128::MIN, 1));
        // MAX'a gore %100 (10_000 bps) sinirinda 0 sapmaz, -1 sapar
        assert!(!sapma_asiyor(0, i128::MAX, 10_000));
        assert!(sapma_asiyor(-1, i128::MAX, 10_000));
        // carp_256 tutarliligi: kucuk sayilarda u128 carpimiyla ayni
        for (x, b) in [(0u128, 0u16), (1, 1), (123_456_789, 10_000), (u64::MAX as u128, 65_535)] {
            assert_eq!(carp_256(x, b), (0, x * b as u128));
        }
        assert_eq!(carp_256(u128::MAX, 2), (1, u128::MAX - 1));
    }

    #[test]
    fn topla_esik_altinda_none() {
        assert_eq!(topla(&[(a(1), 10), (a(2), 11)], 3, 200), None);
    }

    #[test]
    fn topla_asiri_sapan_elenir() {
        let r = [(a(1), 1000), (a(2), 1001), (a(3), 999), (a(4), 5000)];
        let t = topla(&r, 3, 200).expect("3 kalan >= M=3");
        assert_eq!(t.ilk_medyan, 1000);
        assert_eq!(t.elenen, vec![a(4)]);
        assert_eq!(t.kullanilan, vec![a(3), a(1), a(2)]);
        assert_eq!(t.deger, 1000);
    }

    #[test]
    fn topla_eleme_sonrasi_esik_altinda_none() {
        // M=3; iki uc deger elenir -> 2 kalir -> tur kapanmaz.
        let r = [(a(1), 1000), (a(2), 1001), (a(3), 9000), (a(4), 1)];
        assert_eq!(topla(&r, 3, 200), None);
    }

    #[test]
    fn topla_girdi_sirasindan_bagimsiz() {
        let temel = vec![(a(9), 7), (a(1), 7), (a(5), 3), (a(3), 12), (a(7), 7), (a(2), -4)];
        let beklenen = topla(&temel, 3, 10_000).unwrap();
        // Deterministik permutasyonlar (rastgelelik yok): tum donusumler + ters.
        for k in 0..temel.len() {
            let mut p = temel.clone();
            p.rotate_left(k);
            assert_eq!(topla(&p, 3, 10_000).unwrap(), beklenen, "donus {k}");
            p.reverse();
            assert_eq!(topla(&p, 3, 10_000).unwrap(), beklenen, "ters donus {k}");
        }
        // esit degerlerde (7,7,7) adres sirasi tek: a(1) < a(7) < a(9)
        let yedi: Vec<Adres> = beklenen
            .kullanilan
            .iter()
            .copied()
            .filter(|x| [a(1), a(7), a(9)].contains(x))
            .collect();
        assert_eq!(yedi, vec![a(1), a(7), a(9)]);
    }

    #[test]
    fn kesici_ve_bayatlik() {
        assert!(!kesici_tetiklenir(None, 1_000_000, 1_000), "ilk tur tetiklemez");
        assert!(!kesici_tetiklenir(Some(1000), 1100, 1_000), "%10 sinirda");
        assert!(kesici_tetiklenir(Some(1000), 1101, 1_000));
        assert!(kesici_tetiklenir(Some(1000), 899, 1_000));
        assert!(!bayat_mi(100, 160, 60), "tam sinir bayat degil");
        assert!(bayat_mi(100, 161, 60));
        assert!(!bayat_mi(200, 100, 60), "saat geride: bayat degil (doygun)");
    }
}
