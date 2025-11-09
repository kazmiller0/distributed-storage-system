# Manager

分布式存储系统的管理节点，负责路由请求和验证证明。

## 结构

```
manager/
├── Cargo.toml              # 依赖配置
├── README.md               # 本文件
├── consistent_hash/        # 一致性哈希模块（内部依赖）
│   ├── Cargo.toml
│   ├── examples/
│   └── src/
└── src/
    ├── lib.rs              # 库入口
    ├── main.rs             # 可执行文件入口
    ├── manager.rs          # Manager 核心结构和方法
    └── service.rs          # gRPC 服务实现
```

## 模块说明

### `manager.rs`
定义 `Manager` 结构体和核心方法：
- 一致性哈希路由
- 证明验证
- 根哈希管理

### `service.rs`
实现 gRPC 服务接口：
- `add` - 添加关键词
- `query` - 查询（支持单关键词和布尔表达式）
- `delete` - 删除关键词
- `update` - 更新关键词

### `consistent_hash/`
一致性哈希环实现，用于将关键词路由到不同的 storager 节点。

## 运行

```bash
cargo run -p manager
```

## 特性

- ✅ 使用一致性哈希进行负载均衡
- ✅ 支持密码学累加器证明验证
- ✅ 支持布尔表达式查询（AND、OR、NOT）
- ✅ 模块化设计，易于扩展
