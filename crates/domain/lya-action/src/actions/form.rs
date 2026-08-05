//! `form`：向用户发起结构化表单，挂起等答复。
//!
//! 答复不是新的一轮用户输入，而是这次 `form` 调用的结果——由
//! [`render_form_answer`] 渲染成文本后写成 `role=tool` 消息回灌。
//! 模型看到的始终是标准的 tool_call → tool_result 配对，`role=hitl`
//! 那个节点只服务界面与状态恢复。

use lya_session::{FormOption, FormQuestion, FormQuestionKind, HitlBlock};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::args::{opt_bool, req_array, req_str};
use crate::meta::{ActionFlow, ActionMeta, ActionOutcome};
use crate::traits::{Action, ActionCallFuture, ActionCtx};

/// 一张表单最多几道题。
pub const MAX_QUESTIONS: usize = 10;
/// 一道题最多几个选项。
pub const MAX_OPTIONS: usize = 20;

const HINT: &str = "\
用于需要暂停下来、等用户做选择或提供信息才能继续的场景：让用户执行你做不了的\
操作（sudo、插拔设备）、在几个方案之间拍板、补上你找不到的路径或名称。

什么时候**不要**用表单：一句话就能问清楚的事直接在回复里问。表单会打断对话流，\
为了「你想要 A 还是 B」这种问题弹一张表单只会让人烦。

题型：
- `single` / `multi`：必须给 `options`，每个选项有 `key`（回传值）和 `label`（展示文案）。
- `text`：自由填写，不要给 `options`。路径、名称、命令输出这类问不出选项的用它。

每题可以设 `allow_note: true` 额外开一个备注框，适合「选了但想补充说明」的情况；\
不需要就别开，输入框越多用户越懒得填。

一张表单最多 {MAX_QUESTIONS} 题、每题最多 {MAX_OPTIONS} 个选项。一次把需要的\
信息问全，不要连着发好几张表单。发出后本轮会挂起，等用户答复了你再继续。";

/// `form` 动作。
pub struct FormAction {
    meta: ActionMeta,
    params: Value,
    hint: String,
}

impl Default for FormAction {
    fn default() -> Self {
        Self::new()
    }
}

impl FormAction {
    /// 构造。
    pub fn new() -> Self {
        Self {
            meta: ActionMeta::new(
                "form",
                "结构化提问",
                "向用户发送一张结构化表单并等待答复",
                ActionFlow::AwaitHuman,
            ),
            params: json!({
                "type": "object",
                "properties": {
                    "form_id": {
                        "type": "string",
                        "description": "本次表单的标识（英文短横线命名）"
                    },
                    "title": {
                        "type": "string",
                        "description": "表单标题，用户可见"
                    },
                    "questions": {
                        "type": "array",
                        "description": format!("题目列表，最多 {MAX_QUESTIONS} 题"),
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string", "description": "题目标识，表单内唯一" },
                                "text": { "type": "string", "description": "题干" },
                                "kind": {
                                    "type": "string",
                                    "enum": ["single", "multi", "text"],
                                    "description": "题型：单选 / 多选 / 自由文本"
                                },
                                "options": {
                                    "type": "array",
                                    "description": format!(
                                        "选项，单选与多选必填，文本题不要给；最多 {MAX_OPTIONS} 个"
                                    ),
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "key": { "type": "string" },
                                            "label": { "type": "string" }
                                        },
                                        "required": ["key", "label"]
                                    }
                                },
                                "allow_note": {
                                    "type": "boolean",
                                    "description": "是否额外提供备注输入框，默认 false"
                                }
                            },
                            "required": ["id", "text", "kind"]
                        }
                    }
                },
                "required": ["form_id", "title", "questions"]
            }),
            hint: HINT
                .replace("{MAX_QUESTIONS}", &MAX_QUESTIONS.to_string())
                .replace("{MAX_OPTIONS}", &MAX_OPTIONS.to_string()),
        }
    }
}

impl Action for FormAction {
    fn meta(&self) -> &ActionMeta {
        &self.meta
    }

    fn parameters(&self) -> &Value {
        &self.params
    }

    fn prompt_hint(&self) -> &str {
        &self.hint
    }

    fn call<'a>(&'a self, _ctx: ActionCtx<'a>, args: Value) -> ActionCallFuture<'a> {
        Box::pin(async move {
            match parse_form(&args) {
                Ok(block) => ActionOutcome::await_human(block),
                Err(msg) => ActionOutcome::err(msg),
            }
        })
    }
}

