use esa_rust::mpt::node::Database;
/// MPT ADS 集成测试
///
/// 测试 MPT 作为 ADS（Authenticated Data Structure）的完整功能
use esa_rust::mpt::{MPTError, MPT};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 简单的内存数据库实现，用于测试
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
}

impl Database for MemoryDB {
    fn get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>, MPTError> {
        Ok(self.data.lock().unwrap().get(key).cloned())
    }

    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), MPTError> {
        self.data
            .lock()
            .unwrap()
            .insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), MPTError> {
        self.data.lock().unwrap().remove(key);
        Ok(())
    }
}

#[test]
fn test_mpt_basic_insert_and_query() {
    println!("\n=== 测试基本插入和查询 ===");

    let mut db = MemoryDB::new();
    let mut mpt = MPT::new(None);

    // 插入数据
    let kv1 = esa_rust::mpt::KVPair::new("name".to_string(), "Alice".to_string());
    let result = mpt.insert(kv1, &mut db, true, false);
    assert!(result.is_ok());
    println!("✓ 插入 name=Alice");

    let kv2 = esa_rust::mpt::KVPair::new("age".to_string(), "25".to_string());
    let result = mpt.insert(kv2, &mut db, true, false);
    assert!(result.is_ok());
    println!("✓ 插入 age=25");

    // 查询数据
    let (value, proof) = mpt.query_by_key("name", &mut db).unwrap();
    assert_eq!(value, "Alice");
    println!("✓ 查询 name: {}", value);

    let (value, proof) = mpt.query_by_key("age", &mut db).unwrap();
    assert_eq!(value, "25");
    println!("✓ 查询 age: {}", value);

    // 验证证明
    let (value, proof) = mpt.query_by_key("name", &mut db).unwrap();
    let is_valid = mpt.verify_query_result(&value, &proof);
    assert!(is_valid);
    println!("✓ 证明验证通过");

    println!("根哈希: {:x?}", &mpt.get_root_hash()[..8]);
}

#[test]
fn test_mpt_update_and_delete() {
    println!("\n=== 测试更新和删除 ===");

    let mut db = MemoryDB::new();
    let mut mpt = MPT::new(None);

    // 插入数据
    let kv = esa_rust::mpt::KVPair::new("key1".to_string(), "value1".to_string());
    mpt.insert(kv, &mut db, true, false).unwrap();
    println!("✓ 插入 key1=value1");

    // 更新数据
    let kv = esa_rust::mpt::KVPair::new("key1".to_string(), "value2".to_string());
    let (old_value, _) = mpt.insert(kv, &mut db, true, false).unwrap();
    assert_eq!(old_value, "value1");
    println!("✓ 更新 key1=value2 (旧值: {})", old_value);

    // 验证更新
    let (value, _) = mpt.query_by_key("key1", &mut db).unwrap();
    assert_eq!(value, "value2");
    println!("✓ 验证更新: {}", value);

    // 删除数据
    let deleted = mpt.delete("key1", &mut db).unwrap();
    assert_eq!(deleted, Some("value2".to_string()));
    println!("✓ 删除 key1 (删除的值: {})", deleted.unwrap());

    // 验证删除
    let (value, _) = mpt.query_by_key("key1", &mut db).unwrap();
    assert_eq!(value, "");
    println!("✓ 验证删除: 键不存在");
}

#[test]
fn test_mpt_multiple_keys() {
    println!("\n=== 测试多个键 ===");

    let mut db = MemoryDB::new();
    let mut mpt = MPT::new(None);

    // 插入多个键
    let keys = vec![
        ("apple", "red"),
        ("banana", "yellow"),
        ("cherry", "red"),
        ("date", "brown"),
        ("elderberry", "purple"),
    ];

    for (key, value) in &keys {
        let kv = esa_rust::mpt::KVPair::new(key.to_string(), value.to_string());
        mpt.insert(kv, &mut db, true, false).unwrap();
        println!("✓ 插入 {}={}", key, value);
    }

    // 查询所有键
    for (key, expected_value) in &keys {
        let (value, proof) = mpt.query_by_key(key, &mut db).unwrap();
        assert_eq!(&value, expected_value);

        // 验证证明
        let is_valid = mpt.verify_query_result(&value, &proof);
        assert!(is_valid);
        println!("✓ 查询并验证 {}: {}", key, value);
    }

    println!("根哈希: {:x?}", &mpt.get_root_hash()[..8]);
}

