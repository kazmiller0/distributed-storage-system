//! # Consistent Hash Ring
//!
//! 一个独立的、高性能的一致性哈希环实现，支持虚拟节点。
//!
//! ## 特性
//!
//! - ✅ 标准的一致性哈希环算法
//! - ✅ 虚拟节点支持，实现负载均衡
//! - ✅ 动态添加/删除节点
//! - ✅ 最小化数据迁移
//! - ✅ 线程安全
//! - ✅ 零依赖核心实现
//!
//! ## 快速开始
//!
//! ```rust
//! use consistent_hash::ConsistentHashRing;
//!
//! // 创建一个包含3个节点的哈希环，每个节点150个虚拟节点
//! let mut ring = ConsistentHashRing::new();
//! ring.add_node("node1", 150);
//! ring.add_node("node2", 150);
//! ring.add_node("node3", 150);
//!
//! // 查找键应该路由到哪个节点
//! let node = ring.get_node("my_key");
//! assert!(node.is_some());
//! ```
//!
//! ## 工作原理
//!
//! 一致性哈希通过将节点和键都映射到一个固定大小的哈希空间（环）上，
//! 然后通过顺时针查找的方式确定键应该路由到哪个节点。
//!
//! ```text
//!      0° ──────────────────────────── 360°
//!       │                               │
//!       ├─ VNode1.1 (Hash: 30)
//!       ├─ VNode2.1 (Hash: 85)
//!       ├─ VNode1.2 (Hash: 120)
//!       ├─ VNode3.1 (Hash: 200)
//!       ├─ VNode2.2 (Hash: 280)
//!       └─ VNode1.3 (Hash: 340)
//!
//! 查找 key "rust" (hash: 150):
//!   -> 顺时针找最近的节点
//!   -> VNode3.1 (200)
//!   -> 映射到 Node3
//! ```

use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};

/// 哈希函数类型
type HashValue = u64;

/// 一致性哈希环
///
/// 使用虚拟节点实现负载均衡的一致性哈希环。
///
/// # 示例
///
/// ```
/// use consistent_hash::ConsistentHashRing;
///
/// let mut ring = ConsistentHashRing::new();
/// ring.add_node("server1", 100);
/// ring.add_node("server2", 100);
///
/// let node = ring.get_node("user123").unwrap();
/// println!("user123 应该路由到: {}", node);
/// ```
#[derive(Debug, Clone)]
pub struct ConsistentHashRing {
    /// 哈希环: hash_value -> 物理节点名称
    ring: BTreeMap<HashValue, String>,

    /// 节点及其虚拟节点数量: node_name -> virtual_node_count
    nodes: HashMap<String, usize>,

    /// 虚拟节点到物理节点的映射: virtual_node_key -> physical_node_name
    virtual_to_physical: HashMap<String, String>,
}

impl ConsistentHashRing {
    /// 创建一个新的空哈希环
    ///
    /// # 示例
    ///
    /// ```
    /// use consistent_hash::ConsistentHashRing;
    ///
    /// let ring = ConsistentHashRing::new();
    /// ```
    pub fn new() -> Self {
        ConsistentHashRing {
            ring: BTreeMap::new(),
            nodes: HashMap::new(),
            virtual_to_physical: HashMap::new(),
        }
    }

    /// 使用默认虚拟节点数创建哈希环并添加节点
    ///
    /// # 参数
    ///
    /// * `node_names` - 节点名称列表
    /// * `virtual_nodes_per_node` - 每个节点的虚拟节点数量（推荐 100-200）
    ///
    /// # 示例
    ///
    /// ```
    /// use consistent_hash::ConsistentHashRing;
    ///
    /// let nodes = vec!["node1", "node2", "node3"];
    /// let ring = ConsistentHashRing::with_nodes(&nodes, 150);
    /// ```
    pub fn with_nodes(node_names: &[&str], virtual_nodes_per_node: usize) -> Self {
        let mut ring = Self::new();
        for name in node_names {
            ring.add_node(name, virtual_nodes_per_node);
        }
        ring
    }

