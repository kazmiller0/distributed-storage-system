use serde::{Deserialize, Serialize};

// Unique file identifier
pub type Fid = String;

// Keyword for search
pub type Keyword = String;

// Hash of a data chunk or a Merkle tree root
pub type RootHash = Vec<u8>;

// Proof of storage or query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    pub data: Vec<u8>,
}

// ADS Mode - type of authenticated data structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdsMode {
    MerkleTree,
    PatriciaTrie,
}

// Configuration for the distributed storage system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub num_clients: usize,
    pub num_storagers: usize,
    pub ads_mode: AdsMode,
    pub manager_addr: String,
    pub storager_addrs: Vec<String>,
    pub client_addrs: Vec<String>,
}
