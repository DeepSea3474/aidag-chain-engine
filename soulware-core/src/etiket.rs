//! K-06: doğrulanamayan bilgi cevap olmaz. Her cevabın etiketi DETERMİNİSTİK belirlenir (model seçmez):
//!   dogrulanmis : araç cevabı (zincir okuması, hesap, sabit resmî metin) ya da RESMÎ kaynaklı (KARARLAR,
//!                 AIDAG belgeleri) ve içeriği kaynakla yeterince örtüşen cevap (destek oranı >= DESTEK_ESIGI)
//!   oneri       : kaynaksız yöntem/öneri cevabı; kaynakla yeterince desteklenmeyen cevap; genel bilgi deposundan
//!                 (Wikipedia vb.) kaynaklı cevap — içerik henüz otomatik doğrulanmadığı için (başına etiket yazılır)
//!   bilinmiyor  : kaynağı olmayan kesin olgu sorusu (model ÇAĞRILMAZ) ya da modelin "doğrulanmış bilgim yok" demesi
//!   reddedildi  : K-23 kapısı
//!   sohbet      : selamlaşma ve hal-hatır (bilgi iddiası yok)

use crate::retrieval;

pub const DOGRULANMIS: &str = "dogrulanmis";
pub const ONERI: &str = "oneri";
pub const BILINMIYOR: &str = "bilinmiyor";
pub const SOHBET: &str = "sohbet";

pub const ONERI_ONEKI: &str = "Öneri (doğrulanmış bir kaynağa dayanmıyor): ";
pub const KAYNAKLI_ONERI_ONEKI: &str = "Kaynaklı öneri (kaynak içeriği otomatik doğrulanmadı): ";

/// Resmî kaynaklı cevabın "doğrulanmış" sayılması için cevaptaki anlamlı kelimelerin kaynakta bulunma oranı.
/// Ana set gerçek model ölçümü (26 Eylül 2026): desteklenen cevaplar 0,57–0,85; ilgisiz kaynaklı 0,05–0,21;
/// gerekçesi kısmen kaynak dışı olan cevap 0,38. İleride anlamsal doğrulama (NLI) ile değiştirilecek.
pub const DESTEK_ESIGI: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoruTuru {
    Sohbet,
    /// Kesin olgu: sayı, tarih, isim, yer, fiyat, kişisel veri. Kaynak yoksa cevap verilmez.
    KesinOlgu,
    /// Yöntem, açıklama, öneri: kaynak yoksa "öneri" etiketiyle cevaplanabilir.
    Oneri,
}

const KESIN_OLGU: &[&str] = &[
    "kac", "ne kadar", "hangi tarih", "hangi yil", "hangi gun", "ne zaman", "kim ", "kimdir", "kimin",
    "nerede", "neresi", "nereli", "adi ne", "adres", "numara", "telefon", "fiyat", "kac dolar", "tam olarak",
    "yuzde kac", "puan", "ciro", "nufus", "dogum", "how many", "how much", "when ", "who ",
];

const ONERI_ISARETI: &[&str] = &[
    "nasil", "ne yapmali", "ne yapmaliyim", "oner", "tavsiye", "dikkat", "strateji", "surec", "yontem",
    "nedir", "ne ise yarar", "neden onemli", "anlat", "acikla", "fark", "adim", "korun", "onle", "guvenli hale",
    "kullanmali", "secmeli", "nelere", "ne yapmak",
];

/// Sohbet değilse: kesin olgu işareti varsa KesinOlgu (öncelikli), öneri işareti varsa Oneri, yoksa KesinOlgu
/// (belirsizde sıkı taraf: K-06).
pub fn soru_turu(prompt: &str, sohbet: bool) -> SoruTuru {
    if sohbet {
        return SoruTuru::Sohbet;
    }
    let s = format!("{} ", retrieval::sade(prompt));
    if KESIN_OLGU.iter().any(|k| s.contains(k)) {
        return SoruTuru::KesinOlgu;
    }
    if ONERI_ISARETI.iter().any(|k| s.contains(k)) {
        return SoruTuru::Oneri;
    }
    SoruTuru::KesinOlgu
}

