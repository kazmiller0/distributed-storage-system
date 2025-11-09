use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofElement {
    pub level: u32,
    pub proof_type: u8, // 0: leaf node, 1: extension node, 2: branch node
    pub prefix: String,
    pub suffix: String,
    pub value: Vec<u8>,
    pub next_node_hash: Vec<u8>,
    pub children_hashes: [Vec<u8>; 16],
}

impl ProofElement {
    pub fn new(
        level: u32,
        proof_type: u8,
        prefix: String,
        suffix: String,
        value: Vec<u8>,
        next_node_hash: Vec<u8>,
        children_hashes: [Vec<u8>; 16],
    ) -> Self {
        Self {
            level,
            proof_type,
            prefix,
            suffix,
            value,
            next_node_hash,
            children_hashes,
        }
    }

    /// 计算证明元素的大小
    pub fn size_of(&self) -> usize {
        std::mem::size_of::<u32>()
            + std::mem::size_of::<u8>()
            + self.prefix.len()
            + self.suffix.len()
            + self.value.len()
            + self.next_node_hash.len()
            + self.children_hashes.iter().map(|h| h.len()).sum::<usize>()
    }

    /// 打印证明元素
    pub fn print(&self) {
        match self.proof_type {
            0 => {
                println!(
                    "level={}, proofType=leaf node, prefix={:x?}, suffix={:x?}, value={:x?}",
                    self.level,
                    self.prefix.as_bytes(),
                    self.suffix.as_bytes(),
                    self.value
                );
            }
            1 => {
                println!(
                    "level={}, proofType=extension node, prefix={:x?}, suffix={:x?}, nextNodeHash={:x?}",
                    self.level, self.prefix.as_bytes(), self.suffix.as_bytes(), self.next_node_hash
                );
            }
            2 => {
                println!(
                    "level={}, proofType=branch node, value={:x?}",
                    self.level, self.value
                );
                for (i, hash) in self.children_hashes.iter().enumerate() {
                    if !hash.is_empty() {
                        println!("[{}]{:x?}", i, hash);
                    }
                }
            }
            _ => println!("Unknown proof type: {}", self.proof_type),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MPTProof {
    pub is_exist: bool,
    pub levels: u32,
    pub proofs: Vec<ProofElement>,
}

impl MPTProof {
    pub fn new(is_exist: bool, levels: u32, proofs: Vec<ProofElement>) -> Self {
        Self {
            is_exist,
            levels,
            proofs,
        }
    }

    /// 获取证明是否存在
    pub fn get_is_exist(&self) -> bool {
        self.is_exist
    }

    /// 获取层级数
    pub fn get_levels(&self) -> u32 {
        self.levels
    }

    /// 获取证明元素列表
    pub fn get_proofs(&self) -> &[ProofElement] {
        &self.proofs
    }

    /// 计算 MPT 证明的大小
    pub fn size_of(&self) -> usize {
        std::mem::size_of::<bool>()
            + std::mem::size_of::<u32>()
            + self.proofs.iter().map(|p| p.size_of()).sum::<usize>()
    }

    /// 打印 MPT 证明
    pub fn print(&self) {
        println!("打印MPTProof-------------------------------------------------------------------------------------------");
        println!("isExist={}, levels={}", self.is_exist, self.levels);
        for proof in &self.proofs {
            proof.print();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_element_creation() {
        let proof = ProofElement::new(
            1,
            0,
            "prefix".to_string(),
            "suffix".to_string(),
            vec![1, 2, 3],
            vec![],
            Default::default(),
        );

        assert_eq!(proof.level, 1);
        assert_eq!(proof.proof_type, 0);
        assert_eq!(proof.prefix, "prefix");
        assert_eq!(proof.suffix, "suffix");
        assert_eq!(proof.value, vec![1, 2, 3]);
    }

    #[test]
    fn test_mpt_proof_creation() {
        let proof_element = ProofElement::new(
            0,
            0,
            "test".to_string(),
            "key".to_string(),
            vec![1, 2, 3],
            vec![],
            Default::default(),
        );

        let mpt_proof = MPTProof::new(true, 1, vec![proof_element]);

        assert!(mpt_proof.get_is_exist());
        assert_eq!(mpt_proof.get_levels(), 1);
        assert_eq!(mpt_proof.get_proofs().len(), 1);
    }
}

/// Compute MPT root hash from value and proof
pub fn compute_mpt_root(value: &str, mpt_proof: &MPTProof) -> [u8; 32] {
    let proofs = mpt_proof.get_proofs();
    let mut node_hash_0 = [0u8; 32];

    // Initialize with value
    let value_bytes = value.as_bytes();
    if value_bytes.len() <= 32 {
        node_hash_0[..value_bytes.len()].copy_from_slice(value_bytes);
    } else {
        // If value is longer than 32 bytes, hash it first
        let mut hasher = Sha256::new();
        hasher.update(value_bytes);
        node_hash_0 = hasher.finalize().into();
    }

    println!("Starting verification with value: {}", value);
    println!("Initial node_hash_0: {:x?}", node_hash_0);

    // Process proofs from leaf to root (forward order)
    for (i, proof) in proofs.iter().enumerate() {
        println!(
            "\n--- Processing proof {} (level={}, type={}) ---",
            i, proof.level, proof.proof_type
        );
        let mut node_data = Vec::new();

        match proof.proof_type {
            0 => {
                // Leaf node: hash(prefix + suffix + value)
                println!(
                    "Leaf node: prefix='{}', suffix='{}', value={:x?}",
                    proof.prefix, proof.suffix, proof.value
                );

                node_data.extend_from_slice(proof.prefix.as_bytes());
                node_data.extend_from_slice(proof.suffix.as_bytes());

                // If exists, use queried value; otherwise use proof's value
                if mpt_proof.is_exist {
                    node_data.extend_from_slice(value.as_bytes());
                } else {
                    node_data.extend_from_slice(&proof.value);
                }

                println!("Hashing data (len={}): {:x?}", node_data.len(), node_data);
                let mut hasher = Sha256::new();
                hasher.update(&node_data);
                node_hash_0 = hasher.finalize().into();
                println!("Computed hash: {:x?}", node_hash_0);
            }
            1 => {
                // Extension node: verify and hash(prefix + suffix + next_node_hash)
                println!(
                    "Extension node: prefix='{}', suffix='{}', next_node_hash={:x?}",
                    proof.prefix, proof.suffix, proof.next_node_hash
                );

                // If not the bottom level, verify next node hash matches computed hash
                if proof.level != mpt_proof.levels {
                    if proof.next_node_hash.len() == 32 {
                        let mut expected = [0u8; 32];
                        expected.copy_from_slice(&proof.next_node_hash);
                        println!(
                            "Verifying next_node_hash: expected={:x?}, got={:x?}",
                            expected, node_hash_0
                        );
                        if expected != node_hash_0 {
                            println!(
                                "Level {} nextNodeHash={:x?} verification failed",
                                proof.level, node_hash_0
                            );
                            return [0u8; 32];
                        }
                    }
                }

                node_data.extend_from_slice(proof.prefix.as_bytes());
                node_data.extend_from_slice(proof.suffix.as_bytes());
                node_data.extend_from_slice(&proof.next_node_hash);

                println!("Hashing data (len={}): {:x?}", node_data.len(), node_data);
                let mut hasher = Sha256::new();
                hasher.update(&node_data);
                node_hash_0 = hasher.finalize().into();
                println!("Computed hash: {:x?}", node_hash_0);
            }
            2 => {
                // Branch node: verify and hash(all children hashes + value)
                println!(
                    "Branch node: value={:x?}, {} children",
                    proof.value,
                    proof
                        .children_hashes
                        .iter()
                        .filter(|h| !h.is_empty())
                        .count()
                );

                // If not the bottom level, verify computed hash is in children
                if proof.level != mpt_proof.levels {
                    println!(
                        "Verifying computed hash {:x?} is in children...",
                        node_hash_0
                    );
                    let mut is_in = false;
                    for (idx, child_hash) in proof.children_hashes.iter().enumerate() {
                        if child_hash.len() == 32 {
                            let mut hash_arr = [0u8; 32];
                            hash_arr.copy_from_slice(child_hash);
                            if hash_arr == node_hash_0 {
                                println!("Found at index {}", idx);
                                is_in = true;
                                break;
                            }
                        }
                    }
                    if !is_in {
                        println!(
                            "Level {} childrenHashes={:x?} verification failed",
                            proof.level, node_hash_0
                        );
                        return [0u8; 32];
                    }
                }

                // Concatenate all 16 children hashes
                for child_hash in &proof.children_hashes {
                    node_data.extend_from_slice(child_hash);
                }
                node_data.extend_from_slice(&proof.value);

                println!("Hashing data (len={})", node_data.len());
                let mut hasher = Sha256::new();
                hasher.update(&node_data);
                node_hash_0 = hasher.finalize().into();
                println!("Computed hash: {:x?}", node_hash_0);
            }
            _ => {
                println!("Unknown proof type: {}", proof.proof_type);
                return [0u8; 32];
            }
        }
    }

    println!("\n=== Final computed root hash: {:x?} ===", node_hash_0);
    node_hash_0
}
