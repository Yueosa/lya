//! 记忆读写动作。
//!
//! 只有读和写两个动作，没有列表也没有检索——全部记忆的索引常驻 system
//! prompt，模型看得见所有条目，需要正文时按编号读一条就够了。

use std::sync::Arc;

use lya_memory::{MemoryStore, NewMemory};
use serde_json::{Value, json};

use crate::args::{opt_str, opt_str_array, req_i64, req_str};
use crate::meta::{ActionFlow, ActionMeta, ActionOutcome};
use crate::traits::{Action, ActionCallFuture, ActionCtx};

const WRITE_HINT: &str = "\
标题相同即视为同一条记忆并整体覆盖，所以更新已有记忆时沿用原标题即可，不要新造。
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
系统提示词里已经列出了**全部**记忆的索引（编号、标题、标签、摘要），需要某条\
完整正文时用这个动作按编号读。

索引里没有的东西就是不存在，不要反复换关键词试探，也不用先「查一下有没有」\
——你已经全看见了。";

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
                        "description": "记忆标题，全局唯一；沿用已有标题即为更新那条"
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
