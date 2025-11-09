use super::error::MPTError;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, RwLock};

/// FullNode 表示 MPT 中的分支节点
#[derive(Debug)]
pub struct FullNode {
    pub node_hash: [u8; 32],
    pub parent: Option<Arc<RwLock<ShortNode>>>,
    pub children: [Option<Arc<RwLock<ShortNode>>>; 16],
    pub children_hash: [Option<Vec<u8>>; 16],
    pub value: Option<Vec<u8>>,
    pub is_dirty: bool,
    pub to_del_map: HashMap<String, HashMap<String, u32>>,
    pub child_latches: [Arc<RwLock<()>>; 16],
}

/// ShortNode 表示 MPT 中的叶子节点或扩展节点
#[derive(Debug)]
pub struct ShortNode {
    pub node_hash: [u8; 32],
    pub prefix: String,
    pub parent: Option<Arc<RwLock<FullNode>>>,
    pub is_leaf: bool,
    pub is_dirty: bool,
    pub suffix: String,
    pub next_node: Option<Arc<RwLock<FullNode>>>,
    pub next_node_hash: [u8; 32],
    pub value: Option<Vec<u8>>,
    pub to_del_map: HashMap<String, HashMap<String, u32>>,
}

/// 用于序列化的 ShortNode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableShortNode {
    pub node_hash: [u8; 32],
    pub prefix: String,
    pub is_leaf: bool,
    pub suffix: String,
    pub next_node_hash: [u8; 32],
    pub value: Option<Vec<u8>>,
}

/// 用于序列化的 FullNode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableFullNode {
    pub node_hash: [u8; 32],
    pub children_hash: [Option<Vec<u8>>; 16],
    pub value: Option<Vec<u8>>,
}

impl Default for FullNode {
    fn default() -> Self {
        Self {
            node_hash: [0u8; 32],
            parent: None,
            children: Default::default(),
            children_hash: Default::default(),
            value: None,
            is_dirty: false,
            to_del_map: HashMap::new(),
            child_latches: [
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
                Arc::new(RwLock::new(())),
            ],
        }
    }
}

impl FullNode {
    pub fn new(
        children_array: [Option<Arc<RwLock<ShortNode>>>; 16],
        value: Option<Vec<u8>>,
        db: &mut dyn Database,
        cache: Option<&mut NodeCache>,
    ) -> Result<Arc<RwLock<Self>>, MPTError> {
        let mut node = Self {
            children: children_array,
            value: value.clone(),
            ..Default::default()
        };

        // 计算 children_hash
        for (i, child) in node.children.iter().enumerate() {
            if let Some(child_ref) = child {
                let child_guard = child_ref
                    .read()
                    .map_err(|_| MPTError::LockError("Failed to read child".to_string()))?;
                node.children_hash[i] = Some(child_guard.node_hash.to_vec());
            } else {
                node.children_hash[i] = None;
            }
        }

        // 计算节点哈希
        node.update_hash();

        let node_arc = Arc::new(RwLock::new(node));

        // 设置父节点关系
        {
            let node_guard = node_arc
                .read()
                .map_err(|_| MPTError::LockError("Failed to read node".to_string()))?;
            for child in node_guard.children.iter().flatten() {
                if let Ok(mut child_guard) = child.write() {
                    child_guard.parent = Some(Arc::downgrade(&node_arc).upgrade().unwrap());
                }
            }
        }

        // 保存到数据库或缓存
        let node_hash = {
            let node_guard = node_arc
                .read()
                .map_err(|_| MPTError::LockError("Failed to read node".to_string()))?;
            node_guard.node_hash
        };

        if let Some(cache) = cache {
            cache.insert_full_node(node_hash, node_arc.clone(), db)?;
        } else {
            let node_guard = node_arc
                .read()
                .map_err(|_| MPTError::LockError("Failed to read node".to_string()))?;
            let serialized = node_guard.serialize()?;
            db.put(&node_guard.node_hash, &serialized)?;
        }

        Ok(node_arc)
    }

