//! zincir - deterministik zincir sorgu araci
use crate::retrieval::{anahtar_var, sade};
use serde_json::json;

fn niyet_cikar(sorgu: &str) -> Option<(&'static str, serde_json::Value, String)> {
    let s = sorgu.to_lowercase();
    if s.contains("bakiye") || s.contains("balance") {
        if let Some(adr) = adres_bul(sorgu) {
            return Some(("eth_getBalance", json!([adr, "latest"]),
                format!("{} adresinin bakiyesi", adr)));
        }
    }
    if (s.contains("blok") || s.contains("block") || s.contains("yukseklik"))
        && (s.contains("kac") || s.contains("son") || s.contains("number") || s.contains("numara")) {
            return Some(("eth_blockNumber", json!([]), "guncel blok yuksekligi".to_string()));
        }
    None
}

fn adres_bul(s: &str) -> Option<String> {
    for kelime in s.split_whitespace() {
        let k = kelime.trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
        if k.len() == 42 && k.starts_with("0x") && k[2..].chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(k.to_string());
        }
    }
    None
}

fn hex_to_dec_str(hex: &str) -> String {
    let h = hex.trim_start_matches("0x");
    match u128::from_str_radix(h, 16) {
        Ok(v) => v.to_string(),
        Err(_) => hex.to_string(),
    }
}

pub async fn sorgula(http: &reqwest::Client, rpc_url: &str, sorgu: &str) -> Option<String> {
    let (method, params, aciklama) = niyet_cikar(sorgu)?;
    let istek = json!({"jsonrpc":"2.0","method":method,"params":params,"id":1});
    let resp = http.post(rpc_url).json(&istek).send().await.ok()?;
    let v: serde_json::Value = resp.json().await.ok()?;
    let sonuc = v.get("result")?.as_str()?;
    let okunur = if sonuc.starts_with("0x") { hex_to_dec_str(sonuc) } else { sonuc.to_string() };
    Some(format!("{}: {}", aciklama, okunur))
}

// ── AG DURUMU ARACI ─────────────────────────────────────────────────────────
// Canli zincirin /status ucundan anlik ag sagligini ceker. Ayni zamanda
// GENEL DIS-SISTEM ADAPTORU sablonudur: bir HTTP GET -> JSON parse -> insan
// okunur ozet. Kurumsal entegrasyonlar (stok, IK, dokuman sistemi) ayni
// kalipla eklenir: url degistir, alanlari esle, formatla.

fn ag_niyeti_mi(sorgu: &str) -> bool {
    // sade(): Türkçe harfler katlanır → "ağ sağlık" ile "ag saglik" aynı eşleşir.
    let s = sade(sorgu);
    let anahtarlar = [
        "ag durumu", "ag saglik", "saglik durumu", "aidag durumu", "aidag saglik",
        "network durum", "network status", "status", "durum raporu",
        "zincir durum", "zincir calisiyor", "zincir ayakta", "ag calisiyor", "ag ayakta",
        "aidag calisiyor", "mainnet calisiyor", "mainnet durum",
        "kac dugum", "kac node", "dugum sayisi", "tps", "ag nasil",
        "kac vertex", "tip sayisi", "orphan",
    ];
    anahtarlar.iter().any(|a| anahtar_var(&s, a))
}