    /// 计算字符串的哈希值
    ///
    /// 使用 Rust 标准库的 DefaultHasher，提供良好的分布性
    fn hash<T: Hash>(key: &T) -> HashValue {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// 添加一个节点到哈希环
    ///
    /// # 参数
    ///
    /// * `node_name` - 节点名称（必须唯一）
    /// * `virtual_nodes` - 该节点的虚拟节点数量
    ///
    /// # 返回
    ///
    /// * `true` - 成功添加
    /// * `false` - 节点已存在
    ///
    /// # 示例
    ///
    /// ```
    /// use consistent_hash::ConsistentHashRing;
    ///
    /// let mut ring = ConsistentHashRing::new();
    /// assert!(ring.add_node("node1", 150));
    /// assert!(!ring.add_node("node1", 150)); // 重复添加失败
    /// ```
    pub fn add_node(&mut self, node_name: &str, virtual_nodes: usize) -> bool {
        // 检查节点是否已存在
        if self.nodes.contains_key(node_name) {
            return false;
        }

        // 添加虚拟节点到环上
        for i in 0..virtual_nodes {
            let virtual_key = format!("{}#vnode{}", node_name, i);
            let hash = Self::hash(&virtual_key);
            self.ring.insert(hash, node_name.to_string());
            self.virtual_to_physical
                .insert(virtual_key, node_name.to_string());
        }

        // 记录节点信息
        self.nodes.insert(node_name.to_string(), virtual_nodes);
        true
    }

    /// 从哈希环中移除一个节点
    ///
    /// # 参数
    ///
    /// * `node_name` - 要移除的节点名称
    ///
    /// # 返回
    ///
    /// * `true` - 成功移除
    /// * `false` - 节点不存在
    ///
    /// # 示例
    ///
    /// ```
    /// use consistent_hash::ConsistentHashRing;
    ///
    /// let mut ring = ConsistentHashRing::new();
    /// ring.add_node("node1", 150);
    /// assert!(ring.remove_node("node1"));
    /// assert!(!ring.remove_node("node1")); // 已经移除
    /// ```
    pub fn remove_node(&mut self, node_name: &str) -> bool {
        // 检查节点是否存在
        let virtual_count = match self.nodes.remove(node_name) {
            Some(count) => count,
            None => return false,
        };

        // 移除所有虚拟节点
        for i in 0..virtual_count {
            let virtual_key = format!("{}#vnode{}", node_name, i);
            let hash = Self::hash(&virtual_key);
            self.ring.remove(&hash);
            self.virtual_to_physical.remove(&virtual_key);
        }

        true
    }

    /// 获取键应该路由到的节点
    ///
    /// 使用顺时针查找算法，找到第一个哈希值大于等于键的哈希值的虚拟节点，
    /// 然后返回对应的物理节点。
    ///
    /// # 参数
    ///
    /// * `key` - 要查找的键
    ///
    /// # 返回
    ///
    /// * `Some(node_name)` - 找到的节点名称
    /// * `None` - 环为空
    ///
    /// # 示例
    ///
    /// ```
    /// use consistent_hash::ConsistentHashRing;
    ///
    /// let mut ring = ConsistentHashRing::new();
    /// ring.add_node("node1", 150);
    /// ring.add_node("node2", 150);
    ///
    /// let node = ring.get_node("my_key");
    /// assert!(node.is_some());
    /// ```
    pub fn get_node(&self, key: &str) -> Option<String> {
        if self.ring.is_empty() {
            return None;
        }

        let hash = Self::hash(&key);

        // 在环上顺时针查找第一个大于等于 hash 的虚拟节点
        self.ring
            .range(hash..)
            .next()
            // 如果没找到，说明 hash 在环的末尾，回到开头（环形结构）
            .or_else(|| self.ring.iter().next())
            .map(|(_, node_name)| node_name.clone())
    }

    /// 获取多个副本节点
    ///
    /// 对于需要数据冗余的场景，返回多个不同的物理节点。
    ///
    /// # 参数
    ///
    /// * `key` - 要查找的键
    /// * `count` - 需要的副本数量
    ///
    /// # 返回
    ///
    /// 返回最多 `count` 个不同的物理节点（去重）
    ///
    /// # 示例
    ///
    /// ```
    /// use consistent_hash::ConsistentHashRing;
    ///
    /// let mut ring = ConsistentHashRing::new();
    /// ring.add_node("node1", 150);
    /// ring.add_node("node2", 150);
    /// ring.add_node("node3", 150);
    ///
    /// // 获取3个副本节点
    /// let replicas = ring.get_nodes("my_key", 3);
    /// assert!(replicas.len() <= 3);
    /// ```
    pub fn get_nodes(&self, key: &str, count: usize) -> Vec<String> {
        if self.ring.is_empty() || count == 0 {
            return Vec::new();
        }

        let hash = Self::hash(&key);
        let mut result = Vec::new();
        let mut seen = HashSet::new();

        // 从 hash 位置开始顺时针遍历
        for (_, node_name) in self.ring.range(hash..) {
            if !seen.contains(node_name) {
                result.push(node_name.clone());
                seen.insert(node_name.clone());
                if result.len() >= count {
                    return result;
                }
            }
        }

        // 如果还没找够，从头开始找（环形）
        for (_, node_name) in self.ring.iter() {
            if !seen.contains(node_name) {
                result.push(node_name.clone());
                seen.insert(node_name.clone());
                if result.len() >= count {
                    return result;
                }
            }
        }

        result
    }

    /// 获取环中所有物理节点的名称
    ///
    /// # 示例
    ///
    /// ```
    /// use consistent_hash::ConsistentHashRing;
    ///
    /// let mut ring = ConsistentHashRing::new();
    /// ring.add_node("node1", 150);
    /// ring.add_node("node2", 150);
    ///
    /// let nodes = ring.get_all_nodes();
    /// assert_eq!(nodes.len(), 2);
    /// ```
    pub fn get_all_nodes(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }

    /// 获取环中物理节点的数量
    ///
    /// # 示例
    ///
    /// ```
    /// use consistent_hash::ConsistentHashRing;
    ///
    /// let mut ring = ConsistentHashRing::new();
    /// ring.add_node("node1", 150);
    /// ring.add_node("node2", 150);
    ///
    /// assert_eq!(ring.node_count(), 2);
    /// ```
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 获取环中虚拟节点的总数
    ///
    /// # 示例
    ///
    /// ```
    /// use consistent_hash::ConsistentHashRing;
    ///
    /// let mut ring = ConsistentHashRing::new();
    /// ring.add_node("node1", 100);
    /// ring.add_node("node2", 150);
    ///
    /// assert_eq!(ring.virtual_node_count(), 250);
    /// ```
    pub fn virtual_node_count(&self) -> usize {
        self.ring.len()
    }

    /// 获取指定节点的虚拟节点数量
    ///
    /// # 示例
    ///
    /// ```
    /// use consistent_hash::ConsistentHashRing;
    ///
    /// let mut ring = ConsistentHashRing::new();
    /// ring.add_node("node1", 100);
    ///
    /// assert_eq!(ring.get_virtual_node_count("node1"), Some(100));
    /// assert_eq!(ring.get_virtual_node_count("node2"), None);
    /// ```
    pub fn get_virtual_node_count(&self, node_name: &str) -> Option<usize> {
        self.nodes.get(node_name).copied()
    }

    /// 检查环是否为空
    ///
    /// # 示例
    ///
    /// ```
    /// use consistent_hash::ConsistentHashRing;
    ///
    /// let ring = ConsistentHashRing::new();
    /// assert!(ring.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// 计算键在节点间的分布情况
    ///
    /// 用于测试和调试，帮助评估负载均衡效果。
    ///
    /// # 参数
    ///
    /// * `keys` - 要测试的键列表
    ///
    /// # 返回
    ///
    /// HashMap<节点名称, 键的数量>
    ///
    /// # 示例
    ///
    /// ```
    /// use consistent_hash::ConsistentHashRing;
    ///
    /// let mut ring = ConsistentHashRing::new();
    /// ring.add_node("node1", 150);
    /// ring.add_node("node2", 150);
    ///
    /// let keys: Vec<String> = (0..1000).map(|i| format!("key{}", i)).collect();
    /// let distribution = ring.get_distribution(&keys);
    ///
    /// println!("Distribution: {:?}", distribution);
    /// ```
    pub fn get_distribution(&self, keys: &[String]) -> HashMap<String, usize> {
        let mut distribution = HashMap::new();

        for key in keys {
            if let Some(node) = self.get_node(key) {
                *distribution.entry(node).or_insert(0) += 1;
            }
        }

        distribution
    }
}

impl Default for ConsistentHashRing {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_empty_ring() {
        let ring = ConsistentHashRing::new();
        assert!(ring.is_empty());
        assert_eq!(ring.node_count(), 0);
        assert_eq!(ring.virtual_node_count(), 0);
    }

