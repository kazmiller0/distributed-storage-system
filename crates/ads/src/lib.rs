//! ESA (Efficient Set Accumulator) Library
//! 
//! 这个库提供了多种认证数据结构 (Authenticated Data Structures) 的实现
//! 
//! ## 当前实现
//! - **CryptoAccumulator**: 基于 BLS12-381 的密码学累加器
//! 
//! ## 未来扩展
//! 可以添加其他 ADS 实现，例如:
//! - Merkle Tree
//! - Patricia Trie
//! - Vector Commitment
//! 等等

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

// ========================================
// Common utilities (shared across all ADS)
// ========================================

/// Digest utilities - 通用摘要工具
pub mod digest;
pub use digest::*;

/// Set operations - 通用集合操作
pub mod set;
pub use set::*;

// ========================================
// ADS Implementations
// ========================================

/// Cryptographic Accumulator based on BLS12-381
pub mod crypto_accumulator;

// Re-export commonly used types
pub use crypto_accumulator::DynamicAccumulator;
pub use crypto_accumulator::DigestSet;
