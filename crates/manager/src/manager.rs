//! Manager 核心结构
//!
//! 负责协调客户端请求和 storager 节点

use crate::core::{ProofVerifier, Router};
use common::{AdsMode, RootHash};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Manager 结构
///
/// 负责：
/// - 路由请求到对应的 storager 节点
/// - 验证来自 storager 的证明
/// - 维护系统状态（根哈希等）
pub struct Manager {
    /// 路由器（管理一致性哈希和地址映射）
    pub(crate) router: Router,
    /// 证明验证器
    pub(crate) verifier: ProofVerifier,
    /// storager 名称到根哈希的映射
    pub(crate) root_hashes: Arc<RwLock<HashMap<String, RootHash>>>,
}

impl Manager {
    /// 创建新的 Manager 实例
    ///
    /// # Arguments
    /// * `storager_addrs` - storager 地址列表
    /// * `ads_mode` - ADS 模式
    pub fn new(storager_addrs: Vec<String>, ads_mode: AdsMode) -> Self {
        let router = Router::new(storager_addrs, 150); // 每个节点 150 个虚拟节点
        let verifier = ProofVerifier::new(ads_mode);
        let root_hashes = Arc::new(RwLock::new(HashMap::new()));

        Manager {
            router,
            verifier,
            root_hashes,
        }
    }

    /// 使用一致性哈希环获取 keyword 对应的 storager
    pub(crate) fn get_storager_for_keyword(&self, keyword: &str) -> Option<(String, String)> {
        self.router.get_storager_for_keyword(keyword)
    }

    /// 验证证明
    pub(crate) fn verify_proof(&self, proof: &[u8], root_hash: &[u8]) -> bool {
        self.verifier.verify(proof, root_hash)
    }

    /// 更新 storager 的根哈希
    pub(crate) fn update_root_hash(&self, storager_name: String, root_hash: RootHash) {
        let mut hashes = self.root_hashes.write().unwrap();
        hashes.insert(storager_name, root_hash);
    }

    /// 合并多个证明
    pub(crate) fn combine_proofs(&self, proofs: &[Vec<u8>]) -> Vec<u8> {
        self.verifier.combine_proofs(proofs)
    }

    /// 获取当前的 ADS 模式
    pub fn ads_mode(&self) -> AdsMode {
        self.verifier.ads_mode()
    }

    /// 获取所有 storager 节点信息
    pub fn get_storagers(&self) -> Vec<(String, String)> {
        self.router.get_all_storagers()
    }
}
