.PHONY: help build test run clean start stop logs fmt clippy check install

# 默认目标
help:
	@echo "分布式存储系统 - 可用命令:"
	@echo ""
	@echo "  make build       - 编译项目 (debug 模式)"
	@echo "  make release     - 编译项目 (release 模式)"
	@echo "  make test        - 运行所有测试"
	@echo "  make start       - 启动系统"
	@echo "  make stop        - 停止系统"
	@echo "  make restart     - 重启系统"
	@echo "  make run-client  - 运行客户端"
	@echo "  make integration - 运行集成测试"
	@echo "  make logs        - 查看 Manager 日志"
	@echo "  make logs-all    - 查看所有日志"
	@echo "  make fmt         - 格式化代码"
	@echo "  make clippy      - 运行 linter"
	@echo "  make check       - 检查编译"
	@echo "  make clean       - 清理构建产物"
	@echo "  make clean-logs  - 清理日志文件"
	@echo "  make doc         - 生成文档"
	@echo ""

# 编译
build:
	@echo "编译项目 (debug)..."
	@cargo build

release:
	@echo "编译项目 (release)..."
	@cargo build --release

# 测试
test:
	@echo "运行所有测试..."
	@cargo test

test-verbose:
	@echo "运行测试 (详细输出)..."
	@cargo test -- --nocapture

integration:
	@echo "运行集成测试..."
	@cargo run --package client --example integration_test

# 系统控制
start:
	@./scripts/start.sh

stop:
	@./scripts/stop.sh

restart: stop start

# 运行
run-client:
	@./target/debug/client

run-manager:
	@./target/debug/manager

run-storager:
	@./target/debug/storager 50052

# 日志
logs:
	@tail -f logs/manager.log

logs-all:
	@echo "=== Manager ===" && tail -20 logs/manager.log && \
	 echo "\n=== Storager 1 ===" && tail -10 logs/storager1.log && \
	 echo "\n=== Storager 2 ===" && tail -10 logs/storager2.log && \
	 echo "\n=== Storager 3 ===" && tail -10 logs/storager3.log

# 代码质量
fmt:
	@echo "格式化代码..."
	@cargo fmt

clippy:
	@echo "运行 Clippy..."
	@cargo clippy -- -D warnings

check:
	@echo "检查编译..."
	@cargo check

# 清理
clean:
	@echo "清理构建产物..."
	@cargo clean

clean-logs:
	@echo "清理日志文件..."
	@rm -f logs/*.log

clean-all: clean clean-logs
	@echo "清理所有临时文件..."
	@find . -name ".DS_Store" -type f -delete

# 文档
doc:
	@echo "生成文档..."
	@cargo doc --no-deps --open

doc-all:
	@echo "生成文档 (包括依赖)..."
	@cargo doc --open

# 安装
install:
	@echo "安装到系统..."
	@cargo install --path crates/client
	@cargo install --path crates/manager
	@cargo install --path crates/storager

# 基准测试
bench:
	@echo "运行基准测试..."
	@cargo bench

# 统计
stats:
	@echo "代码统计:"
	@echo "  总行数: $$(find crates -name "*.rs" | xargs wc -l | tail -1 | awk '{print $$1}')"
	@echo "  文件数: $$(find crates -name "*.rs" | wc -l)"
	@echo ""
	@echo "各模块行数:"
	@echo "  Client:  $$(find crates/client -name "*.rs" | xargs wc -l | tail -1 | awk '{print $$1}')"
	@echo "  Manager: $$(find crates/manager -name "*.rs" | xargs wc -l | tail -1 | awk '{print $$1}')"
	@echo "  Storager: $$(find crates/storager -name "*.rs" | xargs wc -l | tail -1 | awk '{print $$1}')"
	@echo "  Common:  $$(find crates/common -name "*.rs" | xargs wc -l | tail -1 | awk '{print $$1}')"
