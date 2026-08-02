# lya 路线图

当前仓库的**活文档**。历史计划见 [`docs/archive/`](./archive/)。

## 已完成（近期）

- 后端骨架：会话树、HITL、agent 轮次、HTTP + SSE
- WebUI：Vue 3 三栏、分支树、设置/记忆页
- **托盘**：Linux ksni（WebUI / 退出）+ `lya-core/run.rs` 服务组装
- 安全：`GET /api/config/raw/models` 脱敏 `api_key`
- 稳定性：发消息先占轮次再写库；HITL 确认可 cancel；`pending_hitl` 扫 active 路径
- 提示词：Actions / Tools 标题统一为 `=== [xxx] ===`；记忆标题前缀规范
- Bash：`2>&1` 解析修复；LLM 可选 `steps[]` 结构化确认（展示层）
- `lya-session`：`store/` 子模块拆分

## 进行中

**Backlog**（见 [`lya-user-0-todo.md`](./lya-user-0-todo.md)）：vdo/ado 缓存、web_fetch 翻页、token 估算等。

## 已完成（Wave A）

bash 双引号命令替换、steps 决策解耦、图片路径、默认模型名、分支树 filter 默认。

## 下一步

| 优先级 | 项 | 说明 |
|--------|----|------|
| B | 图片 lightbox、会话显示偏好扩展 | 折叠阈值、流式后自动收起 |
| C | 前端拆层 + 子组件化 + README 规范 | 与用户一起 |
| — | img_cache、web_fetch 翻页、token 估算 | Backlog |

## 刻意不做 / 封存

见 [`lya-user-0-todo.md`](./lya-user-0-todo.md)「封存」一节。
