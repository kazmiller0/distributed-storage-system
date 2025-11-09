//! Storager 服务入口
//!
//! 存储节点负责：
//! - 管理特定分片的数据
//! - 维护认证数据结构 (ADS)
//! - 生成和验证密码学证明

use common::rpc::storager_service_server::StoragerServiceServer;
use storager::Storager;
use tonic::transport::Server;

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
