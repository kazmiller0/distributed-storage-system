#!/bin/bash

# Distributed Storage System - Stop Script

echo "Stopping Distributed Storage System..."

pkill -f "target/debug/manager"
pkill -f "target/debug/storager"

echo "System stopped."
