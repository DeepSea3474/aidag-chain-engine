//! resmi — AIDAG/KUBRA RESMİ KAYNAK katmanı (grounding önceliği).
//! ════════════════════════════════════════════════════════════════════════
//! AIDAG/KUBRA ile ilgili sorular genel korpusa (Wikipedia, makaleler) DÜŞMEZ:
//! önce README + whitepaper'dan derlenmiş, yalnızca doğrulanmış bilgileri içeren
//! kısa resmi belgeler (kb.aidag.json) aranır. Resmi kaynak yoksa model
//! çağrılmaz → sabit "doğrulanmış bilgim yok" (uydurma YOK).
//! Token/ön satış/fiyat bilgisi bu belgelerde bilinçli olarak YOKTUR.

use crate::retrieval::{anahtar_var, sade, Pasaj};
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct ResmiBelge {
    pub baslik: String,
    pub metin: String,
    #[serde(default)]
    pub url: Option<String>,
    /// Sade (ascii, küçük harf) anahtarlar; eşleşme sayısı = skor.
    #[serde(default)]
    pub anahtarlar: Vec<String>,
}

pub const DOGRULANMAMIS: &str = "Bu konuda doğrulanmış bilgim yok.";

pub const ISIM_CEVABI: &str = "Adım KUBRA. Bu ismi bana AIDAG-Chain'in kurucusu verdi.";

/// Diskten yükle (yoksa/bozuksa boş → resmi katman devre dışı, servis çökmez).
pub fn yukle(yol: &str) -> Vec<ResmiBelge> {
    std::fs::read(yol)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Soru AIDAG ekosistemi (zincir/KUBRA/belge doğrulama/RWA) hakkında mı?
pub fn aidag_konusu_mu(prompt: &str) -> bool {
    let s = sade(prompt);
    [
        "aidag", "kubra", "soulware", "soulwareai", "rwa", "dijital ikiz", "digital twin",
        "belge dogrulama", "belge kayd",
    ]
    .iter()
    .any(|a| anahtar_var(&s, a))
}

/// "Adın ne / ismini kim verdi" → sabit cevap (model yorumlamasın).
pub fn isim_sorusu_mu(prompt: &str) -> bool {
    let s = sade(prompt);
    [
        "adin ne", "ismin ne", "adini kim", "ismini kim", "sana kim isim", "sana bu ismi",
        "sana bu adi", "adini ne koy", "ismini ne koy", "adin nereden", "ismin nereden",
        "senin adin", "senin ismin", "what is your name", "who named you",
    ]
    .iter()
    .any(|a| anahtar_var(&s, a))
}

/// Soruya en uygun resmi belgeler (skor = eşleşen anahtar sayısı). En iyinin
/// yarısından zayıf olanlar elenir; en fazla `k` belge. Hiç eşleşme yoksa boş.
pub fn sec(belgeler: &[ResmiBelge], prompt: &str, k: usize) -> Vec<Pasaj> {
    let s = sade(prompt);
    let mut skorlu: Vec<(i64, &ResmiBelge)> = belgeler
        .iter()
        .map(|b| (b.anahtarlar.iter().filter(|a| anahtar_var(&s, a)).count() as i64, b))
        .filter(|(skor, _)| *skor > 0)
        .collect();
    skorlu.sort_by(|a, b| b.0.cmp(&a.0));
    let top = skorlu.first().map(|x| x.0).unwrap_or(0);
    skorlu
        .into_iter()
        .filter(|(skor, _)| skor * 2 >= top)
        .take(k.max(1))
        .map(|(skor, b)| Pasaj {
            kaynak: "aidag-resmi".into(),
            baslik: b.baslik.clone(),
            metin: b.metin.clone(),
            url: b.url.clone(),
            skor,
        })
        .collect()
}

/// Resmi kaynağa KİLİTLİ kullanıcı istemi: kaynak dışı bilgi eklenmez; kaynak soruyu
/// karşılamıyorsa model sabit cümleyi söyler.
pub fn resmi_user(prompt: &str, baglam: &str) -> String {
    format!(
        "ÖNEMLİ: Yanıtını YALNIZCA TÜRKÇE yaz. Aşağıda AIDAG-Chain'in RESMİ ve DOĞRULANMIŞ kaynakları var. \
Soruyu YALNIZCA bu kaynaklardaki bilgiyle cevapla; kaynakta olmayan hiçbir bilgi, rakam, özellik, \
tarih veya iddia EKLEME, tahmin yürütme. Token, fiyat, ön satış veya yatırım konusunda bilgi verme. \
Kaynak soruyla ilgiliyse (kısmen de olsa) kaynaktaki bilgiyi özetleyerek cevap ver; kaynakta belirtilen \
sınırlar varsa onları da söyle. Yalnızca kaynaklar soruyla HİÇ ilgili değilse şunu yaz: \"{DOGRULANMAMIS}\" \
Kısa, net ve resmi bir dille yanıtla (en fazla 5-6 cümle).\n\nRESMİ KAYNAKLAR:\n{baglam}\nSORU:\n{prompt}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn belgeler() -> Vec<ResmiBelge> {
        serde_json::from_str(include_str!("../../soulware-knowledge/kb.aidag.json")).unwrap()
    }

    #[test]
    fn konu_tespiti() {
        for q in ["Aidag chain nedir", "KUBRA nedir", "Rwa icin cozumu nedir", "Kübra'nın görevi ne"] {
            assert!(aidag_konusu_mu(q), "{q}");
        }
        assert!(!aidag_konusu_mu("Türkiye'nin başkenti neresi"));
        assert!(!aidag_konusu_mu("Rwanda nerede"));
    }

    #[test]
    fn dogru_belge_secilir() {
        let b = belgeler();
        assert_eq!(sec(&b, "Aidag chain nedir", 2)[0].baslik, "AIDAG-Chain nedir");
        assert_eq!(sec(&b, "KUBRA nedir", 2)[0].baslik, "KUBRA nedir");
        assert_eq!(sec(&b, "Rwa icin cozumu nedir", 2)[0].baslik, "Dijital ikiz ve RWA yaklaşımı");
        assert_eq!(sec(&b, "Belge doğrulama nasıl çalışır", 2)[0].baslik, "Belge doğrulama nasıl çalışır");
        // Resmi belgeler token/fiyat/ön satış içermez.
        for x in &b {
            let m = sade(&x.metin);
            for yasak in ["fiyat", "on satis", "presale", "usdt", "21 000 000", "arz"] {
                assert!(!anahtar_var(&m, yasak), "{} içinde '{yasak}'", x.baslik);
            }
        }
    }

    #[test]
    fn isim_sorulari() {
        for q in ["Adın ne?", "senin ismini kim verdi", "Sana bu ismi kim verdi"] {
            assert!(isim_sorusu_mu(q), "{q}");
        }
        assert!(!isim_sorusu_mu("Ankara'nın adı nereden gelir"));
    }
}
