use crate::ads::CryptoAccumulatorAds;
use crate::ads_trait::AdsOperations;
use std::sync::{Arc, RwLock};

/// Storager 结构
///
/// 负责管理单个存储节点的 ADS 实例
pub struct Storager {
    pub(crate) ads: Arc<RwLock<Box<dyn AdsOperations>>>,
}

impl Storager {
    /// 创建新的 Storager 实例（默认使用密码学累加器）
    pub fn new() -> Self {
        Self::with_crypto_accumulator()
    }

    /// 使用密码学累加器创建实例
    pub fn with_crypto_accumulator() -> Self {
        let ads: Box<dyn AdsOperations> = Box::new(CryptoAccumulatorAds::new());
        Storager {
            ads: Arc::new(RwLock::new(ads)),
        }
    }

    // 未来可以添加其他 ADS 的构造函数
    // 例如：
    // pub fn with_merkle_tree() -> Self { ... }
    // pub fn with_mpt() -> Self { ... }
    // pub fn with_vector_commitment() -> Self { ... }
}

impl Default for Storager {
    fn default() -> Self {
        Self::new()
    }
}
