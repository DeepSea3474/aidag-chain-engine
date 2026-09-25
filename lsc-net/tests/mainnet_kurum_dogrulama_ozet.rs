//! MAINNET KURUM DOGRULAMA REPLAY KONTROLU (yalniz rwa-oracle-kyc ve sonrasi).
//! Gercek mainnet veri dosyasinin KOPYASINI oynatir ve kurum dogrulama
//! eklemesinin mevcut kayitlari BOZMADIGINI dogrular:
//!  - tum vertex'ler yuklenir (reddedilen / yetim yok),
//!  - her tip=1 belge hash'i hala kayitli,
//!  - her tip=5 kurumu hala kayitli ve "dogrulanmamis" (mainnet'te yonetim
//!    kurulmamis + RWA kapali -> hic kimse dogrulanmis OLAMAZ),
//!  - pinli genesis id ayni.
//!
//!   MAINNET_REPLAY_DOSYA=/yol/kopya.log cargo test --release -p lsc-net \
//!     --test mainnet_kurum_dogrulama_ozet -- --ignored --nocapture

#[test]
#[ignore]
fn mainnet_kurum_dogrulama_mevcut_kayitlari_bozmaz() {
    let yol = std::env::var("MAINNET_REPLAY_DOSYA").expect("MAINNET_REPLAY_DOSYA gerekli");
    let ham = lsc_net::store::load_vertices(std::path::Path::new(&yol)).expect("dosya okunamadi");
    let mut st = lsc_engine::NodeState::new_mainnet();
    let mut vs: Vec<lsc_engine::Vertex> = ham
        .iter()
        .filter_map(|b| lsc_engine::dag::wire::decode(b).ok())
        .collect();
    assert_eq!(vs.len(), ham.len(), "her kayit decode edilmeli");
    let genesis = vs.iter().find(|v| v.parents().is_empty()).map(|v| *v.id());
    assert_eq!(genesis, Some(lsc_engine::mainnet::genesis_id()), "pinli genesis");

    let mut yuklu = std::collections::HashSet::new();
    loop {
        let once = yuklu.len();
        let mut kalan = Vec::new();
        for v in vs.drain(..) {
            if v.parents().iter().all(|p| yuklu.contains(p)) {
                let id = *v.id();
                v.verify().expect("imza gecerli");
                match st.ingest_decoded_preverified(v) {
                    lsc_engine::NetworkIngestOutcome::Integrated(_)
                    | lsc_engine::NetworkIngestOutcome::Duplicate(_) => {
                        yuklu.insert(id);
                    }
                    o => panic!("mevcut mainnet vertex'i reddedildi: {o:?}"),
                }
            } else {
                kalan.push(v);
            }
        }
        vs = kalan;
        if yuklu.len() == once || vs.is_empty() {
            break;
        }
    }
    assert!(vs.is_empty(), "yuklenemeyen vertex yok");
    assert_eq!(st.orphan_count(), 0);

    let (mut belge, mut kurum) = (0usize, 0usize);
    for b in &ham {
        let v = lsc_engine::dag::wire::decode(b).unwrap();
        match v.payload().first() {
            Some(&lsc_engine::TX_TYPE_RECORD) => {
                let r = lsc_engine::Record::decode(v.payload()).expect("record");
                assert!(st.belge_dogrula(&r.data_hash).is_some(), "belge kayitli kalmali");
                belge += 1;
            }
            Some(&lsc_engine::tx::TX_TYPE_KURUM) => {
                let a = lsc_engine::public_key_to_adres(v.public_key());
                assert!(st.kurum_sorgula(&a).is_some(), "kurum kayitli kalmali");
                assert!(!st.kurum_dogrulanmis_mi(&a), "mainnet'te dogrulanmis kurum OLAMAZ");
                kurum += 1;
            }
            _ => {}
        }
    }
    assert!(st.rwa_yonetim().is_none(), "mainnet yonetimi kurulmamis");
    println!(
        "DOGRULAMA-OZET vertex={} belge_vertex={} kurum_vertex={} belge_kayit={} kurum_kayit={} dogrulanmis=0",
        st.vertex_count(), belge, kurum, st.belge_sayisi(), st.kurum_sayisi()
    );
}
