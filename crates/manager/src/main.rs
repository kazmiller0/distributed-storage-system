use common::rpc::manager_service_server::ManagerServiceServer;
use common::AdsMode;
use manager::Manager;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;

    // TODO: Load from config
    let storager_addrs = vec![
        "http://[::1]:50052".to_string(),
        "http://[::1]:50053".to_string(),
    ];

    let manager = Manager::new(storager_addrs, AdsMode::CryptoAccumulator);

    println!(
        "Manager server listening on {} (ADS Mode: CryptoAccumulator)",
        addr
    );

    Server::builder()
        .add_service(ManagerServiceServer::new(manager))
        .serve(addr)
        .await?;

    Ok(())
}
