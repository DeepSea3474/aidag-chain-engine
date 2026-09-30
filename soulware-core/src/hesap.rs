//! hesap — deterministik HESAP MAKİNESİ aracı (araç-kullanımı).
//! ════════════════════════════════════════════════════════════════════════
//! Güçlü AI'lar araç kullanır. KUBRA aritmetiği ZAYIF MODELE değil, KESİN hesaba
//! bırakır: "7 çarpı 8" → 56 (garantili). Türkçe operatör kelimeleri (çarpı/artı/
//! eksi/bölü) sembole çevrilir; güvenli özyinelemeli çözümleyici + - * / ( ) değerlendirir.
//!
//! GÜVENLİK: yalnız AÇIK aritmetik (sayılar arası operatör) tetikler. "Türkiye'nin
//! başkenti" gibi sayısız/operatörsüz sorguda None → normal model/grounding yolu.

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Add,
    Sub,
    Mul,
    Div,
    LP,
    RP,
}

/// Sorgudan aritmetik varsa KESIN sonucu döndür; yoksa None (model yolu).
pub fn hesapla(sorgu: &str) -> Option<String> {
    // Kod/kimlik kalıbı (harf-tire-rakam: "K-20", "BEP-20", "SHA-256") işlem değildir.
    if kod_kalibi_var(sorgu) {
        return None;
    }
    let ifade = ile_kalibi(sorgu)
        .or_else(|| kelime_islemi(sorgu))
        .or_else(|| ifade_cikar(sorgu))?;
    let tokens = tokenle(&ifade)?;
    // En az bir İKİLİ operatör olmalı (iki değer arasında): "-20" tek başına hesap değildir.
    let ikili = tokens.windows(2).any(|w| {
        matches!(w[0], Tok::Num(_) | Tok::RP)
            && matches!(w[1], Tok::Add | Tok::Sub | Tok::Mul | Tok::Div)
    });
    if !ikili {
        return None;
    }
    let mut p = Coz { t: tokens, i: 0 };
    let v = p.expr()?;
    if p.i != p.t.len() {
        return None; // tam tüketilmedi → geçerli tek ifade değil
    }
    if !v.is_finite() {
        return None;
    }
    // Tam sayıysa tam sayı yaz.
    if (v.fract()).abs() < 1e-9 {
        Some(format!("{}", v.round() as i64))
    } else {
        Some(format!("{}", (v * 1e6).round() / 1e6))
    }
}

/// Kod ya da tarih kalıbı var mı? Harf-tire-rakam ("K-20", "BEP-20") ya da tire/nokta/eğik çizgili tarih
/// ("2026-09-26", "26-09-2026", "26/09/2026"): bunlar işlem değildir.
fn kod_kalibi_var(s: &str) -> bool {
    let c: Vec<char> = s.chars().collect();
    if c.windows(3)
        .any(|w| w[0].is_alphabetic() && matches!(w[1], '-' | '–') && w[2].is_ascii_digit())
    {
        return true;
    }
    // Tarih: rakam grupları aynı ayraçla (- / .) üç parça, biri 4 haneli.
    s.split(|ch: char| ch.is_whitespace()).any(|kelime| {
        let k = kelime.trim_matches(|ch: char| !ch.is_ascii_digit());
        ['-', '/', '.'].iter().any(|ayrac| {
            let p: Vec<&str> = k.split(*ayrac).collect();
            p.len() == 3
                && p.iter()
                    .all(|x| !x.is_empty() && x.chars().all(|ch| ch.is_ascii_digit()))
                && p.iter().any(|x| x.len() == 4)
        })
    })
}