/// Cevapta geçersiz [n] atfı var mı (0 ya da kaynak sayısından büyük)? Atıf etiket için şart DEĞİLDİR;
/// ama geçersiz atıf varsa cevap "doğrulanmış" sayılmaz.
pub fn atif_hatali(cevap: &str, kaynak_sayisi: usize) -> bool {
    let b = cevap.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'[' {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && j < b.len() && b[j] == b']' {
                let n: usize = cevap[i + 1..j].parse().unwrap_or(0);
                if n == 0 || n > kaynak_sayisi {
                    return true;
                }
                i = j;
            }
        }
        i += 1;
    }
    false
}

/// İçerik desteği: cevaptaki anlamlı kelimelerin (>= 4 harf, durak kelime değil) kaynak metninde bulunma oranı.
/// Türkçe ekler için 5 harfli kök eşleşmesi kabul edilir ("kararlaştırıldı" ~ "karar...").
pub fn destek_orani(cevap: &str, kaynak: &str) -> f32 {
    let kaynak_k: std::collections::HashSet<String> =
        retrieval::tokenle(kaynak).into_iter().filter(|w| w.chars().count() >= 4).collect();
    let cevap_k: Vec<String> = retrieval::tokenle(cevap).into_iter().filter(|w| w.chars().count() >= 4).collect();
    if cevap_k.is_empty() || kaynak_k.is_empty() {
        return 0.0;
    }
    let kok = |w: &str| w.chars().take(5).collect::<String>();
    let kaynak_kok: std::collections::HashSet<String> = kaynak_k.iter().map(|w| kok(w)).collect();
    let destekli = cevap_k
        .iter()
        .filter(|w| kaynak_k.contains(*w) || (w.chars().count() >= 5 && kaynak_kok.contains(&kok(w))))
        .count();
    destekli as f32 / cevap_k.len() as f32
}

/// Model "doğrulanmış bilgim yok" dedi mi (resmî kaynak talimatındaki sabit metin dahil)?
pub fn bilgi_yok_dedi(cevap: &str) -> bool {
    let s = retrieval::sade(cevap);
    // Gerçek model "bilmiyorum" deyip yine de [1] ekleyebiliyor: ret ifadesi atıftan ÖNCE gelir (dogrulanmis sayılmaz).
    [
        "dogrulanmis bilgim yok", "bu konuda bilgim yok", "kaynaklarda bu bilgi yok", "bilmiyorum",
        "bilgi bulunmamaktadir", "bilgi bulunmuyor", "bilgi yer almiyor", "bilgiye ulasamiyorum",
        "cevap vermek mumkun degil", "yanit vermek mumkun degil", "kesin bir tarih mevcut degil",
        "i don t know", "no information",
    ]
    .iter()
    .any(|k| s.contains(k))
}

/// Kaynaklı model cevabının etiketi.
/// Kaynaklı model cevabının etiketi. `resmi`: kaynaklar KARARLAR.md / AIDAG resmî belgeleri. Genel bilgi deposu
/// (Wikipedia vb.) ve kullanıcının verdiği bağlam, anlamsal doğrulama gelene kadar "doğrulanmış" sayılmaz.
pub fn kaynakli_cevap_etiketi(cevap: &str, kaynak_metni: &str, kaynak_sayisi: usize, resmi: bool) -> &'static str {
    if bilgi_yok_dedi(cevap) {
        BILINMIYOR
    } else if resmi && !atif_hatali(cevap, kaynak_sayisi) && destek_orani(cevap, kaynak_metni) >= DESTEK_ESIGI {
        DOGRULANMIS
    } else {
        ONERI
    }
}

/// Araç cevabının etiketi.
pub fn arac_etiketi(arac_ad: &str) -> &'static str {
    match arac_ad {
        "resmi-kaynak" | "karar-bulunamadi" => BILINMIYOR,
        "yetki-reddi" | "yetki-gasbi" | "guvenlik-reddi" => "reddedildi",
        _ => DOGRULANMIS,
    }
}

/// Kaynaksız öneri modu istemi: rakam, tarih, isim ve kaynak UYDURMA; yalnız genel yöntem.
pub fn oneri_user(prompt: &str) -> String {
    format!(
        "ÖNERİ MODU: Bu soru için elinde doğrulanmış bir kaynak YOK. Yalnızca TÜRKÇE, genel ve yöntemsel bir \
öneri ver. Hiçbir rakam, tarih, isim, kurum, sürüm ya da kaynak UYDURMA; olgu iddiasında bulunma. \
Kısa ve net yanıtla (en fazla 5-6 cümle).\n\nSORU:\n{prompt}"
    )
}

#[cfg(test)]
mod testler {
    use super::*;

