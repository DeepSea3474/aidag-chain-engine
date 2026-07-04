//! POC: AIDAG secp256k1 (Ethereum/MetaMask) imza dogrulayabilir mi?
//! Bu test IZOLE - vertex/DAG/konsensus koduna DOKUNMAZ.

use k256::ecdsa::{SigningKey, VerifyingKey, signature::{Signer, Verifier}, Signature};

#[test]
fn secp256k1_imza_uret_ve_dogrula() {
    let sk = SigningKey::random(&mut rand::rngs::OsRng);
    let vk = VerifyingKey::from(&sk);
    let mesaj = b"AIDAG-Chain secp256k1 uyumluluk testi";
    let imza: Signature = sk.sign(mesaj);
    assert!(vk.verify(mesaj, &imza).is_ok(), "gecerli imza dogrulanmali");
    let bozuk = b"AIDAG-Chain secp256k1 uyumluluk testX";
    assert!(vk.verify(bozuk, &imza).is_err(), "bozuk mesaj reddedilmeli");
    println!("secp256k1 dogrulama CALISTI");
}

#[test]
fn secp256k1_yanlis_anahtar_reddedilir() {
    let sk1 = SigningKey::random(&mut rand::rngs::OsRng);
    let sk2 = SigningKey::random(&mut rand::rngs::OsRng);
    let vk2 = VerifyingKey::from(&sk2);
    let mesaj = b"test mesaji";
    let imza: Signature = sk1.sign(mesaj);
    assert!(vk2.verify(mesaj, &imza).is_err(), "yanlis anahtar reddedilmeli");
}