/// "12 ile 12'yi çarp(arsan)", "5 ile 3'ü topla", "45 ve 55'in toplamı", "10 ile 3'ün farkı" → "12 * 12" vb. (tam iki sayı).
fn ile_kalibi(s: &str) -> Option<String> {
    let sade = crate::retrieval::sade(s);
    let t: Vec<&str> = sade.split(' ').collect();
    if !t.contains(&"ile") && !t.contains(&"ve") {
        return None;
    }
    let op = if t.iter().any(|w| w.starts_with("carp") && *w != "carpi") {
        "*"
    } else if t.iter().any(|w| w.starts_with("topla")) {
        "+"
    } else if t.iter().any(|w| w.starts_with("fark")) {
        "-"
    } else if t
        .iter()
        .any(|w| (w.starts_with("bol") && *w != "bolu") || w.starts_with("bolers"))
    {
        "/"
    } else {
        return None;
    };
    let sayilar: Vec<&str> = t
        .iter()
        .copied()
        .filter(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_digit()))
        .collect();
    if sayilar.len() != 2 {
        return None;
    }
    Some(format!("{} {op} {}", sayilar[0], sayilar[1]))
}

/// Sayı sözcükleri (kat çarpanı için).
fn sayi_sozcugu(t: &str) -> Option<f64> {
    Some(match t {
        "bir" => 1.0,
        "iki" => 2.0,
        "uc" => 3.0,
        "dort" => 4.0,
        "bes" => 5.0,
        "alti" => 6.0,
        "yedi" => 7.0,
        "sekiz" => 8.0,
        "dokuz" => 9.0,
        "on" => 10.0,
        "yuz" => 100.0,
        _ => t.parse::<f64>().ok()?,
    })
}

/// Kelimeyle ifade edilen işlemler: "N'in yarısı", "N'in çeyreği", "N'in karesi/küpü", "N'in X katı",
/// "N'in yüzde X'i" / "yüzde X'i N". Sayı sayısı tam olmalı; aksi hâlde None.
fn kelime_islemi(s: &str) -> Option<String> {
    let sade = crate::retrieval::sade(s);
    let t: Vec<&str> = sade.split(' ').collect();
    let sayi = |w: &str| w.parse::<f64>().ok();
    let sayilar: Vec<f64> = t.iter().filter_map(|w| sayi(w)).collect();
    let var = |adaylar: &[&str]| t.iter().position(|w| adaylar.contains(w));
    if let Some(i) = var(&["yuzde"]) {
        let x = t.get(i + 1).and_then(|w| sayi(w))?;
        let n: Vec<f64> = t
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i + 1)
            .filter_map(|(_, w)| sayi(w))
            .collect();
        return (n.len() == 1).then(|| format!("{} * {x} / 100", n[0]));
    }
    if let Some(i) = var(&["kati", "katini", "katidir", "katina"]) {
        let carpan = t.get(i.checked_sub(1)?).and_then(|w| sayi_sozcugu(w))?;
        let n: Vec<f64> = t
            .iter()
            .enumerate()
            .filter(|(j, _)| *j + 1 != i)
            .filter_map(|(_, w)| sayi(w))
            .collect();
        return (n.len() == 1).then(|| format!("{} * {carpan}", n[0]));
    }
    if sayilar.len() != 1 {
        return None;
    }
    let n = sayilar[0];
    if var(&["yarisi", "yarisini", "yarisidir", "yarisina"]).is_some() {
        Some(format!("{n} / 2"))
    } else if var(&["ceyregi", "ceyregini", "ceyregidir"]).is_some() {
        Some(format!("{n} / 4"))
    } else if var(&["karesi", "karesini", "karesidir"]).is_some() {
        Some(format!("{n} * {n}"))
    } else if var(&["kupu", "kupunu", "kupudur"]).is_some() {
        Some(format!("{n} * {n} * {n}"))
    } else {
        None
    }
}

