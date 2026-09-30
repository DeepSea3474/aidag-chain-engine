//! Kurumsal kayıtlar: KARARLAR.md (karar maddeleri) ve KAYNAKLAR.md (onaylı kaynak listesi).
//! Yalnızca OKUNUR; KUBRA bu dosyaları değiştirmez. Cevaplar dosyadaki metinden deterministik üretilir.

use crate::retrieval;

#[derive(Debug, Clone)]
pub struct KararMadde {
    pub no: u32,
    pub baslik: String,
    pub metin: String,
}

impl KararMadde {
    pub fn kunye(&self) -> String {
        format!("K-{:02} · {}", self.no, self.baslik)
    }
}

/// "### K-NN · Başlık" bölümlerini ayrıştır. Bölüm, bir sonraki başlığa (# ile başlayan satır) ya da "---"ya kadar sürer.
pub fn kararlari_ayristir(md: &str) -> Vec<KararMadde> {
    let mut out: Vec<KararMadde> = Vec::new();
    let mut acik: Option<KararMadde> = None;
    for satir in md.lines() {
        let t = satir.trim_end();
        if let Some(b) = t.strip_prefix("### K-") {
            if let Some(m) = acik.take() {
                out.push(m);
            }
            let rakam: String = b.chars().take_while(|c| c.is_ascii_digit()).collect();
            let baslik = b[rakam.len()..]
                .trim_start_matches(|c: char| c == ' ' || c == '·')
                .trim()
                .to_string();
            if let Ok(no) = rakam.parse() {
                acik = Some(KararMadde {
                    no,
                    baslik,
                    metin: String::new(),
                });
            }
            continue;
        }
        if t.starts_with('#') || t == "---" {
            if let Some(m) = acik.take() {
                out.push(m);
            }
            continue;
        }
        if let Some(m) = acik.as_mut() {
            if !(m.metin.is_empty() && t.is_empty()) {
                m.metin.push_str(t);
                m.metin.push('\n');
            }
        }
    }
    if let Some(m) = acik.take() {
        out.push(m);
    }
    for m in &mut out {
        m.metin = m.metin.trim().to_string();
    }
    out
}

pub fn kararlari_yukle(yol: &str) -> Vec<KararMadde> {
    std::fs::read_to_string(yol)
        .map(|s| kararlari_ayristir(&s))
        .unwrap_or_default()
}

/// Karar aracı cevabı: istenen maddelerin metni (dosyadan birebir). Bulunamayan numara açıkça söylenir.
pub fn karar_cevabi(kararlar: &[KararMadde], nolar: &[u32]) -> (String, bool) {
    let mut parcalar = Vec::new();
    let mut hepsi_bulundu = true;
    for n in nolar {
        match kararlar.iter().find(|k| k.no == *n) {
            Some(k) => parcalar.push(format!("KARARLAR.md {}\n{}", k.kunye(), k.metin)),
            None => {
                hepsi_bulundu = false;
                parcalar.push(format!(
                    "KARARLAR.md'de K-{n:02} diye bir karar bulunamadı."
                ))
            }
        }
    }
    (parcalar.join("\n\n"), hepsi_bulundu)
}

/// KAYNAKLAR.md özeti: "## " başlıkları ve tablo satırlarındaki Durum sayımı.
pub fn kaynak_ozeti(md: &str) -> String {
    let mut basliklar = Vec::new();
    let mut toplam = 0usize;
    let mut durumlar: std::collections::BTreeMap<String, usize> = Default::default();
    for satir in md.lines() {
        let t = satir.trim();
        if let Some(b) = t.strip_prefix("## ") {
            let b = b.trim();
            if b.contains('·') {
                basliklar.push(b.to_string());
            }
        } else if t.starts_with('|') && !t.starts_with("|---") && !t.starts_with("| Kaynak") {
            let hucre: Vec<&str> = t.trim_matches('|').split('|').map(|x| x.trim()).collect();
            if hucre.len() >= 4 {
                toplam += 1;
                *durumlar
                    .entry(hucre[hucre.len() - 1].to_string())
                    .or_default() += 1;
            }
        }
    }
    let durum_metni = durumlar
        .iter()
        .map(|(d, n)| format!("{d}: {n}"))
        .collect::<Vec<_>>()
        .join(", ");
    let eklenen = durumlar.get("Eklendi").copied().unwrap_or(0);
    format!(
        "Onaylı kaynak listem KAYNAKLAR.md'de. Başlıklar: {}. Listede {toplam} kaynak var ({durum_metni}). \
Şu an bilgi tabanıma eklenmiş kaynak sayısı: {eklenen}. Bir kaynak ancak lisansı resmî sayfasından doğrulanıp \
kurucu onayı ve değerlendirme setinden geçtikten sonra eklenir (K-08).",
        basliklar.join("; ")
    )
}

/// Gerekçe sorusu mu ("neden", "niye", "niçin", "gerekçe", "sebebi", "why")?
pub fn neden_sorusu_mu(soru: &str) -> bool {
    let s = retrieval::sade(soru);
    s.split(' ').any(|t| {
        matches!(t, "neden" | "niye" | "nicin" | "why")
            || t.starts_with("gerekce")
            || t.starts_with("sebeb")
    })
}

