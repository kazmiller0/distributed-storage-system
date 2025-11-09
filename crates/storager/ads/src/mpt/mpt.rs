use super::error::MPTError;
use super::node::{Database, FullNode, NodeCache, ShortNode};
use super::proof::{MPTProof, ProofElement};
use super::utils::{byte_to_hex_index, common_prefix_len, key_to_hex_path, KVPair};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// MPT 元数据,用于持久化和恢复
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MPTMetadata {
    pub root_hash: [u8; 32],
    pub version: u32,
    pub timestamp: u64,
}

impl MPTMetadata {
    pub fn new(root_hash: [u8; 32]) -> Self {
        Self {
            root_hash,
            version: 1,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

pub struct MPT {
    pub root_hash: [u8; 32],
    pub root: Option<Arc<RwLock<FullNode>>>,
    pub cache: Option<Mutex<NodeCache>>, // 与 Go 的 *[]interface{} 类似地封装两个 LRU
    pub latch: Arc<RwLock<()>>,          // 用于根节点重构的读写锁
    pub update_latch: Arc<Mutex<()>>,    // 用于更新操作的互斥锁
}

impl Default for MPT {
    fn default() -> Self {
        Self {
            root_hash: [0u8; 32],
            root: None,
            cache: None,
            latch: Arc::new(RwLock::new(())),
            update_latch: Arc::new(Mutex::new(())),
        }
    }
}

impl MPT {
    pub fn new(cache: Option<NodeCache>) -> Self {
        Self {
            root_hash: [0u8; 32],
            root: None,
            cache: cache.map(Mutex::new),
            latch: Arc::new(RwLock::new(())),
            update_latch: Arc::new(Mutex::new(())),
        }
    }

    /// 获取根节点，如为空则尝试从 DB 以 root_hash 读取
    /// 使用 TryLock 确保只有一个线程进行重构
    pub fn get_root(
        &mut self,
        db: &mut dyn Database,
    ) -> Result<Option<Arc<RwLock<FullNode>>>, MPTError> {
        // 如果根节点已存在，直接返回
        if self.root.is_some() {
            return Ok(self.root.clone());
        }

        // 如果根哈希为空，说明 MPT 为空
        if self.root_hash == [0u8; 32] {
            return Ok(None);
        }

        // 尝试获取写锁进行根节点重构
        // 使用 try_write 避免阻塞，只允许一个线程进行重构
        if let Ok(_guard) = self.latch.try_write() {
            // 再次检查，防止在获取锁之前其他线程已经重构完成
            if self.root.is_some() {
                return Ok(self.root.clone());
            }

            // 从数据库加载根节点
            if let Some(data) = db.get(&self.root_hash)? {
                let node = FullNode::deserialize(&data)?;
                let node_arc = Arc::new(RwLock::new(node));
                self.root = Some(node_arc.clone());
                return Ok(Some(node_arc));
            }
        } else {
            // 其他线程正在重构，等待完成
            // 使用读锁等待写锁释放
            let _guard = self
                .latch
                .read()
                .map_err(|_| MPTError::LockError("Failed to acquire read lock".to_string()))?;
        }

        Ok(self.root.clone())
    }

    /// 创建空的根节点，使用锁保证线程安全
    fn ensure_root(&mut self, db: &mut dyn Database) -> Result<Arc<RwLock<FullNode>>, MPTError> {
        // 如果已存在，直接返回
        if let Some(root) = &self.root {
            return Ok(root.clone());
        }

        // 尝试获取写锁，只允许一个线程创建根节点
        if let Ok(_guard) = self.latch.try_write() {
            // 再次检查
            if let Some(root) = &self.root {
                return Ok(root.clone());
            }

            // 创建新的根节点
            let children: [Option<Arc<RwLock<ShortNode>>>; 16] = Default::default();
            let mut cache_opt = self.cache.as_ref().map(|m| m.lock().ok()).flatten();
            let node = FullNode::new(children, None, db, cache_opt.as_deref_mut())?;
            self.root = Some(node.clone());
            return Ok(node);
        } else {
            // 等待其他线程创建完成
            let _guard = self
                .latch
                .read()
                .map_err(|_| MPTError::LockError("Failed to acquire read lock".to_string()))?;

            if let Some(root) = &self.root {
                return Ok(root.clone());
            }
        }

        Err(MPTError::LockError(
            "Failed to ensure root node".to_string(),
        ))
    }

    /// 插入键值对 - 真正的 Patricia Trie 实现
    pub fn insert(
        &mut self,
        kv: KVPair,
        db: &mut dyn Database,
        is_primary: bool,
        flag: bool,
    ) -> Result<(String, bool), MPTError> {
        // 将键转换为十六进制路径
        let key_path = key_to_hex_path(kv.get_key());

        // 确保根节点存在
        let root = self.ensure_root(db)?;

        // 递归插入到 FullNode
        let (old_value, need_delete) = self.recursive_insert_full_node(
            &key_path,
            0, // 当前路径位置
            kv.get_value().as_bytes().to_vec(),
            root.clone(),
            db,
            is_primary,
            flag,
        )?;

        // 更新根哈希
        self.update_root_hash(root)?;

        Ok((old_value, need_delete))
    }

    /// 删除键值对
    pub fn delete(&mut self, key: &str, db: &mut dyn Database) -> Result<Option<String>, MPTError> {
        // 如果MPT为空，直接返回None
        if self.root.is_none() {
            return Ok(None);
        }

        let key_path = key_to_hex_path(key);
        let root = self.root.clone().unwrap();

        // 尝试删除键
        let deleted_value = self.recursive_delete_full_node(&key_path, 0, root.clone(), db)?;

        // 如果删除成功，更新根哈希
        if deleted_value.is_some() {
            // 检查根节点是否需要清理
            self.cleanup_root_if_needed(db)?;

            if let Some(root) = &self.root {
                self.update_root_hash(root.clone())?;
            } else {
                // 如果根节点被删除，重置根哈希
                self.root_hash = [0u8; 32];
            }
        }

        Ok(deleted_value)
    }

    /// 更新根哈希
    fn update_root_hash(&mut self, root: Arc<RwLock<FullNode>>) -> Result<(), MPTError> {
        let root_guard = root.read().map_err(|e| {
            MPTError::LockError(format!("Failed to lock root node for hash update: {}", e))
        })?;

        self.root_hash = root_guard.node_hash;
        Ok(())
    }

    /// 递归插入到 FullNode
    fn recursive_insert_full_node(
        &mut self,
        key_path: &[u8],
        pos: usize,
        value: Vec<u8>,
        full_node: Arc<std::sync::RwLock<FullNode>>,
        db: &mut dyn Database,
        is_primary: bool,
        flag: bool,
    ) -> Result<(String, bool), MPTError> {
        // 在获取 write lock 之前,先获取当前路径索引和对应的 child_latch
        // 避免在持有 write lock 的情况下再次获取 read lock
        let child_latch = if pos < key_path.len() {
            let index = byte_to_hex_index(key_path[pos]);
            let temp_guard = full_node.read().map_err(|_| {
                MPTError::LockError("Failed to read FullNode for child_latch".to_string())
            })?;
            Some(temp_guard.child_latches[index].clone())
        } else {
            None
        };

        // 在获取 write lock 之前先获取 child_latch,避免死锁
        let _child_guard =
            if let Some(ref latch) = child_latch {
                Some(latch.write().map_err(|_| {
                    MPTError::LockError("Failed to acquire child_latch".to_string())
                })?)
            } else {
                None
            };

        let mut guard = full_node
            .write()
            .map_err(|_| MPTError::LockError("Failed to write FullNode".to_string()))?;

        // 如果已经到达键的末尾，将值存储在当前 FullNode
        if pos >= key_path.len() {
            let old_value_str = if let Some(ref old_val) = guard.value {
                String::from_utf8_lossy(old_val).to_string()
            } else {
                String::new()
            };

            let mut is_change = false;
            let mut final_value = value.clone();

            if !is_primary {
                // 辅助索引: 需要追加或删除值
                let key_str = String::from_utf8_lossy(key_path).to_string();
                let value_str = String::from_utf8_lossy(&value).to_string();
                let mut kv = KVPair::new(key_str.clone(), old_value_str.clone());

                if flag {
                    // 删除模式
                    is_change = kv.del_value(&value_str);
                } else {
                    // 插入模式: 先检查延迟删除
                    let to_del_count = *guard
                        .to_del_map
                        .get(&key_str)
                        .and_then(|m| m.get(&value_str))
                        .unwrap_or(&0);

                    if to_del_count > 0 {
                        // 有延迟删除记录,抵消
                        if let Some(inner_map) = guard.to_del_map.get_mut(&key_str) {
                            if let Some(count) = inner_map.get_mut(&value_str) {
                                *count -= 1;
                                if *count == 0 {
                                    inner_map.remove(&value_str);
                                }
                            }
                            if inner_map.is_empty() {
                                guard.to_del_map.remove(&key_str);
                            }
                        }
                        return Ok((String::new(), false));
                    }

                    is_change = kv.add_value(&value_str);
                }

                if !is_change {
                    if flag {
                        // 删除操作但找不到值,记录延迟删除
                        guard
                            .to_del_map
                            .entry(key_str.clone())
                            .or_insert_with(HashMap::new)
                            .entry(value_str.clone())
                            .and_modify(|c| *c += 1)
                            .or_insert(1);
                    }
                    return Ok((String::new(), false));
                }

                final_value = kv.get_value().as_bytes().to_vec();
            } else {
                // 主索引: 直接覆盖值
                is_change = old_value_str.as_bytes() != value.as_slice();
            }

            guard.value = Some(final_value.clone());
            guard.is_dirty = true;

            // 主索引模式下返回旧值,辅助索引模式返回空字符串
            if is_primary {
                return Ok((old_value_str, is_change));
            } else {
                // 辅助索引:检查是否需要删除整个键(当值为空时)
                let need_delete = final_value.is_empty() || String::from_utf8_lossy(&final_value).trim().is_empty();
                return Ok((String::new(), need_delete));
            }
        }

        // 获取当前路径索引
        let index = byte_to_hex_index(key_path[pos]);

        // 如果对应索引的子节点不存在
        if guard.children[index].is_none() {
            // 如果是辅助索引的删除操作,记录延迟删除而不是创建节点
            if !is_primary && flag {
                let key_str = String::from_utf8_lossy(key_path).to_string();
                let value_str = String::from_utf8_lossy(&value).to_string();

                guard
                    .to_del_map
                    .entry(key_str.clone())
                    .or_insert_with(HashMap::new)
                    .entry(value_str.clone())
                    .and_modify(|c| *c += 1)
                    .or_insert(1);

                return Ok((String::new(), false));
            }

            // 如果是辅助索引的插入操作,先检查是否有延迟删除需要抵消
            if !is_primary {
                let key_str = String::from_utf8_lossy(key_path).to_string();
                let value_str = String::from_utf8_lossy(&value).to_string();

                let to_del_count = guard
                    .to_del_map
                    .get(&key_str)
                    .and_then(|m| m.get(&value_str))
                    .copied()
                    .unwrap_or(0);

                if to_del_count > 0 {
                    // 有延迟删除记录,抵消
                    if let Some(inner_map) = guard.to_del_map.get_mut(&key_str) {
                        if let Some(count) = inner_map.get_mut(&value_str) {
                            *count -= 1;
                            if *count == 0 {
                                inner_map.remove(&value_str);
                            }
                        }
                        if inner_map.is_empty() {
                            guard.to_del_map.remove(&key_str);
                        }
                    }
                    // 插入和删除相互抵消,不创建节点
                    return Ok((String::new(), false));
                }
            }

            // 创建新的叶子节点
            let remaining_path = &key_path[pos + 1..];
            let suffix = if remaining_path.is_empty() {
                String::new()
            } else {
                // 将剩余路径转换为字符串表示（用于调试）
                remaining_path
                    .iter()
                    .map(|&b| format!("{:x}", b))
                    .collect::<String>()
            };

            let leaf_node = ShortNode::new(
                format!("{}", key_path[pos]), // prefix: 当前字符
                true,                         // is_leaf
                suffix.clone(),
                None, // next_node
                Some(value.clone()),
                db,
                None, // cache暂不使用
            )?;

            // 从父节点继承相关的延迟删除记录
            // leaf_prefix 是从根到当前叶子节点的完整路径(到pos+1)
            let leaf_prefix = String::from_utf8_lossy(&key_path[..=pos]).to_string();

            let mut to_delete_keys = Vec::new();
            for (del_key, del_map) in guard.to_del_map.iter() {
                // 如果 del_key 以 leaf_prefix 开头,说明这个延迟删除记录会路由到新叶子节点
                if del_key.starts_with(&leaf_prefix) {
                    if let Ok(mut leaf_guard) = leaf_node.write() {
                        leaf_guard
                            .to_del_map
                            .insert(del_key.clone(), del_map.clone());
                    }
                    to_delete_keys.push(del_key.clone());
                }
            }

            // 从父节点删除已经移动的延迟删除记录
            for key in to_delete_keys {
                guard.to_del_map.remove(&key);
            }

            guard.children[index] = Some(leaf_node.clone());
            if let Ok(leaf_guard) = leaf_node.read() {
                guard.children_hash[index] = Some(leaf_guard.node_hash.to_vec());
            }
            guard.is_dirty = true;
            guard.update_hash();

            return Ok((String::new(), false));
        }

        // 子节点存在，需要递归处理
        let child_node = guard.children[index].clone().unwrap();
        drop(guard); // 释放锁避免死锁
        drop(_child_guard); // 释放 child_latch

        let mut child_guard = child_node
            .write()
            .map_err(|_| MPTError::LockError("Failed to write ShortNode".to_string()))?;

        if child_guard.is_leaf {
            // 如果是叶子节点，需要检查键是否匹配
            let current_key_suffix: Vec<u8> = key_path[pos + 1..].to_vec();
            let stored_suffix: Vec<u8> = child_guard
                .suffix
                .chars()
                .filter_map(|c| c.to_digit(16))
                .map(|d| d as u8)
                .collect();

            if current_key_suffix == stored_suffix {
                // 键完全匹配，根据 is_primary 决定是覆盖还是追加值
                let old_value_str = if let Some(ref old_val) = child_guard.value {
                    String::from_utf8_lossy(old_val).to_string()
                } else {
                    String::new()
                };

                let mut is_change = false;
                let mut final_value = value.clone();

                if !is_primary {
                    // 辅助索引: 需要追加或删除值
                    let key_str = String::from_utf8_lossy(key_path).to_string();
                    let value_str = String::from_utf8_lossy(&value).to_string();
                    let mut kv = KVPair::new(key_str.clone(), old_value_str.clone());

                    if flag {
                        // 删除模式: 从值列表中移除
                        is_change = kv.del_value(&value_str);
                    } else {
                        // 插入模式: 先检查延迟删除
                        let to_del_count = child_guard
                            .to_del_map
                            .get(&key_str)
                            .and_then(|m| m.get(&value_str))
                            .copied()
                            .unwrap_or(0);

                        if to_del_count > 0 {
                            // 有延迟删除记录,抵消
                            if let Some(inner_map) = child_guard.to_del_map.get_mut(&key_str) {
                                if let Some(count) = inner_map.get_mut(&value_str) {
                                    *count -= 1;
                                    if *count == 0 {
                                        inner_map.remove(&value_str);
                                    }
                                }
                                if inner_map.is_empty() {
                                    child_guard.to_del_map.remove(&key_str);
                                }
                            }
                            // 插入和删除相互抵消,不做任何改变
                            return Ok((String::new(), false));
                        }

                        // 正常追加值
                        is_change = kv.add_value(&value_str);
                    }

                    if !is_change {
                        // 值没有变化
                        if flag {
                            // 删除操作但找不到值,记录延迟删除
                            child_guard
                                .to_del_map
                                .entry(key_str.clone())
                                .or_insert_with(HashMap::new)
                                .entry(value_str.clone())
                                .and_modify(|c| *c += 1)
                                .or_insert(1);
                        }
                        return Ok((String::new(), false));
                    }

                    // 使用追加/删除后的新值
                    final_value = kv.get_value().as_bytes().to_vec();
                } else {
                    // 主索引: 直接覆盖值
                    is_change = old_value_str.as_bytes() != value.as_slice();
                }

                let had_value = !old_value_str.is_empty();
                
                // 在移动final_value之前计算need_delete
                let need_delete = if !is_primary {
                    final_value.is_empty() || String::from_utf8_lossy(&final_value).trim().is_empty()
                } else {
                    false
                };
                
                child_guard.value = Some(final_value);
                child_guard.is_dirty = true;
                child_guard.update_hash();

                // 更新父节点的子节点哈希
                let mut parent_guard = full_node
                    .write()
                    .map_err(|_| MPTError::LockError("Failed to update parent".to_string()))?;
                parent_guard.children_hash[index] = Some(child_guard.node_hash.to_vec());
                parent_guard.is_dirty = true;
                parent_guard.update_hash();

                // 主索引模式下返回旧值,辅助索引模式返回空字符串
                if is_primary {
                    Ok((old_value_str, is_change))
                } else {
                    // 辅助索引:使用之前计算的need_delete
                    Ok((String::new(), need_delete))
                }
            } else {
                // 键不匹配，需要进行节点分裂
                // 如果是辅助索引的删除操作,记录延迟删除
                if !is_primary && flag {
                    let key_str = String::from_utf8_lossy(key_path).to_string();
                    let value_str = String::from_utf8_lossy(&value).to_string();

                    child_guard
                        .to_del_map
                        .entry(key_str.clone())
                        .or_insert_with(HashMap::new)
                        .entry(value_str.clone())
                        .and_modify(|c| *c += 1)
                        .or_insert(1);

                    return Ok((String::new(), false));
                }

                let old_value = if let Some(ref old_val) = child_guard.value {
                    String::from_utf8_lossy(old_val).to_string()
                } else {
                    String::new()
                };

                let had_old_value = !old_value.is_empty();

                // 计算公共前缀长度
                let common_len = common_prefix_len(&current_key_suffix, &stored_suffix);

                if common_len == 0 {
                    // 没有公共前缀，需要创建分支节点来分离两个不同的键
                    let old_data = child_guard.value.clone();
                    drop(child_guard); // 释放原节点锁

                    // 创建新的分支节点
                    let new_branch = FullNode::new(Default::default(), None, db, None)?;

                    // 处理原有键
                    if !stored_suffix.is_empty() {
                        let old_index = byte_to_hex_index(stored_suffix[0]);
                        if stored_suffix.len() > 1 {
                            // 原有键还有剩余字符，创建叶子节点
                            let old_suffix_str: String = stored_suffix[1..]
                                .iter()
                                .map(|&b| format!("{:x}", b))
                                .collect();
                            let old_leaf = ShortNode::new(
                                String::new(),
                                true,
                                old_suffix_str,
                                None,
                                old_data.clone(),
                                db,
                                None,
                            )?;

                            {
                                let mut branch_guard = new_branch.write().unwrap();
                                let old_hash = old_leaf.read().unwrap().node_hash.to_vec();
                                branch_guard.children[old_index] = Some(old_leaf);
                                branch_guard.children_hash[old_index] = Some(old_hash);
                            }
                        } else {
                            // 原有键在此处结束，也创建一个叶子节点（空后缀）
                            let old_leaf = ShortNode::new(
                                String::new(),
                                true,
                                String::new(), // 空后缀
                                None,
                                old_data.clone(),
                                db,
                                None,
                            )?;

                            {
                                let mut branch_guard = new_branch.write().unwrap();
                                let old_hash = old_leaf.read().unwrap().node_hash.to_vec();
                                branch_guard.children[old_index] = Some(old_leaf);
                                branch_guard.children_hash[old_index] = Some(old_hash);
                            }
                        }
                    } else {
                        // stored_suffix 为空,说明原键已经在当前节点结束
                        // 将值直接存在branch节点上
                        let mut branch_guard = new_branch.write().unwrap();
                        branch_guard.value = old_data;
                    }

                    // 处理新键
                    let new_index = byte_to_hex_index(current_key_suffix[0]);
                    if current_key_suffix.len() > 1 {
                        // 新键还有剩余字符，创建叶子节点
                        let new_suffix_str: String = current_key_suffix[1..]
                            .iter()
                            .map(|&b| format!("{:x}", b))
                            .collect();
                        let new_leaf = ShortNode::new(
                            String::new(),
                            true,
                            new_suffix_str,
                            None,
                            Some(value),
                            db,
                            None,
                        )?;

                        {
                            let mut branch_guard = new_branch.write().unwrap();
                            let new_hash = new_leaf.read().unwrap().node_hash.to_vec();
                            branch_guard.children[new_index] = Some(new_leaf);
                            branch_guard.children_hash[new_index] = Some(new_hash);
                        }
                    } else {
                        // 新键在此处结束，也创建一个叶子节点（空后缀）
                        let new_leaf = ShortNode::new(
                            String::new(),
                            true,
                            String::new(), // 空后缀
                            None,
                            Some(value),
                            db,
                            None,
                        )?;

                        {
                            let mut branch_guard = new_branch.write().unwrap();
                            let new_hash = new_leaf.read().unwrap().node_hash.to_vec();
                            branch_guard.children[new_index] = Some(new_leaf);
                            branch_guard.children_hash[new_index] = Some(new_hash);
                        }
                    }

                    // 更新分支节点哈希并直接替换原叶子节点
                    new_branch.write().unwrap().update_hash();

                    // 直接将分支节点作为 extension node 替换原来的叶子节点
                    let extension_node = ShortNode::new(
                        String::new(),    // prefix
                        false,            // is_leaf: false 表示这是 extension node
                        String::new(),    // suffix
                        Some(new_branch), // next_node 指向分支节点
                        None,             // value
                        db,
                        None, // cache
                    )?;

                    // 获取 extension_node 的哈希
                    let ext_hash = extension_node.read().unwrap().node_hash.to_vec();

                    // 更新父节点
                    {
                        let mut parent_guard = full_node.write().map_err(|_| {
                            MPTError::LockError("Failed to update parent".to_string())
                        })?;
                        parent_guard.children[index] = Some(extension_node);
                        parent_guard.children_hash[index] = Some(ext_hash);
                        parent_guard.is_dirty = true;
                        // 父节点内容变化了，需要更新它的哈希
                        parent_guard.update_hash();
                    }

                    Ok((old_value, had_old_value))
                } else {
                    // 有公共前缀的情况：需要更复杂的分裂逻辑
                    // 例如：original = [1,2,3,4], new = [1,2,5,6], common_len = 2
                    // 需要创建分支节点在位置 2 处分离两条路径

                    let old_data = child_guard.value.clone();
                    drop(child_guard); // 释放原节点锁

                    // 创建分支节点来分离公共前缀之后的路径
                    let new_branch = FullNode::new(Default::default(), None, db, None)?;

                    // 处理原有键的剩余部分（从 common_len 开始）
                    let old_remaining = &stored_suffix[common_len..];
                    if old_remaining.is_empty() {
                        // 原有键正好在公共前缀处结束，设置分支节点的值
                        new_branch.write().unwrap().value = old_data;
                    } else {
                        // 原有键还有剩余，创建叶子节点
                        let old_index = byte_to_hex_index(old_remaining[0]);
                        if old_remaining.len() > 1 {
                            let old_suffix_str: String = old_remaining[1..]
                                .iter()
                                .map(|&b| format!("{:x}", b))
                                .collect();
                            let old_leaf = ShortNode::new(
                                String::new(),
                                true,
                                old_suffix_str,
                                None,
                                old_data,
                                db,
                                None,
                            )?;

                            let mut branch_guard = new_branch.write().unwrap();
                            let old_hash = old_leaf.read().unwrap().node_hash.to_vec();
                            branch_guard.children[old_index] = Some(old_leaf);
                            branch_guard.children_hash[old_index] = Some(old_hash);
                        } else {
                            // 只剩一个字符，创建空后缀的叶子节点
                            let old_leaf = ShortNode::new(
                                String::new(),
                                true,
                                String::new(),
                                None,
                                old_data,
                                db,
                                None,
                            )?;

                            let mut branch_guard = new_branch.write().unwrap();
                            let old_hash = old_leaf.read().unwrap().node_hash.to_vec();
                            branch_guard.children[old_index] = Some(old_leaf);
                            branch_guard.children_hash[old_index] = Some(old_hash);
                        }
                    }

                    // 处理新键的剩余部分（从 common_len 开始）
                    let new_remaining = &current_key_suffix[common_len..];
                    if new_remaining.is_empty() {
                        // 新键正好在公共前缀处结束，设置分支节点的值
                        new_branch.write().unwrap().value = Some(value.clone());
                    } else {
                        // 新键还有剩余，创建叶子节点
                        let new_index = byte_to_hex_index(new_remaining[0]);
                        if new_remaining.len() > 1 {
                            let new_suffix_str: String = new_remaining[1..]
                                .iter()
                                .map(|&b| format!("{:x}", b))
                                .collect();
                            let new_leaf = ShortNode::new(
                                String::new(),
                                true,
                                new_suffix_str,
                                None,
                                Some(value),
                                db,
                                None,
                            )?;

                            let mut branch_guard = new_branch.write().unwrap();
                            let new_hash = new_leaf.read().unwrap().node_hash.to_vec();
                            branch_guard.children[new_index] = Some(new_leaf);
                            branch_guard.children_hash[new_index] = Some(new_hash);
                        } else {
                            // 只剩一个字符，创建空后缀的叶子节点
                            let new_leaf = ShortNode::new(
                                String::new(),
                                true,
                                String::new(),
                                None,
                                Some(value),
                                db,
                                None,
                            )?;

                            let mut branch_guard = new_branch.write().unwrap();
                            let new_hash = new_leaf.read().unwrap().node_hash.to_vec();
                            branch_guard.children[new_index] = Some(new_leaf);
                            branch_guard.children_hash[new_index] = Some(new_hash);
                        }
                    }

                    // 更新分支节点哈希
                    new_branch.write().unwrap().update_hash();
                    let branch_hash = new_branch.read().unwrap().node_hash;

                    // 如果有公共前缀，需要创建 Extension node 来保存公共前缀
                    if common_len > 0 {
                        // 公共前缀字符串
                        let common_prefix_str: String = stored_suffix[..common_len]
                            .iter()
                            .map(|&b| format!("{:x}", b))
                            .collect();

                        // 创建 Extension node 保存公共前缀，指向分支节点
                        let extension_node = ShortNode::new(
                            String::new(),
                            false,
                            common_prefix_str,
                            Some(new_branch),
                            None,
                            db,
                            None,
                        )?;

                        // 获取 extension_node 的哈希
                        let ext_hash = extension_node.read().unwrap().node_hash.to_vec();

                        // 更新父节点
                        let mut parent_guard = full_node.write().map_err(|_| {
                            MPTError::LockError("Failed to update parent".to_string())
                        })?;
                        parent_guard.children[index] = Some(extension_node);
                        parent_guard.children_hash[index] = Some(ext_hash);
                        parent_guard.is_dirty = true;
                        parent_guard.update_hash();
                    } else {
                        // 没有公共前缀（这个分支实际上不会执行，因为 else 分支的条件）
                        let mut parent_guard = full_node.write().map_err(|_| {
                            MPTError::LockError("Failed to update parent".to_string())
                        })?;
                        parent_guard.children_hash[index] = Some(branch_hash.to_vec());
                        parent_guard.is_dirty = true;
                        parent_guard.update_hash();
                    }

                    Ok((old_value, had_old_value))
                }
            }
        } else {
            // Extension node，需要检查并消费后缀
            if let Some(next_node) = &child_guard.next_node {
                // 获取 Extension node 的后缀
                let ext_suffix: Vec<u8> = child_guard
                    .suffix
                    .chars()
                    .filter_map(|c| c.to_digit(16))
                    .map(|d| d as u8)
                    .collect();

                // 检查当前键路径是否与 Extension node 的后缀匹配
                let current_remaining = &key_path[pos + 1..];

                // 计算公共前缀长度
                let common_len = ext_suffix
                    .iter()
                    .zip(current_remaining.iter())
                    .take_while(|(a, b)| a == b)
                    .count();

                // 如果后缀完全匹配,可以继续递归
                if common_len == ext_suffix.len() {
                    let next_node_clone = next_node.clone();
                    let child_node_clone = child_node.clone();
                    let old_next_hash = child_guard.next_node_hash;
                    drop(child_guard);

                    // Extension node 消费后缀的所有字符 + 1（当前索引）
                    let result = self.recursive_insert_full_node(
                        key_path,
                        pos + 1 + ext_suffix.len(),
                        value,
                        next_node_clone.clone(),
                        db,
                        is_primary,
                        flag,
                    )?;

                    // 插入完成后，无条件检查子节点的哈希是否改变
                    let new_next_hash = next_node_clone.read().unwrap().node_hash;

                    if old_next_hash != new_next_hash {
                        // 子节点哈希改变了，需要更新 Extension node
                        let mut ext_guard = child_node_clone.write().unwrap();
                        ext_guard.next_node_hash = new_next_hash;
                        ext_guard.is_dirty = true;
                        ext_guard.update_hash();

                        // 更新父节点的 children_hash
                        let new_ext_hash = ext_guard.node_hash;
                        drop(ext_guard);

                        let mut parent_guard = full_node.write().unwrap();
                        parent_guard.children_hash[index] = Some(new_ext_hash.to_vec());
                        parent_guard.is_dirty = true;
                        parent_guard.update_hash();
                    }

                    Ok(result)
                } else if common_len == current_remaining.len() {
                    // 情况2: 新键被Extension的suffix完全包含
                    // 例如: Extension suffix="abc", 新键剩余="ab"
                    // 需要: 创建Branch节点,Extension节点移到Branch的某个分支,新键的value设为Branch的value

                    let common_prefix_str: String = ext_suffix[..common_len]
                        .iter()
                        .map(|&b| format!("{:x}", b))
                        .collect();

                    drop(child_guard);

                    // 更新原Extension节点: 消费公共前缀+1个字符
                    let mut ext_guard = child_node.write().unwrap();
                    let split_index = ext_suffix[common_len]; // Extension在split点的字符

                    // Extension新的suffix是去掉公共前缀和split字符后的部分
                    let new_ext_suffix_str: String = ext_suffix[common_len + 1..]
                        .iter()
                        .map(|&b| format!("{:x}", b))
                        .collect();

                    ext_guard.suffix = new_ext_suffix_str;
                    ext_guard.is_dirty = true;
                    ext_guard.update_hash();
                    let ext_hash = ext_guard.node_hash.to_vec();
                    drop(ext_guard);

                    // 创建新的Branch节点,包含原Extension节点和新键的value
                    let new_branch =
                        FullNode::new(Default::default(), Some(value.clone()), db, None)?;
                    {
                        let mut branch_guard = new_branch.write().unwrap();
                        branch_guard.children[byte_to_hex_index(split_index)] =
                            Some(child_node.clone());
                        branch_guard.children_hash[byte_to_hex_index(split_index)] = Some(ext_hash);
                        branch_guard.update_hash();
                    }

                    // 如果有公共前缀,创建新Extension节点
                    if common_len > 0 {
                        let new_extension = ShortNode::new(
                            String::new(),
                            false,
                            common_prefix_str,
                            Some(new_branch),
                            None,
                            db,
                            None,
                        )?;

                        let new_ext_hash = new_extension.read().unwrap().node_hash.to_vec();

                        // 更新父节点
                        let mut parent_guard = full_node.write().unwrap();
                        parent_guard.children[index] = Some(new_extension);
                        parent_guard.children_hash[index] = Some(new_ext_hash);
                        parent_guard.is_dirty = true;
                        parent_guard.update_hash();
                    } else {
                        // 没有公共前缀,直接用Branch节点替换Extension节点
                        let branch_hash = new_branch.read().unwrap().node_hash.to_vec();
                        let mut parent_guard = full_node.write().unwrap();
                        parent_guard.children_hash[index] = Some(branch_hash);
                        parent_guard.is_dirty = true;
                        parent_guard.update_hash();
                    }

                    Ok((String::new(), false))
                } else {
                    // 情况3: 部分匹配,需要split成两个分支
                    // 例如: Extension suffix="abc", 新键剩余="adc"
                    // 公共前缀="a", 然后Extension走'b', 新键走'd'

                    let common_prefix_str: String = ext_suffix[..common_len]
                        .iter()
                        .map(|&b| format!("{:x}", b))
                        .collect();

                    drop(child_guard);

                    // Extension节点在split点的字符
                    let ext_split_index = ext_suffix[common_len];

                    // 如果有公共前缀,需要修改原Extension节点
                    // 如果common_len=0,也需要修改原Extension,去掉第一个字符(因为会被Branch消费)
                    let ext_node_to_use = {
                        let mut ext_guard = child_node.write().unwrap();

                        // 去掉公共前缀和split字符(被Branch消费的字符)
                        let new_ext_suffix_str: String = ext_suffix[common_len + 1..]
                            .iter()
                            .map(|&b| format!("{:x}", b))
                            .collect();

                        ext_guard.suffix = new_ext_suffix_str;
                        ext_guard.is_dirty = true;
                        ext_guard.update_hash();
                        drop(ext_guard);

                        child_node.clone()
                    };

                    let ext_hash = ext_node_to_use.read().unwrap().node_hash.to_vec();

                    // 创建新的叶子节点
                    let new_key_split_index = current_remaining[common_len];
                    let new_key_remaining = &current_remaining[common_len + 1..];
                    let new_key_suffix_str: String = new_key_remaining
                        .iter()
                        .map(|&b| format!("{:x}", b))
                        .collect();

                    let new_leaf = ShortNode::new(
                        String::new(),
                        true,
                        new_key_suffix_str,
                        None,
                        Some(value.clone()),
                        db,
                        None,
                    )?;
                    let leaf_hash = new_leaf.read().unwrap().node_hash.to_vec();

                    // 创建新的Branch节点,包含原Extension节点和新叶子节点
                    let new_branch = FullNode::new(Default::default(), None, db, None)?;
                    {
                        let mut branch_guard = new_branch.write().unwrap();
                        branch_guard.children[byte_to_hex_index(ext_split_index)] =
                            Some(ext_node_to_use);
                        branch_guard.children_hash[byte_to_hex_index(ext_split_index)] =
                            Some(ext_hash);
                        branch_guard.children[byte_to_hex_index(new_key_split_index)] =
                            Some(new_leaf);
                        branch_guard.children_hash[byte_to_hex_index(new_key_split_index)] =
                            Some(leaf_hash);
                        branch_guard.update_hash();
                    }

                    // 如果有公共前缀,创建新Extension节点
                    if common_len > 0 {
                        let new_extension = ShortNode::new(
                            String::new(),
                            false,
                            common_prefix_str,
                            Some(new_branch),
                            None,
                            db,
                            None,
                        )?;

                        let new_ext_hash = new_extension.read().unwrap().node_hash.to_vec();

                        // 更新父节点
                        let mut parent_guard = full_node.write().unwrap();
                        parent_guard.children[index] = Some(new_extension);
                        parent_guard.children_hash[index] = Some(new_ext_hash);
                        parent_guard.is_dirty = true;
                        parent_guard.update_hash();
                    } else {
                        // 没有公共前缀,直接用Branch节点替换Extension节点
                        // 需要创建一个Extension节点(suffix为空)指向Branch
                        let wrapper_extension = ShortNode::new(
                            String::new(),
                            false,
                            String::new(), // 空suffix
                            Some(new_branch),
                            None,
                            db,
                            None,
                        )?;

                        let wrapper_hash = wrapper_extension.read().unwrap().node_hash.to_vec();
                        let mut parent_guard = full_node.write().unwrap();
                        parent_guard.children[index] = Some(wrapper_extension);
                        parent_guard.children_hash[index] = Some(wrapper_hash);
                        parent_guard.is_dirty = true;
                        parent_guard.update_hash();
                    }

                    Ok((String::new(), false))
                }
            } else {
                Err(MPTError::NodeNotFound)
            }
        }
    }

    /// 根据键查询值 - 真正的 Patricia Trie 查询
    pub fn query_by_key(
        &mut self,
        key: &str,
        db: &mut dyn Database,
    ) -> Result<(String, MPTProof), MPTError> {
        // 将键转换为十六进制路径
        let key_path = key_to_hex_path(key);

        if let Some(root) = self.get_root(db)? {
            self.recursive_query_full_node(&key_path, 0, 0, root, db)
        } else {
            let empty_proof = ProofElement::new(
                0,
                2,
                String::new(),
                String::new(),
                vec![],
                vec![],
                Default::default(),
            );
            Ok((String::new(), MPTProof::new(false, 0, vec![empty_proof])))
        }
    }

    /// Verify query result using the proof
    pub fn verify_query_result(&self, value: &str, mpt_proof: &MPTProof) -> bool {
        let computed_root = super::proof::compute_mpt_root(value, mpt_proof);

        if computed_root != self.root_hash {
            println!(
                "Root hash {:x?} verification failed, computed {:x?}",
                self.root_hash, computed_root
            );
            return false;
        }

        println!("Root hash {:x?} verified successfully", computed_root);
        true
    }

    /// 递归查询 FullNode
    fn recursive_query_full_node(
        &self,
        key_path: &[u8],
        pos: usize,
        level: u32,
        full_node: Arc<std::sync::RwLock<FullNode>>,
        db: &mut dyn Database,
    ) -> Result<(String, MPTProof), MPTError> {
        let guard = full_node
            .read()
            .map_err(|_| MPTError::LockError("Failed to read FullNode".to_string()))?;

        // 构造当前 FullNode 的证明元素
        let mut children_hashes: [Vec<u8>; 16] = Default::default();
        for (i, hash) in guard.children_hash.iter().enumerate() {
            if let Some(h) = hash {
                children_hashes[i] = h.clone();
            }
        }

        let proof_element = ProofElement::new(
            level,
            2, // FullNode 类型
            String::new(),
            String::new(),
            guard.value.clone().unwrap_or_default(),
            vec![],
            children_hashes,
        );

        // 如果已经到达键的末尾，检查当前节点是否有值
        if pos >= key_path.len() {
            if let Some(ref value) = guard.value {
                let value_str = String::from_utf8_lossy(value).to_string();
                return Ok((value_str, MPTProof::new(true, level, vec![proof_element])));
            } else {
                return Ok((
                    String::new(),
                    MPTProof::new(false, level, vec![proof_element]),
                ));
            }
        }

        // 获取当前路径索引
        let index = byte_to_hex_index(key_path[pos]);

        // 检查子节点是否存在
        let child_node = if let Some(child) = &guard.children[index] {
            // 子节点已经加载到内存
            let child_clone = child.clone();
            drop(guard);
            child_clone
        } else if let Some(child_hash) = &guard.children_hash[index] {
            // 子节点未加载,但哈希存在,从数据库中加载
            let child_hash_clone = child_hash.clone();
            drop(guard);
            if let Some(data) = db.get(&child_hash_clone)? {
                let short_node = ShortNode::deserialize(&data)?;
                let short_node_arc = Arc::new(RwLock::new(short_node));
                
                // 将加载的节点更新到父节点
                if let Ok(mut write_guard) = full_node.write() {
                    write_guard.children[index] = Some(short_node_arc.clone());
                }
                
                short_node_arc
            } else {
                // 数据库中找不到节点
                return Ok((
                    String::new(),
                    MPTProof::new(false, level, vec![proof_element]),
                ));
            }
        } else {
            // 子节点哈希不存在,说明键不存在
            return Ok((
                String::new(),
                MPTProof::new(false, level, vec![proof_element]),
            ));
        };

        let child_guard = child_node
            .read()
            .map_err(|_| MPTError::LockError("Failed to read ShortNode".to_string()))?;

        if child_guard.is_leaf {
            // 叶子节点，检查后缀是否匹配
            let current_key_suffix: Vec<u8> = key_path[pos + 1..].to_vec();
            let stored_suffix: Vec<u8> = child_guard
                .suffix
                .chars()
                .filter_map(|c| c.to_digit(16))
                .map(|d| d as u8)
                .collect();

            let leaf_proof = ProofElement::new(
                level + 1,
                0, // 叶子节点类型
                child_guard.prefix.clone(),
                child_guard.suffix.clone(),
                child_guard.value.clone().unwrap_or_default(),
                vec![],
                Default::default(),
            );

            if current_key_suffix == stored_suffix {
                // 键匹配，返回值
                if let Some(ref value) = child_guard.value {
                    let value_str = String::from_utf8_lossy(value).to_string();
                    return Ok((
                        value_str,
                        MPTProof::new(true, level + 1, vec![leaf_proof, proof_element]),
                    ));
                }
            }

            // 键不匹配或没有值
            Ok((
                String::new(),
                MPTProof::new(false, level + 1, vec![leaf_proof, proof_element]),
            ))
        } else {
            // Extension node，需要检查后缀是否匹配
            let next_node_opt = if let Some(ref next_node) = child_guard.next_node {
                Some(next_node.clone())
            } else if child_guard.next_node_hash != [0u8; 32] {
                // Extension节点的next_node为None,但next_node_hash存在,需要从数据库加载
                let next_hash = child_guard.next_node_hash.clone();
                drop(child_guard);
                if let Some(data) = db.get(&next_hash)? {
                    let full_node = FullNode::deserialize(&data)?;
                    let full_node_arc = Arc::new(RwLock::new(full_node));
                    
                    // 更新Extension节点的next_node
                    if let Ok(mut write_guard) = child_node.write() {
                        write_guard.next_node = Some(full_node_arc.clone());
                    }
                    
                    Some(full_node_arc)
                } else {
                    None
                }
            } else {
                None
            };
            
            if let Some(next_node) = next_node_opt {
                // 重新获取child_guard(如果之前释放了的话)
                let child_guard = child_node
                    .read()
                    .map_err(|_| MPTError::LockError("Failed to read ShortNode".to_string()))?;
                
                // 检查 Extension node 的后缀
                let ext_suffix: Vec<u8> = child_guard
                    .suffix
                    .chars()
                    .filter_map(|c| c.to_digit(16))
                    .map(|d| d as u8)
                    .collect();

                // 当前路径的剩余部分（从 pos+1 开始，因为已经通过了当前索引）
                let current_remaining = &key_path[pos + 1..];

                // 检查后缀是否匹配
                if current_remaining.len() >= ext_suffix.len() {
                    let matches = current_remaining[..ext_suffix.len()] == ext_suffix[..];

                    if matches {
                        // 创建 Extension node 的 proof
                        let ext_proof = ProofElement::new(
                            level + 1,
                            1, // Extension node 类型
                            child_guard.prefix.clone(),
                            child_guard.suffix.clone(),
                            vec![],
                            child_guard.next_node_hash.to_vec(),
                            Default::default(),
                        );

                        // 后缀匹配，继续递归到分支节点
                        let next_node_clone = next_node.clone();
                        drop(child_guard);

                        // 消费 Extension node 的后缀长度 + 1（当前索引）
                        let (value, sub_proof) = self.recursive_query_full_node(
                            key_path,
                            pos + 1 + ext_suffix.len(),
                            level + 2, // Extension node 占了一层，下一层是 level+2
                            next_node_clone,
                            db,
                        )?;

                        // 将证明按顺序组装：子证明 + Extension proof + 父 FullNode proof
                        let mut all_proofs = sub_proof.proofs.clone();
                        all_proofs.push(ext_proof);
                        all_proofs.push(proof_element);

                        Ok((
                            value,
                            MPTProof::new(sub_proof.is_exist, sub_proof.levels, all_proofs),
                        ))
                    } else {
                        // 后缀不匹配，键不存在
                        Ok((
                            String::new(),
                            MPTProof::new(false, level, vec![proof_element]),
                        ))
                    }
                } else {
                    // 剩余路径长度不足，键不存在
                    Ok((
                        String::new(),
                        MPTProof::new(false, level, vec![proof_element]),
                    ))
                }
            } else {
                Ok((
                    String::new(),
                    MPTProof::new(false, level, vec![proof_element]),
                ))
            }
        }
    }

    /// 清空缓存，所有节点写入数据库
    pub fn purge_cache(&self, db: &mut dyn Database) -> Result<(), MPTError> {
        if let Some(cache_mutex) = &self.cache {
            if let Ok(mut cache) = cache_mutex.lock() {
                cache.purge(db)?;
            }
        }
        Ok(())
    }

    /// 获取根哈希
    pub fn get_root_hash(&self) -> [u8; 32] {
        self.root_hash
    }

    /// 序列化 MPT 元数据
    pub fn serialize_metadata(&self) -> Result<Vec<u8>, MPTError> {
        let metadata = MPTMetadata::new(self.root_hash);
        serde_json::to_vec(&metadata).map_err(MPTError::SerializationError)
    }

    /// 反序列化 MPT 元数据
    pub fn deserialize_metadata(data: &[u8]) -> Result<MPTMetadata, MPTError> {
        serde_json::from_slice(data).map_err(MPTError::SerializationError)
    }

    /// 从数据库加载 MPT
    /// 
    /// 根据给定的根哈希从数据库中恢复整个 MPT 树结构
    pub fn load_from_db(
        root_hash: &[u8; 32],
        db: &mut dyn Database,
        cache: Option<NodeCache>,
    ) -> Result<Self, MPTError> {
        // 创建新的 MPT 实例
        let mut mpt = MPT::new(cache);
        
        // 设置根哈希
        mpt.root_hash = *root_hash;

        // 如果根哈希不为零,从数据库加载根节点
        if root_hash != &[0u8; 32] {
            // 使用 get_root 方法从数据库加载根节点
            // 这会自动处理节点的反序列化和缓存
            let _root = mpt.get_root(db)?;
        }

        Ok(mpt)
    }

    /// 完整持久化 MPT 到数据库
    /// 
    /// 保存 MPT 元数据和所有节点数据
    pub fn persist_to_db(&mut self, db: &mut dyn Database) -> Result<(), MPTError> {
        // 首先执行 batch_fix 确保所有节点哈希是最新的
        self.batch_fix(db)?;

        // 保存元数据
        let metadata = self.serialize_metadata()?;
        let metadata_key = b"mpt:metadata";
        db.put(metadata_key, &metadata)?;

        // 保存根哈希索引,方便快速查找
        let root_hash_key = b"mpt:root_hash";
        db.put(root_hash_key, &self.root_hash)?;

        Ok(())
    }

    /// 从数据库恢复最新的 MPT
    pub fn restore_from_db(
        db: &mut dyn Database,
        cache: Option<NodeCache>,
    ) -> Result<Self, MPTError> {
        // 读取根哈希
        let root_hash_key = b"mpt:root_hash";
        let root_hash_data = db.get(root_hash_key)?;
        
        if let Some(data) = root_hash_data {
            if data.len() != 32 {
                return Err(MPTError::InvalidData(
                    format!("Invalid root hash length: {} (expected 32)", data.len()),
                ));
            }

            let mut root_hash = [0u8; 32];
            root_hash.copy_from_slice(&data);

            // 使用根哈希加载完整的 MPT
            Self::load_from_db(&root_hash, db, cache)
        } else {
            // 如果没有保存过数据,返回空的 MPT
            Ok(MPT::new(cache))
        }
    }

    /// 批量修复脏节点 - 递归更新所有被修改过的节点的哈希值
    pub fn batch_fix(&mut self, db: &mut dyn Database) -> Result<(), MPTError> {
        // 如果根节点不存在或不是脏的，直接返回
        if self.root.is_none() {
            return Ok(());
        }

        let root = self.root.clone().unwrap();
        let is_dirty = {
            let root_guard = root
                .read()
                .map_err(|_| MPTError::LockError("Failed to read root".to_string()))?;
            root_guard.is_dirty
        };

        if !is_dirty {
            return Ok(());
        }

        // 处理所有脏的子节点（使用并发）
        let mut handles = vec![];
        let children_to_process: Vec<(usize, Arc<RwLock<ShortNode>>)> = {
            let root_guard = root
                .read()
                .map_err(|_| MPTError::LockError("Failed to read root".to_string()))?;

            root_guard
                .children
                .iter()
                .enumerate()
                .filter_map(|(i, child_opt)| {
                    if let Some(child) = child_opt {
                        // 检查是否是脏节点
                        if let Ok(child_guard) = child.read() {
                            if child_guard.is_dirty {
                                return Some((i, child.clone()));
                            }
                        }
                    }
                    None
                })
                .collect()
        };

        // 并行处理每个脏的子节点
        use std::thread;
        for (idx, child) in children_to_process {
            let child_clone = child.clone();
            let handle = thread::spawn(move || {
                // 在每个线程中递归修复
                Self::short_node_batch_fix_no_db(child_clone.clone()).ok();
                (idx, child_clone)
            });
            handles.push(handle);
        }

        // 等待所有线程完成并更新父节点的子节点哈希
        for handle in handles {
            if let Ok((idx, child)) = handle.join() {
                let mut root_guard = root
                    .write()
                    .map_err(|_| MPTError::LockError("Failed to write root".to_string()))?;

                if let Ok(child_guard) = child.read() {
                    root_guard.children_hash[idx] = Some(child_guard.node_hash.to_vec());
                }
            }
        }

        // 更新根节点的哈希
        {
            let mut root_guard = root
                .write()
                .map_err(|_| MPTError::LockError("Failed to write root".to_string()))?;
            root_guard.update_hash();
            root_guard.is_dirty = false;

            // 保存到数据库或缓存
            let root_hash = root_guard.node_hash;
            let serialized = root_guard.serialize()?;

            if let Some(cache_mutex) = &self.cache {
                if let Ok(mut cache) = cache_mutex.lock() {
                    cache.insert_full_node(root_hash, root.clone(), db)?;
                }
            } else {
                db.put(&root_hash, &serialized)?;
            }

            // 更新 MPT 的根哈希
            self.root_hash = root_hash;
        }

        // 保存所有节点到数据库
        Self::save_tree_to_db(root.clone(), db)?;

        // 更新 MPT 到数据库
        self.update_mpt_in_db(db)?;

        Ok(())
    }
    
    /// 递归保存整个树到数据库
    fn save_tree_to_db(
        node: Arc<RwLock<FullNode>>,
        db: &mut dyn Database,
    ) -> Result<(), MPTError> {
        // 保存当前FullNode
        let (node_hash, serialized, children_to_save) = {
            let guard = node
                .read()
                .map_err(|_| MPTError::LockError("Failed to read FullNode".to_string()))?;
            
            let node_hash = guard.node_hash;
            let serialized = guard.serialize()?;
            let children: Vec<Arc<RwLock<ShortNode>>> = guard
                .children
                .iter()
                .filter_map(|c| c.clone())
                .collect();
            
            (node_hash, serialized, children)
        };
        
        db.put(&node_hash, &serialized)?;
        
        // 递归保存所有子节点
        for child in children_to_save {
            Self::save_short_node_to_db(child, db)?;
        }
        
        Ok(())
    }
    
    /// 递归保存ShortNode及其子树到数据库
    fn save_short_node_to_db(
        node: Arc<RwLock<ShortNode>>,
        db: &mut dyn Database,
    ) -> Result<(), MPTError> {
        let (node_hash, serialized, next_node) = {
            let guard = node
                .read()
                .map_err(|_| MPTError::LockError("Failed to read ShortNode".to_string()))?;
            
            (guard.node_hash, guard.serialize()?, guard.next_node.clone())
        };
        
        db.put(&node_hash, &serialized)?;
        
        // 如果有next_node (Extension节点),递归保存
        if let Some(next) = next_node {
            Self::save_tree_to_db(next, db)?;
        }
        
        Ok(())
    }

    /// 递归批量修复 ShortNode (不保存到数据库,只更新哈希)
    fn short_node_batch_fix_no_db(node: Arc<RwLock<ShortNode>>) -> Result<(), MPTError> {
        let is_dirty = {
            let guard = node
                .read()
                .map_err(|_| MPTError::LockError("Failed to read ShortNode".to_string()))?;
            guard.is_dirty
        };

        if !is_dirty {
            return Ok(());
        }

        // 如果有 next_node 且是脏的，先递归修复它
        let next_node = {
            let guard = node
                .read()
                .map_err(|_| MPTError::LockError("Failed to read ShortNode".to_string()))?;
            guard.next_node.clone()
        };

        if let Some(next) = next_node {
            let next_is_dirty = {
                let next_guard = next
                    .read()
                    .map_err(|_| MPTError::LockError("Failed to read next node".to_string()))?;
                next_guard.is_dirty
            };

            if next_is_dirty {
                Self::full_node_batch_fix_no_db(next.clone())?;

                // 更新 next_node_hash
                let mut guard = node
                    .write()
                    .map_err(|_| MPTError::LockError("Failed to write ShortNode".to_string()))?;
                let next_guard = next
                    .read()
                    .map_err(|_| MPTError::LockError("Failed to read next node".to_string()))?;
                guard.next_node_hash = next_guard.node_hash;
            }
        }

        // 更新当前节点的哈希
        let mut guard = node
            .write()
            .map_err(|_| MPTError::LockError("Failed to write ShortNode".to_string()))?;
        guard.update_hash();
        guard.is_dirty = false;

        Ok(())
    }

    /// 递归批量修复 FullNode (不保存到数据库,只更新哈希)
    fn full_node_batch_fix_no_db(node: Arc<RwLock<FullNode>>) -> Result<(), MPTError> {
        let is_dirty = {
            let guard = node
                .read()
                .map_err(|_| MPTError::LockError("Failed to read FullNode".to_string()))?;
            guard.is_dirty
        };

        if !is_dirty {
            return Ok(());
        }

        // 收集所有脏的子节点
        let dirty_children: Vec<(usize, Arc<RwLock<ShortNode>>)> = {
            let guard = node
                .read()
                .map_err(|_| MPTError::LockError("Failed to read FullNode".to_string()))?;

            guard
                .children
                .iter()
                .enumerate()
                .filter_map(|(i, child_opt)| {
                    if let Some(child) = child_opt {
                        if let Ok(child_guard) = child.read() {
                            if child_guard.is_dirty {
                                return Some((i, child.clone()));
                            }
                        }
                    }
                    None
                })
                .collect()
        };

        // 递归修复所有脏的子节点
        for (idx, child) in dirty_children {
            Self::short_node_batch_fix_no_db(child.clone())?;

            // 更新子节点哈希
            let mut guard = node
                .write()
                .map_err(|_| MPTError::LockError("Failed to write FullNode".to_string()))?;
            let child_guard = child
                .read()
                .map_err(|_| MPTError::LockError("Failed to read child".to_string()))?;
            guard.children_hash[idx] = Some(child_guard.node_hash.to_vec());
        }

        // 更新当前节点的哈希
        let mut guard = node
            .write()
            .map_err(|_| MPTError::LockError("Failed to write FullNode".to_string()))?;
        guard.update_hash();
        guard.is_dirty = false;

        Ok(())
    }

    /// 递归批量修复 ShortNode (已弃用,保留用于向后兼容)
    fn short_node_batch_fix(node: Arc<RwLock<ShortNode>>) -> Result<(), MPTError> {
        Self::short_node_batch_fix_no_db(node)
    }

    /// 递归批量修复 FullNode (已弃用,保留用于向后兼容)
    fn full_node_batch_fix(node: Arc<RwLock<FullNode>>) -> Result<(), MPTError> {
        Self::full_node_batch_fix_no_db(node)
    }

    /// 更新 MPT 到数据库，使用互斥锁保证线程安全
    fn update_mpt_in_db(&mut self, db: &mut dyn Database) -> Result<(), MPTError> {
        use sha2::{Digest, Sha256};

        // 获取更新锁，确保同一时间只有一个线程更新
        let _update_guard = self
            .update_latch
            .lock()
            .map_err(|_| MPTError::LockError("Failed to acquire update lock".to_string()))?;

        // 删除旧的 MPT 哈希（使用哈希的哈希作为 key）
        let mut hasher = Sha256::new();
        hasher.update(&self.root_hash);
        let old_mpt_hash: [u8; 32] = hasher.finalize().into();
        db.delete(&old_mpt_hash)?;

        // 计算新的 MPT 哈希
        let mut hasher = Sha256::new();
        hasher.update(&self.root_hash);
        let new_mpt_hash: [u8; 32] = hasher.finalize().into();

        // 序列化 MPT（只保存 root_hash）
        let mpt_data = serde_json::to_vec(&self.root_hash)?;

        // 写入新的 MPT
        db.put(&new_mpt_hash, &mpt_data)?;

        Ok(())
    }
    /// 打印 MPT 状态
    pub fn print_mpt(&mut self, db: &mut dyn Database) -> Result<(), MPTError> {
        println!("=== MPT 树结构 ===");
        println!("MPT Root Hash: {:x?}", self.root_hash);

        if let Some(root) = self.get_root(db)? {
            self.recursive_print_full_node(root, 0, db)?;
        } else {
            println!("MPT is empty");
        }

        Ok(())
    }

    /// 递归打印 ShortNode
    fn recursive_print_short_node(
        &self,
        node: Arc<RwLock<ShortNode>>,
        level: usize,
        db: &mut dyn Database,
    ) -> Result<(), MPTError> {
        let (is_leaf, node_hash, prefix, suffix, value, next_node_hash, next_node) = {
            let guard = node
                .read()
                .map_err(|_| MPTError::LockError("Failed to read ShortNode".to_string()))?;

            (
                guard.is_leaf,
                guard.node_hash,
                guard.prefix.clone(),
                guard.suffix.clone(),
                guard.value.clone(),
                guard.next_node_hash,
                guard.next_node.clone(),
            )
        };

        let indent = "  ".repeat(level);

        if is_leaf {
            // 叶子节点
            println!(
                "{}level: {}, leafNode: {:x?}",
                indent,
                level,
                &node_hash[..8]
            );
            println!(
                "{}  prefix: {:?}, suffix: {:?}, value: {:?}",
                indent,
                prefix,
                suffix,
                String::from_utf8_lossy(&value.unwrap_or_default())
            );
        } else {
            // Extension 节点
            println!(
                "{}level: {}, extensionNode: {:x?}",
                indent,
                level,
                &node_hash[..8]
            );
            println!(
                "{}  prefix: {:?}, suffix: {:?}, next node: {:x?}",
                indent,
                prefix,
                suffix,
                &next_node_hash[..8]
            );

            // 递归打印 next node
            if let Some(next_node) = next_node {
                self.recursive_print_full_node(next_node, level + 1, db)?;
            }
        }

        Ok(())
    }

    /// 递归打印 FullNode
    fn recursive_print_full_node(
        &self,
        node: Arc<RwLock<FullNode>>,
        level: usize,
        db: &mut dyn Database,
    ) -> Result<(), MPTError> {
        let guard = node
            .read()
            .map_err(|_| MPTError::LockError("Failed to read FullNode".to_string()))?;

        let indent = "  ".repeat(level);

        // 打印当前 FullNode
        println!(
            "{}level: {}, fullNode: {:x?}, value: {:?}",
            indent,
            level,
            &guard.node_hash[..8],
            guard
                .value
                .as_ref()
                .map(|v| String::from_utf8_lossy(v).to_string())
                .unwrap_or_else(|| "None".to_string())
        );

        // 打印所有子节点的哈希
        for (i, child_hash) in guard.children_hash.iter().enumerate() {
            if let Some(hash) = child_hash {
                println!("{}  children[{}]: {:x?}", indent, i, &hash[..8]);
            }
        }

        // 收集所有子节点
        let children: Vec<(usize, Arc<RwLock<ShortNode>>)> = guard
            .children
            .iter()
            .enumerate()
            .filter_map(|(i, child_opt)| child_opt.as_ref().map(|c| (i, c.clone())))
            .collect();

        drop(guard); // 释放锁

        // 递归打印所有子节点
        for (_i, child) in children {
            self.recursive_print_short_node(child, level + 1, db)?;
        }

        Ok(())
    }

    /// 打印查询结果
    pub fn print_query_result(&self, key: &str, value: &str, mpt_proof: &MPTProof) {
        println!("=== 查询结果 ===");
        println!("key: {}", key);
        if value.is_empty() {
            println!("value: 不存在");
        } else {
            println!("value: {}", value);
        }
        mpt_proof.print();
    }

    /// 递归删除FullNode中的键值对
    fn recursive_delete_full_node(
        &mut self,
        key_path: &[u8],
        pos: usize,
        full_node: Arc<RwLock<FullNode>>,
        db: &mut dyn Database,
    ) -> Result<Option<String>, MPTError> {
        let mut guard = full_node
            .write()
            .map_err(|_| MPTError::LockError("Failed to write FullNode".to_string()))?;

        // 如果已经消费完所有路径，删除当前节点的值
        if pos >= key_path.len() {
            if let Some(value) = guard.value.take() {
                guard.update_hash();
                return Ok(Some(String::from_utf8_lossy(&value).to_string()));
            } else {
                return Ok(None);
            }
        }

        // 获取当前路径字符对应的索引
        let index = key_path[pos] as usize;
        if index >= 16 {
            return Ok(None);
        }

        // 如果没有对应的子节点，返回None
        let child_arc = match guard.children[index].as_ref() {
            Some(child) => child.clone(),
            None => return Ok(None),
        };

        drop(guard); // 释放写锁

        // 递归删除子节点
        let deleted_value =
            self.recursive_delete_short_node(key_path, pos + 1, child_arc.clone(), db)?;

        // 如果删除成功，需要检查子节点是否应该被移除
        if deleted_value.is_some() {
            let mut guard = full_node
                .write()
                .map_err(|_| MPTError::LockError("Failed to write FullNode".to_string()))?;

            // 检查子节点是否变为空
            let should_remove_child = {
                let child_guard = child_arc
                    .read()
                    .map_err(|_| MPTError::LockError("Failed to read child".to_string()))?;

                // 如果是空的Leaf节点或空的Extension节点，则移除
                child_guard.is_leaf && child_guard.value.is_none()
                    || !child_guard.is_leaf && child_guard.next_node.is_none()
            };

            if should_remove_child {
                guard.children[index] = None;
                guard.children_hash[index] = None;
                guard.update_hash();
            }
        }

        Ok(deleted_value)
    }

    /// 递归删除ShortNode中的键值对
    fn recursive_delete_short_node(
        &mut self,
        key_path: &[u8],
        pos: usize,
        short_node: Arc<RwLock<ShortNode>>,
        db: &mut dyn Database,
    ) -> Result<Option<String>, MPTError> {
        let mut guard = short_node
            .write()
            .map_err(|_| MPTError::LockError("Failed to write ShortNode".to_string()))?;

        if guard.is_leaf {
            // 叶子节点：检查剩余路径是否匹配suffix
            let remaining_path = &key_path[pos..];
            // 将stored suffix从十六进制字符串转换为u8数组（与查询逻辑一致）
            let stored_suffix: Vec<u8> = guard
                .suffix
                .chars()
                .filter_map(|c| c.to_digit(16))
                .map(|d| d as u8)
                .collect();

            if remaining_path == stored_suffix.as_slice() {
                // 路径匹配，删除值
                if let Some(value) = guard.value.take() {
                    guard.update_hash();
                    return Ok(Some(String::from_utf8_lossy(&value).to_string()));
                }
            }
            return Ok(None);
        } else {
            // Extension节点：检查suffix是否匹配
            // 将stored suffix从十六进制字符串转换为u8数组（与查询逻辑一致）
            let stored_suffix: Vec<u8> = guard
                .suffix
                .chars()
                .filter_map(|c| c.to_digit(16))
                .map(|d| d as u8)
                .collect();
            let remaining_path = &key_path[pos..];

            if remaining_path.len() < stored_suffix.len() {
                return Ok(None);
            }

            if &remaining_path[..stored_suffix.len()] == stored_suffix.as_slice() {
                // Suffix匹配，继续到下一个节点
                let next_pos = pos + stored_suffix.len();

                if let Some(next_node) = guard.next_node.as_ref() {
                    let next_node_clone = next_node.clone();
                    drop(guard); // 释放锁

                    let deleted_value =
                        self.recursive_delete_full_node(key_path, next_pos, next_node_clone, db)?;

                    // 如果删除成功，检查下一个节点是否变空
                    if deleted_value.is_some() {
                        let mut guard = short_node.write().map_err(|_| {
                            MPTError::LockError("Failed to write ShortNode".to_string())
                        })?;

                        // 检查next_node是否变为空
                        let should_remove_next = {
                            if let Some(next_node) = guard.next_node.as_ref() {
                                let next_guard = next_node.read().map_err(|_| {
                                    MPTError::LockError("Failed to read next node".to_string())
                                })?;

                                // 如果FullNode没有值且没有子节点，则认为是空的
                                next_guard.value.is_none()
                                    && next_guard.children.iter().all(|child| child.is_none())
                            } else {
                                false
                            }
                        };

                        if should_remove_next {
                            guard.next_node = None;
                            guard.next_node_hash = [0u8; 32];
                            guard.update_hash();
                        }
                    }

                    return Ok(deleted_value);
                }
            }
            return Ok(None);
        }
    }

    /// 清理根节点（如果需要）
    fn cleanup_root_if_needed(&mut self, _db: &mut dyn Database) -> Result<(), MPTError> {
        if let Some(root) = &self.root {
            let should_remove_root = {
                let root_guard = root
                    .read()
                    .map_err(|_| MPTError::LockError("Failed to read root".to_string()))?;

                // 如果根节点没有值且没有子节点，则移除根节点
                root_guard.value.is_none()
                    && root_guard.children.iter().all(|child| child.is_none())
            };

            if should_remove_root {
                self.root = None;
                self.root_hash = [0u8; 32];
            }
        }
        Ok(())
    }
}
