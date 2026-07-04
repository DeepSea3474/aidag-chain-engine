//! LSC ag katmani P2P - Asama 2: gossipsub ile vertex yayini.
//! Peer identity + TCP/Noise/Yamux transport + (gossipsub + ping) behaviour.
//!
//! Gossipsub: vertex'ler "lsc-vertices" topic'ine yayinlanir; abone dugumler
//! alir. Bu, DAG aginin dogal yayilim modelidir.
//!
//! ASAMA NOTU: gelen gossipsub mesajini NodeState::ingest'e besleme Parca 3'te.
//! Su an: altyapi + abonelik + gelen mesaji loglama.

pub mod rpc;
pub mod store;

use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use libp2p::futures::StreamExt;
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{
    gossipsub, mdns, noise, ping, request_response, tcp, yamux, Multiaddr, PeerId, SwarmBuilder,
};

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use rand::RngCore;

/// LSC birlesik ag davranisi: vertex yayini (gossipsub) + canlilik (ping).
/// Pull-sync protokolu mesajlari (request-response — gossipsub'dan AYRI kanal,
/// seen-cache YOK, "Duplicate" sorunu YOK). Gec katilan node, bagli oldugu
/// peer'dan gecmis vertex'leri ISTER; peer dogrudan cevap gonderir.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SyncRequest {
    /// CHUNKED SYNC: topolojik sirali listede bu indexten itibaren iste.
    /// 0 = bastan. Alici her parcayi aldikca offset'i ilerletir.
    offset: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SyncResponse {
    /// Wire-encoded vertex baytlari (her biri wire::encode ciktisi) — bu PARCA.
    vertices: Vec<Vec<u8>>,
    /// Bu parcanin basladigi offset (istekle ayni; teyit/sira icin).
    offset: u64,
    /// Gonderenin toplam vertex sayisi (alici "daha var mi" anlamasi icin).
    total: u64,
}

#[derive(NetworkBehaviour)]
struct LscBehaviour {
    gossipsub: gossipsub::Behaviour,
    ping: ping::Behaviour,
    /// Pull-sync: gecmis vertex'leri talep/cevap (request-response, CBOR codec).
    sync: request_response::cbor::Behaviour<SyncRequest, SyncResponse>,
    /// Otomatik peer kesfi (yerel ag, mDNS). Manuel IP girmeden node'lar
    /// birbirini bulur. NOT: sadece yerel ag (LAN); internet-olcegi kesif
    /// (bootstrap/Kademlia) ileride.
    mdns: mdns::tokio::Behaviour,
}

/// Vertex'lerin yayinlandigi gossipsub topic adi.
const VERTEX_TOPIC: &str = "lsc-vertices";

/// Anahtar dosyasi algoritma kimligi: ed25519 (bugun).
/// KRIPTO-CEVIKLIK: dosya formati [algo_id: 1 bayt][seed: N bayt]. Ileride
/// post-quantum imza (or. ML-DSA/Dilithium) olgunlasinca, YENI bir algo_id
/// (=2) dali eklenir; mevcut format ve yukleme mantigi bozulmadan genisler.
/// Bugun PQC KODU YOK (olgunlasmamis + imzalar cok buyuk); sadece KAPI hazir.
/// Bu, anahtar KIMLIGI seviyesinde ceviklik; vertex imza yolunun (wire) tam
/// cevikligi ayri/ileri bir is.
const KEY_ALGO_ED25519: u8 = 1;

/// Dugum imzalama anahtarini diskten yukle; yoksa uret + kaydet (kalici kimlik).
/// `path` None ise: kalici DEGIL — her cagride yeni rastgele anahtar (gecici).
/// Format: [algo_id][32 bayt ed25519 seed]. Bilinmeyen algo_id -> hata.
fn load_or_create_signing_key(
    path: Option<&std::path::Path>,
) -> Result<SigningKey, Box<dyn Error>> {
    if let Some(p) = path {
        match store::load_bytes(p)? {
            Some(data) => {
                // Mevcut anahtar: algo_id + seed.
                if data.is_empty() {
                    return Err("anahtar dosyasi bos/bozuk".into());
                }
                let algo = data[0];
                match algo {
                    KEY_ALGO_ED25519 => {
                        if data.len() != 33 {
                            return Err(format!(
                                "ed25519 anahtar dosyasi gecersiz uzunluk: {} (33 bekleniyor)",
                                data.len()
                            )
                            .into());
                        }
                        let mut seed = [0u8; 32];
                        seed.copy_from_slice(&data[1..33]);
                        tracing::info!("Kalici kimlik diskten yuklendi (ed25519): {p:?}");
                        Ok(SigningKey::from_bytes(&seed))
                    }
                    other => Err(format!(
                        "desteklenmeyen anahtar algoritmasi: algo_id={other} (bu surum sadece ed25519=1 destekler)"
                    )
                    .into()),
                }
            }
            None => {
                // Yeni kimlik uret + kaydet.
                let mut seed = [0u8; 32];
                OsRng.fill_bytes(&mut seed);
                let mut file_bytes = Vec::with_capacity(33);
                file_bytes.push(KEY_ALGO_ED25519);
                file_bytes.extend_from_slice(&seed);
                store::save_bytes(p, &file_bytes)?;
                tracing::info!("Yeni kalici kimlik uretildi + kaydedildi (ed25519): {p:?}");
                Ok(SigningKey::from_bytes(&seed))
            }
        }
    } else {
        // Kalici degil: gecici rastgele anahtar.
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        tracing::info!("Gecici (kalici olmayan) kimlik uretildi.");
        Ok(SigningKey::from_bytes(&seed))
    }
}

