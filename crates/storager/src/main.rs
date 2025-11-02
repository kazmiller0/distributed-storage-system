//! Storager 服务实现
//!
//! 存储节点负责：
//! - 管理特定分片的数据
//! - 维护认证数据结构 (ADS)
//! - 生成和验证密码学证明

use common::rpc::{
    storager_service_server::{StoragerService, StoragerServiceServer},
    StoragerAddRequest, StoragerAddResponse, StoragerDeleteRequest, StoragerDeleteResponse,
    StoragerQueryRequest, StoragerQueryResponse,
};
use std::sync::{Arc, RwLock};
use tonic::{transport::Server, Request, Response, Status};

mod ads_trait;
mod ads;

use ads_trait::AdsOperations;
use ads::CryptoAccumulatorAds;

/// Storager 结构
///
/// 负责管理单个存储节点的 ADS 实例
pub struct Storager {
    ads: Arc<RwLock<Box<dyn AdsOperations>>>,
}

impl Storager {
    /// 创建新的 Storager 实例（使用密码学累加器）
    pub fn new() -> Self {
        let ads: Box<dyn AdsOperations> = Box::new(CryptoAccumulatorAds::new());

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

    let storager = Storager::new();

    println!("Storager server listening on {} (CryptoAccumulator)", addr);

    Server::builder()
        .add_service(StoragerServiceServer::new(storager))
        .serve(addr)
        .await?;

    Ok(())
}
