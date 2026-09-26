//! K-06: doğrulanamayan bilgi cevap olmaz. Her cevabın etiketi DETERMİNİSTİK belirlenir (model seçmez):
//!   dogrulanmis : araç cevabı (zincir okuması, hesap, sabit resmî metin) ya da geçerli [n] atfı olan kaynaklı cevap
//!   oneri       : kaynaksız yöntem/öneri cevabı ya da atfı doğrulanamayan kaynaklı cevap (başına etiket yazılır)
//!   bilinmiyor  : kaynağı olmayan kesin olgu sorusu (model ÇAĞRILMAZ) ya da modelin "doğrulanmış bilgim yok" demesi
//!   reddedildi  : K-23 kapısı
//!   sohbet      : selamlaşma ve hal-hatır (bilgi iddiası yok)

use crate::retrieval;

pub const DOGRULANMIS: &str = "dogrulanmis";
pub const ONERI: &str = "oneri";
pub const BILINMIYOR: &str = "bilinmiyor";
pub const SOHBET: &str = "sohbet";

pub const ONERI_ONEKI: &str = "Öneri (doğrulanmış bir kaynağa dayanmıyor): ";

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

/// Cevaptaki [n] atıfları: en az bir tane olmalı ve her biri 1..=kaynak_sayisi aralığında olmalı.
pub fn atif_gecerli(cevap: &str, kaynak_sayisi: usize) -> bool {
    let mut bulundu = false;
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
                    return false;
                }
                bulundu = true;
                i = j;
            }
        }
        i += 1;
    }
    bulundu
}

/// Model "doğrulanmış bilgim yok" dedi mi (resmî kaynak talimatındaki sabit metin dahil)?
pub fn bilgi_yok_dedi(cevap: &str) -> bool {
    let s = retrieval::sade(cevap);
    ["dogrulanmis bilgim yok", "bu konuda bilgim yok", "kaynaklarda bu bilgi yok"].iter().any(|k| s.contains(k))
}

/// Kaynaklı model cevabının etiketi.
pub fn kaynakli_cevap_etiketi(cevap: &str, kaynak_sayisi: usize) -> &'static str {
    if bilgi_yok_dedi(cevap) {
        BILINMIYOR
    } else if kaynak_sayisi > 0 && atif_gecerli(cevap, kaynak_sayisi) {
        DOGRULANMIS
    } else {
        ONERI
    }
}

/// Araç cevabının etiketi.
pub fn arac_etiketi(arac_ad: &str) -> &'static str {
    match arac_ad {
        "resmi-kaynak" | "karar-bulunamadi" => BILINMIYOR,
        "yetki-reddi" | "guvenlik-reddi" => "reddedildi",
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
        assert!(atif_gecerli("Kaynağa göre X [1].", 1));
        assert!(atif_gecerli("A [1], B [2].", 2));
        assert!(!atif_gecerli("Atıf yok.", 3));
        assert!(!atif_gecerli("Olmayan kaynak [4].", 3));
        assert!(!atif_gecerli("Sıfır [0].", 3));
        assert!(!atif_gecerli("[1]", 0));
        assert!(atif_gecerli("Dizi [a] değil ama [2] var", 2));
    }

    #[test]
    fn kaynakli_etiket() {
        assert_eq!(kaynakli_cevap_etiketi("Bilgi şöyle [1].", 2), DOGRULANMIS);
        assert_eq!(kaynakli_cevap_etiketi("Bilgi şöyle.", 2), ONERI);
        assert_eq!(kaynakli_cevap_etiketi("Bu konuda doğrulanmış bilgim yok.", 2), BILINMIYOR);
        assert_eq!(arac_etiketi("resmi-kaynak"), BILINMIYOR);
        assert_eq!(arac_etiketi("hesap-makinesi"), DOGRULANMIS);
    }
}