/// /status'tan ag durumu ceker, insan-okunur ozet doner. RPC yaninda status
/// ucu (8645) ayni sunucuda; buradaki base_url RPC ile ayni ana makinedir.
pub async fn ag_durumu(http: &reqwest::Client, rpc_url: &str, sorgu: &str) -> Option<String> {
    if !ag_niyeti_mi(sorgu) {
        return None;
    }
    // RPC url'inden status url tureti: .../  -> ayni host, /status yolu.
    // rpc_url ornegi: http://127.0.0.1:8645  (JSON-RPC ayni portta /status sunar)
    let status_url = format!("{}/status", rpc_url.trim_end_matches('/'));
    let resp = http.get(&status_url).send().await.ok()?;
    let v: serde_json::Value = resp.json().await.ok()?;

    let vertex = v.get("vertex_count").and_then(|x| x.as_u64()).unwrap_or(0);
    let tip = v.get("tip_count").and_then(|x| x.as_u64()).unwrap_or(0);
    let orphan = v.get("orphan_count").and_then(|x| x.as_u64()).unwrap_or(0);
    let net = v.get("network_id").and_then(|x| x.as_u64()).unwrap_or(0);
    let staker = v.get("staker_count").and_then(|x| x.as_u64()).unwrap_or(0);

    // Basit saglik yorumu (deterministik, uydurma yok).
    let saglik = if orphan == 0 && tip >= 1 {
        "saglikli (tek tip, kopuk blok yok)"
    } else if orphan > 0 {
        "dikkat: kopuk (orphan) blok var"
    } else {
        "tip yok (baslangic/bekleme)"
    };

    Some(format!(
        "AIDAG ag durumu (canli, network {}): {} blok (vertex), {} aktif tip, {} orphan, {} staker. Durum: {}.",
        net, vertex, tip, orphan, staker, saglik
    ))
}

// ── BELGE DOGRULAMA ARACI ───────────────────────────────────────────────────
// Kullanici bir belge hash'i (64 hex = 32 bayt) verip "bu belge gecerli mi /
// zincirde var mi / degistirilmis mi" diye sorunca, /belge/:hash ucundan
// dogrular. KURUMSAL DEGER: sahte/degistirilmis belge saniyede yakalanir,
// cevap tahrif edilemez zincir kaydina dayanir. Dis-sistem adaptor sablonu.

fn belge_niyeti_mi(sorgu: &str) -> bool {
    // Nesne + doğrulama fiili birlikte olmalı ("doğrulanmış bilgi" belge sorusu değildir).
    let s = sade(sorgu);
    let nesne = ["belge", "hash", "document", "dosya", "evrak", "sertifika", "diploma"];
    let fiil = [
        "dogrula", "gecerli", "sahte", "degistirilmis", "orijinal", "kayitli", "zincirde var",
        "kontrol", "sorgula", "verify",
    ];
    nesne.iter().any(|a| anahtar_var(&s, a)) && fiil.iter().any(|a| anahtar_var(&s, a))
}

/// "Belge doğrulama nasıl çalışır / nedir" gibi AÇIKLAMA sorusu mu? Bunlar araca değil
/// resmi kaynak belgesine gider (KUBRA süreci anlatır).
fn aciklama_sorusu_mu(sorgu: &str) -> bool {
    let s = sade(sorgu);
    ["nasil calisir", "nasil isler", "nedir", "ne ise yarar", "ne demek", "mantigi"]
        .iter()
        .any(|a| anahtar_var(&s, a))
}

/// Kullanıcı belge KAYDETMEK/oluşturmak istiyor (doğrulamak değil).
pub fn belge_kayit_niyeti_mi(sorgu: &str) -> bool {
    let s = sade(sorgu);
    let nesne = ["belge", "dosya", "sertifika", "diploma", "evrak", "hash"];
    let fiil = [
        "olustur", "kaydet", "kaydede", "kayit et", "kayit yap", "kaydini yap", "kaydolu",
        "zincire yaz", "zincire ekle", "zincire isle", "damgala", "tescil", "belgelendir",
    ];
    nesne.iter().any(|a| anahtar_var(&s, a)) && fiil.iter().any(|a| anahtar_var(&s, a))
}

/// Mesajdaki hex parçaları (0x öneki atılmış) + ardından "..."/"…" gelip gelmediği.
fn hex_parcalari(s: &str) -> Vec<(String, bool)> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        if !cs[i].is_ascii_alphanumeric() {
            i += 1;
            continue;
        }
        let bas = i;
        while i < cs.len() && cs[i].is_ascii_alphanumeric() {
            i += 1;
        }
        let kelime: String = cs[bas..i].iter().collect();
        let k = kelime.strip_prefix("0x").unwrap_or(&kelime);
        if !k.is_empty() && k.chars().all(|c| c.is_ascii_hexdigit()) {
            let kalan: String = cs[i..].iter().take(3).collect();
            let noktali = kalan.starts_with("...") || kalan.starts_with('…');
            out.push((k.to_ascii_lowercase(), noktali));
        }
    }
    out
}

