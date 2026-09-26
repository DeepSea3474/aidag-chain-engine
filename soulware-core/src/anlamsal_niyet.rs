//! P2 · Anlamsal yedek niyet eşleştirme. Kural tabanlı yönlendirici (yonlendirici.rs) eşleşmezse, soru
//! çok dilli gömme modeli (paraphrase-multilingual-MiniLM) ile niyet başına örnek cümlelere benzetilir.
//! Yalnızca SALT OKUNUR, zararsız araç niyetleri; "diger" sınıfı (araca gitmemesi gereken sorular) karşıtlık sağlar.
//! Karar: en iyi sınıf "diger" değil, benzerlik >= esik ve en iyi - ikinci sınıf >= fark. Aksi hâlde None.
//! Örnekler değerlendirme setlerinden (ana, gizli v1/v2/v3) ALINMADI.

use crate::embed::{kosinus, Embedder};

pub const ORNEKLER: &[(&str, &[&str])] = &[
    ("kimlik", &[
        "Senin adın ne?", "İsmin nedir?", "Sana nasıl hitap etmeliyim?", "Kim olduğunu ve adını söyle",
        "Bu asistanın adı ne?", "What is your name?",
    ]),
    ("ag-durumu", &[
        "Ağ şu an çalışıyor mu?", "Zincir sağlıklı mı, bir sorun var mı?", "Düğümler ayakta mı?",
        "Mainnet'in şu anki durumu nedir?", "Ağda kesinti var mı?", "Kaç düğüm aktif çalışıyor?",
    ]),
    ("on-satis", &[
        "Ön satışta şimdiye kadar ne kadar satıldı?", "Ön satışın güncel durumu nedir?", "Hangi fiyat kademesindeyiz?",
        "Ön satışta ne kadar AIDAG kaldı?", "TGE tarihi belli mi?", "Kaç kişi ön satıştan aldı?",
    ]),
    ("kaynak-listesi", &[
        "Hangi kaynaklardan öğreniyorsun?", "Bilgilerini nereden alıyorsun?", "Kaynak listen nedir?",
        "Hangi belgelerle eğitildin?", "Bilgi tabanında hangi kaynaklar var?",
    ]),
    ("diger", &[
        "Blokzincir nedir?", "Ön satış nasıl çalışır?", "Ön satışa nasıl katılırım?", "Ağ güvenliği için ne önerirsin?",
        "Merhaba, nasılsın?", "Bir şiir yazar mısın?", "Türkiye'nin başkenti neresi?", "Parolamı nasıl güçlü yaparım?",
        "Neden yalnızca USDT kabul ediliyor?", "Akıllı kontrat nedir?", "Hava bugün nasıl?", "Bana bir tarif öner",
        "Düğüm nasıl kurulur?", "AIDAG token fiyatı gelecek yıl ne olur?", "Dosyanın adını nasıl değiştiririm?",
        "Kaynak kodu nerede?", "Yapay zekâ nasıl öğrenir?", "Güvenlik açığı nasıl bildirilir?",
    ]),
];

/// Varsayılan eşikler (26 Eylül 2026 kalibrasyonu; ana set araç dışı soruları + birim testi ifadeleri, bkz.
/// testler::kalibrasyon): araca gitmemesi gereken sorularda araç sınıfı benzerliği en fazla 0,52; doğru
/// eşleşmeler 0,56–0,96. Yanlış araca gitmek kaçırmaktan kötü olduğu için temkinli seçildi.
pub const ESIK: f32 = 0.60;
pub const FARK: f32 = 0.05;

/// Saf karar: (sınıf, en yüksek benzerlik) listesinden niyet.
pub fn karar_ver<'a>(skorlar: &[(&'a str, f32)], esik: f32, fark: f32) -> Option<&'a str> {
    let mut s: Vec<(&str, f32)> = skorlar.to_vec();
    s.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let (en, en_skor) = *s.first()?;
    let ikinci = s.get(1).map(|x| x.1).unwrap_or(0.0);
    if en == "diger" || en_skor < esik || en_skor - ikinci < fark {
        return None;
    }
    Some(en)
}

pub struct AnlamsalNiyet {
    vektorler: Vec<(&'static str, Vec<f32>)>,
    pub esik: f32,
    pub fark: f32,
}

impl AnlamsalNiyet {
    pub fn yukle(e: &Embedder, esik: f32, fark: f32) -> anyhow::Result<Self> {
        let mut vektorler = Vec::new();
        for (sinif, cumleler) in ORNEKLER {
            for c in *cumleler {
                vektorler.push((*sinif, e.embed(c)?));
            }
        }
        Ok(AnlamsalNiyet { vektorler, esik, fark })
    }