    #[test]
    fn test_add_node() {
        let mut ring = ConsistentHashRing::new();
        assert!(ring.add_node("node1", 100));
        assert_eq!(ring.node_count(), 1);
        assert_eq!(ring.virtual_node_count(), 100);

        // 重复添加应该失败
        assert!(!ring.add_node("node1", 100));
    }

    #[test]
    fn test_remove_node() {
        let mut ring = ConsistentHashRing::new();
        ring.add_node("node1", 100);
        assert!(ring.remove_node("node1"));
        assert!(ring.is_empty());

        // 移除不存在的节点应该失败
        assert!(!ring.remove_node("node2"));
    }

    #[test]
    fn test_get_node() {
        let mut ring = ConsistentHashRing::new();
        ring.add_node("node1", 150);
        ring.add_node("node2", 150);
        ring.add_node("node3", 150);

        let node = ring.get_node("test_key");
        assert!(node.is_some());
        assert!(["node1", "node2", "node3"].contains(&node.unwrap().as_str()));
    }

    #[test]
    fn test_get_node_empty_ring() {
        let ring = ConsistentHashRing::new();
        assert!(ring.get_node("test_key").is_none());
    }

    #[test]
    fn test_consistent_mapping() {
        let mut ring = ConsistentHashRing::new();
        ring.add_node("node1", 150);
        ring.add_node("node2", 150);

        // 同一个键应该总是映射到同一个节点
        let node1 = ring.get_node("test_key").unwrap();
        let node2 = ring.get_node("test_key").unwrap();
        assert_eq!(node1, node2);
    }

