#!/bin/bash

# Distributed Storage System - Start Script
# 启动分布式存储系统的所有组件

set -e  # 遇到错误立即退出

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 项目根目录
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

echo -e "${BLUE}=== 分布式存储系统启动脚本 ===${NC}"
echo ""

# 创建日志目录
mkdir -p logs

# 检查并清理旧进程
echo -e "${YELLOW}[1/4] 清理旧进程...${NC}"
pkill -f "target/debug/manager" 2>/dev/null && echo "  - 已停止旧的 Manager 进程" || true
pkill -f "target/debug/storager" 2>/dev/null && echo "  - 已停止旧的 Storager 进程" || true
sleep 1

# 编译项目
echo -e "${YELLOW}[2/4] 编译项目...${NC}"
cargo build --quiet
if [ $? -eq 0 ]; then
    echo -e "  ${GREEN}✓ 编译成功${NC}"
else
    echo -e "  ${RED}✗ 编译失败${NC}"
    exit 1
fi

# 启动 Storager 节点
echo -e "${YELLOW}[3/4] 启动 Storager 节点 (使用 MPT)...${NC}"
./target/debug/storager 50052 mpt > logs/storager1.log 2>&1 &
STORAGER1_PID=$!
echo "  - Storager 1 启动 (PID: $STORAGER1_PID, Port: 50052, ADS: MPT)"

./target/debug/storager 50053 mpt > logs/storager2.log 2>&1 &
STORAGER2_PID=$!
echo "  - Storager 2 启动 (PID: $STORAGER2_PID, Port: 50053, ADS: MPT)"

./target/debug/storager 50054 mpt > logs/storager3.log 2>&1 &
STORAGER3_PID=$!
echo "  - Storager 3 启动 (PID: $STORAGER3_PID, Port: 50054, ADS: MPT)"

sleep 2

# 启动 Manager
echo -e "${YELLOW}[4/4] 启动 Manager 节点 (使用 MPT)...${NC}"
./target/debug/manager --ads-mode mpt > logs/manager.log 2>&1 &
MANAGER_PID=$!
echo "  - Manager 启动 (PID: $MANAGER_PID, Port: 50051, ADS: MPT)"

sleep 2

# 验证服务是否正常启动
echo ""
echo -e "${GREEN}=== 系统启动成功！===${NC}"
echo ""
echo -e "${BLUE}服务信息:${NC}"
echo "  📊 Manager:    [::1]:50051 (PID: $MANAGER_PID)"
echo "  💾 Storager 1: [::1]:50052 (PID: $STORAGER1_PID)"
echo "  💾 Storager 2: [::1]:50053 (PID: $STORAGER2_PID)"
echo "  💾 Storager 3: [::1]:50054 (PID: $STORAGER3_PID)"
echo ""
echo -e "${BLUE}使用方法:${NC}"
echo "  📝 运行客户端:     ./target/debug/client"
echo "  🧪 运行集成测试:   cargo run --package client --example integration_test"
echo "  📋 查看日志:       tail -f logs/manager.log"
echo "  🛑 停止系统:       ./scripts/stop.sh"
echo ""
echo -e "${YELLOW}提示: 日志文件保存在 logs/ 目录${NC}"