    /// Sınıf başına en yüksek benzerlik.
    pub fn skorlar(&self, qv: &[f32]) -> Vec<(&'static str, f32)> {
        let mut en: std::collections::BTreeMap<&'static str, f32> = Default::default();
        for (sinif, v) in &self.vektorler {
            let b = kosinus(qv, v);
            let e = en.entry(sinif).or_insert(f32::MIN);
            if b > *e {
                *e = b;
            }
        }
        en.into_iter().collect()
    }

    pub fn sinifla(&self, e: &Embedder, soru: &str) -> Option<&'static str> {
        let qv = e.embed(soru).ok()?;
        karar_ver(&self.skorlar(&qv), self.esik, self.fark)
    }
}

#[cfg(test)]
mod testler {
    use super::*;

    #[test]
    fn karar_kurali() {
        assert_eq!(karar_ver(&[("ag-durumu", 0.82), ("diger", 0.60), ("kimlik", 0.30)], 0.7, 0.05), Some("ag-durumu"));
        assert_eq!(karar_ver(&[("ag-durumu", 0.82), ("diger", 0.85)], 0.7, 0.05), None); // "diger" kazandı
        assert_eq!(karar_ver(&[("ag-durumu", 0.65), ("diger", 0.40)], 0.7, 0.05), None); // eşik altı
        assert_eq!(karar_ver(&[("ag-durumu", 0.80), ("on-satis", 0.78)], 0.7, 0.05), None); // belirsiz
        assert_eq!(karar_ver(&[], 0.7, 0.05), None);
    }

    /// Kalibrasyon (elle): gerçek model dosyasıyla benzerlikleri basar. `cargo test -- --ignored kalibrasyon --nocapture`
    #[test]
    #[ignore]
    fn kalibrasyon() {
        let yol = std::env::var("KALIBRASYON_MODEL").unwrap_or_else(|_| "/root/aidag-lsc/soulware-models/embed-minilm".into());
        let e = Embedder::load(&yol).expect("model");
        let a = AnlamsalNiyet::yukle(&e, ESIK, FARK).unwrap();
        let olumlu: &[(&str, &str)] = &[
            ("kimlik", "Adın neydi?"), ("kimlik", "Size hangi isimle seslenmeliyim?"), ("kimlik", "Kendine ne isim veriyorsun?"),
            ("ag-durumu", "AIDAG ağı canlı mı?"), ("ag-durumu", "Ağ nasıl gidiyor?"), ("ag-durumu", "Mainnet'te bir arıza var mı?"),
            ("ag-durumu", "Sistem ayakta mı?"), ("ag-durumu", "Ağın sağlığı nasıl?"),
            ("on-satis", "Presale'e kaç kişi katıldı?"), ("on-satis", "Ön satışın son durumu nedir?"),
            ("on-satis", "Satışlar ne durumda?"), ("on-satis", "Şu an hangi kademedeyiz?"),
            ("kaynak-listesi", "Bilgi kaynakların neler?"), ("kaynak-listesi", "Neyle eğitildin?"),
        ];
        // Ana set (geliştirme verisi) araç dışı sorular: hiçbiri araca gitmemeli.
        let olumsuz = [
            "Hash fonksiyonu nedir, basitçe anlatır mısın?", "Merhaba, nasılsın?", "AIDAG'ın 2030'daki fiyatı ne olacak?",
            "Ön satışta neden yalnızca USDT kabul ediliyor?", "KUBRA neden imza atamıyor?", "SQL enjeksiyonuna karşı web uygulamamı nasıl korurum?",
            "Bir güvenlik olayında ilk 24 saatte ne yapmalıyım?", "Nginx'te istek hız sınırını nasıl ayarlarım?",
            "Mars'ta ilk insan yerleşimi hangi yıl kurulacak?", "Yarın BIST 100 kaç puandan kapanır?",
            "Komşumun telefon numarası ne?", "Kurumsal ağlar ile ilgili ne yapıyorsunuz?", "Blok zinciri nasıl çalışır?",
        ];
        for (beklenen, q) in olumlu {
            let qv = e.embed(q).unwrap();
            let mut s = a.skorlar(&qv);
            s.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap());
            println!("OLUMLU  {beklenen:15} {:?} <- {q}  {:?}", karar_ver(&s, ESIK, FARK), &s[..2]);
        }
        for q in olumsuz {
            let qv = e.embed(q).unwrap();
            let mut s = a.skorlar(&qv);
            s.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap());
            println!("OLUMSUZ {:?} <- {q}  {:?}", karar_ver(&s, ESIK, FARK), &s[..2]);
        }
    }
}
