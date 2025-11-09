#!/bin/bash

# Distributed Storage System - Start Script

echo "Starting Distributed Storage System..."

# Kill any existing instances
pkill -f "target/debug/manager" 2>/dev/null
pkill -f "target/debug/storager" 2>/dev/null
sleep 1

# Build the project
echo "Building project..."
cargo build

# Start storagers in background
echo "Starting Storager instances..."
./target/debug/storager 50052 > logs/storager1.log 2>&1 &
./target/debug/storager 50053 > logs/storager2.log 2>&1 &
./target/debug/storager 50054 > logs/storager3.log 2>&1 &

sleep 2

# Start manager in background
echo "Starting Manager..."
./target/debug/manager > logs/manager.log 2>&1 &

sleep 2

echo "System is running!"
echo "  - Manager: [::1]:50051"
echo "  - Storager 1: [::1]:50052"
echo "  - Storager 2: [::1]:50053"
echo "  - Storager 3: [::1]:50054"
echo ""
echo "Run './target/debug/client' to interact with the system"
echo "Check logs/ directory for server logs"
echo ""
echo "To stop: pkill -f 'target/debug/manager' && pkill -f 'target/debug/storager'"