    #[test]
    fn kesin_olgu_ve_oneri_ayrimi() {
        for q in ["AIDAG'ın 2030'daki fiyatı ne olacak?", "Komşumun telefon numarası ne?",
                  "Geçen hafta kaç milimetre yağmur yağdı?", "Kurucunun doğum yeri neresi?", "Mars yerleşimi ne zaman kurulacak?"] {
            assert_eq!(soru_turu(q, false), SoruTuru::KesinOlgu, "{q}");
        }
        for q in ["SQL enjeksiyonuna karşı nasıl korunurum?", "OWASP Top 10 nedir?",
                  "Fidye yazılımına karşı hangi yedekleme stratejisini önerirsin?", "XSS açığı nasıl önlenir?"] {
            assert_eq!(soru_turu(q, false), SoruTuru::Oneri, "{q}");
        }
        assert_eq!(soru_turu("Merhaba", true), SoruTuru::Sohbet);
        // Belirsiz -> sıkı taraf
        assert_eq!(soru_turu("Türkiye'nin başkenti", false), SoruTuru::KesinOlgu);
    }

    #[test]
    fn atif_denetimi() {
        assert!(!atif_hatali("Kaynağa göre X [1].", 1));
        assert!(!atif_hatali("Atıf yok.", 3));
        assert!(atif_hatali("Olmayan kaynak [4].", 3));
        assert!(atif_hatali("Sıfır [0].", 3));
        assert!(!atif_hatali("Dizi [a] değil ama [2] var", 2));
    }

    #[test]
    fn destek_orani_icerige_bakar() {
        let k = "Ön satış ödemeleri yalnızca BNB Smart Chain üzerindeki USDT ile alınır. İzleyici BNB ödemelerini tespit edemiyor; BNB fiyatı değişken.";
        assert!(destek_orani("Ön satışta yalnızca USDT kabul ediliyor çünkü izleyici BNB ödemelerini tespit edemiyor ve BNB fiyatı değişken.", k) >= DESTEK_ESIGI);
        assert!(destek_orani("Yağış miktarını bilmiyorum, meteoroloji raporlarına bakmanı öneririm.", k) < 0.25);
        assert_eq!(destek_orani("", k), 0.0);
    }

    #[test]
    fn kaynakli_etiket() {
        let k = "Genesis vesting başlangıcı ön satış TGE tarihiyle aynı belirsiz tarihe bağlandı; ekip ile yatırımcı arasında simetri.";
        let destekli = "Genesis vesting başlangıcı, ekip ile yatırımcı arasında simetri için ön satış TGE tarihiyle aynı belirsiz tarihe bağlandı.";
        // Resmî kaynak + içerik desteği -> doğrulanmış; [n] şart değil (D03: doğru ama atıfsız cevap)
        assert_eq!(kaynakli_cevap_etiketi(destekli, k, 1, true), DOGRULANMIS);
        // Aynı cevap genel bilgi deposundan -> öneri (anlamsal doğrulama gelene kadar)
        assert_eq!(kaynakli_cevap_etiketi(destekli, k, 1, false), ONERI);
        // Resmî kaynak ama içerik desteklenmiyor (kaynak dışı gerekçe) -> öneri, [1] olsa bile
        assert_eq!(kaynakli_cevap_etiketi("Saldırganlar tek anahtarı ele geçirebilir, güvenlik artar. [1]", k, 1, true), ONERI);
        // Geçersiz atıf -> doğrulanmış olamaz
        assert_eq!(kaynakli_cevap_etiketi(&format!("{destekli} [3]"), k, 1, true), ONERI);
        assert_eq!(kaynakli_cevap_etiketi("Bu konuda doğrulanmış bilgim yok.", k, 2, true), BILINMIYOR);
        // Gerçek model örnekleri (B03/B12/B15): ret + ilgisiz [1] -> bilinmiyor
        assert_eq!(kaynakli_cevap_etiketi("Bilmiyorum, bu bilgiye ulaşamıyorum. [1]", k, 1, false), BILINMIYOR);
        assert_eq!(kaynakli_cevap_etiketi("Verilen kaynaklarda doğrudan bilgi bulunmamaktadır. [1] [2]", k, 3, false), BILINMIYOR);
        assert_eq!(arac_etiketi("resmi-kaynak"), BILINMIYOR);
        assert_eq!(arac_etiketi("hesap-makinesi"), DOGRULANMIS);
    }
}
