//! belge-imzala — kurum personeli icin CEVRIMDISI belge kayit imzalayici.
//!
//! KUBRA'nin hazirladigi IMZASIZ talebi (aidag-belge-kayit-talebi JSON)
//! personelin KENDI anahtariyla imzalar. Ag baglantisi YOKTUR; anahtar
//! makineden cikmaz. Sunucudan gelen "imzalanacak_id"ye GUVENILMEZ: vertex
//! talep alanlarindan yeniden kurulur ve payload'in yalniz [tip=1][belge_hash]
//! oldugu dogrulanir (talep baska bir sey imzalatamaz).
//!
//! Kullanim:
//!   belge-imzala <anahtar_dosyasi> <talep.json> [--talep-ts]
//!     anahtar_dosyasi : [algo=1][32 seed] (33 bayt)
//!     --talep-ts      : talepteki ts'yi kullan (varsayilan: imza ani = simdi)
//!
//! Cikti (stdout): imzali vertex wire-hex. Sonra:
//!   curl -X POST <RPC>/submit -H 'Content-Type: application/json' -d '{"hex":"<HEX>"}'
//!
//! UYARI: Zincirdeki kurum kaydi (tip=5) beyana dayanir; bu arac cevrimdisi
//! oldugu icin kurum kaydini kontrol edemez. Talepteki "imzalayan.kurum"
//! alani yalniz bilgi icindir.

use ed25519_dalek::SigningKey;
use lsc_engine::belge_talep::KayitTalebi;
use lsc_engine::dag::wire;
use lsc_engine::Vertex;
use serde_json::Value;

const TALEP_TUR: &str = "aidag-belge-kayit-talebi";

fn hex32(s: &str, alan: &str) -> Result<[u8; 32], String> {
    let b = hex::decode(s.trim().trim_start_matches("0x")).map_err(|_| format!("{alan}: gecersiz hex"))?;
    b.try_into().map_err(|_| format!("{alan}: 32 bayt (64 hex) olmali"))
}

/// Talepten imzali vertex kur. `ts`: None -> talepteki ts.
fn talepten_vertex(talep: &Value, sk: &SigningKey, ts: Option<u64>) -> Result<Vertex, String> {
    if talep["tur"] != TALEP_TUR {
        return Err(format!("talep turu '{TALEP_TUR}' degil"));
    }
    if talep["surum"] != 1 {
        return Err("desteklenmeyen talep surumu".into());
    }
    if talep["durum"] == "zaten-kayitli" || talep["zincir"]["kayitli"] == true {
        return Err("belge zaten zincirde kayitli; imzalanmadi".into());
    }
    let net_id = talep["network_id"].as_u64().and_then(|n| u32::try_from(n).ok()).ok_or("network_id eksik")?;
    let hash = hex32(talep["belge_hash"].as_str().ok_or("belge_hash eksik")?, "belge_hash")?;
    let parents = talep["parents"].as_array().ok_or("parents eksik")?
        .iter().map(|p| p.as_str().ok_or("parent hex degil".to_string()).and_then(|s| hex32(s, "parent")))
        .collect::<Result<Vec<_>, _>>()?;
    let talep_ts = talep["ts"].as_u64().ok_or("ts eksik")?;
    let kt = KayitTalebi::yeni(net_id, hash, parents, ts.unwrap_or(talep_ts)).map_err(|e| e.to_string())?;
    // Talep yalniz Record(belge_hash) imzalatabilir: payload_hex bununla BIREBIR ayni olmali.
    if talep["payload_hex"].as_str() != Some(hex::encode(kt.payload()).as_str()) {
        return Err("payload_hex belge_hash ile uyusmuyor (talep degistirilmis olabilir); imzalanmadi".into());
    }
    // Talep belirli bir personel icin hazirlandiysa, anahtar o personelin olmali.
    let pk = sk.verifying_key().to_bytes();
    if let Some(beklenen) = talep["imzalayan"]["pubkey"].as_str() {
        if hex32(beklenen, "imzalayan.pubkey")? != pk {
            return Err("talep baska bir imzalayan icin hazirlanmis; imzalanmadi".into());
        }
    }
    kt.imzala(sk).map_err(|e| e.to_string())
}

