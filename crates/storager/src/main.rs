//! Storager 服务入口
//!
//! 存储节点负责：
//! - 管理特定分片的数据
//! - 维护认证数据结构 (ADS)
//! - 生成和验证密码学证明
//!
//! # 使用方法
//! ```bash
//! # 使用默认 ADS (CryptoAccumulator) 和端口 50052
//! cargo run --bin storager
//!
//! # 指定端口
//! cargo run --bin storager -- 50053
//!
//! # 指定 ADS 类型和端口
//! cargo run --bin storager -- 50053 mpt
//! cargo run --bin storager -- 50053 accumulator
//! ```

use common::rpc::storager_service_server::StoragerServiceServer;
use storager::Storager;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 解析命令行参数
    let args: Vec<String> = std::env::args().collect();

    // 第一个参数：端口号（默认 50052）
    let port = if args.len() > 1 {
        args[1].parse::<u16>().unwrap_or(50052)
    } else {
        50052
    };

    // 第二个参数：ADS 类型（默认 accumulator）
    let ads_type = if args.len() > 2 {
        args[2].as_str()
    } else {
        "accumulator"
    };

    let addr = format!("[::1]:{}", port).parse()?;

    // 根据配置创建 Storager 实例
    let storager = Storager::from_config(ads_type);

    println!(
        "🚀 Storager server listening on {} (ADS: {})",
        addr, ads_type
    );

    Server::builder()
        .add_service(StoragerServiceServer::new(storager))
        .serve(addr)
        .await?;

    Ok(())
}
