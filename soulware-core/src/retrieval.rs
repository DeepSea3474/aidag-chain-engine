//! retrieval — KUBRA grounding kaynak katmanı (RAG'in "R"si).
//! ════════════════════════════════════════════════════════════════════════
//! "En güçlü yapay zekaların kullandığı kaynaklar" = otoriter bilgi tabanları
//! (Wikipedia, ansiklopedik/olgusal metinler). KUBRA küçük bir model olsa da
//! cevabı BELLEKTEN değil KAYNAKTAN üretirse halüsinasyon çözülür.
//!
//! PLUGGABLE: iki kaynak. (1) EGEMEN YEREL DEPO — offline, her yerde çalışır,
//! sorgu sızdırmaz, offline Wikipedia dump'ıyla büyür (gizli/egemen vizyona uygun).
//! (2) CANLI WIKIPEDIA — bloklu olmayan ağlarda (worker PC'leri) ek kaynak; bu
//! sunucu (Contabo) Wikimedia'dan IP-bloklu olduğu için varsayılan KAPALI.
//!
//! DÜRÜSTLÜK: uydurma kaynak YOK. Bulunan pasajlar gerçek belgelerden; hiçbir
//! kaynak bulunamazsa boş döner → model "Bilmiyorum" der (abstention).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Yerel depodaki bir belge (ansiklopedik metin parçası).
#[derive(Clone, Serialize, Deserialize)]
pub struct Belge {
    pub baslik: String,
    pub metin: String,
    #[serde(default)]
    pub url: Option<String>,
}

/// Getirilen bir pasaj (kaynak + metin + skor). KUBRA'ya bağlam olarak sunulur.
#[derive(Clone, Serialize)]
pub struct Pasaj {
    pub kaynak: String, // "yerel" | "wikipedia"
    pub baslik: String,
    pub metin: String,
    pub url: Option<String>,
    pub skor: i64,
}

/// Türkçe/İngilizce durak kelimeler + ek parçaları (apostrof bölmesinden gelen
/// "türkiye'nin" → "nin" gibi ekler skoru yanıltmasın). Bunlar grounding'de
/// gürültü; içerik kelimesi ("ankara") ayırt edici olmalı.
const DURAK: &[&str] = &[
    "ve", "ile", "bir", "bu", "su", "da", "de", "ki", "mi", "mu", "ne", "en",
    "icin", "gibi", "kadar", "nedir", "kimdir", "nerede", "neresi", "hangi", "kac",
    // ek/çekim parçaları (apostrof sonrası):
    "nin", "nun", "nen", "den", "dan", "deki", "daki", "ler", "lar", "leri", "lari",
    "dir", "dur", "del", "ten", "tan", "nde", "nda", "nden", "ndan",
    // İngilizce:
    "the", "an", "of", "is", "are", "what", "who", "where", "which", "how", "and", "to", "in",
];

/// Türkçe özel harfleri ascii'ye katla: kullanıcı "baskenti" yazsa da "başkenti"
/// ile eşleşsin. Aksi halde ş/ç/ğ/ı/ö/ü uyuşmazlığı retrieval'ı kaçırır.
fn katla(c: char) -> char {
    match c {
        'ç' => 'c', 'ğ' => 'g', 'ı' => 'i', 'ş' => 's', 'ö' => 'o', 'ü' => 'u', 'â' => 'a', 'î' => 'i', 'û' => 'u',
        other => other,
    }
}

/// Sorguyu/metni token'lara ayır: küçük harf + Türkçe→ascii katlama, alfanümerik
/// dışını ayraç yap, kısa (<3) ve durak kelimeleri at.
pub fn tokenle(s: &str) -> Vec<String> {
    s.to_lowercase()
        .chars()
        .map(katla)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .filter(|w| w.chars().count() >= 3 && !DURAK.contains(w))
        .map(|w| w.to_string())
        .collect()
}

/// Egemen yerel bilgi deposu. JSON dizisi olarak diskte tutulur; ingest ile büyür.
pub struct Depo {
    pub belgeler: Vec<Belge>,
    pub yol: String,
}

impl Depo {
    /// Diskten yükle (yoksa boş). Bozuk/eksikse boş depo (servis çökmez).
    pub fn yukle(yol: &str) -> Depo {
        let belgeler = std::fs::read(yol)
            .ok()
            .and_then(|b| serde_json::from_slice::<Vec<Belge>>(&b).ok())
            .unwrap_or_default();
        Depo { belgeler, yol: yol.to_string() }
    }

