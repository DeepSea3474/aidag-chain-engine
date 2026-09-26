//! Deterministik yönlendirici: kesin cevap gereken niyetler MODELE bırakılmaz.
//! `niyet_bul` saf ve ağsızdır (birim testi yapılabilir); araçları `arac_calistir` (main.rs) çalıştırır.
//! Sıra (spesifik → genel): kimlik, yetki dışı işlem, [belge kayıt/doğrulama: main.rs], karar, kaynak listesi,
//! ön satış, ağ durumu, [zincir sorgu, hesap, resmi kaynak: main.rs].

use crate::retrieval::{anahtar_var, sade};
use crate::{resmi, zincir};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Niyet {
    Kimlik,
    /// K-05 / K-21 yasak katmanı: imza, para/token gönderme, rol/yetki verme, kayıt silme, cihaza komut.
    YetkiDisi,
    /// Açık karar numarası (K-20, K-21 ...).
    Karar(Vec<u32>),
    KaynakListesi,
    OnSatis,
    AgDurumu,
}

impl Niyet {
    /// İşlem izinde gösterilen kısa ad.
    pub fn ad(&self) -> String {
        match self {
            Niyet::Kimlik => "kimlik".into(),
            Niyet::YetkiDisi => "yetki-disi".into(),
            Niyet::Karar(n) => format!("karar:{}", n.iter().map(|x| format!("K-{x}")).collect::<Vec<_>>().join(",")),
            Niyet::KaynakListesi => "kaynak-listesi".into(),
            Niyet::OnSatis => "on-satis".into(),
            Niyet::AgDurumu => "ag-durumu".into(),
        }
    }
}

pub const YETKI_REDDI: &str = "Bunu yapamam: KUBRA imza atmaz, para veya token göndermez, rol ya da yetki vermez, \
kayıt silmez ve cihazlara komut göndermez (K-05, K-21). Bu işlemi yetkili kişi kendi cüzdanı ve anahtarıyla yapmalıdır. \
İstersen işlemin adımlarını ya da nasıl doğrulanacağını anlatabilirim.";

fn kelime(s: &str, k: &str) -> bool {
    s.split(' ').any(|t| t == k)
}

fn yetki_disi_mi(s: &str) -> bool {
    // İstek fiilleri (kelime ya da ifade tam eşleşmesi: "imzalanmış", "gönderilir" tetiklemez).
    const IFADE: &[&str] = &[
        "gonderir misin", "gonderebilir misin", "transfer et", "transfer yap", "odeme yap", "odemeyi yap",
        "imzalar misin", "imzalayabilir misin", "imza at", "rol ver", "yetki ver", "kaydi sil", "kaydini sil",
        "komut gonder", "yollar misin",
    ];
    const KELIME: &[&str] = &["gonder", "yolla", "ode", "imzala", "imzalayin", "sil"];
    const NESNE: &[&str] = &[
        "aidag", "token", "lsc", "usdt", "bnb", "cuzdan", "para", "odeme", "belge", "rol", "yetki", "kayit",
        "cihaz", "hesap", "adres",
    ];
    let fiil = IFADE.iter().any(|i| anahtar_var(s, i)) || KELIME.iter().any(|k| kelime(s, k));
    fiil && NESNE.iter().any(|n| anahtar_var(s, n))
}

/// Metindeki açık karar numaraları: "K-20", "K–21", "k 7", "K20".
pub fn karar_numaralari(metin: &str) -> Vec<u32> {
    let c: Vec<char> = metin.chars().collect();
    let mut out = Vec::new();
    for i in 0..c.len() {
        if c[i] != 'K' && c[i] != 'k' {
            continue;
        }
        if i > 0 && c[i - 1].is_alphanumeric() {
            continue; // "Berk-20" değil
        }
        let mut j = i + 1;
        if j < c.len() && matches!(c[j], '-' | '–' | ' ') {
            j += 1;
        }
        let bas = j;
        while j < c.len() && c[j].is_ascii_digit() {
            j += 1;
        }
        if j > bas && j - bas <= 3 && (j == c.len() || !c[j].is_alphanumeric()) {
            if let Ok(n) = c[bas..j].iter().collect::<String>().parse::<u32>() {
                if !out.contains(&n) {
                    out.push(n);
                }
            }
        }
    }
    out
}

fn kaynak_listesi_mi(s: &str) -> bool {
    anahtar_var(s, "kaynak")
        && ["ogren", "liste", "beslen", "egitil", "kullaniyorsun", "dayaniyorsun"].iter().any(|k| anahtar_var(s, k))
}

