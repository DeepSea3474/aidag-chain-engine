//! POC 2: secp256k1 public key -> Ethereum tarzi 0x adres (keccak256).
//! IZOLE - vertex/DAG/konsensus koduna DOKUNMAZ.
//! Kanit: bizim turetme, Ethereum'un urettigi adresle BIREBIR AYNI mi?

use k256::ecdsa::{SigningKey, VerifyingKey};
use k256::elliptic_curve::sec1::ToEncodedPoint as _;
use sha3::{Digest, Keccak256};

/// secp256k1 public key -> 20 baytlik Ethereum adresi.
/// Ethereum yontemi: keccak256(uncompressed_pubkey[1..])[12..]
fn eth_adres(vk: &VerifyingKey) -> [u8; 20] {
    let nokta = vk.to_encoded_point(false); // sikistirilmamis: 0x04 + X(32) + Y(32) = 65 bayt
    let bytes = nokta.as_bytes();
    // ilk bayt (0x04) atilir, kalan 64 bayt hashlenir
    let hash = Keccak256::digest(&bytes[1..]);
    let mut adres = [0u8; 20];
    adres.copy_from_slice(&hash[12..]); // son 20 bayt
    adres
}

#[test]
fn secp256k1_ethereum_adres_turetme_calisir() {
    let sk = SigningKey::random(&mut rand::rngs::OsRng);
    let vk = VerifyingKey::from(&sk);
    let adres = eth_adres(&vk);
    // 20 bayt uretildi mi
    assert_eq!(adres.len(), 20, "adres 20 bayt olmali (Ethereum ile ayni)");
    println!("uretilen adres: 0x{}", hex::encode(adres));
}

#[test]
fn bilinen_test_vektoru_ethereum_ile_ayni() {
    // Bilinen Ethereum test vektoru:
    // private key = 0x4646...4646 (32 bayt, hepsi 0x46)
    // Bu anahtarin Ethereum adresi bilinir ve sabittir.
    let sk_bytes = [0x46u8; 32];
    let sk = SigningKey::from_bytes((&sk_bytes).into()).expect("gecerli anahtar");
    let vk = VerifyingKey::from(&sk);
    let adres = eth_adres(&vk);
    let uretilen = hex::encode(adres);
    println!("0x46*32 -> 0x{}", uretilen);
    // Bu anahtarin gercek Ethereum adresi:
    let beklenen = "9d8a62f656a8d1615c1294fd71e9cfb3e4855a4f";
    assert_eq!(uretilen, beklenen, "Ethereum ile BIREBIR ayni olmali");
}
