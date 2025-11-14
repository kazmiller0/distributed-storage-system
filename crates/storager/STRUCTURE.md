# Storager 模块结构

## 📁 目录结构

```
crates/storager/
├── src/
│   ├── ads/                    # 认证数据结构 (ADS) 模块
│   │   ├── mod.rs             # ADS trait 定义和模块导出
│   │   ├── crypto_accumulator.rs  # 密码学累加器实现
│   │   └── mpt.rs             # Merkle Patricia Trie 实现
│   ├── lib.rs                 # 库入口
│   ├── main.rs                # 服务入口
│   ├── service.rs             # gRPC 服务实现
│   └── storager.rs            # Storager 核心结构
└── ads/                       # ADS 底层实现库
    └── src/
        ├── crypto_accumulator/ # 密码学累加器核心
        └── mpt/               # MPT 核心实现
```

## 🎯 核心组件

### 1. ADS 模块 (`src/ads/`)

这是一个统一的模块，用于管理所有的认证数据结构实现。

#### `AdsOperations` Trait

所有 ADS 实现必须遵循的通用接口：

```rust
pub trait AdsOperations: Send + Sync {
    fn add(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash);
    fn query(&self, keyword: &str) -> (Vec<String>, Vec<u8>);
    fn delete(&mut self, keyword: &str, fid: &str) -> (Vec<u8>, RootHash);
}
```

#### 可用的 ADS 实现

1.  **CryptoAccumulatorAds** (`crypto_accumulator.rs`)
    *   基于 BLS12-381 椭圆曲线
    *   提供恒定大小的成员资格证明（~201 字节）
    *   适用于对证明大小有严格要求的场景

2.  **MptAds** (`mpt.rs`)
    *   基于以太坊风格的 Merkle Patricia Trie
    *   证明大小与树深度成正比
    *   更新和验证速度通常更快
    *   适用于对性能要求较高的场景

### 2. Storager 结构 (`storager.rs`)

负责管理 ADS 实例，提供多种构造方式：

```rust
// 默认使用密码学累加器
let storager = Storager::new();

// 显式选择密码学累加器
let storager = Storager::with_crypto_accumulator();

// 使用 Merkle Patricia Trie
let storager = Storager::with_mpt();

// 根据配置字符串创建
let storager = Storager::from_config("mpt");
```

### 3. 服务入口 (`main.rs`)

支持通过命令行参数选择 ADS 类型：

```bash
# 使用默认 ADS (Crypto Accumulator) 和端口 50052
cargo run --bin storager

# 指定端口
cargo run --bin storager -- 50053

# 指定 ADS 类型和端口
cargo run --bin storager -- 50053 mpt
cargo run --bin storager -- 50053 accumulator
```

## 🔬 性能测试

### 启动不同 ADS 的 Storager 实例

```bash
# 终端 1: 使用密码学累加器
cargo run --bin storager -- 50052 accumulator

# 终端 2: 使用 MPT
cargo run --bin storager -- 50053 mpt
```

### 性能对比指标

在进行性能测试时，应关注以下指标：

1.  **写操作延迟**: `Add` 和 `Delete` 操作的耗时
2.  **读操作延迟**: `Query` 操作的耗时
3.  **证明大小**: 返回的 proof 字节数
4.  **吞吐量**: QPS (Queries Per Second)
5.  **内存占用**: 不同数据量下的内存使用情况

## 🚀 使用示例

### 客户端代码

```rust
use common::rpc::storager_service_client::StoragerServiceClient;
use common::rpc::{StoragerAddRequest, StoragerQueryRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 连接到使用 MPT 的 storager
    let mut client = StoragerServiceClient::connect("http://[::1]:50053").await?;
    
    // 添加数据
    let request = tonic::Request::new(StoragerAddRequest {
        keyword: "rust".to_string(),
        fid: "file123".to_string(),
    });
    
    let response = client.add(request).await?;
    println!("Proof size: {} bytes", response.get_ref().proof.len());
    
    // 查询数据
    let request = tonic::Request::new(StoragerQueryRequest {
        keyword: "rust".to_string(),
    });
    
    let response = client.query(request).await?;
    println!("Found {} files", response.get_ref().fids.len());
    
    Ok(())
}
```

## 📊 预期性能差异

### 密码学累加器 (Crypto Accumulator)

**优势**:
*   证明大小恒定 (~201 字节)，不随数据量增长
*   适合带宽受限的环境

**劣势**:
*   更新操作计算成本较高（涉及椭圆曲线运算）
*   初始化时间较长

### Merkle Patricia Trie (MPT)

**优势**:
*   更新和查询速度快
*   广泛应用于以太坊等区块链项目，经过大量实战验证

**劣势**:
*   证明大小与树深度成正比，通常比累加器大
*   内存占用可能更高（需要存储树结构）

## 🔧 扩展新的 ADS

要添加新的 ADS 实现（例如 Vector Commitment），请执行以下步骤：

1.  在 `src/ads/` 下创建新文件，例如 `vector_commitment.rs`
2.  实现 `AdsOperations` trait
3.  在 `src/ads/mod.rs` 中添加模块声明和导出
4.  在 `storager.rs` 中添加新的构造函数
5.  更新 `from_config()` 方法以支持新的配置选项

## 📝 注意事项

1.  当前 MPT 实现使用内存数据库，不支持持久化。如需持久化，请替换为 `RocksDbAdapter`。
2.  两种 ADS 实现都是线程安全的（实现了 `Send + Sync`）。
3.  每个 keyword 维护独立的 ADS 实例，适合关键字数量适中的场景。
4.  在生产环境中，建议为 MPT 配置合适的缓存大小以优化性能。

---

**最后更新**: 2025-11-14
**维护者**: kazmiller
