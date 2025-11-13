#!/bin/bash

# Distributed Storage System - Test Script
# 运行完整的集成测试

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 项目根目录
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

echo -e "${BLUE}=== 分布式存储系统集成测试 ===${NC}"
echo ""

# 检查系统是否运行
if ! pgrep -f "target/debug/manager" > /dev/null; then
    echo -e "${YELLOW}系统未运行，正在启动...${NC}"
    ./scripts/start.sh
    sleep 3
else
    echo -e "${GREEN}系统已运行${NC}"
fi

echo ""
echo -e "${YELLOW}开始运行集成测试...${NC}"
echo ""

# 运行集成测试
cargo run --package client --example integration_test

echo ""
echo -e "${GREEN}=== 测试完成 ===${NC}"
echo ""

# 询问是否查看详细日志
read -p "是否查看详细日志? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo ""
    echo -e "${BLUE}=== Manager 日志 ===${NC}"
    tail -20 logs/manager.log
    echo ""
    echo -e "${BLUE}=== Storager 1 日志 ===${NC}"
    tail -10 logs/storager1.log
    echo ""
    echo -e "${BLUE}=== Storager 2 日志 ===${NC}"
    tail -10 logs/storager2.log
fi

echo ""
echo -e "${BLUE}提示:${NC}"
echo "  - 查看完整日志: tail -f logs/manager.log"
echo "  - 查看测试报告: cat logs/test_report.md"
