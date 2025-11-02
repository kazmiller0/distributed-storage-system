# ESA (Efficient Set Accumulator) Library

基于 BLS12-381 椭圆曲线的密码学累加器库。

## 目录结构

```
src/
├── lib.rs                          # 库入口文件
├── digest.rs                       # 通用摘要工具
├── set.rs                          # 通用集合操作
└── crypto_accumulator/             # 密码学累加器模块
    ├── mod.rs                      # 累加器模块入口
    └── acc/                        # 累加器核心实现
        ├── mod.rs                  # 核心模块入口
        ├── dynamic_accumulator.rs  # 动态累加器（支持增删）
        ├── digest_set.rs           # 摘要集合
        ├── utils.rs                # 工具函数
        └── serde_impl.rs           # 序列化实现
```

## 核心组件

### DynamicAccumulator
支持动态增删元素的密码学累加器：
- `add()` - 添加元素并生成证明
- `delete()` - 删除元素并生成证明
- `membership()` - 成员查询并生成证明

### DigestSet
用于存储和管理元素摘要的集合结构。

## 使用方法

```rust
use esa_rust::DynamicAccumulator;

let mut acc = DynamicAccumulator::new();

// 添加元素
let add_proof = acc.add(&element).unwrap();

// 查询成员
let membership_proof = acc.membership(&element).unwrap();

// 删除元素
let delete_proof = acc.delete(&element).unwrap();
```

## 扩展性

如需添加其他 ADS 实现（如 Merkle Tree、Patricia Trie 等），可在 `src/` 目录下创建新的模块目录，参考 `crypto_accumulator/` 的结构。
