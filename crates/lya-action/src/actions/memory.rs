//! 记忆读写动作。
//!
//! 只有读和写两个动作，没有列表也没有检索——全部记忆的索引常驻 system
//! prompt，模型看得见所有条目，需要正文时按编号读一条就够了。

use std::sync::Arc;

use lya_memory::{MatchField, MemoryStore, NewMemory};
use serde_json::{Value, json};

use crate::args::{opt_str, opt_str_array, req_i64, req_str};
use crate::meta::{ActionFlow, ActionMeta, ActionOutcome};
use crate::traits::{Action, ActionCallFuture, ActionCtx};

const WRITE_HINT: &str = "\
标题相同即视为同一条记忆并整体覆盖，所以更新已有记忆时沿用原标题即可，不要新造。

标题写法（前缀 + 简短描述，一条一主题，8–40 字）：
- 用户偏好: ……（习惯、喜好、排斥）
- 人物: ……（某个具体的人）
- 项目: ……（项目背景与约定）
- 技术: ……（排错结论、配置方案）
- 约定: ……（与你之间的协作约定）
更新已有记忆时必须沿用原标题；不要为同一主题造多个近义标题。

正文要写清结论与适用范围，并包含日后能想起来的关键词；summary 一句话概括；\
tags 放具体名词（工具名、项目名、报错关键字），把最具体的放前面。

什么时候该写：
- 用户说「记住 / 以后都 / 我的偏好 / 我不喜欢 / 常用 / 默认」这类稳定信息。
- 排错、迁移、配置、项目约定确认完成后，得到了未来可复用的结论。
- 用户纠正了你对 TA 或项目的理解，更新对应记忆，别让同样的误会再发生。

什么时候不要写：
- 临时闲聊、一次性任务的中间过程、马上就过期的状态。
- 你对用户的心理推测，除非用户自己确认过。
- 敏感私密细节，除非用户明确要求记住。

记忆是长期资产，宁可少写几条扎实的，也不要攒一堆流水账。";

const READ_HINT: &str = "\
系统提示词里的记忆索引给了编号、标题、标签和摘要，要看某条的完整正文就用这个\
动作按编号读。

索引里已经能看到标题和摘要，不要为了确认「这条讲的是不是我要的」而挨个读一遍；\
先看摘要，真需要细节再读。";

const SEARCH_HINT: &str = "\
索引里只有标题、标签和摘要，**搜不到正文**；条目多的时候索引还会截断，末尾会\
写明「另有 N 条未列出」。这两种情况下用检索。

什么时候用：
- 想找的关键词可能只出现在正文里（某个命令、某个报错、某个人名）。
- 索引提示还有未列出的条目，而你要找的可能在里面。

索引里明明白白列着的条目，直接用 memory_read 按编号读，不用先搜一遍。
检索只返回命中片段，要完整内容仍需按编号读。";

/// `memory_write`：写入或覆盖一条长期记忆。
pub struct MemoryWriteAction {
    meta: ActionMeta,
    params: Value,
    store: Arc<MemoryStore>,
}

impl MemoryWriteAction {
    /// 绑定记忆仓储。
    pub fn new(store: Arc<MemoryStore>) -> Self {
        Self {
            meta: ActionMeta::new(
                "memory_write",
                "写入记忆",
                "写入或更新一条长期记忆；标题相同则覆盖",
                ActionFlow::Continue,
            ),
            params: json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "记忆标题，全局唯一；沿用已有标题即为更新。格式：前缀: 描述，如「用户偏好: 喜欢猫娘风格互动」「项目: lya 架构约定」"
                    },
                    "body": {
                        "type": "string",
                        "description": "正文：结论、适用范围与可检索关键词"
                    },
                    "summary": {
                        "type": "string",
                        "description": "一句话概括，会显示在常驻索引里"
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "具体名词标签，最具体的放前面"
                    }
                },
                "required": ["title", "body"]
            }),
            store,
        }
    }
}

impl Action for MemoryWriteAction {
    fn meta(&self) -> &ActionMeta {
        &self.meta
    }

    fn parameters(&self) -> &Value {
        &self.params
    }

    fn prompt_hint(&self) -> &str {
        WRITE_HINT
    }

