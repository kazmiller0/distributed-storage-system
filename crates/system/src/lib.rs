//! System-level helper library for the distributed-storage-system workspace.
//!
//! 提供整个分布式存储系统的初始化与配置读写工具：
//! - `initialize` 用于根据参数构造 `SystemConfig`
//! - `load_config` / `save_config` 用于从文件加载和保存配置

use common::{AdsMode, SystemConfig};
use std::error::Error;

/// Initialize the distributed storage system
///
/// 当前实现仅构造并返回 `SystemConfig`，不负责真正拉起各个进程。
/// 可以在测试或实验脚本中复用。
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
            AdsMode::CryptoAccumulator,
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
