//! 认证数据结构 (Authenticated Data Structures) 模块
//!
//! 该模块定义了所有 ADS 实现必须遵守的通用接口，
//! 并提供了多种 ADS 的具体实现。
//!
//! ## 可用的 ADS 实现
//! - **CryptoAccumulatorAds**: 基于 BLS12-381 的密码学累加器
//! - **MptAds**: Merkle Patricia Trie (以太坊风格)

use common::RootHash;

/// ADS 操作的通用 trait
///
/// 所有认证数据结构都需要实现这个 trait
pub trait AdsOperations: Send + Sync {
    /// 添加 (keyword, fid) 对到 ADS
    /// 返回: (proof, root_hash)
    fn add(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash);

    /// 查询 keyword 对应的所有 fid
    /// 返回: (fids, proof)
    fn query(&self, keyword: &str) -> (Vec<String>, Vec<u8>);

    /// 从 ADS 中删除 (keyword, fid) 对
    /// 返回: (proof, root_hash)
    fn delete(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash);
}

// ADS 实现模块
pub mod crypto_accumulator;
pub mod mpt;

// 导出 ADS 实现
pub use crypto_accumulator::CryptoAccumulatorAds;
pub use mpt::MptAds;
