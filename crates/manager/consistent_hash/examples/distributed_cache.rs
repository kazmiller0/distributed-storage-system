//! 分布式缓存示例
//!
//! 演示如何使用一致性哈希环构建分布式缓存系统

use consistent_hash::ConsistentHashRing;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// 分布式缓存
struct DistributedCache {
    /// 一致性哈希环
    ring: Arc<RwLock<ConsistentHashRing>>,
    /// 模拟的缓存服务器存储
    servers: HashMap<String, Arc<RwLock<HashMap<String, String>>>>,
}

impl DistributedCache {
    /// 创建新的分布式缓存
    fn new(server_names: Vec<&str>) -> Self {
        let ring = ConsistentHashRing::with_nodes(&server_names, 150);
        let mut servers = HashMap::new();

        for name in server_names {
            servers.insert(name.to_string(), Arc::new(RwLock::new(HashMap::new())));
        }

        DistributedCache {
            ring: Arc::new(RwLock::new(ring)),
            servers,
        }
    }

    /// 设置缓存值
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        // 查找应该使用的服务器
        let server_name = self
            .ring
            .read()
            .unwrap()
            .get_node(key)
            .ok_or("No server available")?;

        // 写入对应的服务器
        if let Some(server_storage) = self.servers.get(&server_name) {
            server_storage
                .write()
                .unwrap()
                .insert(key.to_string(), value.to_string());
            println!("✅ SET {} = {} (服务器: {})", key, value, server_name);
            Ok(())
        } else {
            Err(format!("Server {} not found", server_name))
        }
    }

    /// 获取缓存值
    fn get(&self, key: &str) -> Option<String> {
        let server_name = self.ring.read().unwrap().get_node(key)?;

        if let Some(server_storage) = self.servers.get(&server_name) {
            let value = server_storage.read().unwrap().get(key).cloned();
            match &value {
                Some(v) => println!("✅ GET {} = {} (服务器: {})", key, v, server_name),
                None => println!("❌ GET {} (未找到, 服务器: {})", key, server_name),
            }
            value
        } else {
            None
        }
    }

    /// 添加缓存服务器
    fn add_server(&mut self, server_name: &str) {
        self.ring.write().unwrap().add_node(server_name, 150);
        self.servers.insert(
            server_name.to_string(),
            Arc::new(RwLock::new(HashMap::new())),
        );
        println!("🔧 添加服务器: {}", server_name);
    }

    /// 删除缓存服务器
    fn remove_server(&mut self, server_name: &str) {
        self.ring.write().unwrap().remove_node(server_name);
        self.servers.remove(server_name);
        println!("🔧 移除服务器: {}", server_name);
    }

    /// 获取统计信息
    fn get_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        for (server_name, storage) in &self.servers {
            let count = storage.read().unwrap().len();
            stats.insert(server_name.clone(), count);
        }
        stats
    }
}

fn main() {
    println!("=== 分布式缓存示例 ===\n");

    // 1. 创建包含3个服务器的缓存集群
    println!("1. 初始化缓存集群");
    let mut cache =
        DistributedCache::new(vec!["cache-server-1", "cache-server-2", "cache-server-3"]);
    println!(
        "   集群包含 {} 个服务器\n",
        cache.ring.read().unwrap().node_count()
    );

    // 2. 存储一些数据
    println!("2. 存储数据");
    cache.set("user:1001", "Alice").unwrap();
    cache.set("user:1002", "Bob").unwrap();
    cache.set("user:1003", "Charlie").unwrap();
    cache.set("session:abc123", "active").unwrap();
    cache.set("session:def456", "expired").unwrap();
    cache.set("product:2001", "Laptop").unwrap();
    cache.set("product:2002", "Phone").unwrap();
    println!();

    // 3. 读取数据
    println!("3. 读取数据");
    cache.get("user:1001");
    cache.get("session:abc123");
    cache.get("product:2001");
    cache.get("nonexistent");
    println!();

    // 4. 查看数据分布
    println!("4. 数据分布统计");
    let stats = cache.get_stats();
    for (server, count) in stats.iter() {
        println!("   {}: {} 个键", server, count);
    }
    println!();

    // 5. 添加新服务器
    println!("5. 扩展集群（添加新服务器）");
    cache.add_server("cache-server-4");
    println!(
        "   集群现有 {} 个服务器",
        cache.ring.read().unwrap().node_count()
    );
    println!();

    // 6. 验证数据仍然可以访问
    println!("6. 验证现有数据（部分键可能已迁移到新服务器）");
    cache.get("user:1001");
    cache.get("session:abc123");
    println!();

    // 7. 添加更多数据
    println!("7. 添加更多数据");
    cache.set("user:1004", "David").unwrap();
    cache.set("user:1005", "Eve").unwrap();
    println!();

    // 8. 最终统计
    println!("8. 最终数据分布");
    let final_stats = cache.get_stats();
    let total_keys: usize = final_stats.values().sum();
    println!("   总键数: {}", total_keys);
    for (server, count) in final_stats.iter() {
        let percentage = (*count as f64 / total_keys as f64) * 100.0;
        println!("   {}: {} 个键 ({:.1}%)", server, count, percentage);
    }

    println!("\n=== 示例结束 ===");
}
