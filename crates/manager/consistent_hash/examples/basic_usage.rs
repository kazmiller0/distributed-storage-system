//! 基本使用示例
//!
//! 演示一致性哈希环的基本操作

use consistent_hash::ConsistentHashRing;

fn main() {
    println!("=== 一致性哈希环基本示例 ===\n");

    // 1. 创建哈希环
    println!("1. 创建哈希环并添加3个节点");
    let mut ring = ConsistentHashRing::new();
    ring.add_node("server1", 150);
    ring.add_node("server2", 150);
    ring.add_node("server3", 150);
    println!("   物理节点数: {}", ring.node_count());
    println!("   虚拟节点数: {}", ring.virtual_node_count());

    // 2. 路由键到节点
    println!("\n2. 测试键路由");
    let test_keys = vec!["user_123", "user_456", "user_789", "session_abc"];
    for key in test_keys {
        let node = ring.get_node(key).unwrap();
        println!("   {} -> {}", key, node);
    }

    // 3. 验证一致性
    println!("\n3. 验证一致性（同一个键总是映射到同一个节点）");
    let key = "user_12345";
    let node1 = ring.get_node(key).unwrap();
    let node2 = ring.get_node(key).unwrap();
    let node3 = ring.get_node(key).unwrap();
    println!("   第1次查询: {} -> {}", key, node1);
    println!("   第2次查询: {} -> {}", key, node2);
    println!("   第3次查询: {} -> {}", key, node3);
    assert_eq!(node1, node2);
    assert_eq!(node2, node3);
    println!("   ✅ 一致性验证通过");

    // 4. 获取多个副本节点
    println!("\n4. 获取多个副本节点（用于数据冗余）");
    let replicas = ring.get_nodes("important_data", 3);
    println!("   important_data 应该存储在: {:?}", replicas);

    // 5. 动态添加节点
    println!("\n5. 动态添加新节点");
    println!(
        "   添加前，user_12345 -> {}",
        ring.get_node("user_12345").unwrap()
    );
    ring.add_node("server4", 150);
    println!("   添加 server4 后");
    println!("   物理节点数: {}", ring.node_count());
    println!("   user_12345 -> {}", ring.get_node("user_12345").unwrap());

    // 6. 移除节点
    println!("\n6. 移除节点");
    ring.remove_node("server1");
    println!("   移除 server1 后");
    println!("   物理节点数: {}", ring.node_count());
    println!("   所有节点: {:?}", ring.get_all_nodes());

    // 7. 使用 with_nodes 快速创建
    println!("\n7. 使用 with_nodes 快速创建");
    let nodes = vec!["cache1", "cache2", "cache3"];
    let cache_ring = ConsistentHashRing::with_nodes(&nodes, 100);
    println!("   创建了包含 {} 个节点的缓存环", cache_ring.node_count());
    println!("   虚拟节点总数: {}", cache_ring.virtual_node_count());

    println!("\n=== 示例结束 ===");
}
