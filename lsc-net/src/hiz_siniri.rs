//! DENETIM K-07 (ara cozum): yerel hiz siniri (token bucket).
//!
//! Konsensus kurali DEGILDIR: yalnizca bu dugumun disaridan kabul edip isledigi /
//! agina ilettigi yazma trafigini sinirlar. Protokol seviyesinde ucretsiz vertex
//! sorunu (herkes sinirsiz vertex ekleyebilir) ayrica karar gerektirir; bu modul
//! tek bir kaynagin (IP / p2p esi) ve dugumun toplam yazma hizini sinirlar.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;
use std::time::Instant;

/// Anahtar sayisi bu esigi asarsa dolu (bosta) kovalar temizlenir -> bellek sinirli.
const AZAMI_ANAHTAR: usize = 10_000;

#[derive(Clone, Copy)]
struct Kova {
    jeton: f64,
    son: Instant,
}

/// Anahtar basina (IP, PeerId, ...) token bucket + istege bagli genel (tum
/// anahtarlar toplami) kova.
pub struct HizSiniri<K: Hash + Eq + Clone> {
    /// Anahtar basina saniyede eklenen jeton.
    oran: f64,
    /// Anahtar basina azami birikim (ani yuk).
    kapasite: f64,
    /// Genel kova (None = genel sinir yok).
    genel: Option<(f64, f64)>,
    durum: Mutex<(HashMap<K, Kova>, Option<Kova>)>,
}

impl<K: Hash + Eq + Clone> HizSiniri<K> {
    /// `oran`/`kapasite`: anahtar basina; `genel`: (oran, kapasite) toplam sinir.
    pub fn yeni(oran: f64, kapasite: f64, genel: Option<(f64, f64)>) -> Self {
        HizSiniri {
            oran,
            kapasite,
            genel,
            durum: Mutex::new((HashMap::new(), None)),
        }
    }

    /// Bir islem hakki iste. `true` = izin (jeton harcandi), `false` = sinir asildi.
    pub fn izin(&self, anahtar: &K) -> bool {
        self.izin_zamanla(anahtar, Instant::now())
    }

    /// Yalnizca GENEL kovadan hak iste (anahtar basina kova kullanilmaz). Kaynagi
    /// ayirt edilemeyen trafik icin (or. istemci IP basligi olmayan yerel vekil).
    /// Genel sinir tanimli degilse her zaman izin verir.
    pub fn izin_genel(&self) -> bool {
        self.izin_genel_zamanla(Instant::now())
    }

    fn izin_genel_zamanla(&self, simdi: Instant) -> bool {
        let Some((g_oran, g_kapasite)) = self.genel else {
            return true;
        };
        let mut d = self.durum.lock().unwrap_or_else(|z| z.into_inner());
        let g = d.1.unwrap_or(Kova {
            jeton: g_kapasite,
            son: simdi,
        });
        let g = doldur(g, g_oran, g_kapasite, simdi);
        if g.jeton < 1.0 {
            d.1 = Some(g);
            return false;
        }
        d.1 = Some(Kova {
            jeton: g.jeton - 1.0,
            son: g.son,
        });
        true
    }

    fn izin_zamanla(&self, anahtar: &K, simdi: Instant) -> bool {
        let mut d = self.durum.lock().unwrap_or_else(|z| z.into_inner());
        let (kovalar, genel_kova) = &mut *d;

        if kovalar.len() >= AZAMI_ANAHTAR && !kovalar.contains_key(anahtar) {
            let (oran, kapasite) = (self.oran, self.kapasite);
            kovalar.retain(|_, k| doldur(*k, oran, kapasite, simdi).jeton < kapasite);
            if kovalar.len() >= AZAMI_ANAHTAR {
                return false;
            }
        }

        let k = kovalar.get(anahtar).copied().unwrap_or(Kova {
            jeton: self.kapasite,
            son: simdi,
        });
        let k = doldur(k, self.oran, self.kapasite, simdi);
        if k.jeton < 1.0 {
            kovalar.insert(anahtar.clone(), k);
            return false;
        }

        if let Some((g_oran, g_kapasite)) = self.genel {
            let g = genel_kova.unwrap_or(Kova {
                jeton: g_kapasite,
                son: simdi,
            });
            let g = doldur(g, g_oran, g_kapasite, simdi);
            if g.jeton < 1.0 {
                *genel_kova = Some(g);
                kovalar.insert(anahtar.clone(), k);
                return false;
            }
            *genel_kova = Some(Kova {
                jeton: g.jeton - 1.0,
                son: g.son,
            });
        }

        kovalar.insert(
            anahtar.clone(),
            Kova {
                jeton: k.jeton - 1.0,
                son: k.son,
            },
        );
        true
    }
}

