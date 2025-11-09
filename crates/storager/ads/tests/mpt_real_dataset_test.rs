/// MPT ADS 真实数据集测试
/// 
/// 使用 data/testdata 中的真实数据测试 MPT 的性能和正确性

use esa_rust::mpt::{MPT, MPTError, KVPair};
use esa_rust::mpt::node::Database;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

/// 简单的内存数据库实现
#[derive(Clone)]
struct MemoryDB {
    data: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl MemoryDB {
    fn new() -> Self {
        MemoryDB {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    fn size(&self) -> usize {
        self.data.lock().unwrap().len()
    }
}

impl Database for MemoryDB {
    fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>, MPTError> {
        Ok(self.data.lock().unwrap().get(key).cloned())
    }

    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), MPTError> {
        self.data.lock().unwrap().insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), MPTError> {
        self.data.lock().unwrap().remove(key);
        Ok(())
    }
}

/// 数据记录结构
#[derive(Debug, Clone)]
struct DataRecord {
    fid: String,          // 文件ID
    category: String,     // 类别
    item: String,         // 物品
    attributes: Vec<String>, // 属性列表
}

impl DataRecord {
    fn parse(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 3 {
            return None;
        }
        
        Some(DataRecord {
            fid: parts[0].to_string(),
            category: parts[1].to_string(),
            item: parts[2].to_string(),
            attributes: parts[3..].iter().map(|s| s.to_string()).collect(),
        })
    }
}

#[test]
fn test_load_real_dataset() {
    println!("\n=== 加载真实数据集 ===");
    
    let data_path = "../../../data/testdata";
    let file = File::open(data_path).expect("无法打开数据文件");
    let reader = BufReader::new(file);
    
    let mut records = Vec::new();
    for line in reader.lines() {
        if let Ok(line) = line {
            if let Some(record) = DataRecord::parse(&line) {
                records.push(record);
            }
        }
    }
    
    println!("✓ 成功加载 {} 条记录", records.len());
    assert_eq!(records.len(), 100, "应该加载 100 条记录");
    
    // 显示前5条记录
    println!("\n前5条记录:");
    for (i, record) in records.iter().take(5).enumerate() {
        println!("  {}. fid={}, category={}, item={}, attrs={:?}", 
                 i+1, record.fid, record.category, record.item, record.attributes);
    }
}

#[test]
fn test_mpt_with_primary_index() {
    println!("\n=== 测试主索引：fid -> 完整记录 ===");
    
    let mut db = MemoryDB::new();
    let mut mpt = MPT::new(None);
    
    // 加载数据
    let data_path = "../../../data/testdata";
    let file = File::open(data_path).expect("无法打开数据文件");
    let reader = BufReader::new(file);
    
    let mut records = Vec::new();
    for line in reader.lines() {
        if let Ok(line) = line {
            if let Some(record) = DataRecord::parse(&line) {
                records.push(record);
            }
        }
    }
    
    println!("加载了 {} 条记录", records.len());
    
    // 插入主索引：fid -> category,item,attr1,attr2,...
    let start = Instant::now();
    for record in &records {
        let key = format!("fid:{}", record.fid);
        let mut value_parts = vec![record.category.clone(), record.item.clone()];
        value_parts.extend(record.attributes.clone());
        let value = value_parts.join(",");
        
        let kv = KVPair::new(key, value);
        mpt.insert(kv, &mut db, true, false).expect("插入失败");
    }
    let insert_time = start.elapsed();
    
    println!("✓ 插入 {} 条记录耗时: {:?}", records.len(), insert_time);
    println!("✓ 平均每条: {:?}", insert_time / records.len() as u32);
    
    // 批量修复（持久化所有节点）
    let start = Instant::now();
    mpt.batch_fix(&mut db).expect("batch_fix 失败");
    let fix_time = start.elapsed();
    println!("✓ batch_fix 耗时: {:?}", fix_time);
    
    let root_hash = mpt.get_root_hash();
    println!("✓ 根哈希: {:x?}", &root_hash[..8]);
    println!("✓ 数据库节点数: {}", db.size());
    
    // 随机查询测试
    println!("\n随机查询验证:");
    let test_indices = [0, 10, 50, 99]; // 测试第1, 11, 51, 100条记录
    
    let start = Instant::now();
    for &idx in &test_indices {
        let record = &records[idx];
        let key = format!("fid:{}", record.fid);
        
        let (value, proof) = mpt.query_by_key(&key, &mut db).expect("查询失败");
        
        // 验证值
        let mut expected_parts = vec![record.category.clone(), record.item.clone()];
        expected_parts.extend(record.attributes.clone());
        let expected = expected_parts.join(",");
        assert_eq!(value, expected);
        
        // 验证证明
        let is_valid = mpt.verify_query_result(&value, &proof);
        assert!(is_valid);
        
        println!("  ✓ fid:{} -> {} (证明有效)", record.fid, value);
    }
    let query_time = start.elapsed();
    println!("✓ 查询 {} 条记录耗时: {:?}", test_indices.len(), query_time);
    println!("✓ 平均每条: {:?}", query_time / test_indices.len() as u32);
}

#[test]
fn test_mpt_with_secondary_index() {
    println!("\n=== 测试辅助索引：category -> fid列表 ===");
    
    let mut db = MemoryDB::new();
    let mut mpt = MPT::new(None);
    
    // 加载数据
    let data_path = "../../../data/testdata";
    let file = File::open(data_path).expect("无法打开数据文件");
    let reader = BufReader::new(file);
    
    let mut records = Vec::new();
    for line in reader.lines() {
        if let Ok(line) = line {
            if let Some(record) = DataRecord::parse(&line) {
                records.push(record);
            }
        }
    }
    
    println!("加载了 {} 条记录", records.len());
    
    // 插入辅助索引：category -> fid (一个类别对应多个fid)
    let start = Instant::now();
    for record in &records {
        let key = format!("category:{}", record.category);
        let value = record.fid.clone();
        
        let kv = KVPair::new(key, value);
        // is_primary=false 表示辅助索引，允许多值
        mpt.insert(kv, &mut db, false, false).expect("插入失败");
    }
    let insert_time = start.elapsed();
    
    println!("✓ 插入 {} 条辅助索引耗时: {:?}", records.len(), insert_time);
    
    // 批量修复
    mpt.batch_fix(&mut db).expect("batch_fix 失败");
    
    let root_hash = mpt.get_root_hash();
    println!("✓ 根哈希: {:x?}", &root_hash[..8]);
    
    // 统计每个类别的记录数
    let mut category_counts: HashMap<String, Vec<String>> = HashMap::new();
    for record in &records {
        category_counts.entry(record.category.clone())
            .or_insert_with(Vec::new)
            .push(record.fid.clone());
    }
    
    println!("\n类别统计:");
    for (category, fids) in &category_counts {
        println!("  {} 类: {} 条记录", category, fids.len());
    }
    
    // 查询每个类别
    println!("\n查询验证:");
    for (category, expected_fids) in &category_counts {
        let key = format!("category:{}", category);
        let (value, proof) = mpt.query_by_key(&key, &mut db).expect("查询失败");
        
        // 验证值包含所有fid
        let result_fids: Vec<&str> = value.split(',').collect();
        assert_eq!(result_fids.len(), expected_fids.len());
        
        for fid in expected_fids {
            assert!(value.contains(fid), "category:{} 应该包含 fid:{}", category, fid);
        }
        
        // 验证证明
        let is_valid = mpt.verify_query_result(&value, &proof);
        assert!(is_valid);
        
        println!("  ✓ category:{} -> {} 个文件 (证明有效)", category, result_fids.len());
    }
}

#[test]
fn test_mpt_with_attribute_index() {
    println!("\n=== 测试属性索引：color -> fid列表 ===");
    
    let mut db = MemoryDB::new();
    let mut mpt = MPT::new(None);
    
    // 加载数据
    let data_path = "../../../data/testdata";
    let file = File::open(data_path).expect("无法打开数据文件");
    let reader = BufReader::new(file);
    
    let mut records = Vec::new();
    for line in reader.lines() {
        if let Ok(line) = line {
            if let Some(record) = DataRecord::parse(&line) {
                records.push(record);
            }
        }
    }
    
    println!("加载了 {} 条记录", records.len());
    
    // 插入属性索引：对于每个属性，建立 attr_value -> fid 的映射
    let start = Instant::now();
    for record in &records {
        for attr in &record.attributes {
            let key = format!("attr:{}", attr);
            let value = record.fid.clone();
            
            let kv = KVPair::new(key, value);
            mpt.insert(kv, &mut db, false, false).expect("插入失败");
        }
    }
    let insert_time = start.elapsed();
    
    println!("✓ 插入属性索引耗时: {:?}", insert_time);
    
    // 批量修复
    mpt.batch_fix(&mut db).expect("batch_fix 失败");
    
    let root_hash = mpt.get_root_hash();
    println!("✓ 根哈希: {:x?}", &root_hash[..8]);
    
    // 统计属性分布
    let mut attr_counts: HashMap<String, Vec<String>> = HashMap::new();
    for record in &records {
        for attr in &record.attributes {
            attr_counts.entry(attr.clone())
                .or_insert_with(Vec::new)
                .push(record.fid.clone());
        }
    }
    
    println!("\n属性统计 (Top 10):");
    let mut sorted_attrs: Vec<_> = attr_counts.iter().collect();
    sorted_attrs.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    
    for (attr, fids) in sorted_attrs.iter().take(10) {
        println!("  attr:{} -> {} 个文件", attr, fids.len());
    }
    
    // 测试查询常见属性
    println!("\n查询验证:");
    let test_attrs = ["red", "black", "white", "art"];
    
    for attr in &test_attrs {
        if let Some(expected_fids) = attr_counts.get(*attr) {
            let key = format!("attr:{}", attr);
            let (value, proof) = mpt.query_by_key(&key, &mut db).expect("查询失败");
            
            // 验证值包含所有fid
            let result_fids: Vec<&str> = value.split(',').collect();
            assert_eq!(result_fids.len(), expected_fids.len());
            
            // 验证证明
            let is_valid = mpt.verify_query_result(&value, &proof);
            assert!(is_valid);
            
            println!("  ✓ attr:{} -> {} 个文件 (证明有效)", attr, result_fids.len());
        }
    }
}

#[test]
fn test_mpt_performance_summary() {
    println!("\n=== MPT 性能综合测试 ===");
    
    let mut db = MemoryDB::new();
    let mut mpt = MPT::new(None);
    
    // 加载数据
    let data_path = "../../../data/testdata";
    let file = File::open(data_path).expect("无法打开数据文件");
    let reader = BufReader::new(file);
    
    let mut records = Vec::new();
    for line in reader.lines() {
        if let Ok(line) = line {
            if let Some(record) = DataRecord::parse(&line) {
                records.push(record);
            }
        }
    }
    
    println!("数据集大小: {} 条记录\n", records.len());
    
    // 1. 主索引插入性能
    println!("【主索引插入】");
    let start = Instant::now();
    for record in &records {
        let key = format!("fid:{}", record.fid);
        let mut value_parts = vec![record.category.clone(), record.item.clone()];
        value_parts.extend(record.attributes.clone());
        let value = value_parts.join(",");
        
        let kv = KVPair::new(key, value);
        mpt.insert(kv, &mut db, true, false).expect("插入失败");
    }
    let insert_time = start.elapsed();
    println!("  总耗时: {:?}", insert_time);
    println!("  平均: {:?}/条", insert_time / records.len() as u32);
    println!("  吞吐量: {:.2} 条/秒", records.len() as f64 / insert_time.as_secs_f64());
    
    // 2. 批量修复性能
    println!("\n【批量修复】");
    let start = Instant::now();
    mpt.batch_fix(&mut db).expect("batch_fix 失败");
    let fix_time = start.elapsed();
    println!("  耗时: {:?}", fix_time);
    
    // 3. 查询性能（所有记录）
    println!("\n【查询性能】");
    let start = Instant::now();
    for record in &records {
        let key = format!("fid:{}", record.fid);
        let (value, proof) = mpt.query_by_key(&key, &mut db).expect("查询失败");
        let is_valid = mpt.verify_query_result(&value, &proof);
        assert!(is_valid);
    }
    let query_time = start.elapsed();
    println!("  总耗时: {:?}", query_time);
    println!("  平均: {:?}/条", query_time / records.len() as u32);
    println!("  吞吐量: {:.2} 条/秒", records.len() as f64 / query_time.as_secs_f64());
    
    // 4. 存储统计
    println!("\n【存储统计】");
    println!("  数据库节点数: {}", db.size());
    println!("  根哈希: {:x?}", &mpt.get_root_hash()[..16]);
    
    // 5. 更新性能
    println!("\n【更新性能】");
    let start = Instant::now();
    for i in 0..10 {
        let record = &records[i];
        let key = format!("fid:{}", record.fid);
        let value = format!("updated_{}", i);
        
        let kv = KVPair::new(key, value);
        mpt.insert(kv, &mut db, true, false).expect("更新失败");
    }
    let update_time = start.elapsed();
    println!("  更新10条耗时: {:?}", update_time);
    println!("  平均: {:?}/条", update_time / 10);
    
    // 6. 删除性能
    println!("\n【删除性能】");
    let start = Instant::now();
    for i in 0..10 {
        let record = &records[i];
        let key = format!("fid:{}", record.fid);
        mpt.delete(&key, &mut db).expect("删除失败");
    }
    let delete_time = start.elapsed();
    println!("  删除10条耗时: {:?}", delete_time);
    println!("  平均: {:?}/条", delete_time / 10);
}
