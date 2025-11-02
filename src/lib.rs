use common::{AdsMode, SystemConfig};
use std::error::Error;

/// Initialize the distributed storage system
///
/// This function creates and starts:
/// - One Manager instance
/// - Multiple Storager instances based on `num_storagers`
/// - Multiple Client instances based on `num_clients`
///
/// # Arguments
/// * `num_clients` - Number of client instances to create
/// * `num_storagers` - Number of storager instances to create
/// * `ads_mode` - Type of authenticated data structure (MerkleTree or PatriciaTrie)
/// * `manager_addr` - Network address for the Manager
/// * `storager_addrs` - List of network addresses for Storagers
/// * `client_addrs` - List of network addresses for Clients (optional)
pub async fn initialize(
    num_clients: usize,
    num_storagers: usize,
    ads_mode: AdsMode,
    manager_addr: String,
    storager_addrs: Vec<String>,
    client_addrs: Vec<String>,
) -> Result<SystemConfig, Box<dyn Error>> {
    println!("Initializing distributed storage system...");
    println!("  Clients: {}", num_clients);
    println!("  Storagers: {}", num_storagers);
    println!("  ADS Mode: {:?}", ads_mode);
    println!("  Manager Address: {}", manager_addr);

    // Validate configuration
    if storager_addrs.len() != num_storagers {
        return Err("Number of storager addresses must match num_storagers".into());
    }

    if !client_addrs.is_empty() && client_addrs.len() != num_clients {
        return Err("Number of client addresses must match num_clients or be empty".into());
    }

    let config = SystemConfig {
        num_clients,
        num_storagers,
        ads_mode,
        manager_addr,
        storager_addrs,
        client_addrs,
    };

    println!("System initialized successfully!");

    Ok(config)
}

/// Load system configuration from a file
pub fn load_config(path: &str) -> Result<SystemConfig, Box<dyn Error>> {
    let content = std::fs::read_to_string(path)?;
    let config: SystemConfig = serde_json::from_str(&content)?;
    Ok(config)
}

/// Save system configuration to a file
pub fn save_config(config: &SystemConfig, path: &str) -> Result<(), Box<dyn Error>> {
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initialize() {
        let config = initialize(
            2,
            3,
            AdsMode::MerkleTree,
            "http://[::1]:50051".to_string(),
            vec![
                "http://[::1]:50052".to_string(),
                "http://[::1]:50053".to_string(),
                "http://[::1]:50054".to_string(),
            ],
            vec![],
        )
        .await;

        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.num_clients, 2);
        assert_eq!(config.num_storagers, 3);
    }
}