/// "Neden" soruları için en ilgili karar maddeleri (kelime örtüşmesi; en iyinin yarısından zayıflar elenir).
pub fn ilgili_kararlar<'a>(
    kararlar: &'a [KararMadde],
    soru: &str,
    k: usize,
) -> Vec<&'a KararMadde> {
    let q: std::collections::BTreeSet<String> = retrieval::tokenle(soru).into_iter().collect();
    if q.is_empty() {
        return vec![];
    }
    let mut skor: Vec<(usize, &KararMadde)> = kararlar
        .iter()
        .map(|m| {
            // Başlık kararın konusudur: başlık eşleşmesi 2 puan, yalnız gövde eşleşmesi 1 puan.
            let kume = |metin: &str| -> std::collections::BTreeSet<String> {
                retrieval::tokenle(metin).into_iter().collect()
            };
            let (bas, gov) = (kume(&m.baslik), kume(&m.metin));
            let var = |t: &std::collections::BTreeSet<String>, w: &String| {
                t.iter()
                    .any(|x| x == w || (w.len() >= 5 && x.starts_with(w.as_str())))
            };
            let puan: usize = q
                .iter()
                .map(|w| {
                    if var(&bas, w) {
                        2
                    } else if var(&gov, w) {
                        1
                    } else {
                        0
                    }
                })
                .sum();
            (puan, m)
        })
        .filter(|(s, _)| *s >= 3)
        .collect();
    skor.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.no.cmp(&b.1.no)));
    let en_iyi = skor.first().map(|x| x.0).unwrap_or(0);
    skor.into_iter()
        .filter(|(s, _)| s * 2 >= en_iyi)
        .take(k)
        .map(|(_, m)| m)
        .collect()
}

#[cfg(test)]
mod testler {
    use super::*;

    const ORNEK: &str = "# Kararlar\n\n## 2. Yapay zekâ\n\n### K-05 · KUBRA'nın imza yetkisi yoktur (Eylül 2026)\n- İlke: öneri.\n\n\
### K-20 · Ön satışta yalnızca USDT (BEP-20) kabul edilir (Eylül 2026)\n- Karar: yalnız USDT.\n- Minimum 10 USDT.\n\n---\n\n## 5. Yöntem\n";

    #[test]
    fn ayristirma() {
        let k = kararlari_ayristir(ORNEK);
        assert_eq!(k.len(), 2);
        assert_eq!(k[1].no, 20);
        assert!(k[1].baslik.starts_with("Ön satışta yalnızca USDT"));
        assert!(k[1].metin.contains("Minimum 10 USDT"));
        assert!(!k[0].metin.contains("K-20"));
        assert_eq!(
            k[0].kunye(),
            "K-05 · KUBRA'nın imza yetkisi yoktur (Eylül 2026)"
        ); // KARARLAR.md ile aynı biçim
    }

    #[test]
    fn cevap_ve_bulunamayan() {
        let k = kararlari_ayristir(ORNEK);
        let (c, ok) = karar_cevabi(&k, &[20]);
        assert!(ok && c.starts_with("KARARLAR.md K-20 ·") && c.contains("yalnız USDT"));
        let (c, ok) = karar_cevabi(&k, &[99]);
        assert!(!ok && c.contains("K-99"));
    }

    #[test]
    fn ilgili_karar_bulunur() {
        let k = kararlari_ayristir(ORNEK);
        let r = ilgili_kararlar(&k, "Ön satışta neden yalnızca USDT kabul ediliyor?", 2);
        assert_eq!(r.first().map(|m| m.no), Some(20));
        assert!(ilgili_kararlar(&k, "Mars'a ne zaman gidilir?", 2).is_empty());
    }

    #[test]
    fn depodaki_kararlar_neden_sorularini_karsilar() {
        // Değerlendirme seti D kategorisi: soru -> beklenen karar maddesi (gerçek KARARLAR.md).
        let yol = concat!(env!("CARGO_MANIFEST_DIR"), "/../KARARLAR.md");
        let k = kararlari_yukle(yol);
        assert!(k.len() >= 24, "KARARLAR.md okunamadı: {yol}");
        for (soru, no) in [
            ("Ön satışta neden yalnızca USDT kabul ediliyor?", 20),
            ("Ön satışta minimum alım neden 10 USDT?", 20),
            ("Genesis vesting başlangıcı neden 2100'e alındı?", 16),
            ("KUBRA neden imza atamıyor?", 5),
            ("AIDAG'da neden DAO yok?", 1),
            (
                "KUBRA cevap kanıtları neden tuzlu hash ile kaydediliyor?",
                7,
            ),
            ("Ana ağda neden hâlâ ed25519 kullanılıyor?", 22),
            ("Kritik yetkiler neden çoklu imzaya bağlı?", 2),
            ("KUBRA neden kendi kendine güncellenmiyor?", 8),
            ("KUBRA neden istismar kodu yazmıyor?", 23),
        ] {
            assert!(neden_sorusu_mu(soru), "{soru}");
            let r: Vec<u32> = ilgili_kararlar(&k, soru, 2).iter().map(|m| m.no).collect();
            assert!(r.contains(&no), "{soru} -> {r:?} (beklenen K-{no})");
        }
        assert!(!neden_sorusu_mu("Ön satış ne durumda?"));
    }

    #[test]
    fn kaynak_ozeti_sayar() {
        let md = "## 1 · Rust\n\n| Kaynak | Lisans (ilk tahmin) | Lisans (doğrulandı) | Öncelik | Durum |\n|---|---|---|---|---|\n\
| A | MIT | MIT | Yüksek | Bekliyor |\n| B | x | y | — | Eklenmez, adıyla anılır |\n";
        let o = kaynak_ozeti(md);
        assert!(
            o.contains("1 · Rust")
                && o.contains("2 kaynak")
                && o.contains("Bekliyor: 1")
                && o.contains("eklenmiş kaynak sayısı: 0")
        );
    }
}
