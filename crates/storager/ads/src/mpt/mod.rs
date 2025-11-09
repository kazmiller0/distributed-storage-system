pub mod db;
pub mod error;
pub mod mpt;
pub mod node;
pub mod proof;
pub mod utils;

pub use db::RocksDbAdapter;
pub use error::MPTError;
pub use mpt::MPT;
pub use node::{FullNode, ShortNode, NodeCache};
pub use proof::{MPTProof, ProofElement};
pub use utils::KVPair;

#[cfg(test)]
mod tests {
    #[test]
    fn test_basic_operations() {
        // 基本测试将在这里添加
    }
}
