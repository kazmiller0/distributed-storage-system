use crate::manager::Manager;
use common::parse_boolean_expr;
use common::rpc::{
    manager_service_server::ManagerService, storager_service_client::StoragerServiceClient,
    AddRequest, AddResponse, DeleteRequest, DeleteResponse, QueryRequest, QueryResponse,
    StoragerAddRequest, StoragerDeleteRequest, StoragerQueryRequest, UpdateRequest, UpdateResponse,
};
use std::collections::{HashMap, HashSet};
use tonic::{Request, Response, Status};

#[tonic::async_trait]
impl ManagerService for Manager {
    async fn add(&self, request: Request<AddRequest>) -> Result<Response<AddResponse>, Status> {
        let req = request.into_inner();
        println!("Manager received Add request for fid: {}", req.fid);

        // Deduplicate keywords to avoid adding the same element twice
        let unique_keywords: HashSet<String> = req.keywords.into_iter().collect();
        let keyword_count = unique_keywords.len();
        
        if keyword_count == 0 {
            return Ok(Response::new(AddResponse {
                success: false,
                message: "No keywords provided".to_string(),
            }));
        }
        
        println!("  Processing {} unique keyword(s)", keyword_count);

        // Process each unique keyword
        for keyword in &unique_keywords {
            let (node_name, storager_addr) = self
                .get_storager_for_keyword(keyword)
                .ok_or_else(|| Status::internal("No storager available"))?;

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
                self.update_root_hash(node_name, resp.root_hash);
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

        match req.query_type {
            Some(common::rpc::query_request::QueryType::Keyword(keyword)) => {
                // 单关键词查询
                self.query_single_keyword(&keyword).await
            }
            Some(common::rpc::query_request::QueryType::BooleanFunction(func)) => {
                // 布尔函数查询
                self.query_boolean_function(&func).await
            }
            None => Err(Status::invalid_argument("No query type specified")),
        }
    }

    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();
        println!("Manager received Delete request for fid: {}", req.fid);

        // Deduplicate keywords to avoid deleting the same element twice
        let unique_keywords: HashSet<String> = req.keywords.into_iter().collect();
        let keyword_count = unique_keywords.len();
        
        if keyword_count == 0 {
            return Ok(Response::new(DeleteResponse {
                success: false,
                message: "No keywords provided".to_string(),
            }));
        }
        
        println!("  Processing {} unique keyword(s)", keyword_count);

        // Process each unique keyword
        for keyword in &unique_keywords {
            let (node_name, storager_addr) = self
                .get_storager_for_keyword(keyword)
                .ok_or_else(|| Status::internal("No storager available"))?;

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
                self.update_root_hash(node_name, resp.root_hash);
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

        // Deduplicate old and new keywords
        let unique_old_keywords: HashSet<String> = req.old_keywords.into_iter().collect();
        let unique_new_keywords: HashSet<String> = req.new_keywords.into_iter().collect();
        
        println!("  Deleting {} unique old keyword(s)", unique_old_keywords.len());
        println!("  Adding {} unique new keyword(s)", unique_new_keywords.len());

        // Delete old keywords
        for keyword in &unique_old_keywords {
            let (node_name, storager_addr) = self
                .get_storager_for_keyword(keyword)
                .ok_or_else(|| Status::internal("No storager available"))?;

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
                self.update_root_hash(node_name, resp.root_hash);
            }
        }

        // Add new keywords
        for keyword in &unique_new_keywords {
            let (node_name, storager_addr) = self
                .get_storager_for_keyword(keyword)
                .ok_or_else(|| Status::internal("No storager available"))?;

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
                self.update_root_hash(node_name, resp.root_hash);
            }
        }

        Ok(Response::new(UpdateResponse {
            success: true,
            message: "Update operation completed successfully".to_string(),
        }))
    }
}

impl Manager {
    /// 单关键词查询
    pub(crate) async fn query_single_keyword(
        &self,
        keyword: &str,
    ) -> Result<Response<QueryResponse>, Status> {
        println!("  Query type: Single keyword '{}'", keyword);

        let (node_name, storager_addr) = self
            .get_storager_for_keyword(keyword)
            .ok_or_else(|| Status::internal("No storager available"))?;

        // Connect to storager and send Query request
        let mut client = StoragerServiceClient::connect(storager_addr.clone())
            .await
            .map_err(|e| Status::internal(format!("Failed to connect to storager: {}", e)))?;

        let storager_req = StoragerQueryRequest {
            keyword: keyword.to_string(),
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
            .get(&node_name)
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

    /// 布尔函数查询
    pub(crate) async fn query_boolean_function(
        &self,
        func: &str,
    ) -> Result<Response<QueryResponse>, Status> {
        println!("  Query type: Boolean function '{}'", func);

        // 1. 解析布尔表达式
        let expr = parse_boolean_expr(func).map_err(|e| {
            Status::invalid_argument(format!("Failed to parse boolean expression: {}", e))
        })?;

        println!("  Parsed expression: {}", expr.to_string());

        // 2. 获取所有关键词
        let keywords = expr.get_keywords();
        println!("  Keywords: {:?}", keywords);

        // 3. 并发查询所有关键词
        let mut keyword_results = HashMap::new();
        let mut all_proofs = Vec::new();

        for keyword in keywords.iter() {
            let (node_name, storager_addr) = self
                .get_storager_for_keyword(keyword)
                .ok_or_else(|| Status::internal("No storager available"))?;

            // Connect to storager
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
                .get(&node_name)
                .cloned()
                .unwrap_or_default();

            // Verify individual proof
            if !self.verify_proof(&resp.proof, &root_hash) {
                return Err(Status::internal(format!(
                    "Proof verification failed for keyword: {}",
                    keyword
                )));
            }

            // 存储查询结果
            let fid_set: HashSet<String> = resp.fids.into_iter().collect();
            keyword_results.insert(keyword.clone(), fid_set);

            // 收集证明
            all_proofs.push(resp.proof);

            println!(
                "    '{}' -> {} files",
                keyword,
                keyword_results.get(keyword).unwrap().len()
            );
        }

        // 4. 对布尔表达式求值
        let result_set = expr.evaluate(&keyword_results);
        let result_fids: Vec<String> = result_set.into_iter().collect();

        println!("  Final result: {} files", result_fids.len());

        // 5. 生成组合证明
        let combined_proof = self.combine_proofs(&all_proofs);

        // 6. 使用第一个 storager 的 root hash 作为代表
        let root_hash = self
            .root_hashes
            .read()
            .unwrap()
            .values()
            .next()
            .cloned()
            .unwrap_or_default();

        Ok(Response::new(QueryResponse {
            fids: result_fids,
            proof: combined_proof,
            root_hash,
            verified: true, // 已经验证过各个子查询的证明
        }))
    }
}