    fn call<'a>(&'a self, ctx: ActionCtx<'a>, args: Value) -> ActionCallFuture<'a> {
        Box::pin(async move {
            let parsed = (|| {
                Ok::<_, String>(NewMemory {
                    title: req_str(&args, "title")?,
                    body: req_str(&args, "body")?,
                    summary: opt_str(&args, "summary")?.unwrap_or_default(),
                    tags: opt_str_array(&args, "tags")?,
                    source_session_id: Some(ctx.session_id.to_string()),
                })
            })();

            let new = match parsed {
                Ok(new) => new,
                Err(msg) => return ActionOutcome::err(msg),
            };

            match self.store.upsert_by_title(new) {
                Ok(memory) => {
                    ActionOutcome::ok(format!("已记住 #{}「{}」。", memory.id, memory.title))
                }
                Err(err) => ActionOutcome::err(format!("写入记忆失败：{err}")),
            }
        })
    }
}

/// `memory_search`：按关键词检索记忆，含正文。
pub struct MemorySearchAction {
    meta: ActionMeta,
    params: Value,
    store: Arc<MemoryStore>,
}

impl MemorySearchAction {
    /// 绑定记忆仓储。
    pub fn new(store: Arc<MemoryStore>) -> Self {
        Self {
            meta: ActionMeta::new(
                "memory_search",
                "检索记忆",
                "按关键词检索长期记忆，能搜到索引里看不见的正文内容",
                ActionFlow::Continue,
            ),
            params: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "关键词。会在标题、标签、摘要和正文里找。"
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "最多返回几条，默认 5。"
                    }
                },
                "required": ["query"]
            }),
            store,
        }
    }
}

impl Action for MemorySearchAction {
    fn meta(&self) -> &ActionMeta {
        &self.meta
    }

    fn parameters(&self) -> &Value {
        &self.params
    }

    fn prompt_hint(&self) -> &str {
        SEARCH_HINT
    }

    fn call<'a>(&'a self, _ctx: ActionCtx<'a>, args: Value) -> ActionCallFuture<'a> {
        Box::pin(async move {
            let query = match req_str(&args, "query") {
                Ok(query) => query,
                Err(msg) => return ActionOutcome::err(msg),
            };
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .map(|n| (n as usize).clamp(1, 20))
                .unwrap_or(5);

            match self.store.search(&query, limit) {
                Ok(hits) if hits.is_empty() => {
                    ActionOutcome::ok(format!("没有记忆命中「{query}」。"))
                }
                Ok(hits) => {
                    let mut out = format!("命中 {} 条：\n", hits.len());
                    for hit in hits {
                        out.push_str(&format!(
                            "\n#{} {}（命中于{}）\n   {}\n",
                            hit.id,
                            hit.title,
                            field_label(hit.matched_in),
                            hit.snippet
                        ));
                    }
                    out.push_str("\n要看完整正文用 memory_read 按编号读。");
                    ActionOutcome::ok(out)
                }
                Err(err) => ActionOutcome::err(format!("检索失败：{err}")),
            }
        })
    }
}

/// 命中字段的中文说法。
fn field_label(field: MatchField) -> &'static str {
    match field {
        MatchField::Title => "标题",
        MatchField::Summary => "摘要",
        MatchField::Tag => "标签",
        MatchField::Body => "正文",
    }
}

/// `memory_read`：按编号读取一条记忆的正文。
pub struct MemoryReadAction {
    meta: ActionMeta,
    params: Value,
    store: Arc<MemoryStore>,
}

impl MemoryReadAction {
    /// 绑定记忆仓储。
    pub fn new(store: Arc<MemoryStore>) -> Self {
        Self {
            meta: ActionMeta::new(
                "memory_read",
                "读取记忆",
                "按索引里的编号读取一条长期记忆的完整正文",
                ActionFlow::Continue,
            ),
            params: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "integer",
                        "description": "记忆编号，即索引里 # 后面的数字"
                    }
                },
                "required": ["id"]
            }),
            store,
        }
    }
}

impl Action for MemoryReadAction {
    fn meta(&self) -> &ActionMeta {
        &self.meta
    }

    fn parameters(&self) -> &Value {
        &self.params
    }

    fn prompt_hint(&self) -> &str {
        READ_HINT
    }

    fn call<'a>(&'a self, _ctx: ActionCtx<'a>, args: Value) -> ActionCallFuture<'a> {
        Box::pin(async move {
            let id = match req_i64(&args, "id") {
                Ok(id) => id,
                Err(msg) => return ActionOutcome::err(msg),
            };

            match self.store.get(id) {
                Ok(memory) => {
                    let mut out = format!("#{} {}\n", memory.id, memory.title);
                    if !memory.tags.is_empty() {
                        out.push_str(&format!("标签: {}\n", memory.tags.join(", ")));
                    }
                    out.push('\n');
                    out.push_str(&memory.body);
                    ActionOutcome::ok(out)
                }
                Err(err) => ActionOutcome::err(format!("读取记忆 #{id} 失败：{err}")),
            }
        })
    }
}
