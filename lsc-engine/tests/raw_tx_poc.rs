//! POC 5: raw Ethereum tx cozme (alloy-consensus). eth_sendRawTransaction temeli.
//! IZOLE - calisan sisteme dokunmaz. Kanit: MetaMask formatindaki bir raw tx'i
//! cozup gonderen/to/value/nonce cikarabiliyor muyuz?

use alloy_consensus::TxEnvelope;
// hex: kendi hex crate'imiz (dev-dependency)

#[test]
fn raw_ethereum_tx_cozulur() {
    // Bilinen bir imzali Ethereum LEGACY raw tx (EIP-155, chainId=1).
    // Bu, MetaMask'in eth_sendRawTransaction ile gonderdigi formattir.
    // Kaynak: yaygin Ethereum test vektoru.
    let raw_hex = "f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83";
    let raw = hex::decode(raw_hex).expect("hex");

    // alloy-consensus ile cöz (RLP decode + tx parse + imza)
    use alloy_eips::eip2718::Decodable2718;
    use alloy_consensus::transaction::SignerRecoverable;
    let tx = TxEnvelope::decode_2718(&mut raw.as_slice()).expect("tx cozulmeli");

    // Gondereni imzadan kurtar (ecrecover, alloy icinde)
    let gonderen = tx.recover_signer().expect("gonderen kurtarilmali");
    println!("raw tx cozuldu, gonderen: {gonderen}");

    // to ve value'yu cikar (tx tipine gore)
    use alloy_consensus::Transaction;
    let to = tx.to();
    let value = tx.value();
    let nonce = tx.nonce();
    println!("to: {to:?}, value: {value}, nonce: {nonce}");

    // Bilinen vektorde nonce=9
    assert_eq!(nonce, 9, "nonce 9 olmali (bilinen vektor)");
}