/// 解析并校验表单参数。
fn parse_form(args: &Value) -> Result<HitlBlock, String> {
    let form_id = req_str(args, "form_id")?;
    let title = req_str(args, "title")?;
    let raw_questions = req_array(args, "questions")?;

    if raw_questions.is_empty() {
        return Err("表单至少要有一道题".into());
    }
    if raw_questions.len() > MAX_QUESTIONS {
        return Err(format!(
            "表单最多 {MAX_QUESTIONS} 题，收到 {} 题；请拆分或精简",
            raw_questions.len()
        ));
    }

    let mut questions = Vec::with_capacity(raw_questions.len());
    let mut seen_ids: Vec<String> = Vec::with_capacity(raw_questions.len());
    for (idx, raw) in raw_questions.iter().enumerate() {
        let question = parse_question(raw).map_err(|msg| format!("第 {} 题：{msg}", idx + 1))?;
        if seen_ids.contains(&question.id) {
            return Err(format!("题目 id 重复：{}", question.id));
        }
        seen_ids.push(question.id.clone());
        questions.push(question);
    }

    Ok(HitlBlock::Form {
        form_id,
        title,
        questions,
    })
}

/// 解析单道题。
fn parse_question(raw: &Value) -> Result<FormQuestion, String> {
    let id = req_str(raw, "id")?;
    let text = req_str(raw, "text")?;
    let kind = match req_str(raw, "kind")?.as_str() {
        "single" => FormQuestionKind::Single,
        "multi" => FormQuestionKind::Multi,
        "text" => FormQuestionKind::Text,
        other => return Err(format!("未知题型 {other:?}，应为 single / multi / text")),
    };
    let allow_note = opt_bool(raw, "allow_note")?;
    let options = parse_options(raw, kind)?;

    Ok(FormQuestion {
        id,
        text,
        kind,
        options,
        allow_note,
    })
}

/// 解析选项，并检查与题型是否匹配。
fn parse_options(raw: &Value, kind: FormQuestionKind) -> Result<Vec<FormOption>, String> {
    let provided = match raw.get("options") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(FormOption {
                    key: req_str(item, "key")?,
                    label: req_str(item, "label")?,
                });
            }
            out
        }
        Some(_) => return Err("`options` 应为数组".into()),
    };

    if kind == FormQuestionKind::Text {
        if !provided.is_empty() {
            return Err("文本题不应带 `options`；要给选项请改用 single 或 multi".into());
        }
        return Ok(provided);
    }

    if provided.is_empty() {
        return Err("单选与多选必须提供 `options`；要让用户自由填写请改用 text".into());
    }
    if provided.len() > MAX_OPTIONS {
        return Err(format!(
            "选项最多 {MAX_OPTIONS} 个，收到 {}",
            provided.len()
        ));
    }
    let mut seen: Vec<&str> = Vec::with_capacity(provided.len());
    for option in &provided {
        if seen.contains(&option.key.as_str()) {
            return Err(format!("选项 key 重复：{}", option.key));
        }
        seen.push(&option.key);
    }
    Ok(provided)
}

/// 用户对一道题的作答。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormAnswerItem {
    /// 对应的题目 id。
    pub question_id: String,
    /// 选中的选项 key（单选一个、多选多个），或文本题的内容。
    #[serde(default)]
    pub values: Vec<String>,
    /// 备注（题目开了 `allow_note` 时才会有）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// 一次表单作答。
///
/// 上一代把备注塞成 `{题目id}_note` 这样的魔法键混在答案 map 里，这里拆成
/// 正经字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormAnswer {
    /// 对应的表单 id。
    pub form_id: String,
    /// 逐题作答；未作答的题可以直接不出现。
    #[serde(default)]
    pub items: Vec<FormAnswerItem>,
    /// 表单级补充说明。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freetext: Option<String>,
}

/// 把作答渲染成回灌给模型的文本。
///
/// 选项显示 `label` 而不是 `key`——用户看到的是什么，模型就该看到什么，
/// 免得模型拿着内部 key 去跟用户复述。
pub fn render_form_answer(title: &str, questions: &[FormQuestion], answer: &FormAnswer) -> String {
    let mut out = format!("[表单回答: {title}]");

    for question in questions {
        let item = answer
            .items
            .iter()
            .find(|item| item.question_id == question.id);

        let value = match item {
            None => "（未回答）".to_string(),
            Some(item) if item.values.iter().all(|v| v.trim().is_empty()) => {
                "（未回答）".to_string()
            }
            Some(item) => item
                .values
                .iter()
                .map(|value| display_value(question, value))
                .collect::<Vec<_>>()
                .join(", "),
        };

        out.push_str(&format!("\n- {}: {}", question.text, value));
        if let Some(note) = item.and_then(|item| item.note.as_deref())
            && !note.trim().is_empty()
        {
            out.push_str(&format!("（备注: {}）", note.trim()));
        }
    }

    if let Some(freetext) = answer.freetext.as_deref()
        && !freetext.trim().is_empty()
    {
        out.push_str(&format!("\n- 补充说明: {}", freetext.trim()));
    }
    out
}

/// 选项题把 key 换成 label；文本题原样显示。未知 key 原样保留，便于排查。
fn display_value(question: &FormQuestion, value: &str) -> String {
    if question.kind == FormQuestionKind::Text {
        return value.trim().to_string();
    }
    question
        .options
        .iter()
        .find(|option| option.key == value)
        .map(|option| option.label.clone())
        .unwrap_or_else(|| value.to_string())
}
