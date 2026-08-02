# lya-core

组装层：把 [`lya-agent`] 的一轮驱动接到 HTTP 上。

## 职责

- [`SessionHub`]：轮次串行、SSE 广播、实时缓冲、取消
- REST 写操作 + 订阅流读结果（发消息 202，正文走 SSE）
- 静态 UI、`img_cache` 媒体端点、配置/bootstrap API

## 两条原则

**写走 REST，读走订阅。** 同一 Session 在网页与多端看到的是同一份流。

**订阅 = 先快照再增量。** 断线重连与首次打开走同一条路，不需要事件序号对齐。

## 用法

```rust
use lya_core::{start_server, SessionHub};

// 通常由 lya 二进制调用 start_server
```

[`SessionHub`]: ../lya-core/src/hub.rs