    pub fn get_child(
        &mut self,
        index: usize,
        db: &mut dyn Database,
        mut cache: Option<&mut NodeCache>,
    ) -> Result<Option<Arc<RwLock<ShortNode>>>, MPTError> {
        if index >= 16 {
            return Ok(None);
        }

        if self.children[index].is_none() {
            if let Some(child_hash) = &self.children_hash[index] {
                // 尝试从缓存获取
                if let Some(ref mut cache_ref) = cache {
                    if let Some(child) = cache_ref
                        .get_short_node(child_hash.as_slice().try_into().unwrap_or([0u8; 32]))
                    {
                        self.children[index] = Some(child.clone());
                        return Ok(Some(child));
                    }
                }

                // 从数据库获取
                let data = db.get(child_hash)?;
                if let Some(data) = data {
                    let child = ShortNode::deserialize(&data)?;
                    let child_arc = Arc::new(RwLock::new(child));
                    self.children[index] = Some(child_arc.clone());

                    if let Some(ref mut cache_ref) = cache {
                        cache_ref.insert_short_node(
                            child_hash.as_slice().try_into().unwrap_or([0u8; 32]),
                            child_arc.clone(),
                            db,
                        )?;
                    }

                    return Ok(Some(child_arc));
                }
            }
        }

        Ok(self.children[index].clone())
    }

    pub fn update_hash(&mut self) {
        let mut hasher = Sha256::new();

        // 添加所有子节点哈希
        for child_hash in &self.children_hash {
            if let Some(hash) = child_hash {
                hasher.update(hash);
            }
        }

        // 添加值
        if let Some(value) = &self.value {
            hasher.update(value);
        }

        self.node_hash = hasher.finalize().into();
    }

    pub fn serialize(&self) -> Result<Vec<u8>, MPTError> {
        let serializable = SerializableFullNode {
            node_hash: self.node_hash,
            children_hash: self.children_hash.clone(),
            value: self.value.clone(),
        };

        Ok(serde_json::to_vec(&serializable)?)
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, MPTError> {
        let serializable: SerializableFullNode = serde_json::from_slice(data)?;

        Ok(Self {
            node_hash: serializable.node_hash,
            children_hash: serializable.children_hash,
            value: serializable.value,
            ..Default::default()
        })
    }
}

impl Default for ShortNode {
    fn default() -> Self {
        Self {
            node_hash: [0u8; 32],
            prefix: String::new(),
            parent: None,
            is_leaf: false,
            is_dirty: false,
            suffix: String::new(),
            next_node: None,
            next_node_hash: [0u8; 32],
            value: None,
            to_del_map: HashMap::new(),
        }
    }
}

impl ShortNode {
    pub fn new(
        prefix: String,
        is_leaf: bool,
        suffix: String,
        next_node: Option<Arc<RwLock<FullNode>>>,
        value: Option<Vec<u8>>,
        db: &mut dyn Database,
        cache: Option<&mut NodeCache>,
    ) -> Result<Arc<RwLock<Self>>, MPTError> {
        let mut node = Self {
            prefix: prefix.clone(),
            is_leaf,
            suffix: suffix.clone(),
            next_node: next_node.clone(),
            value: value.clone(),
            ..Default::default()
        };

        // 设置 next_node_hash
        if let Some(next_ref) = &next_node {
            let next_guard = next_ref
                .read()
                .map_err(|_| MPTError::LockError("Failed to read next node".to_string()))?;
            node.next_node_hash = next_guard.node_hash;
        }

        // 计算节点哈希
        node.update_hash();

        let node_arc = Arc::new(RwLock::new(node));

        // 设置父节点关系
        if let Some(next_ref) = &next_node {
            if let Ok(mut next_guard) = next_ref.write() {
                next_guard.parent = Some(Arc::downgrade(&node_arc).upgrade().unwrap());
            }
        }

        // 保存到数据库或缓存
        let node_hash = {
            let node_guard = node_arc
                .read()
                .map_err(|_| MPTError::LockError("Failed to read node".to_string()))?;
            node_guard.node_hash
        };

        if let Some(cache) = cache {
            cache.insert_short_node(node_hash, node_arc.clone(), db)?;
        } else {
            let node_guard = node_arc
                .read()
                .map_err(|_| MPTError::LockError("Failed to read node".to_string()))?;
            let serialized = node_guard.serialize()?;
            db.put(&node_guard.node_hash, &serialized)?;
        }

        Ok(node_arc)
    }

