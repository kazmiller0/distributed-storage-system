/// ADS implementations module
/// 
/// 这个模块包含所有认证数据结构的具体实现
/// 
/// 当前使用: CryptoAccumulator (基于 BLS12-381 的密码学累加器)

pub mod crypto_accumulator;

pub use crypto_accumulator::CryptoAccumulatorAds;
