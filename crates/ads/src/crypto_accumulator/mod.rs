//! Cryptographic Accumulator Module
//!
//! 基于 BLS12-381 椭圆曲线的密码学累加器实现
//!
//! ## 主要组件
//! - `DynamicAccumulator`: 动态累加器，支持增删元素
//! - `DigestSet`: 摘要集合，用于存储元素
//! - 证明生成和验证功能

pub mod acc;

pub use acc::digest_set::DigestSet;
pub use acc::dynamic_accumulator::DynamicAccumulator;
pub use acc::*;
