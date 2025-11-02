use ads::merkle_tree::MerkleTree;
use ads::patricia_trie::PatriciaTrie;
use common::rpc::{
    storager_service_server::{StoragerService, StoragerServiceServer},
    StoragerAddRequest, StoragerAddResponse, StoragerDeleteRequest, StoragerDeleteResponse,
    StoragerQueryRequest, StoragerQueryResponse,
};
use common::{AdsMode, RootHash};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tonic::{transport::Server, Request, Response, Status};

// Trait for ADS operations
trait AdsOperations: Send + Sync {
    fn add(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash);
    fn query(&self, keyword: &str) -> (Vec<String>, Vec<u8>);
    fn delete(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash);
}

// Wrapper for MerkleTree
struct MerkleTreeAds {
    tree: MerkleTree,
    data: HashMap<String, Vec<String>>, // keyword -> list of fids
}

impl MerkleTreeAds {
    fn new() -> Self {
        MerkleTreeAds {
            tree: MerkleTree::new(),
            data: HashMap::new(),
        }
    }
}

impl AdsOperations for MerkleTreeAds {
    fn add(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash) {
        self.data
            .entry(keyword.to_string())
            .or_insert_with(Vec::new)
            .push(fid.to_string());

        // Update Merkle tree
        let data = format!("{}:{}", keyword, fid);
        self.tree.insert(data.as_bytes());

        let proof = self.tree.get_proof(data.as_bytes());
        let root_hash = self.tree.root_hash();

        (proof, root_hash)
    }

    fn query(&self, keyword: &str) -> (Vec<String>, Vec<u8>) {
        let fids = self.data.get(keyword).cloned().unwrap_or_default();
        let proof = if !fids.is_empty() {
            let data = format!("{}:{}", keyword, fids[0]);
            self.tree.get_proof(data.as_bytes())
        } else {
            vec![]
        };
        (fids, proof)
    }

    fn delete(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash) {
        if let Some(fids) = self.data.get_mut(keyword) {
            fids.retain(|f| f != fid);
            if fids.is_empty() {
                self.data.remove(keyword);
            }
        }

        // Update Merkle tree
        let data = format!("{}:{}", keyword, fid);
        self.tree.delete(data.as_bytes());

        let proof = self.tree.get_proof(data.as_bytes());
        let root_hash = self.tree.root_hash();

        (proof, root_hash)
    }
}

// Wrapper for PatriciaTrie
struct PatriciaTrieAds {
    trie: PatriciaTrie,
    data: HashMap<String, Vec<String>>, // keyword -> list of fids
}

impl PatriciaTrieAds {
    fn new() -> Self {
        PatriciaTrieAds {
            trie: PatriciaTrie::new(),
            data: HashMap::new(),
        }
    }
}

impl AdsOperations for PatriciaTrieAds {
    fn add(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash) {
        self.data
            .entry(keyword.to_string())
            .or_insert_with(Vec::new)
            .push(fid.to_string());

        // Update Patricia trie
        let value = self.data.get(keyword).unwrap().join(",");
        self.trie.insert(keyword.as_bytes(), value.as_bytes());

        let proof = self.trie.get_proof(keyword.as_bytes());
        let root_hash = self.trie.root_hash();

        (proof, root_hash)
    }

    fn query(&self, keyword: &str) -> (Vec<String>, Vec<u8>) {
        let fids = self.data.get(keyword).cloned().unwrap_or_default();
        let proof = self.trie.get_proof(keyword.as_bytes());
        (fids, proof)
    }

    fn delete(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash) {
        if let Some(fids) = self.data.get_mut(keyword) {
            fids.retain(|f| f != fid);
            if fids.is_empty() {
                self.data.remove(keyword);
                self.trie.delete(keyword.as_bytes());
            } else {
                let value = fids.join(",");
                self.trie.insert(keyword.as_bytes(), value.as_bytes());
            }
        }

        let proof = self.trie.get_proof(keyword.as_bytes());
        let root_hash = self.trie.root_hash();

        (proof, root_hash)
    }
}

// Storager structure
pub struct Storager {
    ads: Arc<RwLock<Box<dyn AdsOperations>>>,
}

impl Storager {
    pub fn new(mode: AdsMode) -> Self {
        let ads: Box<dyn AdsOperations> = match mode {
            AdsMode::MerkleTree => Box::new(MerkleTreeAds::new()),
            AdsMode::PatriciaTrie => Box::new(PatriciaTrieAds::new()),
        };

        Storager {
            ads: Arc::new(RwLock::new(ads)),
        }
    }
}

#[tonic::async_trait]
impl StoragerService for Storager {
    async fn add(
        &self,
        request: Request<StoragerAddRequest>,
    ) -> Result<Response<StoragerAddResponse>, Status> {
        let req = request.into_inner();
        println!(
            "Storager received Add request: keyword={}, fid={}",
            req.keyword, req.fid
        );

        let mut ads = self.ads.write().unwrap();
        let (proof, root_hash) = ads.add(&req.keyword, &req.fid);

        Ok(Response::new(StoragerAddResponse { proof, root_hash }))
    }

    async fn query(
        &self,
        request: Request<StoragerQueryRequest>,
    ) -> Result<Response<StoragerQueryResponse>, Status> {
        let req = request.into_inner();
        println!("Storager received Query request: keyword={}", req.keyword);

        let ads = self.ads.read().unwrap();
        let (fids, proof) = ads.query(&req.keyword);

        Ok(Response::new(StoragerQueryResponse { fids, proof }))
    }

    async fn delete(
        &self,
        request: Request<StoragerDeleteRequest>,
    ) -> Result<Response<StoragerDeleteResponse>, Status> {
        let req = request.into_inner();
        println!(
            "Storager received Delete request: keyword={}, fid={}",
            req.keyword, req.fid
        );

        let mut ads = self.ads.write().unwrap();
        let (proof, root_hash) = ads.delete(&req.keyword, &req.fid);

        Ok(Response::new(StoragerDeleteResponse { proof, root_hash }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments for port
    let args: Vec<String> = std::env::args().collect();
    let port = if args.len() > 1 {
        args[1].parse::<u16>().unwrap_or(50052)
    } else {
        50052
    };

    let addr = format!("[::1]:{}", port).parse()?;

    // TODO: Load from config
    let storager = Storager::new(AdsMode::MerkleTree);

    println!("Storager server listening on {}", addr);

    Server::builder()
        .add_service(StoragerServiceServer::new(storager))
        .serve(addr)
        .await?;

    Ok(())
}
