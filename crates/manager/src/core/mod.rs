//! Manager 核心模块
//!
//! 包含路由、验证等核心功能

pub mod routing;
pub mod verification;

pub use routing::Router;
pub use verification::ProofVerifier;
