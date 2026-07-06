//! POC 3: secp256k1 imzali ISLEM dogrulama (islem/payload katmani).
//! IZOLE - vertex/DAG/node/konsensus koduna DOKUNMAZ.
//! Kanit: MetaMask tarzi bir kullanici, bir islemi secp256k1 ile imzalar;
//! sistem islemi dogrular ve sahibinin 0x adresini cikarir.

use k256::ecdsa::{
    signature::{Signer, Verifier},
    Signature, SigningKey, VerifyingKey,
};
#[allow(unused_imports)]
use sha3::{Digest, Keccak256};

/// secp256k1 pubkey -> 20 baytlik 0x adres (Ethereum yontemi).
fn eth_adres(vk: &VerifyingKey) -> [u8; 20] {
    let nokta = vk.to_encoded_point(false);
    let hash = Keccak256::digest(&nokta.as_bytes()[1..]);
    let mut a = [0u8; 20];
    a.copy_from_slice(&hash[12..]);
    a
}

/// Bir islem mesaji uret (ornek: "tip=4 transfer, alici, miktar").
/// Gercekte bu, islemin kanonik kodlamasi olur.
fn islem_mesaji(tip: u8, alici: &[u8; 20], miktar: u64) -> Vec<u8> {
    let mut m = Vec::new();
    m.push(tip);
    m.extend_from_slice(alici);
    m.extend_from_slice(&miktar.to_be_bytes());
    m
}

/// Islemi dogrula: imza gecerliyse, gonderenin 0x adresini dondur.
fn islem_dogrula(mesaj: &[u8], imza: &Signature, vk: &VerifyingKey) -> Option<[u8; 20]> {
    if vk.verify(mesaj, imza).is_ok() {
        Some(eth_adres(vk))
    } else {
        None
    }
}

#[test]
fn secp256k1_imzali_islem_bastan_sona_dogrulanir() {
    // 1) MetaMask tarzi kullanici: secp256k1 anahtar
    let kullanici_sk = SigningKey::random(&mut rand::rngs::OsRng);
    let kullanici_vk = VerifyingKey::from(&kullanici_sk);
    let gonderen_adres = eth_adres(&kullanici_vk);

    // 2) Bir transfer islemi olustur (tip=4, bir aliciya 1000 birim)
    let alici = [0x11u8; 20];
    let mesaj = islem_mesaji(4, &alici, 1000);

    // 3) Kullanici islemi secp256k1 ile IMZALAR
    let imza: Signature = kullanici_sk.sign(&mesaj);

    // 4) SISTEM islemi dogrular -> gonderenin adresini cikarir
    let cikan = islem_dogrula(&mesaj, &imza, &kullanici_vk);
    assert!(cikan.is_some(), "gecerli islem dogrulanmali");
    assert_eq!(
        cikan.unwrap(),
        gonderen_adres,
        "cikan adres, gonderenin 0x adresi olmali"
    );
    println!(
        "islem dogrulandi, gonderen: 0x{}",
        hex::encode(gonderen_adres)
    );
}

#[test]
fn tahrif_edilen_islem_reddedilir() {
    let sk = SigningKey::random(&mut rand::rngs::OsRng);
    let vk = VerifyingKey::from(&sk);
    let alici = [0x22u8; 20];
    let mesaj = islem_mesaji(4, &alici, 1000);
    let imza: Signature = sk.sign(&mesaj);

    // Mesaj tahrif edilir: miktar 1000 -> 9999
    let tahrif = islem_mesaji(4, &alici, 9999);
    let cikan = islem_dogrula(&tahrif, &imza, &vk);
    assert!(cikan.is_none(), "tahrif edilen islem REDDEDILMELI");
}
