# 同批 tool_calls（调用组）

活文档。替换当前「串行 + 单 HITL + 后续 call 写失败 stub」的行为。

## 问题（现状 = Bug）

模型一轮返回多个 `tool_calls` 时，agent 用 for 循环**串行**处理，且 **`already_awaiting` 只允许一个 HITL**：第一个需确认的 call 挂起后，同批其余 call 立刻落成错误 tool 结果（「等这个处理完再发下一个」），用户尚未审批。

## 目标行为

1. 同一条 assistant 消息里的 `tool_calls` = **一个调用组**（batch）。
2. **不触发审核**的 call：组内**并行**执行，结果照常写回。
3. **触发审核**的 call：只建 HITL 节点，**不执行**；用户在一个确认 UI 里用 **`‹ 1/3 ›`** 逐条审，审完本组后，**已批准的并行执行**。
4. 组内并行上限 **`max_parallel_tools`**（默认 3，见 `runtime.toml`）；超出则整组失败并告知模型。
5. assistant 消息仍**不加**时间戳；user/tool 前缀语义见 `TIME_ANCHOR`。

## 后端阶段（单轮内）

```text
LLM 返回 assistant + tool_calls[0..n)
        │
        ▼
  n > max_parallel_tools ? ──是──► 整组错误回灌，不执行
        │否
        ▼
  逐 call 分类：auto | needs_hitl
        │
        ├─ auto 集合 ──► tokio::join 并行 dispatch ──► tool 结果落库
        │
        └─ hitl 集合 ──► 并行创建 HITL 节点（不执行）
                │
                ▼
        有待审项 ? ──是──► turn_end: awaiting_human
                │              （batch 元数据见下）
                │否
                ▼
        继续下一轮 LLM（同现有逻辑）
```

用户在本组 HITL UI 审完（每条 approve/reject + 可选备注）后：

- **reject**：立刻写拒绝 tool 结果（带备注）。
- **approve**：进入「待执行队列」；本组全部审完后，对 approve 集合 **并行 `execute_confirmed`**，再写 tool 结果。
- 本组清空后 `start_turn` 继续。

## 数据与事件

### 组标识

在 assistant 消息的 `lya.meta`（或等价字段）写入：

```json
{
  "tool_batch": {
    "id": "<uuid>",
    "call_ids": ["c1", "c2", "c3"],
    "needs_review": ["c2", "c3"]
  }
}
```

HITL 节点 `lya.meta` 带 `{ "batch_id", "call_id", "index": 1 }` 供前端 `‹ i/n ›` 对齐。

### SSE（建议新增/扩展）

| 事件 | 用途 |
|------|------|
| `tool_batch_started` | `{ batch_id, calls: [{ call_id, name, needs_review }] }` |
| `tool_batch_review` | 同 `await_human`，或扩展 payload 含 batch 与 index |
| `tool_batch_executing` | 本组批准项开始并行跑（可选，进度用） |

notify 对 HITL：按 **`hitl_message_id`** 去重，同组多条 HITL 各弹各的。

### session 约束

- `pending_hitl` 从「单个 id」扩展为 **同 batch 的 pending 列表**（或 batch 头 + 计数）。
- 同 batch 未清空前，仍阻塞新的用户消息（与现 `PendingHitl` 一致）。

## 前端

### 时间线 / 工具块

- 一条 assistant 消息下：**一个「调用组」卡片**（折叠默认），标题如「3 个工具 · 2 待确认」。
- 展开后：各 tool 子块（名、参数、状态：已完成 / 待审 / 执行中 / 失败）。
- 流式阶段：`tool_batch_started` 先到，再更新各 call 状态，避免像三条无关流水线。

### HITL 确认 UI

- 单对话框 + **`‹ i/n ›`** 只遍历 **needs_review** 的子集。
- 每条：批准 / 拒绝 / 备注（与现 tool_confirm 一致）。
- 最后一项确认后触发后端「执行本组已批准项」；或提供「执行已批准」按钮（实现时二选一）。

## 配置（`runtime.toml` 草案）

```toml
[agent]
max_parallel_tools = 3   # 单批 tool_calls 上限
```

## 实现顺序（建议）

1. **Wave D1 — 后端**：去掉 `already_awaiting` stub；batch 分类 + auto 并行；多 HITL 节点；批后并行 execute。
2. **Wave D2 — 协议/事件**：batch 元数据 + SSE。
3. **Wave D3 — 前端**：组卡片 + HITL `‹ i/n ›` + timeline 折叠。
4. **notify**（依赖 D2 的 batch/hitl 语义）。
5. 其余 Backlog 不变。

## 时间戳（与 `build_messages`）

- **user** 前缀时间 = 该 user 消息 `created_at`（用户发送时刻）。
- **tool** 前缀时间 = 该 tool **结果落库**的 `created_at`（= 模型看到这条结果的时刻：自动工具≈执行结束；需确认工具≈批准并跑完后的结束）。
- 不在 TIME_ANCHOR 里规定「以哪条为准算现在」；模型看到前缀即可感知节奏。assistant 不加前缀。