/// Sorgudan tam 64-hex belge hash'i çıkar (0x opsiyonel, metnin herhangi bir yerinde).
pub fn belge_hash_bul(s: &str) -> Option<String> {
    hex_parcalari(s).into_iter().find(|(h, _)| h.len() == 64).map(|(h, _)| h)
}

/// Kısaltılmış hash var mı? ("0dcce43d9a70..." veya "0dcce43d...0c544f")
fn kisaltilmis_hash_var(s: &str) -> bool {
    // ≥8 hane + en az bir rakam: "decade..." gibi hex-harfli kelimeler hash sayılmaz.
    hex_parcalari(s).iter().any(|(h, noktali)| {
        *noktali && h.len() >= 8 && h.len() < 64 && h.chars().any(|c| c.is_ascii_digit())
    })
}

/// Unix saniye → "YYYY-AA-GG SS:DD" (UTC). Takvim: Hinnant civil_from_days.
fn utc_tarih(unix: u64) -> String {
    let gun = (unix / 86_400) as i64;
    let sn = unix % 86_400;
    let z = gun + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", sn / 3600, (sn % 3600) / 60)
}

const BELGE_KAYIT_SURECI: &str = "Belge kaydını kurum personeli yapar; ben (KUBRA) kayıt talebini hazırlarım ama İMZALAMAM ve zincire göndermem:\n\
1) https://aidag-chain.com/belge sayfasında belgeni seç: tarayıcın belgenin özetini (64 haneli hash) hesaplar. Dosya sunucuya gönderilmez.\n\
2) \"Kayıt Talebi Hazırla\"ya bas: zincir durumu kontrol edilir ve imzasız bir talep oluşur. 64 haneli hash'i bana yazıp \"kaydet\" dersen talebi ben de hazırlarım.\n\
3) Kurum personeli kendi anahtar dosyasını seçip talebi onaylar (anahtar tarayıcıdan çıkmaz) ya da talep dosyasını indirip çevrimdışı belge-imzala aracıyla imzalar. Kayıt yalnızca bu onayla zincire gider ve geri alınamaz.\n\
4) Kayıttan sonra https://aidag-chain.com/belge/<hash> sayfasından doğrulayabilir, QR'lı belge kapağı veya parça etiketi yazdırabilirsin.\n\
Not: Kurum kaydı zincirde beyana dayanır; kurum kimliği henüz bağımsız olarak doğrulanmamıştır.";

const BELGE_DOGRULA_YONLENDIR: &str = "Belgeni doğrulamak için https://aidag-chain.com/belge sayfasındaki \"Belge Doğrula\" bölümünü kullan: dosyanı (PDF, resim, Word) seç, sistem zincirde kayıtlı mı, orijinal mi yoksa değiştirilmiş mi söyler. Belgen tarayıcından çıkmaz; yalnızca matematiksel özeti kontrol edilir. Elinde belgenin 64 haneli hash'i varsa bana doğrudan yazabilirsin, hemen doğrularım.";