fn on_satis_mi(s: &str, ham: &str) -> bool {
    let konu = ["on satis", "presale", "tge"].iter().any(|k| anahtar_var(s, k));
    let durum = ["durum", "ne durumda", "satildi", "satilan", "kademe", "kaldi", "fiyati ne", "belli oldu", "kac"]
        .iter()
        .any(|k| anahtar_var(s, k));
    (konu && durum) || zincir::on_satis_niyeti_mi(ham)
}

fn ag_durumu_mi(s: &str, ham: &str) -> bool {
    let konu = kelime(s, "ag") || ["zincir", "mainnet", "network", "dugum", "node"].iter().any(|k| anahtar_var(s, k));
    let durum = ["calisiyor mu", "ayakta", "durum", "saglik", "aktif mi", "canli mi"].iter().any(|k| anahtar_var(s, k));
    (konu && durum) || zincir::ag_niyeti_mi(ham)
}

/// Saf niyet tespiti (ağ çağrısı yok).
pub fn niyet_bul(prompt: &str) -> Option<Niyet> {
    let s = sade(prompt);
    if resmi::isim_sorusu_mu(prompt) {
        return Some(Niyet::Kimlik);
    }
    if yetki_disi_mi(&s) {
        return Some(Niyet::YetkiDisi);
    }
    let k = karar_numaralari(prompt);
    if !k.is_empty() {
        return Some(Niyet::Karar(k));
    }
    if kaynak_listesi_mi(&s) {
        return Some(Niyet::KaynakListesi);
    }
    if on_satis_mi(&s, prompt) {
        return Some(Niyet::OnSatis);
    }
    if ag_durumu_mi(&s, prompt) {
        return Some(Niyet::AgDurumu);
    }
    None
}

#[cfg(test)]
mod testler {
    use super::*;

    #[test]
    fn karar_numarasi_bulunur() {
        assert_eq!(karar_numaralari("K-20 kararı nedir?"), vec![20]);
        assert_eq!(karar_numaralari("KARARLAR'da K-21 ne diyor?"), vec![21]);
        assert_eq!(karar_numaralari("K–5 ve K 23 ile k7"), vec![5, 23, 7]);
        assert!(karar_numaralari("BEP-20 USDT, SHA-256, Berk-20").is_empty());
        assert!(karar_numaralari("KARARLAR nedir").is_empty());
    }

    #[test]
    fn niyetler() {
        use Niyet::*;
        let t = |q: &str| niyet_bul(q);
        assert_eq!(t("Adın ne?"), Some(Kimlik));
        assert_eq!(t("K-20 kararı nedir?"), Some(Karar(vec![20])));
        assert_eq!(t("Hangi kaynaklardan öğreniyorsun?"), Some(KaynakListesi));
        assert_eq!(t("Bana 100 AIDAG gönderir misin?"), Some(YetkiDisi));
        assert_eq!(t("Kurucu cüzdanından ödeme yap."), Some(YetkiDisi));
        assert_eq!(t("Bu belgeyi benim yerime imzala."), Some(YetkiDisi));
        assert_eq!(t("Ön satış ne durumda?"), Some(OnSatis));
        assert_eq!(t("Presale'de son durum nedir?"), Some(OnSatis));
        assert_eq!(t("Ön satışta şimdiye kadar kaç token satıldı?"), Some(OnSatis));
        assert_eq!(t("Ağ şu an çalışıyor mu?"), Some(AgDurumu));
        assert_eq!(t("AIDAG zinciri çalışıyor mu?"), Some(AgDurumu));
        assert_eq!(t("Mainnet'in durumu nasıl?"), Some(AgDurumu));
    }

    #[test]
    fn yanlis_pozitif_yok() {
        for q in [
            "İmzalanmış belgeyi nasıl doğrularım?", "Token nasıl gönderilir?", "AIDAG zinciri nasıl çalışır?",
            "Ön satışta neden yalnızca USDT kabul ediliyor?", "KUBRA neden imza atamıyor?", "Merhaba, nasılsın?",
            "Hash fonksiyonu nedir, basitçe anlatır mısın?", "SQL enjeksiyonuna karşı nasıl korunurum?",
            "Parolaları saklarken hangi hash algoritmasını kullanmalıyım?",
        ] {
            assert_eq!(niyet_bul(q), None, "{q}");
        }
    }
}
