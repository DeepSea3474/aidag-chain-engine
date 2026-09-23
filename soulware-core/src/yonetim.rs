//! Yonetim uclari (/kb/ingest, /kb/stats, /retrieve, /models) icin kimlik + sinirlar.
//!
//! - `SOULWARE_YONETIM_TOKEN` ayarli degilse (ya da 16 karakterden kisaysa) uclar
//!   KAPALI: 403. Ayarliysa `Authorization: Bearer <token>` zorunlu; yoksa/yanlissa 401.
//! - Karsilastirma sabit-zamanli: iki taraf blake3 ile 32 bayta indirilir ve XOR
//!   birikimiyle karsilastirilir (uzunluk ve ilk-farkli-bayt zamanlamasi sizmaz).
//! - Govde sinirlari (axum DefaultBodyLimit) ve belge basina alan sinirlari.

use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// /v1/ask, /v1/ask-stream, /retrieve govde siniri.
pub const ASK_GOVDE_SINIRI: usize = 32 * 1024;
/// /v1/verify: prompt + answer + context (context en fazla ask siniri kadar).
pub const VERIFY_GOVDE_SINIRI: usize = 128 * 1024;
/// /v1/image, /v1/video (yalniz istem + cuzdan).
pub const MEDYA_GOVDE_SINIRI: usize = 16 * 1024;
/// /kb/ingest govde siniri.
pub const INGEST_GOVDE_SINIRI: usize = 256 * 1024;
/// Belge basina alan sinirlari (bayt).
pub const BASLIK_MAKS: usize = 512;
pub const METIN_MAKS: usize = 200 * 1024;
pub const URL_MAKS: usize = 2048;
/// Token en az bu uzunlukta olmali (kisa token = kapali sayilir).
pub const TOKEN_MIN: usize = 16;

/// Env degerinden token: bos/kisa -> None (uclar kapali).
pub fn token_coz(v: Option<String>) -> Option<String> {
    let t = v?.trim().to_string();
    if t.len() < TOKEN_MIN {
        if !t.is_empty() {
            eprintln!("⚠ SOULWARE_YONETIM_TOKEN {TOKEN_MIN} karakterden kisa → yonetim uclari KAPALI");
        }
        return None;
    }
    Some(t)
}

/// Sabit-zamanli esitlik (blake3 ozetleri uzerinden).
pub fn sabit_zamanli_esit(a: &[u8], b: &[u8]) -> bool {
    let ha = blake3::hash(a);
    let hb = blake3::hash(b);
    let mut fark = 0u8;
    for (x, y) in ha.as_bytes().iter().zip(hb.as_bytes().iter()) {
        fark |= x ^ y;
    }
    fark == 0
}

/// Yetki karari (saf). Ok(()) = gecer; Err(kod) = 403 (kapali) / 401 (yok/yanlis).
pub fn yetki_karari(h: &HeaderMap, token: Option<&str>) -> Result<(), StatusCode> {
    let Some(beklenen) = token else { return Err(StatusCode::FORBIDDEN) };
    let gelen = h
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim);
    match gelen {
        Some(g) if sabit_zamanli_esit(g.as_bytes(), beklenen.as_bytes()) => Ok(()),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Handler yardimcisi: yetkisizse hazir HTTP yaniti.
#[allow(clippy::result_large_err)]
pub fn yetki(h: &HeaderMap, token: Option<&str>) -> Result<(), Response> {
    yetki_karari(h, token).map_err(|k| {
        let hata = if k == StatusCode::FORBIDDEN {
            "yonetim ucu kapali (SOULWARE_YONETIM_TOKEN ayarli degil)"
        } else {
            "yetkisiz: Authorization: Bearer <token> gerekli"
        };
        (k, Json(json!({ "ok": false, "hata": hata }))).into_response()
    })
}

/// Belge basina boyut siniri.
pub fn ingest_sinir(baslik: &str, metin: &str, url: Option<&str>) -> Result<(), String> {
    if baslik.len() > BASLIK_MAKS {
        return Err(format!("baslik en fazla {BASLIK_MAKS} bayt"));
    }
    if metin.len() > METIN_MAKS {
        return Err(format!("metin en fazla {METIN_MAKS} bayt"));
    }
    if url.map(str::len).unwrap_or(0) > URL_MAKS {
        return Err(format!("url en fazla {URL_MAKS} bayt"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    const T: &str = "cok-gizli-yonetim-tokeni-0123456789";

    fn baslik(v: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, HeaderValue::from_str(v).unwrap());
        h
    }

    #[test]
    fn token_yoksa_kapali_403() {
        assert_eq!(token_coz(None), None);
        assert_eq!(token_coz(Some("".into())), None);
        assert_eq!(token_coz(Some("kisa".into())), None, "kisa token kapali sayilir");
        assert_eq!(yetki_karari(&HeaderMap::new(), None), Err(StatusCode::FORBIDDEN));
        // Token yokken dogru gorunen baslik da ise yaramaz.
        assert_eq!(yetki_karari(&baslik(&format!("Bearer {T}")), None), Err(StatusCode::FORBIDDEN));
    }

    #[test]
    fn baslik_yok_veya_yanlis_401() {
        let t = token_coz(Some(format!("  {T} "))).unwrap();
        assert_eq!(yetki_karari(&HeaderMap::new(), Some(&t)), Err(StatusCode::UNAUTHORIZED));
        for yanlis in [T.to_string(), format!("Basic {T}"), format!("Bearer {T}x"), format!("Bearer {}", &T[..T.len() - 1]), "Bearer ".into()] {
            assert_eq!(yetki_karari(&baslik(&yanlis), Some(&t)), Err(StatusCode::UNAUTHORIZED), "{yanlis}");
        }
        assert_eq!(yetki_karari(&baslik(&format!("Bearer {T}")), Some(&t)), Ok(()));
    }

    #[test]
    fn sabit_zamanli_esitlik_dogru() {
        assert!(sabit_zamanli_esit(b"abc", b"abc"));
        assert!(!sabit_zamanli_esit(b"abc", b"abd"));
        assert!(!sabit_zamanli_esit(b"abc", b"abcd"));
        assert!(!sabit_zamanli_esit(b"", b"a"));
    }

    #[test]
    fn ingest_boyut_sinirlari() {
        assert!(ingest_sinir("b", "m", None).is_ok());
        assert!(ingest_sinir(&"b".repeat(BASLIK_MAKS + 1), "m", None).is_err());
        assert!(ingest_sinir("b", &"m".repeat(METIN_MAKS + 1), None).is_err());
        assert!(ingest_sinir("b", "m", Some(&"u".repeat(URL_MAKS + 1))).is_err());
        const { assert!(METIN_MAKS < INGEST_GOVDE_SINIRI) };
    }
}
