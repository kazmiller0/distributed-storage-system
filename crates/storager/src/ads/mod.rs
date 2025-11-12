/// ADS implementations module
///
/// 这个模块包含所有认证数据结构的具体实现
///
/// ## 当前实现
/// - **CryptoAccumulator**: 基于 BLS12-381 的密码学累加器
///
/// ## 添加新的 ADS 实现步骤
/// 1. 在 src/ads/ 下创建新文件（如 merkle_tree.rs）
/// 2. 实现 AdsOperations trait
/// 3. 在此文件中添加 `pub mod your_ads;`
/// 4. 在此文件中添加 `pub use your_ads::YourAds;`
/// 5. 在 src/storager.rs 中根据配置选择使用哪个 ADS
pub mod crypto_accumulator;

// 未来的 ADS 实现示例：
// pub mod merkle_tree;
// pub mod mpt;
// pub mod vector_commitment;

pub use crypto_accumulator::CryptoAccumulatorAds;