    /// Atomik kaydet (.tmp + rename).
    pub fn kaydet(&self) {
        if let Some(dir) = std::path::Path::new(&self.yol).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = format!("{}.tmp", self.yol);
        if serde_json::to_vec_pretty(&self.belgeler)
            .ok()
            .and_then(|b| std::fs::write(&tmp, b).ok())
            .is_some()
        {
            let _ = std::fs::rename(&tmp, &self.yol);
        }
    }

    /// Belge ekle (ingest) + kaydet. Aynı başlık varsa metni günceller.
    pub fn ekle(&mut self, b: Belge) {
        if let Some(mevcut) = self.belgeler.iter_mut().find(|x| x.baslik.eq_ignore_ascii_case(&b.baslik)) {
            mevcut.metin = b.metin;
            mevcut.url = b.url;
        } else {
            self.belgeler.push(b);
        }
        self.kaydet();
    }

    /// IDF (ters belge frekansı) ağırlığı ×100. Yaygın kelime ("başkenti" birçok
    /// belgede) düşük; nadir/ayırt edici kelime ("Ankara") yüksek ağırlık alır.
    /// Böylece SADECE yaygın kelime eşleşmesi grounding saymaz (Japonya→İzmir tuzağı).
    fn idf100(&self) -> HashMap<String, i64> {
        let n = self.belgeler.len().max(1) as f64;
        let mut df: HashMap<String, usize> = HashMap::new();
        for b in &self.belgeler {
            let mut set: HashSet<String> = HashSet::new();
            set.extend(tokenle(&b.baslik));
            set.extend(tokenle(&b.metin));
            for t in set {
                *df.entry(t).or_insert(0) += 1;
            }
        }
        df.into_iter()
            .map(|(t, d)| (t, ((1.0 + n / d as f64).ln() * 100.0) as i64))
            .collect()
    }

    /// En iyi k pasajı getir (IDF-ağırlıklı skor). Başlık eşleşmesi ×3.
    /// `min_skor` altındaki belgeler ELENIR — alakasız/yaygın-kelime eşleşmesi
    /// kaynak sayılmaz → model uydurmaya zorlanmaz, sahte cite yapılmaz.
    /// `nispi_yuzde`: 2.+ pasajlar en iyi skorun bu yüzdesinin altındaysa ELENIR
    /// (zayıf "dolgu" kaynak sunulmaz — grounding temiz kalır). 0 = kapalı.
    pub fn ara(&self, sorgu: &str, k: usize, min_skor: i64, nispi_yuzde: i64) -> Vec<Pasaj> {
        let q = tokenle(sorgu);
        if q.is_empty() {
            return vec![];
        }
        let w = self.idf100();
        let mut skorlu: Vec<Pasaj> = self
            .belgeler
            .iter()
            .filter_map(|b| {
                let baslik_tok = tokenle(&b.baslik);
                let metin_tok = tokenle(&b.metin);
                let mut skor = 0i64;
                for qt in &q {
                    let agirlik = *w.get(qt).unwrap_or(&0); // korpusta yoksa 0 (katkı yok)
                    if baslik_tok.iter().any(|t| t == qt) {
                        skor += agirlik * 3;
                    }
                    let say = metin_tok.iter().filter(|t| *t == qt).count().min(3) as i64;
                    skor += agirlik * say;
                }
                if skor < min_skor {
                    return None;
                }
                Some(Pasaj {
                    kaynak: "yerel".into(),
                    baslik: b.baslik.clone(),
                    metin: b.metin.clone(),
                    url: b.url.clone(),
                    skor,
                })
            })
            .collect();
        skorlu.sort_by(|a, b| b.skor.cmp(&a.skor));
        skorlu.truncate(k);
        // NİSPİ EŞİK: en iyiye göre çok zayıf pasajları at (dolgu kaynak sunma).
        if nispi_yuzde > 0 {
            if let Some(top) = skorlu.first().map(|p| p.skor) {
                let esik = top.saturating_mul(nispi_yuzde) / 100;
                skorlu.retain(|p| p.skor >= esik);
            }
        }
        skorlu
    }
}