/// Türkçe operatör kelimelerini sembole çevir + yalnız matematik karakterlerini tut.
fn ifade_cikar(s: &str) -> Option<String> {
    let mut t = format!(" {} ", s.to_lowercase());
    for (w, sym) in [
        (" artı ", " + "),
        (" arti ", " + "),
        (" topla ", " + "),
        (" toplam ", " + "),
        (" eksi ", " - "),
        (" çıkar ", " - "),
        (" cikar ", " - "),
        (" çarpı ", " * "),
        (" carpi ", " * "),
        (" çarp ", " * "),
        (" carp ", " * "),
        (" kere ", " * "),
        (" bölü ", " / "),
        (" bolu ", " / "),
        (" böl ", " / "),
        (" bol ", " / "),
    ] {
        t = t.replace(w, sym);
    }
    t = t.replace('×', "*").replace('÷', "/").replace(',', ".");
    // Yalnız matematik karakterleri (harfler ayraç olur).
    let math: String = t
        .chars()
        .map(|c| {
            if c.is_ascii_digit() || "+-*/(). ".contains(c) {
                c
            } else {
                ' '
            }
        })
        .collect();
    let cleaned = math.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() || !cleaned.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(cleaned)
}

fn tokenle(s: &str) -> Option<Vec<Tok>> {
    let mut out = Vec::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '+' => {
                out.push(Tok::Add);
                i += 1;
            }
            '-' => {
                out.push(Tok::Sub);
                i += 1;
            }
            '*' => {
                out.push(Tok::Mul);
                i += 1;
            }
            '/' => {
                out.push(Tok::Div);
                i += 1;
            }
            '(' => {
                out.push(Tok::LP);
                i += 1;
            }
            ')' => {
                out.push(Tok::RP);
                i += 1;
            }
            _ if c.is_ascii_digit() || c == '.' => {
                let mut j = i;
                while j < cs.len() && (cs[j].is_ascii_digit() || cs[j] == '.') {
                    j += 1;
                }
                let num: String = cs[i..j].iter().collect();
                i = j;
                // Rakamsız "." (cümle noktası gibi) → gürültü, atla.
                if !num.chars().any(|x| x.is_ascii_digit()) {
                    continue;
                }
                out.push(Tok::Num(num.parse().ok()?));
            }
            _ => return None,
        }
    }
    Some(out)
}

