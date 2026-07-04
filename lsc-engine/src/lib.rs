#![allow(clippy::doc_overindented_list_items)] // dokuman liste girinti estetigi (kod degil)
#![allow(clippy::doc_lazy_continuation)] // dokuman devam-satiri estetigi (kod degil)

//! LSC Engine — AIDAG Chain'in DAG tabanlı L1 motoru.
//!
//! Bu kütüphane sıfırdan yazılır, sahte/placeholder içermez.
//! Her adım `cargo test` ile doğrulanır ve bağımsız AI denetçilerinden
//! geçer (anti-fake politikası: docs/LSC_DAG_DEVELOPMENT_INSTRUCTIONS.md).
//!
//! Adım 1: `dag::vertex` — DAG'ın temel düğümü.
//!   - blake3 ile içerik adresli, domain-separated ID
//!   - ed25519 imza ve `verify_strict` doğrulama
//!   - genesis (parent'sız) ve normal vertex desteği
//!   - güvenli deserialize girişi (`from_parts`)

pub mod avm;
pub mod consensus;
pub mod genesis;
pub mod dag;
pub mod node;
pub mod registry;
pub mod tx;

pub use consensus::finality::{
    equivocators, extends_final, final_block, is_final, pruning_anchor, FinalityDepth,
    FinalityState, DEFAULT_FINALITY_DEPTH, DEFAULT_PRUNING_DEPTH,
};
pub use consensus::ghostdag::{
    CommitteeWeight, Ghostdag, GhostdagData, KType, UniformWeight, Weigher, DEFAULT_K,
};
pub use dag::graph::{
    GenesisPolicy, Graph, GraphError, CAUSALITY_MAX_SKEW_SECS, MAX_CLOCK_SKEW_SECS,
};
pub use dag::vertex::{
    Vertex, VertexError, VertexId, DOMAIN_TAG, FORMAT_VERSION, MAX_PARENTS, MAX_PAYLOAD_BYTES,
};

pub use node::{NetworkIngestOutcome, NodeState};
pub use registry::{
    public_key_to_adres, KayitSonucu, KurumKategori, KurumKaydi, StakeRegistry, TokenRegistry,
};
pub use tx::{
    Record, StakeKaydi, TokenKaydi, TxError, TX_TYPE_RECORD, TX_TYPE_STAKE, TX_TYPE_TOKEN,
};
