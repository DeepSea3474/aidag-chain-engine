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
    let ifade = ifade_cikar(sorgu)?;
    let tokens = tokenle(&ifade)?;
    // En az bir operatör olmalı (tek sayı "hesap" değildir).
    if !tokens.iter().any(|t| matches!(t, Tok::Add | Tok::Sub | Tok::Mul | Tok::Div)) {
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

/// Türkçe operatör kelimelerini sembole çevir + yalnız matematik karakterlerini tut.
fn ifade_cikar(s: &str) -> Option<String> {
    let mut t = format!(" {} ", s.to_lowercase());
    for (w, sym) in [
        (" artı ", " + "), (" arti ", " + "), (" topla ", " + "), (" toplam ", " + "),
        (" eksi ", " - "), (" çıkar ", " - "), (" cikar ", " - "),
        (" çarpı ", " * "), (" carpi ", " * "), (" çarp ", " * "), (" carp ", " * "), (" kere ", " * "),
        (" bölü ", " / "), (" bolu ", " / "), (" böl ", " / "), (" bol ", " / "),
    ] {
        t = t.replace(w, sym);
    }
    t = t.replace('×', "*").replace('÷', "/").replace(',', ".");
    // Yalnız matematik karakterleri (harfler ayraç olur).
    let math: String = t
        .chars()
        .map(|c| if c.is_ascii_digit() || "+-*/(). ".contains(c) { c } else { ' ' })
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
            '+' => { out.push(Tok::Add); i += 1; }
            '-' => { out.push(Tok::Sub); i += 1; }
            '*' => { out.push(Tok::Mul); i += 1; }
            '/' => { out.push(Tok::Div); i += 1; }
            '(' => { out.push(Tok::LP); i += 1; }
            ')' => { out.push(Tok::RP); i += 1; }
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
                Some(Tok::Add) => { self.i += 1; v += self.term()?; }
                Some(Tok::Sub) => { self.i += 1; v -= self.term()?; }
                _ => break,
            }
        }
        Some(v)
    }
    fn term(&mut self) -> Option<f64> {
        let mut v = self.factor()?;
        loop {
            match self.t.get(self.i) {
                Some(Tok::Mul) => { self.i += 1; v *= self.factor()?; }
                Some(Tok::Div) => {
                    self.i += 1;
                    let r = self.factor()?;
                    if r == 0.0 { return None; }
                    v /= r;
                }
                _ => break,
            }
        }
        Some(v)
    }
    fn factor(&mut self) -> Option<f64> {
        match self.t.get(self.i) {
            Some(Tok::Num(n)) => { let n = *n; self.i += 1; Some(n) }
            Some(Tok::Sub) => { self.i += 1; Some(-self.factor()?) }
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
        assert_eq!(hesapla("7 carpi 8 kactir? Sadece rakam yaz.").as_deref(), Some("56"));
        assert_eq!(hesapla("2 + 2 kactir").as_deref(), Some("4"));
        assert_eq!(hesapla("100 eksi 37 kactir? Sadece rakam.").as_deref(), Some("63"));
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
}