/// CANLI WIKIPEDIA (opsiyonel): sorguyu başlık kabul edip summary REST ucundan
/// özet çeker (Wikipedia başlığı normalize/yönlendirir). Bloklu ağda None döner.
/// Bu sunucuda (Contabo) Wikimedia IP-bloklu → varsayılan kapalı; worker PC'de açık.
pub async fn wiki_getir(
    http: &reqwest::Client,
    langs: &[String],
    sorgu: &str,
) -> Option<Pasaj> {
    let baslik = sorgu.trim().replace(' ', "_");
    for lang in langs {
        let url = format!("https://{lang}.wikipedia.org/api/rest_v1/page/summary/{baslik}");
        let resp = http
            .get(&url)
            .header("User-Agent", "SoulwareAI-KUBRA/0.1 (grounding)")
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            continue;
        }
        let v: serde_json::Value = resp.json().await.ok()?;
        let extract = v.get("extract").and_then(|e| e.as_str()).unwrap_or("");
        if extract.trim().len() < 20 {
            continue;
        }
        let baslik = v.get("title").and_then(|t| t.as_str()).unwrap_or(sorgu).to_string();
        let sayfa_url = v
            .get("content_urls")
            .and_then(|c| c.get("desktop"))
            .and_then(|d| d.get("page"))
            .and_then(|p| p.as_str())
            .map(|s| s.to_string());
        return Some(Pasaj {
            kaynak: format!("wikipedia:{lang}"),
            baslik,
            metin: extract.to_string(),
            url: sayfa_url,
            skor: 5,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ornek_depo() -> Depo {
        Depo {
            yol: String::new(),
            belgeler: vec![
                Belge { baslik: "Ankara".into(), metin: "Ankara, Türkiye'nin başkenti ve İç Anadolu'da bir şehirdir.".into(), url: None },
                Belge { baslik: "Türkiye".into(), metin: "Türkiye bir ülkedir. Başkenti Ankara, en büyük şehri İstanbul'dur.".into(), url: None },
                Belge { baslik: "İstanbul".into(), metin: "İstanbul Türkiye'nin en kalabalık şehridir; başkenti değildir.".into(), url: None },
                Belge { baslik: "Fransa".into(), metin: "Fransa bir ülkedir. Başkenti Paris'tir.".into(), url: None },
            ],
        }
    }

    #[test]
    fn korpustaki_soru_grounded_olur() {
        let d = ornek_depo();
        let p = d.ara("Türkiye'nin başkenti neresi?", 2, 150, 40);
        assert!(!p.is_empty(), "Türkiye sorusu kaynak bulmalı");
        // En iyi pasaj Türkiye ya da Ankara olmalı (ikisi de doğru cevabı içerir).
        assert!(matches!(p[0].baslik.as_str(), "Türkiye" | "Ankara"));
    }

    #[test]
    fn korpusta_olmayan_soru_kaynaksiz() {
        let d = ornek_depo();
        // "Japonya" korpusta yok; yalnız "başkenti" (yaygın) eşleşir → IDF+eşik eler.
        let p = d.ara("Japonya'nın başkenti neresi?", 2, 150, 40);
        assert!(p.is_empty(), "Japonya kaynaksız olmalı (uydurmaya zorlanmaz), bulunan: {:?}",
            p.iter().map(|x| &x.baslik).collect::<Vec<_>>());
    }

    #[test]
    fn ascii_turkce_katlama() {
        // Kullanıcı Türkçe harf yazmadan sorsa da eşleşmeli.
        let d = ornek_depo();
        let p = d.ara("turkiye baskenti", 2, 150, 40);
        assert!(!p.is_empty(), "ascii 'baskenti' Türkçe 'başkenti' ile eşleşmeli");
    }

    #[test]
    fn nispi_esik_zayif_dolguyu_atar() {
        // Güçlü bir eşleşme + zayıf bir eşleşme olduğunda, nispi eşik zayıfı atmalı.
        let d = ornek_depo();
        // "Ankara başkenti Türkiye" → Ankara/Türkiye güçlü; başka doc zayıf kalır.
        let p = d.ara("Ankara başkenti", 3, 150, 40);
        assert!(!p.is_empty());
        // Dönen tüm pasajlar en iyinin %40'ından iyi olmalı (dolgu yok).
        let top = p[0].skor;
        assert!(p.iter().all(|x| x.skor * 100 >= top * 40), "zayıf dolgu pasaj kaldı: {:?}",
            p.iter().map(|x| (&x.baslik, x.skor)).collect::<Vec<_>>());
    }

    #[test]
    fn ek_parcasi_gurultu_yapmaz() {
        // "nin" eki tek başına kaynak eşleştirmemeli (durak kelime).
        assert!(!tokenle("türkiye'nin").contains(&"nin".to_string()));
    }
}

/// Pasajları KUBRA'ya sunulacak bağlam metnine çevir (numaralı, kırpılmış).
pub fn baglam_yap(pasajlar: &[Pasaj], max_metin: usize) -> String {
    let mut out = String::new();
    for (i, p) in pasajlar.iter().enumerate() {
        let metin: String = p.metin.chars().take(max_metin).collect();
        out.push_str(&format!("[{}] {} ({}): {}\n", i + 1, p.baslik, p.kaynak, metin));
    }
    out
}
