//! 负载均衡测试示例
//!
//! 演示一致性哈希环如何实现负载均衡

use consistent_hash::ConsistentHashRing;
use std::collections::HashMap;

fn main() {
    println!("=== 负载均衡测试 ===\n");

    // 创建包含3个节点的哈希环
    let mut ring = ConsistentHashRing::new();
    ring.add_node("node1", 150);
    ring.add_node("node2", 150);
    ring.add_node("node3", 150);

    println!("配置:");
    println!("  节点数: {}", ring.node_count());
    println!("  每节点虚拟节点数: 150");
    println!("  虚拟节点总数: {}", ring.virtual_node_count());

    // 生成测试键
    let num_keys = 10000;
    let keys: Vec<String> = (0..num_keys).map(|i| format!("key_{}", i)).collect();

    println!("\n生成 {} 个测试键", num_keys);

    // 计算分布
    let distribution = ring.get_distribution(&keys);

    println!("\n负载分布:");
    println!("  {:<10} {:>8} {:>8}", "节点", "键数量", "百分比");
    println!("  {}", "-".repeat(30));

    let mut total_keys = 0;
    for (node, count) in distribution.iter() {
        let percentage = (*count as f64 / num_keys as f64) * 100.0;
        println!("  {:<10} {:>8} {:>7.2}%", node, count, percentage);
        total_keys += count;
    }

    println!("  {}", "-".repeat(30));
    println!("  {:<10} {:>8}", "总计", total_keys);

    // 计算标准差
    let avg = num_keys as f64 / ring.node_count() as f64;
    let variance: f64 = distribution
        .values()
        .map(|&count| {
            let diff = count as f64 - avg;
            diff * diff
        })
        .sum::<f64>()
        / ring.node_count() as f64;
    let std_dev = variance.sqrt();
    let cv = std_dev / avg; // 变异系数

    println!("\n负载均衡度:");
    println!("  平均值: {:.2}", avg);
    println!("  标准差: {:.2}", std_dev);
    println!("  变异系数: {:.4}", cv);

    if cv < 0.05 {
        println!("  ✅ 负载分布非常均衡");
    } else if cv < 0.10 {
        println!("  ✅ 负载分布良好");
    } else {
        println!("  ⚠️  负载分布有偏差");
    }

    // 测试添加节点的影响
    println!("\n=== 添加新节点测试 ===");

    // 记录添加前的映射
    let before: HashMap<String, String> = keys
        .iter()
        .map(|k| (k.clone(), ring.get_node(k).unwrap()))
        .collect();

    // 添加新节点
    ring.add_node("node4", 150);
    println!("添加 node4 后:");
    println!("  节点数: {}", ring.node_count());

    // 记录添加后的映射
    let after: HashMap<String, String> = keys
        .iter()
        .map(|k| (k.clone(), ring.get_node(k).unwrap()))
        .collect();

    // 计算变化
    let changed = keys
        .iter()
        .filter(|k| before.get(*k) != after.get(*k))
        .count();

    let change_ratio = changed as f64 / num_keys as f64;
    let expected_ratio = 1.0 / ring.node_count() as f64;

    println!("\n迁移统计:");
    println!(
        "  改变的键: {} / {} ({:.2}%)",
        changed,
        num_keys,
        change_ratio * 100.0
    );
    println!("  理论值: {:.2}%", expected_ratio * 100.0);
    println!(
        "  偏差: {:.2}%",
        (change_ratio - expected_ratio).abs() * 100.0
    );

    if (change_ratio - expected_ratio).abs() < 0.05 {
        println!("  ✅ 迁移量接近理论值");
    }

    // 新的分布
    let new_distribution = ring.get_distribution(&keys);
    println!("\n新的负载分布:");
    println!("  {:<10} {:>8} {:>8}", "节点", "键数量", "百分比");
    println!("  {}", "-".repeat(30));

    for (node, count) in new_distribution.iter() {
        let percentage = (*count as f64 / num_keys as f64) * 100.0;
        println!("  {:<10} {:>8} {:>7.2}%", node, count, percentage);
    }

    println!("\n=== 测试结束 ===");
}
