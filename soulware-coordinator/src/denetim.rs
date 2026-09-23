//! Koordinator guvenlik yardimcilari (saf, test edilebilir):
//!  - istemci IP'si (yalniz yerel ters vekil basliklarina guvenilir),
//!  - yedeklilik oylamasi (ayni cuzdan / ayni IP tek oy),
//!  - ucretsiz is gunluk kotasi (istemci basina),
//!  - ucretli is odemesinin belirli bir tip=7 LSC transferine baglanmasi.

use axum::http::HeaderMap;
use lsc_engine::dag::wire;
use lsc_engine::tx::LscTransferKaydi;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};

/// Istemci IP'si. Dogrudan baglanti (loopback degil) -> soket adresi. Loopback
/// (nginx vb. yerel vekil) -> X-Real-IP, yoksa X-Forwarded-For'un SON elemani
/// (vekilin ekledigi; istemcinin yazdigi onceki elemanlara guvenilmez). Hicbiri
/// yoksa None (IP bilinmiyor -> yalniz cuzdan tekilligi uygulanir).
pub fn istemci_ip(peer: Option<SocketAddr>, h: &HeaderMap) -> Option<String> {
    let peer = peer?;
    if !peer.ip().is_loopback() {
        return Some(peer.ip().to_string());
    }
    let gecerli = |s: &str| s.trim().parse::<IpAddr>().ok().map(|ip| ip.to_string());
    if let Some(ip) = h.get("x-real-ip").and_then(|v| v.to_str().ok()).and_then(gecerli) {
        return Some(ip);
    }
    h.get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .and_then(gecerli)
}

/// Oylamadaki bir sonuc (sirali: gonderim sirasi).
pub struct Oy<'a> {
    pub worker: &'a str,
    pub hash: &'a str,
    pub ip: Option<&'a str>,
}

/// Yedeklilik karari: bir cevap-hash'i icin FARKLI cuzdan VE (biliniyorsa) FARKLI IP
/// sayisi >= `gereken` ise dogrulandi. Donus: (kazanan hash, oy sayilan cuzdanlar).
/// Ayni IP'den ikinci cuzdan (sybil) oy SAYILMAZ ve odul ALMAZ.
pub fn oylama(sonuclar: &[Oy], gereken: usize) -> Option<(String, Vec<String>)> {
    struct Grup<'a> {
        hash: &'a str,
        sayilan: Vec<String>,
        ipler: HashSet<&'a str>,
        cuzdanlar: HashSet<&'a str>,
    }
    let mut gruplar: Vec<Grup> = Vec::new();
    for s in sonuclar {
        let idx = match gruplar.iter().position(|g| g.hash == s.hash) {
            Some(i) => i,
            None => {
                gruplar.push(Grup { hash: s.hash, sayilan: vec![], ipler: HashSet::new(), cuzdanlar: HashSet::new() });
                gruplar.len() - 1
            }
        };
        let g = &mut gruplar[idx];
        if !g.cuzdanlar.insert(s.worker) {
            continue; // ayni cuzdan tekrar
        }
        if let Some(ip) = s.ip {
            if !g.ipler.insert(ip) {
                continue; // ayni IP'den ikinci cuzdan
            }
        }
        g.sayilan.push(s.worker.to_string());
    }
    gruplar
        .into_iter()
        .find(|g| g.sayilan.len() >= gereken.max(1))
        .map(|g| (g.hash.to_string(), g.sayilan))
}

/// Ucretsiz is gunluk kotasi: tum anahtarlar sinir altindaysa hepsini artir ve true.
/// kayit: anahtar -> (gun, sayac). Gun degisince sayac sifirlanir.
pub fn gunluk_kota_dene(kayit: &mut HashMap<String, (u64, u32)>, anahtarlar: &[String], gun: u64, sinir: u32) -> bool {
    if sinir == 0 {
        return false;
    }
    let dolu = anahtarlar.iter().any(|k| matches!(kayit.get(k), Some((g, n)) if *g == gun && *n >= sinir));
    if dolu {
        return false;
    }
    for k in anahtarlar {
        let e = kayit.entry(k.clone()).or_insert((gun, 0));
        if e.0 != gun {
            *e = (gun, 0);
        }
        e.1 += 1;
    }
    // eski gunleri temizle (bellek sismesin)
    kayit.retain(|_, (g, _)| *g == gun);
    true
}