/// Belge aracı (deterministik, model YOK):
///  1. Mesajda tam 64-hex hash varsa → HER ZAMAN zincirde doğrula (yanında metin olsun olmasın).
///  2. Kısaltılmış hash ("..." ile) → tam 64 haneyi iste.
///  3. Kayıt niyeti ("belge oluşturacağım/kaydetmek istiyorum") → kayıt süreci.
///  4. Doğrulama niyeti (açıklama sorusu değilse) → doğrulama sayfasına yönlendir.
pub async fn belge_dogrula(http: &reqwest::Client, rpc_url: &str, kubra_adres: &[u8; 20], sorgu: &str) -> Option<String> {
    let hash = match belge_hash_bul(sorgu) {
        Some(h) => h,
        None => {
            if kisaltilmis_hash_var(sorgu) {
                return Some("Gönderdiğin hash kısaltılmış görünüyor (\"...\" ile bitiyor). Doğrulama yapabilmem için hash'in TAMAMINI yaz: 64 haneli, yalnızca 0-9 ve a-f karakterlerinden oluşan değer.".to_string());
            }
            if belge_kayit_niyeti_mi(sorgu) {
                return Some(BELGE_KAYIT_SURECI.to_string());
            }
            if belge_niyeti_mi(sorgu) && !aciklama_sorusu_mu(sorgu) {
                return Some(BELGE_DOGRULA_YONLENDIR.to_string());
            }
            return None;
        }
    };
    let taban = rpc_url.trim_end_matches('/');
    let url = format!("{taban}/belge/{hash}");
    // Hash verildiyse modele DÜŞME: zincire ulaşılamazsa bunu dürüstçe söyle.
    let v: serde_json::Value = match http.get(&url).send().await {
        Ok(r) => match r.json().await {
            Ok(v) => v,
            Err(_) => return Some("Belge doğrulama şu an yapılamadı: zincir yanıtı çözülemedi. Lütfen biraz sonra tekrar dene.".to_string()),
        },
        Err(_) => return Some("Belge doğrulama şu an yapılamadı: zincire ulaşılamıyor. Lütfen biraz sonra tekrar dene.".to_string()),
    };
    // Kaydeden KUBRA değilse kurum kaydını sor (/kurum/<40 hex>). Hata → None (belirtilir).
    let kaydeden = kaydeden_coz(&v);
    let kubra_hex = hex::encode(kubra_adres);
    let kurum = match kaydeden.as_deref() {
        Some(k) if k != kubra_hex => {
            match http.get(format!("{taban}/kurum/{k}")).send().await {
                Ok(r) => r.json::<serde_json::Value>().await.ok(),
                Err(_) => None,
            }
        }
        _ => None,
    };
    Some(belge_dogrula_metni(&hash, &v, &kubra_hex, kurum.as_ref()))
}

/// /belge yanitindaki kaydeden adres (kucuk harf, 0x'siz, 40 hex) — gecersizse None.
fn kaydeden_coz(v: &serde_json::Value) -> Option<String> {
    let a = v.get("kaydeden").and_then(|x| x.as_str())?.trim().trim_start_matches("0x").to_lowercase();
    (a.len() == 40 && a.bytes().all(|b| b.is_ascii_hexdigit())).then_some(a)
}

