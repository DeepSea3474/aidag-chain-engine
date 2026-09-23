//! MAINNET REPLAY OZETI (konsensus kurali degisikligi oncesi/sonrasi karsilastirma).
//! Gercek bir mainnet veri dosyasini, canli dugumun acilistaki yukleme yoluyla
//! (decode + verify + ingest_decoded_preverified, parent-once) yeniden oynatir ve
//! turetilmis durumun deterministik bir ozetini basar. Iki binary (eski/yeni kod)
//! ayni dosyada AYNI ozeti uretiyorsa, kural degisikligi gecmis durumu DEGISTIRMEZ.
//!
//! Calistirma (dosyanin KOPYASI ile; canli dosyaya dokunmaz):
//!   MAINNET_REPLAY_DOSYA=/yol/kopya.log cargo test --release -p lsc-net \
//!     --test mainnet_replay_ozet -- --ignored --nocapture

use std::collections::{BTreeSet, HashSet};

#[test]
#[ignore]
fn mainnet_replay_ozet() {
    let yol = std::env::var("MAINNET_REPLAY_DOSYA").expect("MAINNET_REPLAY_DOSYA gerekli");
    let ham = lsc_net::store::load_vertices(std::path::Path::new(&yol)).expect("dosya okunamadi");
    let mut st = lsc_engine::NodeState::new_mainnet();

    // Canli acilis yolunun aynisi: decode + imza dogrula, parent-once ingest.
    let mut bekleyen: Vec<lsc_engine::Vertex> = ham
        .iter()
        .filter_map(|b| {
            let v = lsc_engine::dag::wire::decode(b).ok()?;
            v.verify().ok()?;
            Some(v)
        })
        .collect();
    let mut yuklu: HashSet<[u8; 32]> = HashSet::new();
    let mut imzalayanlar: BTreeSet<[u8; 20]> = BTreeSet::new();
    loop {
        let once = yuklu.len();
        let mut kalan = Vec::new();
        for v in bekleyen.drain(..) {
            if v.parents().iter().all(|p| yuklu.contains(p)) {
                let id = *v.id();
                imzalayanlar.insert(lsc_engine::public_key_to_adres(v.public_key()));
                match st.ingest_decoded_preverified(v) {
                    lsc_engine::NetworkIngestOutcome::Integrated(_)
                    | lsc_engine::NetworkIngestOutcome::Duplicate(_) => {
                        yuklu.insert(id);
                    }
                    _ => {}
                }
            } else {
                kalan.push(v);
            }
        }
        bekleyen = kalan;
        if yuklu.len() == once || bekleyen.is_empty() {
            break;
        }
    }

    // Kontrol edilecek adresler: dagitim dilimleri + kurucu + tum imzalayanlar + tum alicilar.
    let mut adresler: BTreeSet<[u8; 20]> = imzalayanlar;
    adresler.extend(lsc_engine::mainnet::dagitim_adresleri());
    adresler.insert(lsc_engine::mainnet::kurucu_adres());
    let satislar = st.on_satis_liste();
    for (_, k) in &satislar {
        adresler.insert(k.alici);
    }

    println!("OZET vertex={} yuklenemeyen={} orphan={}", st.vertex_count(), bekleyen.len(), st.orphan_count());
    println!("OZET tge={} on_satis_sayisi={} on_satis_toplam={}", st.on_satis_tge(), st.on_satis_sayisi(), st.on_satis_toplam_aidag());
    for (r, k) in &satislar {
        println!("OZET satis ref={r} {k:?}");
    }
    println!(
        "OZET belge={} kurum={} token={} staker={} stake={} aidag_arz={} aidag_hesap={} lsc_arz={} lsc_hesap={}",
        st.belge_sayisi(), st.kurum_sayisi(), st.token_sayisi(), st.staker_sayisi(), st.toplam_stake(),
        st.toplam_bakiye_arzi(), st.bakiye_hesap_sayisi(), st.lsc_toplam_arzi(), st.lsc_hesap_sayisi()
    );
    for a in &adresler {
        println!(
            "OZET adres=0x{} aidag={} lsc={} nonce={}",
            hex::encode(a), st.bakiye(a), st.lsc_bakiye(a), st.beklenen_nonce(a)
        );
    }
    // BELGE / KURUM KAYITLARI (tek tek): dosyadaki her tip=1 hash'i ve her tip=5
    // imzalayani icin turetilmis kayit. Yalniz main'de de var olan API kullanilir
    // -> eski ve yeni kodla AYNI dosyada birebir karsilastirilabilir.
    let mut belge_hashleri: BTreeSet<[u8; 32]> = BTreeSet::new();
    let mut kurum_adresleri: BTreeSet<[u8; 20]> = BTreeSet::new();
    for b in &ham {
        let Ok(v) = lsc_engine::dag::wire::decode(b) else { continue };
        match v.payload().first() {
            Some(&lsc_engine::TX_TYPE_RECORD) => {
                if let Ok(r) = lsc_engine::Record::decode(v.payload()) {
                    belge_hashleri.insert(r.data_hash);
                }
            }
            Some(&5) => {
                kurum_adresleri.insert(lsc_engine::public_key_to_adres(v.public_key()));
            }
            _ => {}
        }
    }
    for h in &belge_hashleri {
        println!("OZET belge hash={} {:?}", hex::encode(h), st.belge_dogrula(h));
    }
    for a in &kurum_adresleri {
        println!("OZET kurum adres=0x{} {:?}", hex::encode(a), st.kurum_sorgula(a));
    }
    // AVM: deploy edilmis kontratlar ve bakiyeleri (precompile/AVM yolu degisikliklerinde).
    let mut kontratlar = st.avm_kontrat_adresleri();
    kontratlar.sort();
    println!("OZET avm_kontrat_sayisi={}", kontratlar.len());
    for a in &kontratlar {
        println!(
            "OZET avm_kontrat=0x{} aidag={} lsc={}",
            hex::encode(a), st.bakiye(a), st.lsc_bakiye(a)
        );
    }
}