pub async fn run_node(
    listen_addr: &str,
    dial_addr: Option<&str>,
    produce_genesis: bool,
    produce_interval: Option<Duration>,
    data_file: Option<PathBuf>,
    key_file: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let mut swarm = SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|key| {
            // Gossipsub: mesajlar yazarin anahtariyla imzali (Signed authenticity).
            // message_id_fn: mesaj kimligi = ICERIGIN blake3 hash'i (icerik-adresli).
            // Ayni vertex farkli peer'lardan gelse bile AYNI id -> gossipsub eler.
            let message_id_fn = |message: &gossipsub::Message| {
                let hash = blake3::hash(&message.data);
                gossipsub::MessageId::from(hash.as_bytes().to_vec())
            };
            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(10))
                .validation_mode(gossipsub::ValidationMode::Strict)
                .message_id_fn(message_id_fn)
                .build()
                .map_err(|e| std::io::Error::other(e.to_string()))?;

            let gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )?;

            let ping = ping::Behaviour::default();

            // Pull-sync behaviour: request-response + CBOR codec.
            // Protokol: /lsc/sync/1, Full (hem istek gonder hem cevap ver).
            let sync = request_response::cbor::Behaviour::<SyncRequest, SyncResponse>::new(
                [(
                    libp2p::StreamProtocol::new("/lsc/sync/1"),
                    request_response::ProtocolSupport::Full,
                )],
                request_response::Config::default(),
            );

            // Otomatik peer kesfi (mDNS, yerel ag). Manuel IP girmeden node'lar
            // birbirini bulur; kesfedilen peer'a otomatik dial edilir (event
            // kolunda). NOT: sadece yerel ag (LAN); internet-olcegi kesif ileride.
            let mdns =
                mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())?;

            Ok(LscBehaviour {
                gossipsub,
                ping,
                sync,
                mdns,
            })
        })?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    // DAG motorunu düğüme bağla: NodeState (graph + ghostdag).
    // Devnet: graph boş başlar, ilk parent'sız vertex genesis olur.
    // NodeState artik Arc<RwLock<...>> ile paylasilabilir: ag event loop'u VE
    // (B2'de) periyodik uretici task ayni duruma guvenli erisir.
    // Deadlock kurali: ayni anda TEK kilit tut, isini yap, HEMEN birak.
    let node_state = Arc::new(RwLock::new(lsc_engine::NodeState::new_devnet(1)));

    // TESTNET FAUCET: LSC_FAUCET_OWNER env'i (owner adresi, 40 hex) ayarliysa,
    // faucet'i ac (sadece bu adres test AIDAG basabilir). Ayarli degilse faucet
    // KAPALI kalir (mainnet guvenligi). Test AIDAG'in gercek degeri yoktur.
    if let Ok(owner_hex) = std::env::var("LSC_FAUCET_OWNER") {
        match hex::decode(owner_hex.trim()) {
            Ok(b) if b.len() == 20 => {
                let mut owner = [0u8; 20];
                owner.copy_from_slice(&b);
                node_state.write().await.faucet_owner_ayarla(owner);
                tracing::info!("Faucet owner ayarlandi: {}", owner_hex.trim());
            }
            _ => tracing::warn!("LSC_FAUCET_OWNER gecersiz (40 hex olmali): {owner_hex}"),
        }
    }

    // RPC -> ag dongusu kanali: /submit ile gelen+ingest edilen vertex'i AGA yayinlamak icin.
    // RPC kendi dugumune ingest eder (kilit altinda); baytlari bu kanaldan ag dongusune
    // yollar, ag dongusu gossipsub ile publish eder (swarm sadece bu task'ta erisilebilir).
    let (submit_tx, mut submit_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    // FAUCET ZINCIR: signing_key RPC'den ONCE yuklenir ki faucet vertex'i imzalanabilsin.
    let rpc_signing_key = load_or_create_signing_key(key_file.as_deref())?;

    // RPC sunucusu: zincir durumunu HTTP/JSON ile disariya acar (okuma).
    // Ayri task; ag event loop'unu bloklamaz. Adres: LSC_RPC_ADDR ya da 0.0.0.0:8645.
    {
        let rpc_node = node_state.clone();
        let rpc_tx = submit_tx.clone();
        let rpc_key = rpc_signing_key.clone();
        let rpc_addr = std::env::var("LSC_RPC_ADDR").unwrap_or_else(|_| "0.0.0.0:8645".to_string());
        tokio::spawn(async move {
            if let Err(e) = rpc::serve(rpc_addr, rpc_node, rpc_tx, rpc_key).await {
                tracing::warn!("RPC sunucusu durdu: {e}");
            }
        });
    }
    {
        let st = node_state.read().await;
        tracing::info!(
            "DAG state kuruldu: network_id={}, vertex_sayisi={}, genesis={:?}",
            st.network_id(),
            st.vertex_count(),
            st.genesis_id()
        );
    } // kilit burada birakilir

    // Vertex topic'ine abone ol.
    let topic = gossipsub::IdentTopic::new(VERTEX_TOPIC);
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;
    tracing::info!("Gossipsub topic'ine abone olundu: {VERTEX_TOPIC}");

    // Vertex imzalama anahtari: key_file varsa KALICI kimlik (diskten yukle/
    // uret+kaydet), yoksa gecici. Kripto-cevik format (algo_id + seed).
    let signing_key = rpc_signing_key.clone();

    // FAUCET/ON SATIS OWNER: Eger LSC_FAUCET_OWNER env'i verilmISSE (yukarida
    // ayarlandi), owner ODUR (senin kontrolundeki anahtar; on satis dagitimini
    // SEN disaridan imzalarsin -> node otomatik dagitmaz = guvenli). Env YOKSA,
    // owner = node kendi adresi (testnet kolayligi: RPC GET /faucet calissin).
    // Owner adresini belirle (env ya da node adresi).
    let owner_adres_final: [u8; 20] = if let Ok(owner_hex) = std::env::var("LSC_FAUCET_OWNER") {
        let mut a = [0u8; 20];
        if let Ok(b) = hex::decode(owner_hex.trim()) {
            if b.len() == 20 { a.copy_from_slice(&b); }
        }
        tracing::info!("Faucet/on-satis owner = LSC_FAUCET_OWNER (env, disaridan imzali dagitim)");
        a
    } else {
        let owner_adres = lsc_engine::public_key_to_adres(&signing_key.verifying_key().to_bytes());
        node_state.write().await.faucet_owner_ayarla(owner_adres);
        tracing::info!("Faucet owner = node adresi (env yok, zincir faucet): {owner_adres:?}");
        owner_adres
    };

    // GENESIS HAZINE: LSC_GENESIS_HAZINE env'i ayarliysa, owner'a (hazine) o kadar
    // baslangic AIDAG'i ver. RPC ucu DEGIL -> disaridan tetiklenemez (guvenli).
    // Deger 18-ondalik ham birim (ornek: 1000 AIDAG = "1000000000000000000000").
    // NOT: Bu gecici bir kurulum; gercek mainnet genesis'i pinli/vesting'li olacak.
    if let Ok(hazine_str) = std::env::var("LSC_GENESIS_HAZINE") {
        if let Ok(miktar) = hazine_str.trim().parse::<u128>() {
            node_state.write().await.test_bakiye_ekle(owner_adres_final, miktar);
            tracing::warn!("GENESIS HAZINE: owner'a {miktar} birim AIDAG yuklendi (env LSC_GENESIS_HAZINE).");
        } else {
            tracing::warn!("LSC_GENESIS_HAZINE gecersiz (u128 olmali): {hazine_str}");
        }
    }

    // Bir genesis vertex (parent'siz) uret, imzala, wire ile encode et,
    // kendi DAG state'ine ingest et (vertex 0 -> 1). Yayin 4b'de.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // KALICILIK: diskte kayitli vertex varsa, genesis uretiminden ONCE yukle.
    // Yukleme orphan-bilincli (ingest_networked); dosya sirasi garantisiz olsa
    // da cascade dogru kurar. Acilista rakip task yok -> tek kilit guvenli.
    if let Some(ref path) = data_file {
        match store::load_vertices(path) {
            Ok(vertices) => {
                let n = vertices.len();
                if n > 0 {
                    let mut st = node_state.write().await;
                    // REPLAY yukleme: ingest_synced (saat politikasi YOK — eski
                    // timestamp'ler reddedilmez). Vertex'ler SIRASIZ olabilir;
                    // YAKINSAYANA KADAR tekrar dene (genesis hangi sirada gelirse
                    // gelsin, cascade tum zinciri cozer).
                    // TOPOLOJIK SIRALI YUKLEME: vertex'leri parent-once sirala,
                    // boylece orphan havuzuna (MAX_ORPHANS=1024) HIC dusmezler.
                    // Her vertex, TUM parent'lari yuklendikten SONRA ingest edilir.
                    use std::collections::HashSet;
                    // (id, parents, vertex) - paralel fazda decode edilmis Vertex saklanir
                    // (tekrar decode YOK; INTEGRATE'in %73'u bu israfti).
                    type DecodedVertex = ([u8; 32], Vec<[u8; 32]>, lsc_engine::dag::vertex::Vertex);
                    // PARALEL VERIFY: her vertex'i decode + ed25519 imza dogrula
                    // (rayon, cok cekirdek). SADECE verify GECEN vertex decoded'a
                    // girer -> gecemeyen ASLA eklenmez (guvenlik korunur). Verify
                    // burada BIR KEZ yapilir; ingest_synced_preverified tekrar etmez.
                    use rayon::prelude::*;
                    let decoded: Vec<DecodedVertex> = vertices
                        .par_iter()
                        .filter_map(|bytes| {
                            let v = lsc_engine::dag::wire::decode(bytes).ok()?;
                            // KRITIK: imza+butunluk dogrula; gecmezse None -> elenir.
                            v.verify().ok()?;
                            let id = *v.id();
                            let parents: Vec<[u8; 32]> = v.parents().to_vec();
                            Some((id, parents, v))
                        })
                        .collect();
                    let mut loaded: HashSet<[u8; 32]> = HashSet::new();
                    let mut pending = decoded;
                    loop {
                        let before = loaded.len();
                        let mut still: Vec<DecodedVertex> = Vec::new();
                        for (id, parents, vertex) in pending.drain(..) {
                            // Parent'larin HEPSI yuklenmis mi? (parent yoksa = genesis, hazir)
                            let ready = parents.iter().all(|pp| loaded.contains(pp));
                            if ready {
                                // Vertex ZATEN decode+verify edildi (paralel faz) -> tekrar decode YOK.
                                match st.ingest_decoded_preverified(vertex) {
                                    lsc_engine::NetworkIngestOutcome::Integrated(_)
                                    | lsc_engine::NetworkIngestOutcome::Duplicate(_) => {
                                        loaded.insert(id);
                                    }
                                    _ => { /* ready olup integrate olmayan: dusur (nadir/bozuk) */ }
                                }
                            } else {
                                still.push((id, parents, vertex));
                            }
                        }
                        pending = still;
                        // Bu turda hic ilerleme olmadiysa dur (kalan = gercekten cozulemez).
                        if loaded.len() == before || pending.is_empty() {
                            break;
                        }
                    }
                    if !pending.is_empty() {
                        tracing::warn!(
                            "Yuklenemeyen {} vertex (parent zinciri kopuk?)",
                            pending.len()
                        );
                    }
                    tracing::info!(
                        "Diskten {n} vertex yuklendi: toplam_vertex={}, bekleyen_orphan={}",
                        st.vertex_count(),
                        st.orphan_count()
                    );
                } else {
                    tracing::info!("Disk dosyasi bos/yeni: {path:?}");
                }
            }
            Err(e) => tracing::warn!("Disk yukleme hatasi ({path:?}): {e}"),
        }
    }

    // Disk'te zaten vertex (ve genesis) varsa, YENIDEN genesis uretme.
    let has_genesis = { node_state.read().await.vertex_count() > 0 };

    // Genesis: produce_genesis ise VE henuz genesis yoksa uretilir + graf'a
    // ingest edilir + diske yazilir. Yayin AYRI degil: push sync (Subscribed
    // kolu) zaten export_vertices ile genesis dahil her seyi yayinlar.
    if produce_genesis && !has_genesis {
        match lsc_engine::Vertex::new_signed(1, vec![], b"lsc-genesis".to_vec(), now, &signing_key)
        {
            Ok(genesis) => {
                let bytes = lsc_engine::dag::wire::encode(&genesis);
                {
                    let mut st = node_state.write().await;
                    match st.ingest(&bytes, now) {
                        Ok(id) => tracing::info!(
                            "Genesis uretildi + ingest: id={}, vertex_sayisi={}",
                            hex::encode(&id[..8]),
                            st.vertex_count()
                        ),
                        Err(e) => tracing::warn!("Genesis ingest hatasi: {e}"),
                    }
                } // kilit birakilir
                  // KALICILIK: genesis'i de diske yaz (yoksa restart'ta cocuklar
                  // parent'siz kalir -> hepsi orphan). Kalicilik buguydu.
                if let Some(ref path) = data_file {
                    if let Err(e) = store::append_vertex(path, &bytes) {
                        tracing::warn!("Genesis diske yazilamadi: {e}");
                    }
                }
            }
            Err(e) => tracing::error!("Genesis vertex uretilemedi: {e:?}"),
        }
    } else {
        tracing::info!("Dinleyici modu: genesis uretilmiyor, agdan vertex bekleniyor.");
    }

    let local_peer_id = *swarm.local_peer_id();
    tracing::info!("Local peer id: {local_peer_id}");

    let listen: Multiaddr = listen_addr.parse()?;
    swarm.listen_on(listen)?;

    if let Some(addr) = dial_addr {
        match addr.parse::<Multiaddr>() {
            Ok(remote) => {
                if let Err(e) = swarm.dial(remote) {
                    tracing::error!("Dial failed for {addr}: {e}");
                } else {
                    tracing::info!("Dialing {addr}");
                }
            }
            Err(e) => {
                tracing::error!("Invalid dial address {addr}: {e}");
            }
        }
    }

    // TESTNET BOOTSTRAP: internet uzerinden dugum kesfi. mDNS sadece YEREL agda
    // calisir; testnet (farkli sunucular/sehirler) icin bilinen "bootstrap"
    // dugum adreslerine baglaniriz. LSC_BOOTSTRAP env'i virgulle ayrilmis
    // multiaddr listesi: "/ip4/1.2.3.4/tcp/40001,/ip4/5.6.7.8/tcp/40001".
    // Yeni dugum acilinca bu bilinen dugumlere dial eder -> aga katilir.
    if let Ok(bootstrap_list) = std::env::var("LSC_BOOTSTRAP") {
        for addr in bootstrap_list
            .split(',')
            .map(|a| a.trim())
            .filter(|a| !a.is_empty())
        {
            match addr.parse::<Multiaddr>() {
                Ok(remote) => {
                    if let Err(e) = swarm.dial(remote) {
                        tracing::warn!("Bootstrap dial basarisiz {addr}: {e}");
                    } else {
                        tracing::info!("Bootstrap dugumune baglaniliyor: {addr}");
                    }
                }
                Err(e) => tracing::warn!("Gecersiz bootstrap adresi {addr}: {e}"),
            }
        }
    }

    // B2: Periyodik uretici. produce_interval Some ise, her tick'te yeni bir
    // vertex uretilir (mevcut tips'leri parent alarak) ve yayinlanir.
    // interval HER ZAMAN kurulur; tick kolunda guard ile sadece uretici calisir.
    let tick_period = produce_interval.unwrap_or_else(|| Duration::from_secs(3600));
    let mut produce_tick = tokio::time::interval(tick_period);
    // Ilk tick hemen gelir; onu atla (baglanti kurulsun once).
    produce_tick.tick().await;
    let mut vertex_counter: u64 = 0;

    // GOZLEMLENEBILIRLIK (Adim 7): periyodik durum logu. Her node, belirli
    // araliklarla anlik durumunu loglar (vertex/peer/orphan). Gelistirme +
    // ileride operasyon icin: node sagligi/senkronu bir bakista gorunur.
    let mut status_tick = tokio::time::interval(Duration::from_secs(15));
    status_tick.tick().await; // ilk tick'i atla

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutdown signal received, stopping node.");
                break;
            }
            // B2: Periyodik vertex uretimi (sadece uretici dugumde).
            _ = produce_tick.tick(), if produce_interval.is_some() => {
                // IKINCI URETICI (produce modu, genesis URETMEYEN): agdaki ORTAK
                // genesis'i edinene kadar URETME. Yoksa bos grafta uretirsek
                // "ilk parent'siz vertex = genesis" kuraliyla KENDI genesis'imizi
                // yaratiriz -> iki genesis -> bolunmus ag (kanitlandi: 8 Haz).
                // Ana uretici (produce_genesis=true) bu kontrolu atlar; o genesis'i
                // bizzat uretir.
                if !produce_genesis {
                    let has_gen = {
                        let st = node_state.read().await;
                        st.genesis_id().is_some()
                    };
                    if !has_gen {
                        tracing::debug!(
                            "Ikinci uretici: ortak genesis henuz gelmedi, uretim bekliyor."
                        );
                        continue;
                    }
                }
                // 1) tips'leri oku (kilit AL -> kopyala -> BIRAK).
                let mut parents = {
                    let st = node_state.read().await;
                    st.tips()
                };
                parents.sort(); // canonical (artan) sira sart.

                // tips bossa (genesis yok) uretme — listener zaten buraya girmez,
                // ama guvenlik icin kontrol.
                if parents.is_empty() {
                    tracing::debug!("Uretim atlandi: tips bos (genesis yok).");
                } else {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    // Adim 5 (tam): ANLAMLI islem = Record (belge/veri hash kaydi).
                    // Bir "belge icerigi"nin GERCEK blake3 hash'ini (kriptografik
                    // parmak izi) Record'a koyariz. ONEMLI: icerik DEGIL, sadece
                    // hash zincire girer (gizlilik + boyut). "Kim" -> vertex imzasi,
                    // "ne zaman" -> vertex timestamp'i, "ne" -> bu hash.
                    // NOT: belge icerigi su an sentetik ("belge-N"); gercek dosya/
                    // girdi okuma, kullanim senaryosu netlesince eklenecek (yapi hazir).
                    // KALKAN TESTI: ilk uretimde (sayac=0) bir ornek TOKEN KAYDI
                    // (tip=2) vertex'i yayinla -> aga yayilir -> diger node'lar
                    // ingest edince kalkana_yonlendir ile kendi registry'lerine
                    // isler. Boylece DAGITIK kalkan canli kanitlanir. Sonraki
                    // tick'lerde normal Record (belge hash) uretimine devam.
                    // CANLI STAKING+KALKAN TESTI:
                    //  sayac=0 -> STAKE (tip=3): uretici kendi adresini stake eder
                    //             (signing_key'in public key'inden turetilen adres).
                    //  sayac=1 -> TOKEN (tip=2): USDC kaydi. Uretici STAKE'li oldugu
                    //             icin 9d kapisindan gecer, kabul edilir.
                    //  diger   -> Record (belge hash).
                    // Boylece dagitik staking+kalkan zinciri canli kanitlanir.
                    let payload = if vertex_counter == 0 {
                        let benim_adres = lsc_engine::public_key_to_adres(
                            &signing_key.verifying_key().to_bytes(),
                        );
                        tracing::info!(
                            "STAKE vertex'i uretiliyor: adres={} miktar=1000",
                            hex::encode(&benim_adres[..6])
                        );
                        lsc_engine::StakeKaydi::new(benim_adres, 1000).encode()
                    } else if vertex_counter == 1 {
                        let mut sym = [0u8; 8];
                        sym[..4].copy_from_slice(b"USDC");
                        tracing::info!(
                            "TOKEN KAYDI vertex'i uretiliyor (KALKAN): sembol=USDC adres=0xAA.."
                        );
                        lsc_engine::TokenKaydi::new([0xAA; 20], sym).encode()
                    } else if vertex_counter == 2 {
                        // CANLI SLASHING TESTI: ayni sembol (USDC) FARKLI adres (0xBB)
                        // = TAKLIT. Kalkan reddeder + kaydedenin (uretici) TUM stake'i
                        // YANAR. DURUM'da stake=1000(1 kisi) -> 0(0 kisi) gozlemlenir.
                        let mut sym = [0u8; 8];
                        sym[..4].copy_from_slice(b"USDC");
                        tracing::info!(
                            "TAKLIT TOKEN deneniyor (SLASHING bekleniyor): sembol=USDC adres=0xBB.."
                        );
                        lsc_engine::TokenKaydi::new([0xBB; 20], sym).encode()
                    } else {
                        // Adim 5 (tam): ANLAMLI islem = Record (belge/veri hash kaydi).
                        let document_content = format!("belge-{vertex_counter}").into_bytes();
                        let data_hash: [u8; 32] = blake3::hash(&document_content).into();
                        lsc_engine::Record::new(data_hash).encode()
                    };
                    match lsc_engine::Vertex::new_signed(1, parents, payload, now, &signing_key) {
                        Ok(v) => {
                            let bytes = lsc_engine::dag::wire::encode(&v);
                            // 2) ingest (kilit AL -> ingest -> BIRAK).
                            let ingested = {
                                let mut st = node_state.write().await;
                                st.ingest(&bytes, now)
                            };
                            match ingested {
                                Ok(id) => {
                                    vertex_counter += 1;
                                    tracing::info!(
                                        "Vertex URETILDI + ingest: id={}, sayac={}",
                                        hex::encode(&id[..8]),
                                        vertex_counter
                                    );
                                    // KALICILIK: uretilen vertex'i diske ekle (publish'ten ONCE,
                                    // cunku publish bytes'i tasiyor/move eder).
                                    if let Some(ref path) = data_file {
                                        if let Err(e) = store::append_vertex(path, &bytes) {
                                            tracing::warn!("Uretilen vertex diske yazilamadi: {e}");
                                        }
                                    }
                                    // 3) yayinla (kilit DISINDA).
                                    if let Err(e) = swarm
                                        .behaviour_mut()
                                        .gossipsub
                                        .publish(topic.clone(), bytes)
                                    {
                                        tracing::warn!("Uretilen vertex yayinlanamadi: {e:?}");
                                    }
                                }
                                Err(e) => tracing::warn!("Uretilen vertex ingest hatasi: {e}"),
                            }
                        }
                        Err(e) => tracing::error!("Vertex uretilemedi: {e:?}"),
                    }
                }
            }
            // GOZLEMLENEBILIRLIK arm'i: periyodik durum logu.
            // RPC /submit kanalindan gelen vertex: AGA yayinla (gossipsub publish).
            // RPC zaten kendi dugumune ingest etti; burada sadece diger dugumlere duyuruyoruz.
            Some(yayin_bytes) = submit_rx.recv() => {
                // KALICILIK ONARIMI: RPC /submit'ten gelen vertex (faucet/transfer
                // dahil) DISKE YAZILMALI. Onceden sadece gossip publish ediliyordu;
                // diske yazilmadigi icin reboot'ta kayboluyordu (parent zinciri kopuk).
                // ingest RPC tarafinda zaten yapildi; burada kalici kayit + yayin.
                if let Some(ref path) = data_file {
                    if let Err(e) = store::append_vertex(path, &yayin_bytes) {
                        tracing::warn!("RPC /submit vertex'i diske yazilamadi: {e}");
                    }
                }
                if let Err(e) = swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(topic.clone(), yayin_bytes)
                {
                    tracing::warn!("RPC /submit vertex'i yayinlanamadi: {e:?}");
                } else {
                    tracing::info!("RPC /submit vertex'i aga yayinlandi");
                }
            }
            _ = status_tick.tick() => {
                let (vc, oc, tc, ts, sn) = {
                    let st = node_state.read().await;
                    (
                        st.vertex_count(),
                        st.orphan_count(),
                        st.token_sayisi(),
                        st.toplam_stake(),
                        st.staker_sayisi(),
                    )
                };
                let peers = swarm.connected_peers().count();
                tracing::info!(
                    "DURUM: vertex={vc}, peer={peers}, orphan={oc}, token={tc}, stake={ts}({sn} kisi)"
                );
            }
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        tracing::info!("Listening on {address}");
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        tracing::info!("Connected to {peer_id}");
                        // PULL SYNC: yeni baglandigim peer'dan gecmis vertex'leri ISTE.
                        // request-response AYRI kanal (gossipsub seen-cache YOK) ->
                        // "Duplicate" sorunu yok. Gec katilsam bile genesis dahil
                        // her seyi bu istekle alirim. Cevap, Sync event'inde islenir.
                        swarm
                            .behaviour_mut()
                            .sync
                            .send_request(&peer_id, SyncRequest { offset: 0 });
                        tracing::info!("Pull-sync istegi gonderildi -> {peer_id}");
                    }
                    SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                        tracing::debug!("Connection closed with {peer_id}: {cause:?}");
                    }
                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        tracing::debug!("Outgoing connection error to {peer_id:?}: {error}");
                    }
                    SwarmEvent::IncomingConnectionError { send_back_addr, error, .. } => {
                        tracing::debug!("Incoming connection error from {send_back_addr}: {error}");
                    }
                    SwarmEvent::Behaviour(LscBehaviourEvent::Gossipsub(
                        gossipsub::Event::Message { propagation_source, message, .. }
                    )) => {
                        // Agdan gelen vertex baytlari -> NodeState::ingest.
                        // Bozuk/gecersiz vertex graf durumunu DEGISTIRMEZ (yapiya gomulu).
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        // ingest_networked: ORPHAN-BILINCLI. Eksik parent'li vertex
                        // (sirasiz/cok-hop geldiginde olur) reddedilmez; orphan havuzuna
                        // alinir, parent gelince cascade ile cozulur. Cok-node yakinsama
                        // icin SART (zincir topolojisinde vertex'ler sirasiz gelebilir).
                        let accepted = {
                            let mut st = node_state.write().await;
                            match st.ingest_networked(&message.data, now) {
                                lsc_engine::NetworkIngestOutcome::Integrated(id) => {
                                    tracing::info!(
                                        "Vertex ingest edildi: id={}, toplam_vertex={}, kaynak={propagation_source}",
                                        hex::encode(&id[..8]),
                                        st.vertex_count()
                                    );
                                    // 5c GOZLEM (zorunlu degil, sadece okuma): gelen vertex'in
                                    // payload'ini Record olarak oku. Basarili -> islem tasiniyor
                                    // + okunabiliyor (kanit). Genesis gibi Record-olmayan -> sessiz.
                                    // DOGRULAMA degil: engine vertex'i zaten dogruladi; katman
                                    // temiz (engine DAG'i, net islemi yorumlar).
                                    if let Ok(v) = lsc_engine::dag::wire::decode(&message.data) {
                                        if let Ok(rec) = lsc_engine::Record::decode(v.payload()) {
                                            tracing::info!(
                                                "  -> Record okundu (belge kaydi): hash[..8]={}",
                                                hex::encode(&rec.data_hash[..8])
                                            );
                                        }
                                    }
                                    true
                                }
                                lsc_engine::NetworkIngestOutcome::Buffered(id) => {
                                    tracing::info!(
                                        "Vertex orphan'a alindi (parent bekleniyor): id={}, bekleyen={}, kaynak={propagation_source}",
                                        hex::encode(&id[..8]),
                                        st.orphan_count()
                                    );
                                    // KALICILIK ONARIMI: orphan da GECERLI bir vertex (decode
                                    // edildi, sadece parent'i henuz gelmedi). Diske YAZILMALI;
                                    // yoksa orphan cascade ile cozulunce yazilma firsati kacar
                                    // -> reboot'ta "parent zinciri kopuk" olur. Dosya ham olay
                                    // kaydidir; reboot'ta topolojik yukleme dogru sirayi kurar.
                                    true
                                }
                                lsc_engine::NetworkIngestOutcome::Duplicate(_) => false,
                                lsc_engine::NetworkIngestOutcome::Rejected(e) => {
                                    tracing::warn!(
                                        "Vertex reddedildi ({} bayt): {e}, kaynak={propagation_source}",
                                        message.data.len()
                                    );
                                    false
                                }
                                lsc_engine::NetworkIngestOutcome::OrphanPoolFull(_) => {
                                    tracing::warn!(
                                        "Orphan havuzu DOLU, vertex dusuruldu, kaynak={propagation_source}"
                                    );
                                    false
                                }
                            }
                        }; // kilit birakilir
                        // KALICILIK: kabul edilen vertex'i diske ekle (kilit DISINDA).
                        if accepted {
                            if let Some(ref path) = data_file {
                                if let Err(e) = store::append_vertex(path, &message.data) {
                                    tracing::warn!("Agdan gelen vertex diske yazilamadi: {e}");
                                }
                            }
                        }
                    }
                    SwarmEvent::Behaviour(LscBehaviourEvent::Gossipsub(
                        gossipsub::Event::Subscribed { peer_id, topic: sub_topic }
                    )) => {
                        tracing::info!("Peer {peer_id} abone oldu: {sub_topic}");
                        // PUSH SYNC (GECICI — bkz NOTLAR_BILINEN_SINIRLAR.md #1):
                        // Yeni peer abone olunca, sahip oldugum TUM vertex'leri ona
                        // yayinlarim (genesis dahil). Boylece gec katilan node gecmisi
                        // alir (gossipsub canli mesajlari gondermez, gecmisi gondermez).
                        // export_vertices() genesis'i de icerir -> ayri genesis yayini
                        // gereksiz. Alici ingest_networked (orphan+cascade) ile sirasiz
                        // gelen vertex'leri cozer.
                        // TODO(olceklenme): Bu, her yeni peer'da TUM gecmisi TUM aga
                        // yeniden yayinlar (verimsiz). Gercek testnet oncesi PULL SYNC
                        // (request-response) ile degistirilmeli.
                        let all_vertices = {
                            let st = node_state.read().await;
                            st.export_vertices()
                        };
                        let n = all_vertices.len();
                        if n > 0 {
                            let mut sent = 0;
                            for v_bytes in all_vertices {
                                if swarm
                                    .behaviour_mut()
                                    .gossipsub
                                    .publish(topic.clone(), v_bytes)
                                    .is_ok()
                                {
                                    sent += 1;
                                }
                            }
                            tracing::info!(
                                "Push sync: {sent}/{n} vertex yayinlandi -> yeni peer {peer_id}"
                            );
                        }
                    }
                    // PULL SYNC event'leri (request-response, gossipsub'dan AYRI).
                    SwarmEvent::Behaviour(LscBehaviourEvent::Sync(
                        request_response::Event::Message { peer, message, .. }
                    )) => {
                        match message {
                            // ISTEK geldi: topolojik sirali listeden, istenen offset'ten
                            // itibaren EN FAZLA SYNC_CHUNK vertex gonder (buyuk graf tek
                            // mesaja sigmaz; alici "total"e gore devamini ister).
                            request_response::Message::Request { request, channel, .. } => {
                                const SYNC_CHUNK: usize = 2000;
                                let off = request.offset as usize;
                                let (parca, total) = {
                                    let st = node_state.read().await;
                                    let all = st.export_vertices();
                                    let total = all.len() as u64;
                                    let parca: Vec<Vec<u8>> =
                                        all.into_iter().skip(off).take(SYNC_CHUNK).collect();
                                    (parca, total)
                                };
                                let n = parca.len();
                                let resp = SyncResponse {
                                    vertices: parca,
                                    offset: request.offset,
                                    total,
                                };
                                if swarm
                                    .behaviour_mut()
                                    .sync
                                    .send_response(channel, resp)
                                    .is_ok()
                                {
                                    tracing::info!("Pull-sync cevabi gonderildi: {n} vertex (offset={off}/{total}) -> {peer}");
                                } else {
                                    tracing::warn!("Pull-sync cevabi gonderilemedi -> {peer}");
                                }
                            }
                            // CEVAP geldi: gelen vertex'leri ingest_synced (orphan+cascade)
                            // ile yukle. ingest_synced = replay yolu (clock-policy YOK).
                            request_response::Message::Response { response, .. } => {
                                let alinan = response.vertices.len() as u64;
                                let resp_offset = response.offset;
                                let resp_total = response.total;
                                let mut integrated = 0u32;
                                let mut buffered = 0u32;
                                for v_bytes in response.vertices {
                                    let outcome = {
                                        let mut st = node_state.write().await;
                                        st.ingest_synced(&v_bytes)
                                    };
                                    match outcome {
                                        lsc_engine::NetworkIngestOutcome::Integrated(_) => {
                                            integrated += 1;
                                            // KALICILIK: graf'a giren vertex'i diske yaz.
                                            if let Some(ref path) = data_file {
                                                let _ = store::append_vertex(path, &v_bytes);
                                            }
                                        }
                                        lsc_engine::NetworkIngestOutcome::Buffered(_) => {
                                            buffered += 1;
                                        }
                                        _ => {}
                                    }
                                }
                                let count = { node_state.read().await.vertex_count() };
                                let yeni_offset = resp_offset + alinan;
                                tracing::info!(
                                    "Pull-sync cevabi islendi: {integrated} entegre, {buffered} orphan ({alinan} gelen, offset={resp_offset}/{resp_total}), toplam_vertex={count}, kaynak={peer}"
                                );
                                // CHUNKED: daha vertex varsa, devamini ISTE (bir sonraki parca).
                                if alinan > 0 && yeni_offset < resp_total {
                                    swarm
                                        .behaviour_mut()
                                        .sync
                                        .send_request(&peer, SyncRequest { offset: yeni_offset });
                                    tracing::info!("Pull-sync devam istegi -> {peer} (offset={yeni_offset}/{resp_total})");
                                }
                            }
                        }
                    }
                    SwarmEvent::Behaviour(LscBehaviourEvent::Ping(_)) => {
                        tracing::debug!("Ping event");
                    }
                    // mDNS: otomatik peer kesfi (yerel ag). Kesfedilen her peer'a
                    // dial et -> baglanti -> ConnectionEstablished -> pull-sync ->
                    // yakinsama. Manuel IP girmeye gerek kalmaz.
                    SwarmEvent::Behaviour(LscBehaviourEvent::Mdns(
                        mdns::Event::Discovered(peers),
                    )) => {
                        for (peer_id, addr) in peers {
                            tracing::info!("mDNS kesfetti: {peer_id} @ {addr}");
                            if let Err(e) = swarm.dial(addr) {
                                tracing::debug!("mDNS dial hatasi ({peer_id}): {e}");
                            }
                        }
                    }
                    SwarmEvent::Behaviour(LscBehaviourEvent::Mdns(
                        mdns::Event::Expired(peers),
                    )) => {
                        for (peer_id, _addr) in peers {
                            tracing::debug!("mDNS suresi doldu: {peer_id}");
                        }
                    }
                    other => {
                        tracing::debug!("Other swarm event: {other:?}");
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn generate_peer_id() -> PeerId {
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    PeerId::from(keypair.public())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_id_is_generated() {
        let id1 = generate_peer_id();
        let id2 = generate_peer_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn peer_id_is_deterministic_from_keypair() {
        let kp = libp2p::identity::Keypair::generate_ed25519();
        let a = PeerId::from(kp.public());
        let b = PeerId::from(kp.public());
        assert_eq!(a, b);
    }
}