fn doldur(k: Kova, oran: f64, kapasite: f64, simdi: Instant) -> Kova {
    let gecen = simdi.saturating_duration_since(k.son).as_secs_f64();
    Kova {
        jeton: (k.jeton + gecen * oran).min(kapasite),
        son: simdi,
    }
}

/// Ortam degiskeninden pozitif sayi oku; yoksa/gecersizse varsayilan.
pub fn ortam_sayisi(ad: &str, varsayilan: f64) -> f64 {
    std::env::var(ad)
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(varsayilan)
}

#[cfg(test)]
mod testler {
    use super::*;
    use std::time::Duration;

    #[test]
    fn kapasite_kadar_izin_sonra_red_zamanla_dolar() {
        let h = HizSiniri::yeni(2.0, 3.0, None);
        let t0 = Instant::now();
        assert!(h.izin_zamanla(&"a", t0));
        assert!(h.izin_zamanla(&"a", t0));
        assert!(h.izin_zamanla(&"a", t0));
        assert!(!h.izin_zamanla(&"a", t0), "kapasite bitti");
        assert!(h.izin_zamanla(&"b", t0), "baska anahtar etkilenmez");
        assert!(
            h.izin_zamanla(&"a", t0 + Duration::from_millis(600)),
            "0.5 sn'de 1 jeton"
        );
    }

    #[test]
    fn genel_sinir_tum_anahtarlari_kapsar() {
        let h = HizSiniri::yeni(100.0, 100.0, Some((1.0, 2.0)));
        let t0 = Instant::now();
        assert!(h.izin_zamanla(&1u32, t0));
        assert!(h.izin_zamanla(&2u32, t0));
        assert!(!h.izin_zamanla(&3u32, t0), "genel kova bitti");
        assert!(h.izin_zamanla(&3u32, t0 + Duration::from_secs(1)));
    }

    #[test]
    fn yalniz_genel_kova_anahtar_kovalarini_tuketmez() {
        let h = HizSiniri::yeni(1.0, 1.0, Some((1.0, 3.0)));
        let t0 = Instant::now();
        assert!(h.izin_genel_zamanla(t0));
        assert!(h.izin_genel_zamanla(t0));
        assert!(
            h.izin_zamanla(&"a", t0),
            "anahtar kovasi dolu, genelde 1 jeton kaldi"
        );
        assert!(!h.izin_genel_zamanla(t0), "genel kova bitti");
        assert!(
            HizSiniri::<u8>::yeni(1.0, 1.0, None).izin_genel(),
            "genel sinir yoksa serbest"
        );
    }

    #[test]
    fn anahtar_sayisi_sinirli() {
        let h = HizSiniri::yeni(1.0, 1.0, None);
        let t0 = Instant::now();
        for i in 0..AZAMI_ANAHTAR as u32 {
            assert!(h.izin_zamanla(&i, t0));
        }
        // Tum kovalar bos (jeton harcandi) -> temizlenemez, yeni anahtar reddedilir.
        assert!(!h.izin_zamanla(&u32::MAX, t0));
        // Zaman gecince kovalar dolar, temizlenir, yeni anahtar kabul edilir.
        assert!(h.izin_zamanla(&u32::MAX, t0 + Duration::from_secs(5)));
    }
}
