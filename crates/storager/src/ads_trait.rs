/// ADS 操作的通用trait
/// 所有认证数据结构都需要实现这个 trait
use common::RootHash;

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
