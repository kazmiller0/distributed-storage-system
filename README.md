# 分布式存储系统 (Distributed Storage System)

基于可验证数据结构（ADS）的分布式关键词索引存储系统。

## 系统架构

```
┌─────────┐      ┌─────────┐      ┌─────────┐
│ Client  │      │ Client  │      │ Client  │
│    1    │      │    2    │      │   ...   │
└────┬────┘      └────┬────┘      └────┬────┘
     │                │                │
     └────────────────┼────────────────┘
                      │ gRPC
                      ▼
              ┌──────────────┐
              │   Manager    │
              │ (一致性哈希)  │
              └──────┬───────┘
                     │
        ┌────────────┼────────────┐
        │            │            │
        ▼            ▼            ▼
   ┌─────────┐  ┌─────────┐  ┌─────────┐
   │Storager │  │Storager │  │Storager │
   │   1     │  │   2     │  │   3     │
   │  (ADS)  │  │  (ADS)  │  │  (ADS)  │
   └─────────┘  └─────────┘  └─────────┘
```

## 核心组件

### 1. Client (客户端)
- 负责发起文件操作请求
- 实现的操作：
  - `put_file(fid, keywords)` - 上传文件索引
  - `query_by_keyword(keyword)` - 关键词查询
  - `query_by_func(bool_func)` - 布尔查询
  - `delete_file(fid, keywords)` - 删除文件索引
  - `update_file(fid, old_kw, new_kw)` - 更新文件索引

### 2. Manager (管理节点)
- 使用一致性哈希将关键词映射到 Storager
- 验证来自 Storager 的证明
- 维护所有 Storager 的根哈希
- 协调分布式查询和更新操作

### 3. Storager (存储节点)
- 使用可验证数据结构 (ADS) 存储 (keyword, fid) 对
- 支持两种 ADS 模式：
  - Merkle Tree
  - Patricia Trie
- 为每个操作生成加密证明

## 功能特性

### ✅ 已实现的骨架功能

1. **初始化系统**
   - 创建指定数量的 Client 和 Storager
   - 配置网络监听
   - 根据模式初始化 ADS

2. **文件上传** (PutFile)
   - Client 将 (fid, keywords) 拆分为多个 (keyword, fid) 对
   - Manager 使用一致性哈希路由到相应 Storager
   - Storager 插入 ADS 并返回证明

3. **文件查询** (Query)
   - 支持单关键词查询
   - Manager 验证证明
   - 返回文件 ID 列表

4. **文件删除** (DeleteFile)
   - Client 发送删除请求
   - Manager 路由并验证
   - Storager 从 ADS 删除

5. **文件更新** (UpdateFile)
   - 删除旧关键词
   - 添加新关键词
   - 验证所有操作

### 🚧 待实现功能

- [ ] 完整的 Merkle Tree 实现
- [ ] 完整的 Patricia Trie 实现
- [ ] 布尔查询支持
- [ ] 证明生成和验证
- [ ] 可扩展一致性哈希
- [ ] 配置文件加载
- [ ] 错误处理和重试机制
- [ ] 性能优化

## 快速开始

### 前置要求

- Rust 1.70+
- Protocol Buffers 编译器 (`protoc`)

```bash
# macOS
brew install protobuf

# Ubuntu/Debian
apt-get install protobuf-compiler
```

### 构建项目

```bash
cargo build
```

### 运行系统

1. **启动所有服务**
```bash
./start.sh
```

2. **运行客户端测试**
```bash
./target/debug/client
```

3. **停止所有服务**
```bash
./stop.sh
```

### 手动运行

1. **启动 Storager**
```bash
# 终端 1
cargo run --bin storager 50052

# 终端 2
cargo run --bin storager 50053

# 终端 3
cargo run --bin storager 50054
```

2. **启动 Manager**
```bash
# 终端 4
cargo run --bin manager
```

3. **运行 Client**
```bash
# 终端 5
cargo run --bin client
```

## 项目结构

```
distributed-storage-system/
├── Cargo.toml              # 工作空间配置
├── config.json             # 系统配置
├── src/
│   └── lib.rs             # 初始化函数
├── proto/
│   └── storage_service.proto  # gRPC 服务定义
├── crates/
│   ├── common/            # 共享类型和 RPC 定义
│   │   ├── build.rs      # Proto 编译脚本
│   │   └── src/
│   │       ├── types.rs  # 共享类型定义
│   │       └── rpc.rs    # RPC 生成代码
│   ├── ads/              # 可验证数据结构
│   │   └── src/
│   │       ├── merkle_tree.rs
│   │       └── patricia_trie.rs
│   ├── manager/          # 管理节点
│   │   └── src/main.rs
│   ├── storager/         # 存储节点
│   │   └── src/main.rs
│   └── client/           # 客户端
│       └── src/main.rs
├── start.sh              # 启动脚本
├── stop.sh               # 停止脚本
└── logs/                 # 日志目录
```

## RPC 接口

### Manager Service

```protobuf
service ManagerService {
  rpc Add(AddRequest) returns (AddResponse);
  rpc Query(QueryRequest) returns (QueryResponse);
  rpc Delete(DeleteRequest) returns (DeleteResponse);
  rpc Update(UpdateRequest) returns (UpdateResponse);
}
```

### Storager Service

```protobuf
service StoragerService {
  rpc Add(StoragerAddRequest) returns (StoragerAddResponse);
  rpc Query(StoragerQueryRequest) returns (StoragerQueryResponse);
  rpc Delete(StoragerDeleteRequest) returns (StoragerDeleteResponse);
}
```

## 配置

系统配置在 `config.json` 中定义：

```json
{
  "num_clients": 2,
  "num_storagers": 3,
  "ads_mode": "MerkleTree",
  "manager_addr": "http://[::1]:50051",
  "storager_addrs": [
    "http://[::1]:50052",
    "http://[::1]:50053",
    "http://[::1]:50054"
  ],
  "client_addrs": []
}
```

## 开发

### 检查代码
```bash
cargo check --all
```

### 运行测试
```bash
cargo test --all
```

### 格式化代码
```bash
cargo fmt --all
```

### Lint 检查
```bash
cargo clippy --all
```

## 许可证

MIT
