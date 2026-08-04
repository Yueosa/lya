//! 记忆的数据模型与写入约束。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::MemoryError;

/// 一条长期记忆。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    /// SQLite 自增 id。API、前端和模型都用它——模型看到的 `#N` 就是这个数。
    pub id: i64,
    /// 标题，全局唯一。
    pub title: String,
    /// 一句话概括，进常驻索引。
    pub summary: String,
    /// 正文，按需读取。
    pub body: String,
    /// 标签，已去重，保持写入顺序。
    pub tags: Vec<String>,
    /// 写下这条记忆的会话；仅溯源用。
    pub source_session_id: Option<String>,
    /// 创建时间。
    pub created_at: DateTime<Utc>,
    /// 最后更新时间；常驻索引超预算时，丢的是这个最旧的几条。
    pub updated_at: DateTime<Utc>,
}

/// 新建记忆的入参。
#[derive(Debug, Clone, Default)]
pub struct NewMemory {
    /// 标题，必填。
    pub title: String,
    /// 一句话概括。
    pub summary: String,
    /// 正文。
    pub body: String,
    /// 标签。
    pub tags: Vec<String>,
    /// 来源会话。
    pub source_session_id: Option<String>,
}

impl NewMemory {
    /// 只给标题与正文的快捷构造。
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            ..Default::default()
        }
    }

    /// 设置摘要。
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    /// 设置标签。
    pub fn with_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// 记录来源会话。
    pub fn from_session(mut self, session_id: impl Into<String>) -> Self {
        self.source_session_id = Some(session_id.into());
        self
    }
}

/// 命中出现在哪个字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchField {
    /// 标题。
    Title,
    /// 摘要。
    Summary,
    /// 标签。
    Tag,
    /// 正文。
    Body,
}

/// 一条检索命中。
///
/// 不带完整正文——检索是为了「找到哪一条」，看内容用 `memory_read` 按展示编号取。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryHit {
    /// SQLite 自增 id（前端/API 用）。
    pub id: i64,
    /// 标题。
    pub title: String,
    /// 摘要。
    pub summary: String,
    /// 标签。
    pub tags: Vec<String>,
    /// 命中在哪个字段。
    pub matched_in: MatchField,
    /// 命中处的上下文片段。
    pub snippet: String,
}

/// 局部更新：`None` 的字段保持原值。
#[derive(Debug, Clone, Default)]
pub struct MemoryPatch {
    /// 新标题。
    pub title: Option<String>,
    /// 新摘要。
    pub summary: Option<String>,
    /// 新正文。
    pub body: Option<String>,
    /// 新标签，整体替换而非追加。
    pub tags: Option<Vec<String>>,
}

impl MemoryPatch {
    /// 是否什么都没改。
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.summary.is_none() && self.body.is_none() && self.tags.is_none()
    }
}

/// 写入长度上限。
///
/// 先写死默认值，将来由 `lya-config` 的 runtime 层覆盖。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryLimits {
    /// 标题最大字符数。
    pub max_title_chars: usize,
    /// 摘要最大字符数。
    pub max_summary_chars: usize,
    /// 正文最大字符数。
    pub max_body_chars: usize,
    /// 单条记忆最多几个标签。
    pub max_tags: usize,
    /// 单个标签最大字符数。
    pub max_tag_chars: usize,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            max_title_chars: 128,
            max_summary_chars: 2000,
            max_body_chars: 32_000,
            max_tags: 32,
            max_tag_chars: 64,
        }
    }
}

impl MemoryLimits {
    /// 校验标题：非空且不超长。
    pub fn check_title(&self, title: &str) -> Result<String, MemoryError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(MemoryError::Invalid(
                "memory title must not be empty".into(),
            ));
        }
        check_len("title", title, self.max_title_chars)?;
        Ok(title.to_string())
    }

    /// 校验摘要长度。
    pub fn check_summary(&self, summary: &str) -> Result<String, MemoryError> {
        let summary = summary.trim();
        check_len("summary", summary, self.max_summary_chars)?;
        Ok(summary.to_string())
    }

    /// 校验正文长度。
    pub fn check_body(&self, body: &str) -> Result<String, MemoryError> {
        let body = body.trim_end();
        check_len("body", body, self.max_body_chars)?;
        Ok(body.to_string())
    }

    /// 规整标签：去空白、丢空串、去重，并校验数量与长度。
    ///
    /// **保留写入顺序**——标签有主次之分，具体名词通常在前，排序会丢掉这层信息。
    pub fn check_tags(&self, tags: &[String]) -> Result<Vec<String>, MemoryError> {
        let mut out: Vec<String> = Vec::with_capacity(tags.len());
        for tag in tags {
            let tag = tag.trim();
            if tag.is_empty() {
                continue;
            }
            check_len("tag", tag, self.max_tag_chars)?;
            let tag = tag.to_string();
            if !out.contains(&tag) {
                out.push(tag);
            }
        }
        if out.len() > self.max_tags {
            return Err(MemoryError::Invalid(format!(
                "too many tags: {} > {}",
                out.len(),
                self.max_tags
            )));
        }
        Ok(out)
    }
}

/// 按字符数（而非字节数）判断是否超长，避免中文被误判。
fn check_len(field: &str, value: &str, max: usize) -> Result<(), MemoryError> {
    let len = value.chars().count();
    if len > max {
        return Err(MemoryError::Invalid(format!(
            "memory {field} too long: {len} chars > {max}"
        )));
    }
    Ok(())
}