/// Belge doğrulama cevabı (SAF: ağ yok). "Birebir aynı" demeden ÖNCE kaydedenin kim
/// olduğunu açıkça belirtir: KUBRA etkileşim kaydı mı, kayıtlı kurum (beyan) mı,
/// yoksa kurum olarak kayıtlı olmayan bir adres mi.
pub fn belge_dogrula_metni(hash: &str, belge: &serde_json::Value, kubra_hex: &str, kurum: Option<&serde_json::Value>) -> String {
    // /belge/:hash yaniti: kayitli + (varsa) kaydeden/zaman.
    let kayitli = belge.get("kayitli").and_then(|x| x.as_bool())
        .or_else(|| belge.get("var").and_then(|x| x.as_bool()))
        .unwrap_or(false);
    if !kayitli {
        return format!(
            "Belge BULUNAMADI: bu hash ({hash}) AIDAG-Chain'de kayıtlı DEĞİL. Ya hiç kaydedilmemiş ya da belge değiştirilmiş (hash tutmuyor). Orijinal belgenin hash'iyle tekrar dene."
        );
    }
    let zaman = belge.get("zaman").and_then(|x| x.as_u64())
        .map(|z| format!(" Kayıt zamanı: {} UTC.", utc_tarih(z)))
        .unwrap_or_default();
    let kaydeden = kaydeden_coz(belge);
    let adres = kaydeden.as_deref().map(|a| format!("0x{a}")).unwrap_or_else(|| "bilinmiyor".into());
    let kimlik = match kaydeden.as_deref() {
        Some(k) if k == kubra_hex => {
            return format!(
                "Bu hash ({hash}) AIDAG-Chain'de kayıtlı, ANCAK kaydeden KUBRA'nın kendi adresi ({adres}): bu bir KUBRA etkileşim kaydıdır, belge kaydı DEĞİLDİR.{zaman} Bir kurumun belgeyi kaydettiğini göstermez; KUBRA sohbet kanıtını doğrulamak için /v1/verify (prompt, answer, model, ts, salt) kullanılır."
            );
        }
        None => "Kaydeden adres zincir yanıtında yok; kaydedenin kimliği doğrulanamadı.".to_string(),
        Some(_) => match kurum {
            None => format!("Kaydeden: {adres}. Kaydedenin kurum durumu şu an sorgulanamadı; kimliği DOĞRULANMADI."),
            Some(k) if k.get("kayitli").and_then(|x| x.as_bool()) == Some(true) => {
                let ad = k.get("ad").and_then(|x| x.as_str()).unwrap_or("(adsız)");
                let kat = k.get("kategori").and_then(|x| x.as_str()).map(|c| format!(", {c}")).unwrap_or_default();
                match k.get("dogrulanmis").and_then(|x| x.as_bool()) {
                    Some(true) => format!("Kaydeden: {adres} — kurum \"{ad}\"{kat} (zincirde DOĞRULANMIŞ kurum)."),
                    _ => format!("Kaydeden: {adres} — kurum \"{ad}\"{kat}. Kurum kaydı BEYANDIR (doğrulanmamış): kurumun kimliği bağımsız olarak doğrulanmamıştır."),
                }
            }
            Some(_) => format!("Kaydeden: {adres}. Bu adres kurum olarak KAYITLI DEĞİL; kaydedenin kimliği bilinmiyor."),
        },
    };
    format!(
        "Bu hash AIDAG-Chain'de kayıtlı ({hash}). {kimlik}{zaman} Belge DOĞRULANDI (içerik bütünlüğü): bu hash'i üreten belge, zincire kaydedilen belgeyle birebir aynıdır (değiştirilmemiştir). Bu, belgenin kim tarafından ve hangi yetkiyle kaydedildiğini değil, yalnızca değişmediğini kanıtlar."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const H: &str = "0dcce43d9a705bcdeb481f5447a0516d319bbb8cc33031f466f828f10a0c544f";

    #[test]
    fn belge_dogrula_kaydedeni_belirtir() {
        use serde_json::json;
        let kubra = "ab".repeat(20);
        let baska = "cd".repeat(20);
        let kayit = |k: &str| json!({ "kayitli": true, "kaydeden": format!("0x{k}"), "zaman": 0 });
        // KUBRA etkilesim kaydi -> belge kaydi DEGIL, "birebir aynidir" YOK
        let m = belge_dogrula_metni(H, &kayit(&kubra), &kubra, None);
        assert!(m.contains("KUBRA etkileşim kaydıdır") && m.contains("belge kaydı DEĞİLDİR"), "{m}");
        assert!(!m.contains("birebir"), "{m}");
        assert!(!m.contains("DOĞRULANDI"), "{m}");
        // Buyuk harf / 0x'li KUBRA adresi de yakalanir
        let m = belge_dogrula_metni(H, &json!({ "kayitli": true, "kaydeden": format!("0x{}", kubra.to_uppercase()) }), &kubra, None);
        assert!(m.contains("KUBRA etkileşim kaydıdır"), "{m}");
        // Kayitli kurum (beyan) -> ad + "beyan (dogrulanmamis)", birebir cumlesinden ONCE
        let kurum = json!({ "ok": true, "kayitli": true, "ad": "Ornek Belediyesi", "kategori": "devlet" });
        let m = belge_dogrula_metni(H, &kayit(&baska), &kubra, Some(&kurum));
        assert!(m.contains("Ornek Belediyesi") && m.contains("BEYANDIR (doğrulanmamış)"), "{m}");
        assert!(m.find("BEYANDIR").unwrap() < m.find("birebir").unwrap());
        // Zincir dogrulanmis alani dondururse
        let kurum_d = json!({ "kayitli": true, "ad": "X Univ", "dogrulanmis": true });
        let m = belge_dogrula_metni(H, &kayit(&baska), &kubra, Some(&kurum_d));
        assert!(m.contains("DOĞRULANMIŞ kurum") && !m.contains("BEYANDIR"), "{m}");
        let kurum_d = json!({ "kayitli": true, "ad": "X Univ", "dogrulanmis": false });
        assert!(belge_dogrula_metni(H, &kayit(&baska), &kubra, Some(&kurum_d)).contains("BEYANDIR"));
        // Kurum degil
        let m = belge_dogrula_metni(H, &kayit(&baska), &kubra, Some(&json!({ "ok": true, "kayitli": false })));
        assert!(m.contains("kurum olarak KAYITLI DEĞİL"), "{m}");
        // Kurum sorgusu basarisiz
        let m = belge_dogrula_metni(H, &kayit(&baska), &kubra, None);
        assert!(m.contains("sorgulanamadı") && m.contains("DOĞRULANMADI"), "{m}");
        // Kayitsiz
        let m = belge_dogrula_metni(H, &json!({ "kayitli": false }), &kubra, None);
        assert!(m.starts_with("Belge BULUNAMADI"));
    }

    #[test]
    fn tek_basina_hash_bulunur() {
        assert_eq!(belge_hash_bul(H).as_deref(), Some(H));
        assert_eq!(belge_hash_bul(&format!("şunu kontrol et: 0x{H}, lütfen")).as_deref(), Some(H));
        assert_eq!(belge_hash_bul(&format!("({})", H.to_uppercase())).as_deref(), Some(H));
    }

    #[test]
    fn utc_tarih_dogru() {
        assert_eq!(utc_tarih(0), "1970-01-01 00:00");
        assert_eq!(utc_tarih(1_709_210_096), "2024-02-29 12:34"); // artık yıl
    }

    #[test]
    fn kisaltilmis_hash_tespit() {
        assert!(kisaltilmis_hash_var("0dcce43d9a705bcd..."));
        assert!(kisaltilmis_hash_var("bu belge 0dcce43d…0c544f geçerli mi"));
        assert!(!kisaltilmis_hash_var(H));
        assert!(!kisaltilmis_hash_var("bekle..."));
        assert!(!kisaltilmis_hash_var("a decade..."));
    }

    #[test]
    fn kayit_ve_dogrulama_ayrimi() {
        assert!(belge_kayit_niyeti_mi("Belge oluşturacağım"));
        assert!(belge_kayit_niyeti_mi("belgemi zincire kaydetmek istiyorum"));
        assert!(!belge_kayit_niyeti_mi("bu belge zincirde kayıtlı mı"));
        assert!(aciklama_sorusu_mu("Belge doğrulama nasıl çalışır"));
        assert!(belge_niyeti_mi("bu belge geçerli mi"));
        assert!(!belge_niyeti_mi("AIDAG hakkında doğrulanmış bilgi ver"));
    }

    #[test]
    fn ag_niyetleri() {
        for q in ["Aidag sağlık durumu", "ağ durumu", "status", "zincir çalışıyor mu", "AIDAG durumu"] {
            assert!(ag_niyeti_mi(q), "{q}");
        }
        assert!(!ag_niyeti_mi("https://aidag-chain.com nedir"));
    }
}

// ── ON SATIS / TGE CANLI DURUM ARACI ────────────────────────────────────────
// Satilan miktar, aktif kademe ve TGE durumu DEGISKEN bilgidir: belgeye yazilirsa
// eskir. Bu arac her soruda zincirden okur (arka planda kendiliginden guncel).
// Kademe tablosu on-satis-izleyici.py + site (app/on-satis/page.tsx) ile AYNI olmali.
const KADEMELER: [(u64, f64); 8] = [
    (210_000, 0.20), (420_000, 0.25), (630_000, 0.30),
    (840_000, 0.35), (1_050_000, 0.40), (1_260_000, 0.45), (1_470_000, 0.50), (1_680_000, 0.55),
];
const TGE_BELIRSIZ: u64 = 4_102_444_800; // lsc_engine::mainnet::TGE_BELIRSIZ

fn on_satis_niyeti_mi(sorgu: &str) -> bool {
    let s = sade(sorgu);
    let canli = [
        "ne kadar satildi", "kac aidag satildi", "kac satildi", "satilan", "satis durumu",
        "on satis durumu", "presale durumu", "hangi kademe", "aktif kademe", "su anki fiyat",
        "guncel fiyat", "simdiki fiyat", "tge ne zaman", "tge tarihi", "tge belli mi",
        "kalan aidag", "ne kadar kaldi",
    ];
    canli.iter().any(|a| anahtar_var(&s, a))
}

/// Satilan (test haric) miktara gore aktif kademe: (faz, kademe_no 1..8, fiyat, kademede kalan).
fn aktif_kademe(satilan: u64) -> Option<(u8, usize, f64, u64)> {
    KADEMELER.iter().enumerate().find(|(_, (sinir, _))| satilan < *sinir).map(|(i, (sinir, fiyat))| {
        (if i < 3 { 1 } else { 2 }, i + 1, *fiyat, sinir - satilan)
    })
}

fn binlik(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 { out.push('.'); }
        out.push(c);
    }
    out
}

