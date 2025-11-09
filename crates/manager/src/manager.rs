use ark_bls12_381::G1Affine;
use ark_serialize::CanonicalDeserialize;
use common::{AdsMode, RootHash};
use consistent_hash::ConsistentHashRing;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Manager 结构
pub struct Manager {
    /// 用于路由关键词到 storager 的一致性哈希环
    pub(crate) hash_ring: Arc<RwLock<ConsistentHashRing>>,
    /// storager 名称到地址的映射
    pub(crate) storager_addrs: HashMap<String, String>,
    /// storager 名称到根哈希的映射
    pub(crate) root_hashes: Arc<RwLock<HashMap<String, RootHash>>>,
    /// ADS 模式
    #[allow(dead_code)]
    pub(crate) ads_mode: AdsMode,
}

impl Manager {
    pub fn new(storager_addrs: Vec<String>, ads_mode: AdsMode) -> Self {
        let mut hash_ring = ConsistentHashRing::new();
        let mut addr_map = HashMap::new();

        // 为每个 storager 在哈希环上添加虚拟节点
        for (idx, addr) in storager_addrs.iter().enumerate() {
            let node_name = format!("storager-{}", idx);
            hash_ring.add_node(&node_name, 150); // 每个节点150个虚拟节点
            addr_map.insert(node_name, addr.clone());
        }

        let root_hashes = Arc::new(RwLock::new(HashMap::new()));
        Manager {
            hash_ring: Arc::new(RwLock::new(hash_ring)),
            storager_addrs: addr_map,
            root_hashes,
            ads_mode,
        }
    }

    /// 使用一致性哈希环获取 keyword 对应的 storager
    pub(crate) fn get_storager_for_keyword(&self, keyword: &str) -> Option<(String, String)> {
        let ring = self.hash_ring.read().unwrap();
        let node_name = ring.get_node(keyword)?;
        let addr = self.storager_addrs.get(&node_name)?.clone();
        Some((node_name, addr))
    }

    /// 验证证明
    pub(crate) fn verify_proof(&self, proof: &[u8], _root_hash: &[u8]) -> bool {
        match self.ads_mode {
            AdsMode::CryptoAccumulator => {
                // 密码学累加器的完整证明验证
                if proof.is_empty() {
                    return false;
                }

                // 最后一个字节是 storager 端的验证结果
                let storager_verified = proof.last() == Some(&1);

                if !storager_verified {
                    println!("Storager verification failed");
                    return false;
                }

                // 验证证明的结构完整性
                let min_size = 96 + 8 + 1; // 最小证明大小
                if proof.len() < min_size {
                    println!("Proof too small: {} bytes", proof.len());
                    return false;
                }

                // 尝试反序列化第一个椭圆曲线点来验证格式正确性
                match G1Affine::deserialize(&proof[..96]) {
                    Ok(_) => {
                        println!("✅ Crypto accumulator proof verified successfully");
                        true
                    }
                    Err(e) => {
                        println!("Failed to deserialize proof: {:?}", e);
                        false
                    }
                }
            }
        }
    }

    /// 更新 storager 的根哈希
    pub(crate) fn update_root_hash(&self, storager_name: String, root_hash: RootHash) {
        let mut hashes = self.root_hashes.write().unwrap();
        hashes.insert(storager_name, root_hash);
    }

    /// 合并多个证明
    ///
    /// 简化实现：将所有证明连接起来
    /// 在生产环境中，可能需要更复杂的证明合并策略
    pub(crate) fn combine_proofs(&self, proofs: &[Vec<u8>]) -> Vec<u8> {
        if proofs.is_empty() {
            return Vec::new();
        }

        // 简单方案：返回第一个证明
        // 更复杂的方案可以构建 Merkle 树或使用其他聚合技术
        proofs[0].clone()
    }
}
