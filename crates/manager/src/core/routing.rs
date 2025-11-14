//! 路由模块
//!
//! 负责使用一致性哈希将关键字路由到对应的 storager 节点

use consistent_hash::ConsistentHashRing;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// 路由器结构
///
/// 管理一致性哈希环和 storager 地址映射
pub struct Router {
    /// 一致性哈希环
    hash_ring: Arc<RwLock<ConsistentHashRing>>,
    /// storager 名称到地址的映射
    storager_addrs: HashMap<String, String>,
}

impl Router {
    /// 创建新的路由器
    ///
    /// # Arguments
    /// * `storager_addrs` - storager 地址列表
    /// * `virtual_nodes_per_storager` - 每个 storager 的虚拟节点数量（默认 150）
    pub fn new(storager_addrs: Vec<String>, virtual_nodes_per_storager: usize) -> Self {
        let mut hash_ring = ConsistentHashRing::new();
        let mut addr_map = HashMap::new();

        // 为每个 storager 在哈希环上添加虚拟节点
        for (idx, addr) in storager_addrs.iter().enumerate() {
            let node_name = format!("storager-{}", idx);
            hash_ring.add_node(&node_name, virtual_nodes_per_storager);
            addr_map.insert(node_name, addr.clone());
        }

        Router {
            hash_ring: Arc::new(RwLock::new(hash_ring)),
            storager_addrs: addr_map,
        }
    }

    /// 获取关键字对应的 storager
    ///
    /// # Returns
    /// 返回 `Some((节点名称, 节点地址))` 或 `None`
    pub fn get_storager_for_keyword(&self, keyword: &str) -> Option<(String, String)> {
        let ring = self.hash_ring.read().unwrap();
        let node_name = ring.get_node(keyword)?;
        let addr = self.storager_addrs.get(&node_name)?.clone();
        Some((node_name, addr))
    }

    /// 添加新的 storager 节点
    pub fn add_storager(&mut self, addr: String, virtual_nodes: usize) {
        let idx = self.storager_addrs.len();
        let node_name = format!("storager-{}", idx);

        let mut ring = self.hash_ring.write().unwrap();
        ring.add_node(&node_name, virtual_nodes);
        self.storager_addrs.insert(node_name, addr);
    }

    /// 移除 storager 节点
    pub fn remove_storager(&mut self, node_name: &str) {
        let mut ring = self.hash_ring.write().unwrap();
        ring.remove_node(node_name);
        self.storager_addrs.remove(node_name);
    }

    /// 获取所有 storager 节点
    pub fn get_all_storagers(&self) -> Vec<(String, String)> {
        self.storager_addrs
            .iter()
            .map(|(name, addr)| (name.clone(), addr.clone()))
            .collect()
    }

    /// 获取 storager 数量
    pub fn storager_count(&self) -> usize {
        self.storager_addrs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_creation() {
        let addrs = vec![
            "http://[::1]:50052".to_string(),
            "http://[::1]:50053".to_string(),
        ];
        let router = Router::new(addrs, 150);
        assert_eq!(router.storager_count(), 2);
    }

    #[test]
    fn test_keyword_routing() {
        let addrs = vec![
            "http://[::1]:50052".to_string(),
            "http://[::1]:50053".to_string(),
        ];
        let router = Router::new(addrs, 150);

        // 同一个关键字应该总是路由到同一个 storager
        let result1 = router.get_storager_for_keyword("test");
        let result2 = router.get_storager_for_keyword("test");

        assert_eq!(result1, result2);
    }
}