pub async fn on_satis_durumu(http: &reqwest::Client, rpc_url: &str, sorgu: &str) -> Option<String> {
    if !on_satis_niyeti_mi(sorgu) {
        return None;
    }
    let base = rpc_url.trim_end_matches('/');
    let oz: serde_json::Value = match http.get(format!("{base}/on-satis-ozet")).send().await {
        Ok(r) => r.json().await.ok()?,
        Err(_) => return Some("Ön satış durumunu şu an zincirden okuyamadım; lütfen biraz sonra tekrar dene.".to_string()),
    };
    let satilan = oz.get("toplam_satilan_aidag").and_then(|x| x.as_str())
        .and_then(|x| x.parse::<u128>().ok()).map(|w| (w / 1_000_000_000_000_000_000) as u64)?;
    let alim = oz.get("alim_sayisi").and_then(|x| x.as_u64()).unwrap_or(0);
    let tge = http.get(format!("{base}/on-satis-tahsis/0000000000000000000000000000000000000000")).send().await.ok()?
        .json::<serde_json::Value>().await.ok()?
        .get("tge").and_then(|x| x.as_u64())?;
    let kademe = match aktif_kademe(satilan) {
        Some((faz, no, fiyat, kalan)) => format!(
            "Aktif kademe: Faz {faz}{} · kademe {no}/8 · {fiyat:.2} $ / AIDAG · bu kademede kalan {} AIDAG.",
            if faz == 2 { " (Rezerv)" } else { "" }, binlik(kalan)
        ),
        None => "Ön satış toplam tavanı (1.680.000 AIDAG) doldu.".to_string(),
    };
    let tge_metin = if tge >= TGE_BELIRSIZ {
        "TGE tarihi henüz BELİRLENMEDİ: ön satış tamamlanıp listeleme kararı alınınca zincirde en az 3 gün önceden ilan edilecek.".to_string()
    } else {
        format!("TGE zincirde ayarlı: {} UTC.", utc_tarih(tge))
    };
    Some(format!(
        "AIDAG ön satış durumu (zincirden canlı): {} AIDAG satıldı ({} alım) / toplam tavan 1.680.000. {kademe} {tge_metin} Resmi satış yalnızca aidag-chain.com/on-satis sayfasındadır; bu bilgi yatırım tavsiyesi değildir.",
        binlik(satilan), alim
    ))
}

#[cfg(test)]
mod on_satis_testleri {
    use super::*;

    #[test]
    fn kademe_gecisleri() {
        assert_eq!(aktif_kademe(29), Some((1, 1, 0.20, 209_971)));
        assert_eq!(aktif_kademe(630_000), Some((2, 4, 0.35, 210_000)), "Faz 1 dolunca Faz 2 baslar");
        assert_eq!(aktif_kademe(1_679_999).map(|k| k.1), Some(8));
        assert_eq!(aktif_kademe(1_680_000), None);
        assert_eq!(binlik(1_680_000), "1.680.000");
    }

    #[test]
    fn niyet() {
        assert!(on_satis_niyeti_mi("Ön satışta şu ana kadar ne kadar satıldı?"));
        assert!(on_satis_niyeti_mi("TGE ne zaman"));
        assert!(!on_satis_niyeti_mi("Ön satış nasıl çalışır"));
    }
}