    #[test]
    fn test_get_nodes_replicas() {
        let mut ring = ConsistentHashRing::new();
        ring.add_node("node1", 150);
        ring.add_node("node2", 150);
        ring.add_node("node3", 150);

        let replicas = ring.get_nodes("test_key", 2);
        assert_eq!(replicas.len(), 2);

        // 确保副本是不同的节点
        assert_ne!(replicas[0], replicas[1]);
    }

    #[test]
    fn test_with_nodes() {
        let nodes = vec!["node1", "node2", "node3"];
        let ring = ConsistentHashRing::with_nodes(&nodes, 100);

        assert_eq!(ring.node_count(), 3);
        assert_eq!(ring.virtual_node_count(), 300);
    }

    #[test]
    fn test_distribution() {
        let mut ring = ConsistentHashRing::new();
        ring.add_node("node1", 150);
        ring.add_node("node2", 150);
        ring.add_node("node3", 150);

        // 生成1000个测试键
        let keys: Vec<String> = (0..1000).map(|i| format!("key{}", i)).collect();
        let distribution = ring.get_distribution(&keys);

        println!("Distribution: {:?}", distribution);

        // 每个节点应该得到一部分键
        assert_eq!(distribution.len(), 3);
        for (node, count) in distribution.iter() {
            println!(
                "{}: {} keys ({:.1}%)",
                node,
                count,
                (*count as f64 / 1000.0 * 100.0)
            );
        }

        // 检查分布的均衡性（允许一定偏差）
        let avg = 1000 / 3;
        for count in distribution.values() {
            let diff = (*count as i32 - avg as i32).abs();
            let deviation = diff as f64 / avg as f64;
            assert!(
                deviation < 0.3,
                "Distribution too unbalanced: {}",
                deviation
            );
        }
    }

    #[test]
    fn test_node_addition_minimal_disruption() {
        let mut ring = ConsistentHashRing::new();
        ring.add_node("node1", 150);
        ring.add_node("node2", 150);

        // 记录添加前的映射
        let keys: Vec<String> = (0..1000).map(|i| format!("key{}", i)).collect();
        let before: Vec<_> = keys.iter().map(|k| ring.get_node(k)).collect();

        // 添加新节点
        ring.add_node("node3", 150);

        // 记录添加后的映射
        let after: Vec<_> = keys.iter().map(|k| ring.get_node(k)).collect();

        // 计算变化的键数量
        let changed = before
            .iter()
            .zip(after.iter())
            .filter(|(b, a)| b != a)
            .count();

        let change_ratio = changed as f64 / keys.len() as f64;
        println!(
            "Changed: {} / {} ({:.1}%)",
            changed,
            keys.len(),
            change_ratio * 100.0
        );

        // 理论上应该只有 1/3 的键需要迁移（从2个节点变成3个节点）
        // 允许一定误差
        assert!(
            change_ratio < 0.5,
            "Too many keys changed: {:.1}%",
            change_ratio * 100.0
        );
    }

    #[test]
    fn test_virtual_node_count() {
        let mut ring = ConsistentHashRing::new();
        ring.add_node("node1", 100);
        ring.add_node("node2", 200);

        assert_eq!(ring.get_virtual_node_count("node1"), Some(100));
        assert_eq!(ring.get_virtual_node_count("node2"), Some(200));
        assert_eq!(ring.get_virtual_node_count("node3"), None);
    }
}
