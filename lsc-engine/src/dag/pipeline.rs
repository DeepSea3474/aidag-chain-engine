//! Tek-node ingest boru hattı — wire katmanını DAG motoruna bağlar.
//!
//! Akış (sıra DEĞİŞTİRİLEMEZ, tip sistemi zorlar):
//!   gelen baytlar → `wire::decode` → `Graph::insert` → `Ghostdag::update`
//!
//! Tasarım ilkeleri (endişeler UYARI değil, YAPIYA gömülü):
//!   - Bozuk bayt → `decode` aşamasında düşer; graf'a HİÇ ulaşmaz.
//!   - Geçersiz vertex → `insert` reddeder; hata tipiyle döner (atomik).
//!   - Sıra garantili: decode → insert → update; hiçbiri atlanamaz.

use crate::consensus::ghostdag::Ghostdag;
use crate::dag::graph::{Graph, GraphError};
use crate::dag::vertex::VertexId;
use crate::dag::wire::{self, WireError};

/// Ingest sırasında oluşabilecek hata. Her aşamanın hatası KORUNUR.
/// (thiserror kullanılmaz: WireError std::error::Error implement etmiyor;
///  dönüşümler elle yazılır, wire.rs'e dokunulmaz.)
#[derive(Debug)]
pub enum IngestError {
    /// Baytlar Vertex'e çözülemedi. Graf'a HİÇ dokunulmadı.
    Decode(WireError),
    /// Vertex çözüldü ama graf'a eklenemedi (atomik ret).
    Graph(GraphError),
}

impl From<WireError> for IngestError {
    fn from(e: WireError) -> Self {
        IngestError::Decode(e)
    }
}
impl From<GraphError> for IngestError {
    fn from(e: GraphError) -> Self {
        IngestError::Graph(e)
    }
}
impl std::fmt::Display for IngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IngestError::Decode(e) => write!(f, "wire decode failed: {:?}", e),
            IngestError::Graph(e) => write!(f, "graph insert rejected vertex: {}", e),
        }
    }
}
impl std::error::Error for IngestError {}

/// decode → insert → ghostdag update. Başarıda VertexId döner.
pub fn ingest_bytes(
    graph: &mut Graph,
    ghostdag: &mut Ghostdag,
    bytes: &[u8],
    now: u64,
) -> Result<VertexId, IngestError> {
    let vertex = wire::decode(bytes)?;
    let id = *vertex.id();
    graph.insert(vertex, now)?;
    ghostdag.update(graph);
    Ok(id)
}

/// Senkron/replay yolu: saat politikası uygulanmadan.
pub fn ingest_bytes_synced(
    graph: &mut Graph,
    ghostdag: &mut Ghostdag,
    bytes: &[u8],
) -> Result<VertexId, IngestError> {
    let vertex = wire::decode(bytes)?;
    let id = *vertex.id();
    graph.insert_synced(vertex)?;
    ghostdag.update(graph);
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::ghostdag::{Ghostdag, DEFAULT_K};
    use crate::dag::vertex::Vertex;
    use crate::dag::wire;
    use ed25519_dalek::SigningKey;

    const NET: u32 = 7;

    fn signed(parents: Vec<VertexId>, payload: &[u8], ts: u64, sk: &SigningKey) -> Vertex {
        let mut ps = parents;
        ps.sort_unstable();
        Vertex::new_signed(NET, ps, payload.to_vec(), ts, sk).expect("new_signed geçerli olmalı")
    }

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn bootstrap() -> (Graph, Ghostdag, VertexId, SigningKey) {
        let sk = key(1);
        let mut graph = Graph::devnet(NET);
        let mut ghostdag = Ghostdag::new_incremental(DEFAULT_K);
        let genesis = signed(vec![], b"genesis", 1_000, &sk);
        let gbytes = wire::encode(&genesis);
        let gid =
            ingest_bytes(&mut graph, &mut ghostdag, &gbytes, 1_000).expect("genesis ingest etmeli");
        (graph, ghostdag, gid, sk)
    }

    #[test]
    fn roundtrip_encode_ingest() {
        let (graph, _gd, gid, _sk) = bootstrap();
        assert!(graph.contains(&gid), "genesis graf'ta olmalı");
        assert_eq!(graph.len(), 1, "graf'ta tam 1 vertex olmalı");
    }

    #[test]
    fn rejects_garbage_bytes_graph_untouched() {
        let (mut graph, mut gd, _gid, _sk) = bootstrap();
        let before = graph.len();
        let garbage = [0xFF, 0x00, 0x13, 0x37, 0xAB];
        let res = ingest_bytes(&mut graph, &mut gd, &garbage, 2_000);
        assert!(
            matches!(res, Err(IngestError::Decode(_))),
            "Decode hatası beklenir"
        );
        assert_eq!(graph.len(), before, "bozuk veri graf'ı değiştirmemeli");
    }

    #[test]
    fn rejects_unknown_parent() {
        let (mut graph, mut gd, _gid, sk) = bootstrap();
        let before = graph.len();
        let fake_parent: VertexId = [0x42; 32];
        let orphan = signed(vec![fake_parent], b"orphan", 2_000, &sk);
        let bytes = wire::encode(&orphan);
        let res = ingest_bytes(&mut graph, &mut gd, &bytes, 2_000);
        assert!(
            matches!(res, Err(IngestError::Graph(_))),
            "Graph hatası beklenir"
        );
        assert_eq!(
            graph.len(),
            before,
            "reddedilen vertex graf'ı değiştirmemeli"
        );
    }

    #[test]
    fn sequential_ingest_updates_ghostdag() {
        let (mut graph, mut gd, gid, sk) = bootstrap();
        let child = signed(vec![gid], b"child", 1_001, &sk);
        let cid = *child.id();
        let cbytes = wire::encode(&child);
        ingest_bytes(&mut graph, &mut gd, &cbytes, 1_001).expect("child ingest");
        let grand = signed(vec![cid], b"grand", 1_002, &sk);
        let ggid = *grand.id();
        let gbytes = wire::encode(&grand);
        ingest_bytes(&mut graph, &mut gd, &gbytes, 1_002).expect("grand ingest");
        assert_eq!(graph.len(), 3, "graf'ta 3 vertex olmalı");
        let gs_genesis = gd.blue_score(&gid).expect("genesis blue_score");
        let gs_child = gd.blue_score(&cid).expect("child blue_score");
        let gs_grand = gd.blue_score(&ggid).expect("grand blue_score");
        assert!(gs_genesis <= gs_child, "blue_score azalmamalı");
        assert!(gs_child <= gs_grand, "blue_score azalmamalı");
    }

    #[test]
    fn duplicate_ingest_rejected() {
        let (mut graph, mut gd, gid, sk) = bootstrap();
        let child = signed(vec![gid], b"dup", 1_001, &sk);
        let bytes = wire::encode(&child);
        ingest_bytes(&mut graph, &mut gd, &bytes, 1_001).expect("ilk ingest");
        let before = graph.len();
        let res = ingest_bytes(&mut graph, &mut gd, &bytes, 1_001);
        assert!(
            matches!(res, Err(IngestError::Graph(_))),
            "duplicate Graph hatası"
        );
        assert_eq!(graph.len(), before, "duplicate graf'ı değiştirmemeli");
    }
}