    pub fn get_next_node(
        &mut self,
        db: &mut dyn Database,
        mut cache: Option<&mut NodeCache>,
    ) -> Result<Option<Arc<RwLock<FullNode>>>, MPTError> {
        if self.next_node.is_none() && self.next_node_hash != [0u8; 32] {
            // 尝试从缓存获取
            if let Some(ref mut cache_ref) = cache {
                if let Some(next) = cache_ref.get_full_node(self.next_node_hash) {
                    self.next_node = Some(next.clone());
                    return Ok(Some(next));
                }
            }

            // 从数据库获取
            let data = db.get(&self.next_node_hash)?;
            if let Some(data) = data {
                let next = FullNode::deserialize(&data)?;
                let next_arc = Arc::new(RwLock::new(next));
                self.next_node = Some(next_arc.clone());

                if let Some(ref mut cache_ref) = cache {
                    cache_ref.insert_full_node(self.next_node_hash, next_arc.clone(), db)?;
                }

                return Ok(Some(next_arc));
            }
        }

        Ok(self.next_node.clone())
    }

    pub fn update_hash(&mut self) {
        let mut hasher = Sha256::new();

        // 添加前缀和后缀
        hasher.update(self.prefix.as_bytes());
        hasher.update(self.suffix.as_bytes());

        if self.is_leaf {
            // 叶子节点：添加值
            if let Some(value) = &self.value {
                hasher.update(value);
            }
        } else {
            // 扩展节点：添加下一个节点的哈希
            hasher.update(&self.next_node_hash);
        }

        self.node_hash = hasher.finalize().into();
    }

    pub fn serialize(&self) -> Result<Vec<u8>, MPTError> {
        let serializable = SerializableShortNode {
            node_hash: self.node_hash,
            prefix: self.prefix.clone(),
            is_leaf: self.is_leaf,
            suffix: self.suffix.clone(),
            next_node_hash: self.next_node_hash,
            value: self.value.clone(),
        };

        Ok(serde_json::to_vec(&serializable)?)
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, MPTError> {
        let serializable: SerializableShortNode = serde_json::from_slice(data)?;

        Ok(Self {
            node_hash: serializable.node_hash,
            prefix: serializable.prefix,
            is_leaf: serializable.is_leaf,
            suffix: serializable.suffix,
            next_node_hash: serializable.next_node_hash,
            value: serializable.value,
            ..Default::default()
        })
    }
}

/// 数据库trait，抽象化数据库操作
pub trait Database {
    fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>, MPTError>;
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), MPTError>;
    fn delete(&mut self, key: &[u8]) -> Result<(), MPTError>;
}

/// 节点缓存，带淘汰回调功能
pub struct NodeCache {
    short_node_cache: LruCache<[u8; 32], Arc<RwLock<ShortNode>>>,
    full_node_cache: LruCache<[u8; 32], Arc<RwLock<FullNode>>>,
}

impl NodeCache {
    pub fn new(short_node_capacity: usize, full_node_capacity: usize) -> Self {
        Self {
            short_node_cache: LruCache::new(NonZeroUsize::new(short_node_capacity).unwrap()),
            full_node_cache: LruCache::new(NonZeroUsize::new(full_node_capacity).unwrap()),
        }
    }

    pub fn get_short_node(&mut self, hash: [u8; 32]) -> Option<Arc<RwLock<ShortNode>>> {
        self.short_node_cache.get(&hash).cloned()
    }

    /// 插入 ShortNode 到缓存，如果缓存满了，淘汰的节点会被写入数据库
    pub fn insert_short_node(
        &mut self,
        hash: [u8; 32],
        node: Arc<RwLock<ShortNode>>,
        db: &mut dyn Database,
    ) -> Result<(), MPTError> {
        // 检查是否会触发淘汰
        if self.short_node_cache.len() >= self.short_node_cache.cap().get() {
            // 手动淘汰最久未使用的节点
            if let Some((evicted_hash, evicted_node)) = self.short_node_cache.pop_lru() {
                // 在淘汰前将节点写入数据库
                if let Ok(guard) = evicted_node.read() {
                    let serialized = guard.serialize()?;
                    db.put(&evicted_hash, &serialized)?;
                    drop(guard);
                    println!("Evicted ShortNode {:x?} to database", &evicted_hash[..8]);
                }
            }
        }

        self.short_node_cache.put(hash, node);
        Ok(())
    }