/// Özyinelemeli çözümleyici: expr = term (('+'|'-') term)* ; term = factor (('*'|'/') factor)* ;
/// factor = number | '(' expr ')' | '-' factor.
struct Coz {
    t: Vec<Tok>,
    i: usize,
}
impl Coz {
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop {
            match self.t.get(self.i) {
                Some(Tok::Add) => {
                    self.i += 1;
                    v += self.term()?;
                }
                Some(Tok::Sub) => {
                    self.i += 1;
                    v -= self.term()?;
                }
                _ => break,
            }
        }
        Some(v)
    }
    fn term(&mut self) -> Option<f64> {
        let mut v = self.factor()?;
        loop {
            match self.t.get(self.i) {
                Some(Tok::Mul) => {
                    self.i += 1;
                    v *= self.factor()?;
                }
                Some(Tok::Div) => {
                    self.i += 1;
                    let r = self.factor()?;
                    if r == 0.0 {
                        return None;
                    }
                    v /= r;
                }
                _ => break,
            }
        }
        Some(v)
    }
    fn factor(&mut self) -> Option<f64> {
        match self.t.get(self.i) {
            Some(Tok::Num(n)) => {
                let n = *n;
                self.i += 1;
                Some(n)
            }
            Some(Tok::Sub) => {
                self.i += 1;
                Some(-self.factor()?)
            }
            Some(Tok::LP) => {
                self.i += 1;
                let v = self.expr()?;
                if matches!(self.t.get(self.i), Some(Tok::RP)) {
                    self.i += 1;
                    Some(v)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aritmetik_kesin() {
        assert_eq!(
            hesapla("7 carpi 8 kactir? Sadece rakam yaz.").as_deref(),
            Some("56")
        );
        assert_eq!(hesapla("2 + 2 kactir").as_deref(), Some("4"));
        assert_eq!(
            hesapla("100 eksi 37 kactir? Sadece rakam.").as_deref(),
            Some("63")
        );
        assert_eq!(hesapla("7 çarpı 8").as_deref(), Some("56"));
        assert_eq!(hesapla("(2 + 3) * 4").as_deref(), Some("20"));
        assert_eq!(hesapla("10 bolu 4").as_deref(), Some("2.5"));
    }
    #[test]
    fn aritmetik_olmayan_none() {
        // Operatörsüz/sayısız → araç tetiklemez (model yolu).
        assert_eq!(hesapla("Türkiye'nin başkenti neresidir"), None);
        assert_eq!(hesapla("AIDAG-Chain network_id kactir"), None); // tek sayı, operatör yok
        assert_eq!(hesapla("Ali'nin 2 elması vardı 3 daha aldı"), None); // operatör yok
    }
    #[test]
    fn kod_kalibi_hesap_degildir() {
        for q in [
            "K-20 kararı nedir?",
            "KARARLAR'da K-21 ne diyor?",
            "BEP-20 USDT nedir",
            "SHA-256 nedir",
            "-20",
        ] {
            assert_eq!(hesapla(q), None, "{q}");
        }
        assert_eq!(hesapla("20 - 5").as_deref(), Some("15"));
        assert_eq!(hesapla("-3 artı 5").as_deref(), Some("2"));
    }
    #[test]
    fn hesap_ifade_cesitliligi() {
        for (q, c) in [
            ("15 artı 27 kaç?", "42"),
            ("8 kere 9", "72"),
            ("100 bölü 4 kaç eder?", "25"),
            ("(3+4)*2", "14"),
            ("250 eksi 75 nedir?", "175"),
            ("6 ile 7'yi çarp", "42"),
            ("20 ve 30'un toplamı", "50"),
            ("1,5 çarpı 4", "6"),
            ("9 ile 4'ün farkı nedir?", "5"),
        ] {
            assert_eq!(hesapla(q).as_deref(), Some(c), "{q}");
        }
        for q in [
            "K-20 nedir?",
            "BEP-20 ağı hangisi?",
            "3 elma aldım",
            "2026 yılında ne oldu?",
            "2026-09-26 tarihinde ne oldu?",
            "26/09/2026 günü ne var?",
            "SHA-256 güvenli mi?",
            "Ali ile Ayşe 2 kitap okudu",
        ] {
            assert_eq!(hesapla(q), None, "{q}");
        }
    }

    #[test]
    fn kelime_islemleri() {
        for (q, c) in [
            ("50'nin yarısı nedir?", "25"),
            ("80'in çeyreği kaç?", "20"),
            ("12'nin karesi", "144"),
            ("3'ün küpü kaç eder?", "27"),
            ("15'in üç katı kaç?", "45"),
            ("7'nin 4 katı", "28"),
            ("200'ün yüzde 15'i kaç?", "30"),
            ("yüzde 10'u 90 kaç eder?", "9"),
        ] {
            assert_eq!(hesapla(q).as_deref(), Some(c), "{q}");
        }
        for q in [
            "Yarın saat kaçta buluşuyoruz?",
            "Ürünün yarısı bozuk çıktı",
            "Yüzde kaç indirim var?",
            "2 katlı ev fiyatları",
            "İki katı daha hızlı mı?",
            "3 ile 5'in yarısı",
        ] {
            assert_eq!(hesapla(q), None, "{q}");
        }
    }

    #[test]
    fn ile_kalibi_calisir() {
        assert_eq!(
            hesapla("12 ile 12'yi çarparsan ne çıkar?").as_deref(),
            Some("144")
        );
        assert_eq!(hesapla("5 ile 3'ü topla").as_deref(), Some("8"));
        assert_eq!(hesapla("10 ile 4'ü bölersen?").as_deref(), Some("2.5"));
        assert_eq!(hesapla("Ali ile Veli 2 elma aldı"), None);
        assert_eq!(
            hesapla("45 ve 55'in toplamı nedir?").as_deref(),
            Some("100")
        );
        assert_eq!(hesapla("10 ile 3'ün farkı").as_deref(), Some("7"));
        assert_eq!(hesapla("2 elma ve 3 armut aldım"), None);
        assert_eq!(hesapla("K-20 ve K-21'in farkı ne?"), None);
    }
}
