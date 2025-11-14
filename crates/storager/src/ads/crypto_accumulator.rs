//! Cryptographic Accumulator ADS Implementation
//!
//! 基于 BLS12-381 椭圆曲线的密码学累加器
//! 支持恒定大小的成员资格证明

use super::AdsOperations;
use ark_serialize::CanonicalSerialize;
use common::RootHash;
use esa_rust::crypto_accumulator::acc::dynamic_accumulator::{DynamicAccumulator, QueryResult};
use std::collections::HashMap;

/// 密码学累加器 ADS 实现
pub struct CryptoAccumulatorAds {
    /// 存储每个 keyword 对应的累加器和文件列表
    /// HashMap<keyword, (accumulator, fid_list)>
    accumulators: HashMap<String, (DynamicAccumulator, Vec<String>)>,
}

impl CryptoAccumulatorAds {
    pub fn new() -> Self {
        CryptoAccumulatorAds {
            accumulators: HashMap::new(),
        }
    }

    /// 将 keyword+fid 组合转换为累加器元素
    ///
    /// 使用 keyword:fid 格式确保同一个 fid 在不同 keyword 下是不同的元素
    fn fid_to_element(keyword: &str, fid: &str) -> i64 {
        let combined = format!("{}:{}", keyword, fid);
        combined
            .bytes()
            .fold(0i64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as i64))
    }

    /// 序列化添加/删除证明
    ///
    /// 格式: [old_acc(96) | new_acc(96) | element(8) | valid(1)]
    /// 总计: 201 字节
    fn serialize_update_proof(
        old_acc: &ark_bls12_381::G1Affine,
        new_acc: &ark_bls12_381::G1Affine,
        element: i64,
        is_valid: bool,
    ) -> Vec<u8> {
        let mut proof = Vec::new();
        old_acc.serialize(&mut proof).unwrap();
        new_acc.serialize(&mut proof).unwrap();
        proof.extend_from_slice(&element.to_le_bytes());
        proof.push(if is_valid { 1 } else { 0 });
        proof
    }

    /// 序列化成员资格证明
    ///
    /// 格式: [witness(96) | element(8) | acc_value(96) | valid(1)]
    /// 总计: 201 字节
    fn serialize_membership_proof(
        witness: &ark_bls12_381::G1Affine,
        element: i64,
        acc_value: &ark_bls12_381::G1Affine,
        is_valid: bool,
    ) -> Vec<u8> {
        let mut proof = Vec::new();
        witness.serialize(&mut proof).unwrap();
        proof.extend_from_slice(&element.to_le_bytes());
        acc_value.serialize(&mut proof).unwrap();
        proof.push(if is_valid { 1 } else { 0 });
        proof
    }
}

impl AdsOperations for CryptoAccumulatorAds {
    fn add(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash) {
        let element = Self::fid_to_element(keyword, fid);

        let entry = self
            .accumulators
            .entry(keyword.to_string())
            .or_insert_with(|| (DynamicAccumulator::new(), Vec::new()));

        let old_acc_value = entry.0.acc_value;

        // Check if this fid is already in the list (防御性检查)
        if entry.1.contains(&fid.to_string()) {
            println!("Warning: fid '{}' already exists for keyword '{}', skipping add", fid, keyword);
            // Return current state without adding again
            let proof = Self::serialize_update_proof(&old_acc_value, &entry.0.acc_value, element, true);
            let mut root_hash = Vec::new();
            entry.0.acc_value.serialize(&mut root_hash).unwrap();
            return (proof, root_hash);
        }

        // 添加到累加器并验证
        let add_result = entry.0.add(&element);
        
        let is_valid = match add_result {
            Ok(proof) => proof.verify(),
            Err(e) => {
                eprintln!("Error adding element to accumulator for keyword='{}', fid='{}': {:?}", keyword, fid, e);
                // Return empty proof on error
                let proof = Self::serialize_update_proof(&old_acc_value, &old_acc_value, element, false);
                let mut root_hash = Vec::new();
                old_acc_value.serialize(&mut root_hash).unwrap();
                return (proof, root_hash);
            }
        };

        // 记录 fid
        entry.1.push(fid.to_string());

        // 序列化证明
        let proof =
            Self::serialize_update_proof(&old_acc_value, &entry.0.acc_value, element, is_valid);

        // 序列化 root hash
        let mut root_hash = Vec::new();
        entry.0.acc_value.serialize(&mut root_hash).unwrap();

        (proof, root_hash)
    }

    fn query(&self, keyword: &str) -> (Vec<String>, Vec<u8>) {
        if let Some((acc, fids)) = self.accumulators.get(keyword) {
            let proof = if !fids.is_empty() {
                let element = Self::fid_to_element(keyword, &fids[0]);

                match acc.query(&element) {
                    QueryResult::Membership(membership_proof) => {
                        let is_valid = membership_proof.verify(acc.acc_value);

                        Self::serialize_membership_proof(
                            &membership_proof.witness,
                            element,
                            &acc.acc_value,
                            is_valid,
                        )
                    }
                    _ => vec![0],
                }
            } else {
                vec![1] // 空结果有效
            };

            (fids.clone(), proof)
        } else {
            (vec![], vec![1])
        }
    }

    fn delete(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash) {
        let element = Self::fid_to_element(keyword, fid);

        if let Some((acc, fids)) = self.accumulators.get_mut(keyword) {
            let old_acc_value = acc.acc_value;

            // 从累加器删除并验证
            let delete_proof = acc
                .delete(&element)
                .expect("Failed to delete from accumulator");
            let is_valid = delete_proof.verify();

            fids.retain(|f| f != fid);

            // 序列化证明
            let proof =
                Self::serialize_update_proof(&old_acc_value, &acc.acc_value, element, is_valid);

            let root_hash = if fids.is_empty() {
                self.accumulators.remove(keyword);
                vec![]
            } else {
                let mut rh = Vec::new();
                acc.acc_value.serialize(&mut rh).unwrap();
                rh
            };

            (proof, root_hash)
        } else {
            (vec![0], vec![])
        }
    }
}
