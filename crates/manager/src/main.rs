use common::rpc::{
    manager_service_server::{ManagerService, ManagerServiceServer},
    storager_service_client::StoragerServiceClient,
    AddRequest, AddResponse, DeleteRequest, DeleteResponse, QueryRequest, QueryResponse,
    StoragerAddRequest, StoragerDeleteRequest, StoragerQueryRequest, UpdateRequest, UpdateResponse,
};
use common::{AdsMode, RootHash};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tonic::{transport::Server, Request, Response, Status};

// Manager structure
pub struct Manager {
    // Map from storager_id to storager address
    storager_addrs: Vec<String>,
    // Map from storager_id to root hash
    root_hashes: Arc<RwLock<HashMap<usize, RootHash>>>,
    // ADS mode
    #[allow(dead_code)]
    ads_mode: AdsMode,
}

impl Manager {
    pub fn new(storager_addrs: Vec<String>, ads_mode: AdsMode) -> Self {
        let root_hashes = Arc::new(RwLock::new(HashMap::new()));
        Manager {
            storager_addrs,
            root_hashes,
            ads_mode,
        }
    }

    // Consistent hashing: map keyword to storager index
    fn get_storager_index(&self, keyword: &str) -> usize {
        let hash = self.hash_keyword(keyword);
        hash % self.storager_addrs.len()
    }

    // Simple hash function for keyword
    fn hash_keyword(&self, keyword: &str) -> usize {
        keyword.bytes().map(|b| b as usize).sum()
    }

    // Verify proof (placeholder implementation)
    fn verify_proof(&self, _proof: &[u8], _root_hash: &[u8]) -> bool {
        // TODO: Implement actual proof verification based on ADS mode
        true
    }

    // Update root hash for a storager
    fn update_root_hash(&self, storager_id: usize, root_hash: RootHash) {
        let mut hashes = self.root_hashes.write().unwrap();
        hashes.insert(storager_id, root_hash);
    }
}

#[tonic::async_trait]
impl ManagerService for Manager {
    async fn add(&self, request: Request<AddRequest>) -> Result<Response<AddResponse>, Status> {
        let req = request.into_inner();
        println!("Manager received Add request for fid: {}", req.fid);

        // Process each keyword
        for keyword in &req.keywords {
            let storager_idx = self.get_storager_index(keyword);
            let storager_addr = &self.storager_addrs[storager_idx];

            // Connect to storager and send Add request
            let mut client = StoragerServiceClient::connect(storager_addr.clone())
                .await
                .map_err(|e| Status::internal(format!("Failed to connect to storager: {}", e)))?;

            let storager_req = StoragerAddRequest {
                keyword: keyword.clone(),
                fid: req.fid.clone(),
            };

            let response = client
                .add(storager_req)
                .await
                .map_err(|e| Status::internal(format!("Storager Add failed: {}", e)))?;

            let resp = response.into_inner();

            // Verify proof and update root hash
            if self.verify_proof(&resp.proof, &resp.root_hash) {
                self.update_root_hash(storager_idx, resp.root_hash);
            } else {
                return Ok(Response::new(AddResponse {
                    success: false,
                    message: "Proof verification failed".to_string(),
                }));
            }
        }

        Ok(Response::new(AddResponse {
            success: true,
            message: "Add operation completed successfully".to_string(),
        }))
    }

    async fn query(
        &self,
        request: Request<QueryRequest>,
    ) -> Result<Response<QueryResponse>, Status> {
        let req = request.into_inner();
        println!("Manager received Query request");

        // TODO: Handle boolean function queries
        // For now, only handle single keyword queries
        let keyword = match req.query_type {
            Some(common::rpc::query_request::QueryType::Keyword(kw)) => kw,
            Some(common::rpc::query_request::QueryType::BooleanFunction(_)) => {
                return Err(Status::unimplemented(
                    "Boolean function queries not yet implemented",
                ));
            }
            None => return Err(Status::invalid_argument("No query type specified")),
        };

        let storager_idx = self.get_storager_index(&keyword);
        let storager_addr = &self.storager_addrs[storager_idx];

        // Connect to storager and send Query request
        let mut client = StoragerServiceClient::connect(storager_addr.clone())
            .await
            .map_err(|e| Status::internal(format!("Failed to connect to storager: {}", e)))?;

        let storager_req = StoragerQueryRequest {
            keyword: keyword.clone(),
        };

        let response = client
            .query(storager_req)
            .await
            .map_err(|e| Status::internal(format!("Storager Query failed: {}", e)))?;

        let resp = response.into_inner();

        // Get root hash for this storager
        let root_hash = self
            .root_hashes
            .read()
            .unwrap()
            .get(&storager_idx)
            .cloned()
            .unwrap_or_default();

        // Verify proof
        let verified = self.verify_proof(&resp.proof, &root_hash);

        Ok(Response::new(QueryResponse {
            fids: resp.fids,
            proof: resp.proof,
            root_hash,
            verified,
        }))
    }

    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        println!("Manager received Delete request for fid: {}", req.fid);

