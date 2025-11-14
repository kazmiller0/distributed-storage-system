//! Merkle Patricia Trie (MPT) ADS Implementation
//!
//! 使用以太坊风格的 Merkle Patricia Trie 作为认证数据结构
//! 支持高效的键值存储和成员资格证明

use super::AdsOperations;
use common::RootHash;
use esa_rust::mpt::{node::Database, KVPair, MPTError, MPT};
use std::collections::HashMap;

/// 简单的内存数据库实现
struct MemoryDb {
    data: HashMap<Vec<u8>, Vec<u8>>,
}

impl MemoryDb {
    fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
}

impl Database for MemoryDb {
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

/// MPT ADS 实现
pub struct MptAds {
    /// 存储每个 keyword 对应的 MPT 实例、数据库和文件列表
    /// HashMap<keyword, (mpt, db, fid_list)>
    tries: HashMap<String, (MPT, MemoryDb, Vec<String>)>,
}

impl MptAds {
    pub fn new() -> Self {
        MptAds {
            tries: HashMap::new(),
        }
    }

    /// 将 fid 列表编码为字符串
    fn encode_fids(fids: &[String]) -> String {
        fids.join(",")
    }

    /// 从字符串解码 fid 列表
    fn decode_fids(data: &str) -> Vec<String> {
        if data.is_empty() {
            Vec::new()
        } else {
            data.split(',').map(|s| s.to_string()).collect()
        }
    }
}

impl Default for MptAds {
    fn default() -> Self {
        Self::new()
    }
}

impl AdsOperations for MptAds {
    fn add(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash) {
        let entry = self
            .tries
            .entry(keyword.to_string())
            .or_insert_with(|| (MPT::new(None), MemoryDb::new(), Vec::new()));

        // 添加 fid 到列表
        if !entry.2.contains(&fid.to_string()) {
            entry.2.push(fid.to_string());
        }

        // 更新 MPT
        let value = Self::encode_fids(&entry.2);
        let kv = KVPair::new(keyword.to_string(), value);

        let _ = entry.0.insert(kv, &mut entry.1, true, false);

        // 获取根哈希
        let root_hash = entry.0.root_hash.to_vec();

        // 生成简单的证明（包含根哈希）
        let proof = root_hash.clone();

        (proof, root_hash)
    }

    fn query(&self, keyword: &str) -> (Vec<String>, Vec<u8>) {
        if let Some((trie, _db, fids)) = self.tries.get(keyword) {
            // 生成成员资格证明（使用根哈希作为简化的证明）
            let proof = trie.root_hash.to_vec();

            (fids.clone(), proof)
        } else {
            // 关键字不存在，返回空列表
            (vec![], vec![])
        }
    }

    fn delete(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash) {
        if let Some((trie, db, fids)) = self.tries.get_mut(keyword) {
            // 从列表中移除 fid
            fids.retain(|f| f != fid);

            if fids.is_empty() {
                // 如果列表为空，从 MPT 中删除整个键
                let _ = trie.delete(keyword, db);

                let root_hash = trie.root_hash.to_vec();

                // 如果 trie 为空，移除整个条目
                if trie.root_hash == [0; 32] {
                    self.tries.remove(keyword);
                    return (vec![], vec![]);
                }

                (vec![], root_hash)
            } else {
                // 更新 MPT
                let value = Self::encode_fids(fids);
                let kv = KVPair::new(keyword.to_string(), value);

                let _ = trie.insert(kv, db, true, false);

                let root_hash = trie.root_hash.to_vec();
                let proof = root_hash.clone();

                (proof, root_hash)
            }
        } else {
            // 关键字不存在
            (vec![], vec![])
        }
    }
}