fn anahtar_oku(yol: &str) -> Result<SigningKey, String> {
    let d = std::fs::read(yol).map_err(|_| "anahtar dosyasi okunamadi".to_string())?;
    if d.len() != 33 || d[0] != 1 {
        return Err("anahtar bicimi [1][32 seed] (33 bayt) olmali".into());
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&d[1..]);
    Ok(SigningKey::from_bytes(&seed))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let talep_ts = args.iter().any(|a| a == "--talep-ts");
    let konum: Vec<&String> = args.iter().skip(1).filter(|a| !a.starts_with("--")).collect();
    let calis = || -> Result<(), String> {
        if konum.len() != 2 {
            return Err("Kullanim: belge-imzala <anahtar_dosyasi> <talep.json> [--talep-ts]".into());
        }
        let sk = anahtar_oku(konum[0])?;
        let talep: Value = serde_json::from_slice(&std::fs::read(konum[1]).map_err(|_| "talep dosyasi okunamadi")?)
            .map_err(|e| format!("talep JSON degil: {e}"))?;
        let simdi = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_secs();
        let v = talepten_vertex(&talep, &sk, (!talep_ts).then_some(simdi))?;
        println!("{}", hex::encode(wire::encode(&v)));
        eprintln!(
            "OK  belge_hash={}  imzalayan=0x{}  net={}  ts={}  vertex={}",
            talep["belge_hash"].as_str().unwrap_or(""),
            hex::encode(lsc_engine::public_key_to_adres(v.public_key())),
            v.network_id(), v.timestamp(), hex::encode(v.id())
        );
        eprintln!("UYARI: kurum kaydi zincirde beyana dayanir; bu arac cevrimdisidir ve kurum kaydini kontrol etmez.");
        eprintln!("-> curl -X POST <RPC>/submit -H 'Content-Type: application/json' -d '{{\"hex\":\"<yukaridaki>\"}}'");
        Ok(())
    };
    if let Err(e) = calis() {
        eprintln!("HATA: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const H: &str = "0dcce43d9a705bcd6f3b3a8a1b2c3d4e5f60718293a4b5c6d7e8f90112233445";
    fn sk(b: u8) -> SigningKey { SigningKey::from_bytes(&[b; 32]) }
    fn talep() -> Value {
        json!({ "tur": TALEP_TUR, "surum": 1, "network_id": 3474, "belge_hash": H,
                "payload_hex": format!("01{H}"), "parents": [hex::encode([2u8; 32])], "ts": 1000,
                "imzalayan": null, "zincir": {"kayitli": false}, "durum": "imzalayan-bekleniyor" })
    }

    #[test]
    fn gecerli_talep_imzalanir_ve_dogrulanir() {
        let v = talepten_vertex(&talep(), &sk(1), None).unwrap();
        v.verify().unwrap();
        assert_eq!(v.payload(), hex::decode(format!("01{H}")).unwrap());
        assert_eq!(v.timestamp(), 1000);
        let v2 = talepten_vertex(&talep(), &sk(1), Some(2000)).unwrap();
        assert_eq!(v2.timestamp(), 2000);
        assert_eq!(wire::decode(&wire::encode(&v2)).unwrap(), v2);
    }

    #[test]
    fn degistirilmis_payload_reddedilir() {
        let mut t = talep();
        // talep baska bir islem imzalatmaya calisiyor (ör. tip=4 transfer)
        t["payload_hex"] = json!(format!("04{H}"));
        assert!(talepten_vertex(&t, &sk(1), None).unwrap_err().contains("uyusmuyor"));
    }

    #[test]
    fn baska_imzalayan_icin_talep_reddedilir() {
        let mut t = talep();
        t["imzalayan"] = json!({"pubkey": hex::encode(sk(2).verifying_key().to_bytes())});
        assert!(talepten_vertex(&t, &sk(1), None).is_err());
        assert!(talepten_vertex(&t, &sk(2), None).is_ok());
    }

    #[test]
    fn kayitli_veya_bozuk_talep_reddedilir() {
        let mut t = talep(); t["durum"] = json!("zaten-kayitli");
        assert!(talepten_vertex(&t, &sk(1), None).is_err());
        let mut t = talep(); t["tur"] = json!("baska");
        assert!(talepten_vertex(&t, &sk(1), None).is_err());
        let mut t = talep(); t["belge_hash"] = json!(&H[..60]);
        assert!(talepten_vertex(&t, &sk(1), None).is_err());
        let mut t = talep(); t["network_id"] = json!(u64::MAX);
        assert!(talepten_vertex(&t, &sk(1), None).is_err());
    }
}