        // Process each keyword
        for keyword in &req.keywords {
            let storager_idx = self.get_storager_index(keyword);
            let storager_addr = &self.storager_addrs[storager_idx];

            // Connect to storager and send Delete request
            let mut client = StoragerServiceClient::connect(storager_addr.clone())
                .await
                .map_err(|e| Status::internal(format!("Failed to connect to storager: {}", e)))?;

            let storager_req = StoragerDeleteRequest {
                keyword: keyword.clone(),
                fid: req.fid.clone(),
            };

            let response = client
                .delete(storager_req)
                .await
                .map_err(|e| Status::internal(format!("Storager Delete failed: {}", e)))?;

            let resp = response.into_inner();

            // Verify proof and update root hash
            if self.verify_proof(&resp.proof, &resp.root_hash) {
                self.update_root_hash(storager_idx, resp.root_hash);
            } else {
                return Ok(Response::new(DeleteResponse {
                    success: false,
                    message: "Proof verification failed".to_string(),
                }));
            }
        }

        Ok(Response::new(DeleteResponse {
            success: true,
            message: "Delete operation completed successfully".to_string(),
        }))
    }

    async fn update(
        &self,
        request: Request<UpdateRequest>,
    ) -> Result<Response<UpdateResponse>, Status> {
        let req = request.into_inner();
        println!("Manager received Update request for fid: {}", req.fid);

        // Delete old keywords
        for keyword in &req.old_keywords {
            let storager_idx = self.get_storager_index(keyword);
            let storager_addr = &self.storager_addrs[storager_idx];

            let mut client = StoragerServiceClient::connect(storager_addr.clone())
                .await
                .map_err(|e| Status::internal(format!("Failed to connect to storager: {}", e)))?;

            let storager_req = StoragerDeleteRequest {
                keyword: keyword.clone(),
                fid: req.fid.clone(),
            };

            let response = client
                .delete(storager_req)
                .await
                .map_err(|e| Status::internal(format!("Storager Delete failed: {}", e)))?;

            let resp = response.into_inner();
            if self.verify_proof(&resp.proof, &resp.root_hash) {
                self.update_root_hash(storager_idx, resp.root_hash);
            }
        }

        // Add new keywords
        for keyword in &req.new_keywords {
            let storager_idx = self.get_storager_index(keyword);
            let storager_addr = &self.storager_addrs[storager_idx];

            let mut client = StoragerServiceClient::connect(storager_addr.clone())
                .await
                .map_err(|e| Status::internal(format!("Failed to connect to storager: {}", e)))?;

            let storager_req = StoragerAddRequest {
                keyword: keyword.clone(),
                fid: req.fid.clone(),
            };

            let response = client
                .add(storager_req)
                .await
                .map_err(|e| Status::internal(format!("Storager Add failed: {}", e)))?;

            let resp = response.into_inner();
            if self.verify_proof(&resp.proof, &resp.root_hash) {
                self.update_root_hash(storager_idx, resp.root_hash);
            }
        }

        Ok(Response::new(UpdateResponse {
            success: true,
            message: "Update operation completed successfully".to_string(),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;

    // TODO: Load from config
    let storager_addrs = vec![
        "http://[::1]:50052".to_string(),
        "http://[::1]:50053".to_string(),
    ];

    let manager = Manager::new(storager_addrs, AdsMode::MerkleTree);

    println!("Manager server listening on {}", addr);

    Server::builder()
        .add_service(ManagerServiceServer::new(manager))
        .serve(addr)
        .await?;

    Ok(())
}
