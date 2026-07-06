//! POC 4: ecrecover - secp256k1 imzadan public key kurtarma (Ethereum yontemi).
//! IZOLE - vertex/DAG/node/konsensus koduna DOKUNMAZ.
//! Kanit: SADECE imza + mesajdan (pubkey TASIMADAN) gonderenin 0x adresi kurtarilir.
//! Bu, Secenek B'nin temeli: payload'da pubkey tasimaya gerek yok.

use k256::ecdsa::{RecoveryId, Signature, SigningKey, VerifyingKey};
#[allow(unused_imports)]
use sha3::{Digest, Keccak256};

fn eth_adres(vk: &VerifyingKey) -> [u8; 20] {
    let nokta = vk.to_encoded_point(false);
    let hash = Keccak256::digest(&nokta.as_bytes()[1..]);
    let mut a = [0u8; 20];
    a.copy_from_slice(&hash[12..]);
    a
}

#[test]
fn ecrecover_imzadan_adres_kurtarma() {
    // 1) Kullanici anahtari + GERCEK adresi
    let sk = SigningKey::random(&mut rand::rngs::OsRng);
    let vk_gercek = VerifyingKey::from(&sk);
    let adres_gercek = eth_adres(&vk_gercek);

    // 2) Bir mesaj imzala - RECOVERABLE imza (imza + recovery id)
    let mesaj = b"AIDAG transfer: alici=0x11.. miktar=1000";
    let (imza, recid): (Signature, RecoveryId) = sk.sign_recoverable(mesaj).expect("imza");

    // 3) ECRECOVER: SADECE mesaj + imza + recid'den public key'i kurtar
    //    (vk_gercek'i KULLANMIYORUZ - sadece imzadan turetilecek)
    let vk_kurtarilan =
        VerifyingKey::recover_from_msg(mesaj, &imza, recid).expect("public key kurtarilmali");
    let adres_kurtarilan = eth_adres(&vk_kurtarilan);

    // 4) Kurtarilan adres, gercek adresle AYNI olmali
    assert_eq!(
        adres_kurtarilan, adres_gercek,
        "ecrecover ile kurtarilan adres, gonderenin gercek adresi olmali"
    );
    println!(
        "ecrecover CALISTI - kurtarilan adres: 0x{}",
        hex::encode(adres_kurtarilan)
    );
}

#[test]
fn ecrecover_yanlis_mesajda_farkli_adres_verir() {
    let sk = SigningKey::random(&mut rand::rngs::OsRng);
    let adres_gercek = eth_adres(&VerifyingKey::from(&sk));

    let mesaj = b"orijinal mesaj";
    let (imza, recid): (Signature, RecoveryId) = sk.sign_recoverable(mesaj).expect("imza");

    // Farkli mesajla recover -> ya hata ya FARKLI adres (gonderen dogrulanamaz)
    let tahrif = b"tahrif edilmis mesaj";
    match VerifyingKey::recover_from_msg(tahrif, &imza, recid) {
        Ok(vk) => {
            let adres = eth_adres(&vk);
            assert_ne!(
                adres, adres_gercek,
                "tahrif edilen mesaj ayni adresi vermemeli"
            );
            println!("tahrif -> farkli adres (gonderen dogrulanamaz), guvenli");
        }
        Err(_) => {
            println!("tahrif -> recover hatasi, guvenli");
        }
    }
}
