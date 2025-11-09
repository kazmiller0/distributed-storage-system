# Storager

分布式存储系统的存储节点，负责实际的数据存储和认证数据结构（ADS）管理。

## 结构

```
storager/
├── Cargo.toml              # 依赖配置
├── README.md               # 本文件
├── ads/                    # 认证数据结构模块（内部依赖）
│   ├── Cargo.toml
│   ├── README.md
│   └── src/
│       ├── crypto_accumulator/  # 密码学累加器实现
│       ├── mpt/                 # Merkle Patricia Tree
│       ├── digest.rs
│       ├── lib.rs
│       └── set.rs
└── src/
    ├── main.rs             # 可执行文件入口
    ├── ads_trait.rs        # ADS trait 定义
    └── ads/
        └── crypto_accumulator.rs  # 密码学累加器适配器
```

## 模块说明

### `ads/` (esa_rust)
认证数据结构（Authenticated Data Structures）实现：
- **密码学累加器**：基于椭圆曲线的动态累加器
- **Merkle Patricia Tree**：高效的树形结构
- 提供可验证的数据操作证明

### `src/ads_trait.rs`
定义统一的 ADS trait，支持多种 ADS 实现：
- `add()` - 添加元素
- `delete()` - 删除元素
- `query()` - 查询元素

### `src/ads/crypto_accumulator.rs`
密码学累加器的具体实现适配器，包装 `esa_rust` 提供的功能。

## 运行

### 启动单个 Storager
```bash
cargo run -p storager -- <port>
```

例如：
```bash
cargo run -p storager -- 50052
```

### 启动多个 Storager（分布式环境）
```bash
# 终端 1
./target/debug/storager 50052

# 终端 2
./target/debug/storager 50053

# 终端 3
./target/debug/storager 50054
```

## 特性

- ✅ 支持密码学累加器（Crypto Accumulator）
- ✅ 提供可验证的数据操作证明
- ✅ 高效的增删查操作
- ✅ 基于 gRPC 的高性能通信
- ✅ 模块化设计，ADS 实现可扩展

## ADS 实现

目前支持的 ADS 模式：
- **CryptoAccumulator**：基于 BLS12-381 椭圆曲线的密码学累加器

### 添加新的 ADS 实现

如果您想添加新的认证数据结构（如 Merkle Tree、MPT 等），请按照以下步骤：

#### 1. 创建 ADS 实现文件
在 `src/ads/` 目录下创建新文件，例如 `merkle_tree.rs`：

```rust
use crate::ads_trait::AdsOperations;
use common::RootHash;

pub struct MerkleTreeAds {
    // 您的实现
}

impl MerkleTreeAds {
    pub fn new() -> Self {
        // 初始化
    }
}

impl AdsOperations for MerkleTreeAds {
    fn add(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash) {
        // 实现添加逻辑
    }

    fn query(&self, keyword: &str) -> (Vec<String>, Vec<u8>) {
        // 实现查询逻辑
    }

    fn delete(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash) {
        // 实现删除逻辑
    }
}
```

#### 2. 注册模块
在 `src/ads/mod.rs` 中添加：

```rust
pub mod merkle_tree;
pub use merkle_tree::MerkleTreeAds;
```

#### 3. 添加构造函数
在 `src/storager.rs` 中添加新的构造函数：

```rust
pub fn with_merkle_tree() -> Self {
    let ads: Box<dyn AdsOperations> = Box::new(MerkleTreeAds::new());
    Storager {
        ads: Arc::new(RwLock::new(ads)),
    }
}
```

#### 4. 更新 main.rs（可选）
如果需要通过命令行参数选择 ADS，可以在 `main.rs` 中添加：

```rust
let storager = match ads_type {
    "crypto" => Storager::with_crypto_accumulator(),
    "merkle" => Storager::with_merkle_tree(),
    _ => Storager::new(), // 默认
};
```

## ADS 模式

目前支持的 ADS 模式：
- **CryptoAccumulator**：基于 BLS12-381 椭圆曲线的密码学累加器

### 证明结构

#### Add/Delete 证明
```
[old_acc(96字节) | new_acc(96字节) | element(8字节) | verification(1字节)]
```

#### Membership 证明
```
[witness(96字节) | element(8字节) | acc_value(96字节) | verification(1字节)]
```

## 依赖关系

- `common` - 共享的 RPC 定义和类型
- `ads` (esa_rust) - 认证数据结构实现（内部模块）
- `tokio` - 异步运行时
- `tonic` - gRPC 框架
- `ark-*` - 密码学库（椭圆曲线运算）