/// Dogrulanmis odeme bilgisi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Odeme {
    /// Vertex id (64 hex) = odemenin tekil kimligi.
    pub vertex_id: String,
    /// Odeyen (imzalayan) adres, 0x + 40 hex.
    pub payer: String,
    pub miktar_wei: u128,
    pub nonce: u64,
}

impl Odeme {
    /// Ikinci tekillik anahtari: ayni (odeyen, nonce) iki odeme olamaz.
    pub fn nonce_anahtari(&self) -> String {
        format!("{}:{}", self.payer, self.nonce)
    }
}

/// Ucretli is odemesini coz ve ise bagla (ag yok): imzali tip=7 LSC transferi,
/// dogru ag, alici = havuz, miktar >= ucret, imzalayan = isin payer'i.
pub fn odeme_coz(hex_str: &str, net_id: u32, havuz: &[u8; 20], payer: &str, ucret_wei: u128) -> Result<Odeme, String> {
    let bayt = hex::decode(hex_str.trim().trim_start_matches("0x")).map_err(|_| "odeme_hex gecersiz hex")?;
    let v = wire::decode(&bayt).map_err(|e| format!("odeme vertex'i cozulemedi: {e:?}"))?;
    v.verify().map_err(|e| format!("odeme vertex imzasi gecersiz: {e:?}"))?;
    if v.network_id() != net_id {
        return Err("odeme baska aga ait".into());
    }
    let t = LscTransferKaydi::decode(v.payload()).map_err(|_| "odeme bir tip=7 LSC transferi degil")?;
    if &t.alici != havuz {
        return Err("odemenin alicisi havuz adresi degil".into());
    }
    if t.miktar < ucret_wei {
        return Err(format!("odeme miktari yetersiz ({} < {} wei)", t.miktar, ucret_wei));
    }
    let imzalayan = format!("0x{}", hex::encode(lsc_engine::public_key_to_adres(v.public_key())));
    if imzalayan != payer.trim().to_lowercase() {
        return Err("odemeyi imzalayan, isin payer adresi degil".into());
    }
    Ok(Odeme { vertex_id: hex::encode(v.id()), payer: imzalayan, miktar_wei: t.miktar, nonce: t.nonce })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn hm(ciftler: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in ciftler {
            h.insert(*k, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn istemci_ip_yalniz_yerel_vekile_guvenir() {
        let dis: SocketAddr = "203.0.113.5:4000".parse().unwrap();
        let yerel: SocketAddr = "127.0.0.1:4000".parse().unwrap();
        // dis baglantida sahte baslik YOK SAYILIR
        assert_eq!(istemci_ip(Some(dis), &hm(&[("x-forwarded-for", "1.1.1.1"), ("x-real-ip", "2.2.2.2")])).as_deref(), Some("203.0.113.5"));
        // yerel vekil: X-Real-IP
        assert_eq!(istemci_ip(Some(yerel), &hm(&[("x-real-ip", "198.51.100.7")])).as_deref(), Some("198.51.100.7"));
        // yerel vekil: XFF'nin SON elemani (istemci ilkini sahteleyebilir)
        assert_eq!(istemci_ip(Some(yerel), &hm(&[("x-forwarded-for", "9.9.9.9, 198.51.100.8")])).as_deref(), Some("198.51.100.8"));
        // baslik yok / gecersiz -> bilinmiyor
        assert_eq!(istemci_ip(Some(yerel), &HeaderMap::new()), None);
        assert_eq!(istemci_ip(Some(yerel), &hm(&[("x-forwarded-for", "zzz")])), None);
        assert_eq!(istemci_ip(None, &HeaderMap::new()), None);
    }

    #[test]
    fn oylama_ayni_ip_ve_cuzdan_tek_oy() {
        let oy = |w, h, ip| Oy { worker: w, hash: h, ip };
        // SYBIL: ayni IP'den iki cuzdan ayni cevap -> dogrulanmaz
        assert_eq!(oylama(&[oy("a", "H", Some("1.1.1.1")), oy("b", "H", Some("1.1.1.1"))], 2), None);
        // ayni cuzdan iki kez -> tek oy
        assert_eq!(oylama(&[oy("a", "H", None), oy("a", "H", None)], 2), None);
        // farkli IP -> dogrulanir
        assert_eq!(oylama(&[oy("a", "H", Some("1.1.1.1")), oy("b", "H", Some("2.2.2.2"))], 2),
            Some(("H".into(), vec!["a".into(), "b".into()])));
        // IP bilinmiyorsa cuzdan tekilligi yeter
        assert_eq!(oylama(&[oy("a", "H", None), oy("b", "H", None)], 2).map(|x| x.1.len()), Some(2));
        // sybil ucuncu cuzdan odul ALMAZ
        let r = oylama(&[oy("a", "H", Some("1.1.1.1")), oy("s", "H", Some("1.1.1.1")), oy("b", "H", Some("2.2.2.2"))], 2).unwrap();
        assert_eq!(r.1, vec!["a".to_string(), "b".to_string()]);
        // farkli cevaplar ayri gruplar
        assert_eq!(oylama(&[oy("a", "H", None), oy("b", "X", None)], 2), None);
    }

    #[test]
    fn gunluk_kota() {
        let mut k = HashMap::new();
        let a = vec!["ip:1.1.1.1".to_string()];
        for _ in 0..3 {
            assert!(gunluk_kota_dene(&mut k, &a, 10, 3));
        }
        assert!(!gunluk_kota_dene(&mut k, &a, 10, 3), "4. is ayni gun reddedilir");
        // ayni IP farkli cuzdan: IP anahtari dolu -> red
        assert!(!gunluk_kota_dene(&mut k, &["ip:1.1.1.1".into(), "cuzdan:0xb".into()], 10, 3));
        assert!(!k.contains_key("cuzdan:0xb"), "red durumunda sayac artmaz");
        // baska IP serbest
        assert!(gunluk_kota_dene(&mut k, &["ip:2.2.2.2".into()], 10, 3));
        // ertesi gun sifirlanir
        assert!(gunluk_kota_dene(&mut k, &a, 11, 3));
        assert!(!gunluk_kota_dene(&mut k, &a, 11, 0), "sinir 0 = kapali");
    }

    #[test]
    fn odeme_belirli_transfere_baglanir() {
        use lsc_engine::Vertex;
        let net = 99_999;
        let havuz = [0x11u8; 20];
        let sk = ed25519_dalek::SigningKey::from_bytes(&[5u8; 32]);
        let payer = format!("0x{}", hex::encode(lsc_engine::public_key_to_adres(&sk.verifying_key().to_bytes())));
        let yap = |net: u32, alici: [u8; 20], miktar: u128, nonce: u64, sk: &ed25519_dalek::SigningKey| {
            let p = LscTransferKaydi::new(alici, miktar, nonce).encode();
            hex::encode(wire::encode(&Vertex::new_signed(net, vec![], p, 1, sk).unwrap()))
        };
        let ok = odeme_coz(&yap(net, havuz, 10, 3, &sk), net, &havuz, &payer, 10).unwrap();
        assert_eq!((ok.payer.as_str(), ok.miktar_wei, ok.nonce), (payer.as_str(), 10, 3));
        assert_eq!(ok.nonce_anahtari(), format!("{payer}:3"));
        assert_eq!(ok.vertex_id.len(), 64);
        assert!(odeme_coz(&yap(net + 1, havuz, 10, 3, &sk), net, &havuz, &payer, 10).is_err(), "baska ag");
        assert!(odeme_coz(&yap(net, [0x22; 20], 10, 3, &sk), net, &havuz, &payer, 10).is_err(), "alici havuz degil");
        assert!(odeme_coz(&yap(net, havuz, 9, 3, &sk), net, &havuz, &payer, 10).is_err(), "eksik miktar");
        let baska = ed25519_dalek::SigningKey::from_bytes(&[6u8; 32]);
        assert!(odeme_coz(&yap(net, havuz, 10, 3, &baska), net, &havuz, &payer, 10).is_err(), "baskasinin odemesi");
        // tip=1 Record (odeme degil)
        let rec = hex::encode(wire::encode(&Vertex::new_signed(net, vec![], lsc_engine::tx::Record::new([0; 32]).encode(), 1, &sk).unwrap()));
        assert!(odeme_coz(&rec, net, &havuz, &payer, 10).is_err());
        // bozulmus imza
        let mut b = hex::decode(yap(net, havuz, 10, 3, &sk)).unwrap();
        let n = b.len();
        b[n - 1] ^= 1;
        assert!(odeme_coz(&hex::encode(b), net, &havuz, &payer, 10).is_err());
    }
}
