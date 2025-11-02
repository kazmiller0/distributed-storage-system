#!/bin/bash

echo "=== 综合功能测试 ==="
echo ""

# 测试 1: 添加多个文件
echo "📝 测试 1: 添加多个文件"
cat << 'RUST' | ./target/debug/client
// 这里只是示例，实际我们直接运行客户端
RUST

# 显示结果
echo "✅ 测试完成！"
echo ""
echo "查看日志分布："
echo "Storager 1 处理的关键词:"
grep "keyword=" logs/storager1.log | awk -F'keyword=' '{print $2}' | awk -F',' '{print "  - " $1}' | sort -u

echo "Storager 2 处理的关键词:"
grep "keyword=" logs/storager2.log | awk -F'keyword=' '{print $2}' | awk -F',' '{print "  - " $1}' | sort -u

echo "Storager 3 处理的关键词:"
grep "keyword=" logs/storager3.log | awk -F'keyword=' '{print $2}' | awk -F',' '{print "  - " $1}' | sort -u

