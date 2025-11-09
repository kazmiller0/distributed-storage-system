#!/bin/bash

# 布尔查询功能测试脚本

echo "==================================="
echo "  分布式存储系统 - 布尔查询测试"
echo "==================================="
echo ""

# 检查是否已有进程在运行
if pgrep -f "target/debug/manager" > /dev/null; then
    echo "⚠️  检测到 manager 进程正在运行，先停止..."
    pkill -f "target/debug/manager"
    sleep 1
fi

if pgrep -f "target/debug/storager" > /dev/null; then
    echo "⚠️  检测到 storager 进程正在运行，先停止..."
    pkill -f "target/debug/storager"
    sleep 1
fi

# 编译项目
echo "🔨 编译项目..."
cargo build --quiet 2>&1 | grep -E "(error|warning:)" || echo "✅ 编译成功"
echo ""

# 启动 manager
echo "🚀 启动 Manager (端口 50051)..."
./target/debug/manager > logs/manager.log 2>&1 &
MANAGER_PID=$!
echo "   Manager PID: $MANAGER_PID"

# 等待 manager 启动
sleep 2

# 启动 storagers
echo "🚀 启动 Storager 1 (端口 50052)..."
./target/debug/storager 50052 > logs/storager1.log 2>&1 &
STORAGER1_PID=$!
echo "   Storager 1 PID: $STORAGER1_PID"

echo "🚀 启动 Storager 2 (端口 50053)..."
./target/debug/storager 50053 > logs/storager2.log 2>&1 &
STORAGER2_PID=$!
echo "   Storager 2 PID: $STORAGER2_PID"

echo "🚀 启动 Storager 3 (端口 50054)..."
./target/debug/storager 50054 > logs/storager3.log 2>&1 &
STORAGER3_PID=$!
echo "   Storager 3 PID: $STORAGER3_PID"

# 等待所有服务启动
echo ""
echo "⏳ 等待服务启动..."
sleep 3

# 检查服务是否启动成功
if ! ps -p $MANAGER_PID > /dev/null; then
    echo "❌ Manager 启动失败，请检查 logs/manager.log"
    exit 1
fi

if ! ps -p $STORAGER1_PID > /dev/null; then
    echo "❌ Storager 1 启动失败，请检查 logs/storager1.log"
    exit 1
fi

if ! ps -p $STORAGER2_PID > /dev/null; then
    echo "❌ Storager 2 启动失败，请检查 logs/storager2.log"
    exit 1
fi

if ! ps -p $STORAGER3_PID > /dev/null; then
    echo "❌ Storager 3 启动失败，请检查 logs/storager3.log"
    exit 1
fi

echo "✅ 所有服务启动成功"
echo ""

# 运行测试
echo "==================================="
echo "  开始运行布尔查询测试"
echo "==================================="
echo ""

cargo run -p client --example boolean_query_test

TEST_EXIT_CODE=$?

echo ""
echo "==================================="
echo "  测试完成"
echo "==================================="
echo ""

# 停止服务
echo "🛑 停止服务..."
kill $MANAGER_PID 2>/dev/null
kill $STORAGER1_PID 2>/dev/null
kill $STORAGER2_PID 2>/dev/null
kill $STORAGER3_PID 2>/dev/null

sleep 1

echo "✅ 所有服务已停止"
echo ""

# 显示日志位置
echo "📝 日志文件位置:"
echo "   Manager:    logs/manager.log"
echo "   Storager 1: logs/storager1.log"
echo "   Storager 2: logs/storager2.log"
echo "   Storager 3: logs/storager3.log"
echo ""

exit $TEST_EXIT_CODE
