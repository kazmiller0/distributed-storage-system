#!/bin/bash

# Distributed Storage System - Stop Script
# 停止分布式存储系统的所有组件

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== 停止分布式存储系统 ===${NC}"
echo ""

# 停止 Manager
echo -e "${YELLOW}停止 Manager...${NC}"
if pkill -f "target/debug/manager" 2>/dev/null; then
    echo -e "  ${GREEN}✓ Manager 已停止${NC}"
else
    echo -e "  ${YELLOW}⚠ Manager 未运行${NC}"
fi

# 停止 Storager
echo -e "${YELLOW}停止 Storager 节点...${NC}"
if pkill -f "target/debug/storager" 2>/dev/null; then
    echo -e "  ${GREEN}✓ 所有 Storager 已停止${NC}"
else
    echo -e "  ${YELLOW}⚠ Storager 未运行${NC}"
fi

sleep 1

# 验证是否还有残留进程
if pgrep -f "target/debug/(manager|storager)" > /dev/null 2>&1; then
    echo -e "${RED}警告: 发现残留进程，强制终止...${NC}"
    pkill -9 -f "target/debug/manager" 2>/dev/null || true
    pkill -9 -f "target/debug/storager" 2>/dev/null || true
    sleep 1
fi

echo ""
echo -e "${GREEN}=== 系统已完全停止 ===${NC}"
echo ""
echo -e "${BLUE}提示:${NC}"
echo "  - 日志文件保存在 logs/ 目录"
echo "  - 重新启动: ./scripts/start.sh"