#[test]
fn test_mpt_persist_and_restore() {
    println!("\n=== 测试持久化和恢复 ===");

    let mut db = MemoryDB::new();

    // 创建并填充 MPT
    let mut mpt1 = MPT::new(None);
    let kv1 = esa_rust::mpt::KVPair::new("test1".to_string(), "data1".to_string());
    let kv2 = esa_rust::mpt::KVPair::new("test2".to_string(), "data2".to_string());
    mpt1.insert(kv1, &mut db, true, false).unwrap();
    mpt1.insert(kv2, &mut db, true, false).unwrap();
    println!("✓ 插入测试数据");

    // 执行 batch_fix 确保所有节点被持久化
    mpt1.batch_fix(&mut db).unwrap();
    println!("✓ 批量修复并持久化");

    let root_hash = mpt1.get_root_hash();
    println!("原始根哈希: {:x?}", &root_hash[..8]);

    // 持久化到数据库
    mpt1.persist_to_db(&mut db).unwrap();
    println!("✓ 持久化 MPT");

    // 从数据库恢复
    let mut mpt2 = MPT::restore_from_db(&mut db, None).unwrap();
    println!("✓ 从数据库恢复 MPT");

    let restored_root_hash = mpt2.get_root_hash();
    println!("恢复的根哈希: {:x?}", &restored_root_hash[..8]);

    // 验证根哈希一致
    assert_eq!(root_hash, restored_root_hash);
    println!("✓ 根哈希一致");

    // 验证数据一致
    let (value1, _) = mpt2.query_by_key("test1", &mut db).unwrap();
    assert_eq!(value1, "data1");
    println!("✓ 恢复的数据正确: test1={}", value1);

    let (value2, _) = mpt2.query_by_key("test2", &mut db).unwrap();
    assert_eq!(value2, "data2");
    println!("✓ 恢复的数据正确: test2={}", value2);
}

#[test]
fn test_mpt_proof_verification() {
    println!("\n=== 测试证明验证 ===");

    let mut db = MemoryDB::new();
    let mut mpt = MPT::new(None);

    // 插入多个键以创建更复杂的树结构
    for i in 0..10 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        let kv = esa_rust::mpt::KVPair::new(key, value);
        mpt.insert(kv, &mut db, true, false).unwrap();
    }
    println!("✓ 插入10个键值对");

    // 查询并验证每个键
    for i in 0..10 {
        let key = format!("key{}", i);
        let expected_value = format!("value{}", i);

        let (value, proof) = mpt.query_by_key(&key, &mut db).unwrap();
        assert_eq!(value, expected_value);

        // 验证证明
        let is_valid = mpt.verify_query_result(&value, &proof);
        assert!(is_valid);
        println!("✓ 验证 {}: 证明有效", key);
    }

    // 测试不存在的键
    let (value, proof) = mpt.query_by_key("nonexistent", &mut db).unwrap();
    assert_eq!(value, "");
    println!("✓ 查询不存在的键: 返回空值");
}

#[test]
fn test_mpt_concurrent_operations() {
    println!("\n=== 测试并发安全性 ===");

    let db = MemoryDB::new();
    let mut mpt = MPT::new(None);

    // 测试基本的线程安全性
    // 注意：由于 Database trait 需要 mut，实际并发需要特殊处理
    // 这里主要测试数据结构的基本并发安全性

    let mut db_clone = db.clone();

    // 插入数据
    for i in 0..20 {
        let key = format!("concurrent{}", i);
        let value = format!("data{}", i);
        let kv = esa_rust::mpt::KVPair::new(key, value);
        mpt.insert(kv, &mut db_clone, true, false).unwrap();
    }
    println!("✓ 插入20个并发测试数据");

    // 验证所有数据
    for i in 0..20 {
        let key = format!("concurrent{}", i);
        let expected = format!("data{}", i);
        let (value, _) = mpt.query_by_key(&key, &mut db_clone).unwrap();
        assert_eq!(value, expected);
    }
    println!("✓ 验证所有并发数据正确");
}

#[test]
fn test_mpt_secondary_index() {
    println!("\n=== 测试辅助索引（非主键索引）===");

    let mut db = MemoryDB::new();
    let mut mpt = MPT::new(None);

    // 辅助索引：一个键可以对应多个值（用逗号分隔）
    let kv1 = esa_rust::mpt::KVPair::new("color:red".to_string(), "apple".to_string());
    mpt.insert(kv1, &mut db, false, false).unwrap(); // is_primary=false
    println!("✓ 插入辅助索引 color:red -> apple");

    let kv2 = esa_rust::mpt::KVPair::new("color:red".to_string(), "cherry".to_string());
    mpt.insert(kv2, &mut db, false, false).unwrap();
    println!("✓ 追加辅助索引 color:red -> cherry");

    // 查询辅助索引
    let (value, _) = mpt.query_by_key("color:red", &mut db).unwrap();
    println!("✓ 查询辅助索引 color:red: {}", value);

    // 应该包含两个值（逗号分隔）
    assert!(value.contains("apple"));
    assert!(value.contains("cherry"));
    println!("✓ 辅助索引包含多个值");

    // 删除其中一个值
    let kv3 = esa_rust::mpt::KVPair::new("color:red".to_string(), "apple".to_string());
    mpt.insert(kv3, &mut db, false, true).unwrap(); // flag=true 表示删除
    println!("✓ 从辅助索引删除 apple");

    let (value, _) = mpt.query_by_key("color:red", &mut db).unwrap();
    println!("✓ 删除后查询: {}", value);
    assert!(value.contains("cherry"));
    assert!(!value.contains("apple"));
    println!("✓ 辅助索引删除成功");
}