    pub fn get_full_node(&mut self, hash: [u8; 32]) -> Option<Arc<RwLock<FullNode>>> {
        self.full_node_cache.get(&hash).cloned()
    }

    /// 插入 FullNode 到缓存，如果缓存满了，淘汰的节点会被写入数据库
    pub fn insert_full_node(
        &mut self,
        hash: [u8; 32],
        node: Arc<RwLock<FullNode>>,
        db: &mut dyn Database,
    ) -> Result<(), MPTError> {
        // 检查是否会触发淘汰
        if self.full_node_cache.len() >= self.full_node_cache.cap().get() {
            // 手动淘汰最久未使用的节点
            if let Some((evicted_hash, evicted_node)) = self.full_node_cache.pop_lru() {
                // 在淘汰前将节点写入数据库
                if let Ok(guard) = evicted_node.read() {
                    let serialized = guard.serialize()?;
                    db.put(&evicted_hash, &serialized)?;
                    drop(guard);
                    println!("Evicted FullNode {:x?} to database", &evicted_hash[..8]);
                }
            }
        }

        self.full_node_cache.put(hash, node);
        Ok(())
    }

    /// 清空缓存，所有节点写入数据库
    pub fn purge(&mut self, db: &mut dyn Database) -> Result<(), MPTError> {
        // 将所有 ShortNode 写入数据库
        while let Some((hash, node)) = self.short_node_cache.pop_lru() {
            if let Ok(guard) = node.read() {
                let serialized = guard.serialize()?;
                db.put(&hash, &serialized)?;
            }
        }

        // 将所有 FullNode 写入数据库
        while let Some((hash, node)) = self.full_node_cache.pop_lru() {
            if let Ok(guard) = node.read() {
                let serialized = guard.serialize()?;
                db.put(&hash, &serialized)?;
            }
        }

        self.short_node_cache.clear();
        self.full_node_cache.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// 测试用的内存数据库
    struct MemoryDatabase {
        data: HashMap<Vec<u8>, Vec<u8>>,
    }

    impl MemoryDatabase {
        fn new() -> Self {
            Self {
                data: HashMap::new(),
            }
        }
    }

    impl Database for MemoryDatabase {
        fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>, MPTError> {
            Ok(self.data.get(key).cloned())
        }

        fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), MPTError> {
            self.data.insert(key.to_vec(), value.to_vec());
            Ok(())
        }

        fn delete(&mut self, key: &[u8]) -> Result<(), MPTError> {
            self.data.remove(key);
            Ok(())
        }
    }

    #[test]
    fn test_short_node_creation() {
        let mut db = MemoryDatabase::new();
        let mut cache = NodeCache::new(100, 100);

        let node = ShortNode::new(
            "prefix".to_string(),
            true,
            "suffix".to_string(),
            None,
            Some(b"value".to_vec()),
            &mut db,
            Some(&mut cache),
        )
        .unwrap();

        let node_guard = node.read().unwrap();
        assert_eq!(node_guard.prefix, "prefix");
        assert_eq!(node_guard.suffix, "suffix");
        assert!(node_guard.is_leaf);
        assert_eq!(node_guard.value, Some(b"value".to_vec()));
    }

    #[test]
    fn test_full_node_creation() {
        let mut db = MemoryDatabase::new();
        let mut cache = NodeCache::new(100, 100);

        let children: [Option<Arc<RwLock<ShortNode>>>; 16] = Default::default();
        let node = FullNode::new(
            children,
            Some(b"branch_value".to_vec()),
            &mut db,
            Some(&mut cache),
        )
        .unwrap();

        let node_guard = node.read().unwrap();
        assert_eq!(node_guard.value, Some(b"branch_value".to_vec()));
    }

    #[test]
    fn test_node_serialization() {
        let short_node = ShortNode {
            prefix: "test".to_string(),
            is_leaf: true,
            suffix: "key".to_string(),
            value: Some(b"test_value".to_vec()),
            ..Default::default()
        };

        let serialized = short_node.serialize().unwrap();
        let deserialized = ShortNode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.prefix, "test");
        assert_eq!(deserialized.suffix, "key");
        assert!(deserialized.is_leaf);
        assert_eq!(deserialized.value, Some(b"test_value".to_vec()));
    }
}
