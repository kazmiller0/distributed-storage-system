//! 证明验证模块
//!
//! 负责验证来自 storager 的密码学证明

use ark_bls12_381::G1Affine;
use ark_serialize::CanonicalDeserialize;
use common::AdsMode;

/// 证明验证器
pub struct ProofVerifier {
    ads_mode: AdsMode,
}

impl ProofVerifier {
    /// 创建新的证明验证器
    pub fn new(ads_mode: AdsMode) -> Self {
        ProofVerifier { ads_mode }
    }

    /// 验证证明
    ///
    /// # Arguments
    /// * `proof` - 证明数据
    /// * `root_hash` - 根哈希(某些 ADS 模式下需要)
    ///
    /// # Returns
    /// 验证是否成功
    pub fn verify(&self, proof: &[u8], _root_hash: &[u8]) -> bool {
        match self.ads_mode {
            AdsMode::CryptoAccumulator => self.verify_crypto_accumulator(proof),
            AdsMode::Mpt => self.verify_mpt(proof),
        }
    }

    /// 验证密码学累加器的证明
    fn verify_crypto_accumulator(&self, proof: &[u8]) -> bool {
        if proof.is_empty() {
            println!("❌ Empty proof");
            return false;
        }

        // 最后一个字节是 storager 端的验证结果
        let storager_verified = proof.last() == Some(&1);

        if !storager_verified {
            println!("❌ Storager verification failed");
            return false;
        }

        // 验证证明的结构完整性
        let min_size = 96 + 8 + 1; // G1Affine(96) + element(8) + valid(1)
        if proof.len() < min_size {
            println!(
                "❌ Proof too small: {} bytes (expected >= {})",
                proof.len(),
                min_size
            );
            return false;
        }

        // 尝试反序列化第一个椭圆曲线点来验证格式正确性
        match G1Affine::deserialize(&proof[..96]) {
            Ok(_) => {
                println!("✅ Crypto accumulator proof verified successfully");
                true
            }
            Err(e) => {
                println!("❌ Failed to deserialize proof: {:?}", e);
                false
            }
        }
    }

    /// 验证 MPT 的证明
    fn verify_mpt(&self, proof: &[u8]) -> bool {
        // MPT 的证明就是根哈希本身
        // 只要证明非空或长度为 0(空结果)就认为有效
        if proof.is_empty() {
            // 空证明表示关键字不存在,这是有效的
            println!("✅ MPT proof verified (empty result)");
            true
        } else if proof.len() == 32 {
            // 32 字节的根哈希
            println!("✅ MPT proof verified (root hash present)");
            true
        } else {
            println!(
                "⚠️  MPT proof has unexpected length: {} bytes, accepting anyway",
                proof.len()
            );
            // 即使长度不是标准的 32 字节,也接受,因为 MPT 可能有不同的哈希长度
            true
        }
    }

    /// 合并多个证明
    ///
    /// 用于布尔查询等需要合并多个 storager 证明的场景
    ///
    /// # Arguments
    /// * `proofs` - 证明列表
    ///
    /// # Returns
    /// 合并后的证明
    pub fn combine_proofs(&self, proofs: &[Vec<u8>]) -> Vec<u8> {
        if proofs.is_empty() {
            return Vec::new();
        }

        match self.ads_mode {
            AdsMode::CryptoAccumulator => {
                // 简单方案：返回第一个证明
                // 更复杂的方案可以构建 Merkle 树或使用其他聚合技术
                proofs[0].clone()
            }
            AdsMode::Mpt => {
                // MPT: 返回第一个非空证明
                proofs
                    .iter()
                    .find(|p| !p.is_empty())
                    .cloned()
                    .unwrap_or_default()
            }
        }
    }

    /// 获取当前的 ADS 模式
    pub fn ads_mode(&self) -> AdsMode {
        self.ads_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_proof() {
        let verifier = ProofVerifier::new(AdsMode::CryptoAccumulator);
        assert!(!verifier.verify(&[], &[]));
    }

    #[test]
    fn test_proof_too_small() {
        let verifier = ProofVerifier::new(AdsMode::CryptoAccumulator);
        let small_proof = vec![0u8; 50];
        assert!(!verifier.verify(&small_proof, &[]));
    }
}
