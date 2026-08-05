# crates 分层

四个目录就是四层，**依赖只能向内**（`app → domain → capability → infra`），同层之间
也只允许指向更靠内的那些。这条约束原本就成立——依赖图一直是无环的 DAG——但十七个
crate 平铺在一个目录里的时候，光看名字分不出 `lya-storage` 和 `lya-hub` 差着六层。
分组只是把已经存在的层次画出来。

workspace members 用的是 `crates/*/*` 通配：加 crate 不必回来登记，也就不会漏。

## infra —— 不认识 lya 的领域概念

| crate | 干什么 |
|-------|--------|
| `lya-base` | 公共词汇：数据根、`Mode`、`Permission`、`ApiMode`、capability 键。**没有任何 lya 依赖** |
| `lya-http` | 出站 HTTP 客户端与连接池 |
| `lya-db` | SQLite 连接、写事务、全库 schema |
| `lya-config` | `~/.lya` 下四份 TOML 的读写 |

把它们单独拎出来的判据很直接：这一层的代码换个项目也能用，它们不知道「会话」「记忆」
是什么。

## capability —— 模型那一侧的能力

| crate | 干什么 |
|-------|--------|
| `lya-prompt` | system prompt 的固定骨架与拼装顺序 |
| `lya-llm` | 两套 API 栈的请求体、SSE 解析、流式事件 |
| `lya-tool` | 工具定义、注册中心、按权限筛选 |
| `lya-media` | 聊天媒体的取用与留存 |

## domain —— lya 自己的数据

| crate | 干什么 |
|-------|--------|
| `lya-session` | 会话与消息树 |
| `lya-memory` | 长期记忆与常驻索引 |
| `lya-storage` | 数据目录的占用统计 |
| `lya-action` | 元能力动作（记忆读写、模式切换、表单打断） |

`lya-action` 排在最后：它要同时用到会话和记忆，所以在这一层的上沿。

## app —— 编排与对外

| crate | 干什么 |
|-------|--------|
| `lya-agent` | 一轮对话的完整编排：装配提示词、跑工具、落库 |
| `lya-hub` | 会话级并发闸门与事件广播 |
| `lya-api` | HTTP 路由、SSE 端点、内嵌 WebUI |
| `lya-core` | 装配与启动 |
| `lya` | 二进制入口与托盘 |

这一层的 crate 各自依赖十来个别的 crate，那是装配根该有的样子——不是耦合失控，是
「有人得把所有零件摆在一起」。

## 加 crate 的时候

先回答它属于哪一层，再回答它能依赖谁。如果答案是「它得依赖上一层的东西」，说明分层
判断错了，或者那个被依赖的东西该往下沉——`lya-base` 就是这么来的：`Mode` 原先住在一个
依赖 `lya-tool` 的 crate 里，把 `lya-config` 垫到了整个工具层之上。
