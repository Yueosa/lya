//! 常驻索引：把全部记忆的标题/标签/摘要渲染成一段提示词。
//!
//! 记忆量小的时候，把索引整个放进 system prompt 比任何检索都准——模型看得见
//! 全部条目，要正文再按编号读。代价是索引会随条数增长，所以有 [`IndexBudget`]
//! 兜底：超预算就只留最近更新的若干条，并明确告知还有多少没列出。
//!
//! ## 编号为什么直接用库内 id
//!
//! 这里曾经算一套 1 起连续的展示序号。它的问题不在缓存，在于**同一个号会指两条
//! 记忆**：动作的结果（`已记住 #2「X」`、`#3 标题…正文`）会永久留在消息树里，而
//! 索引每轮按当前排序重新编号，写一条记忆就全体顺延。于是历史说 #2 是 X、当前索引
//! 说 #2 是 Y，模型没有任何办法分辨。
//!
//! 换成 id 之后编号是身份而不是位置，代价只是号会断（删过的不补位）。头一行说明
//! 一句就够——模型对不连续的标识符（issue 号、行号）本来就习惯。
//!
//! 顺带把展示顺序也定成 id 升序：新记忆永远追加在末尾，不再插队重排。

use crate::types::Memory;

/// 索引段落标题。
pub const MEMORY_SECTION_TITLE: &str = "=== [记忆] 长期记忆索引 ===";

/// 常驻索引的体积上限。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexBudget {
    /// 最多列出几条。
    pub max_entries: usize,
    /// 整段最多多少字符（不含标题与结尾提示）。
    pub max_chars: usize,
    /// 单条摘要截断到多少字符。
    pub summary_chars: usize,
}

impl Default for IndexBudget {
    fn default() -> Self {
        Self {
            max_entries: 100,
            max_chars: 4000,
            summary_chars: 120,
        }
    }
}

/// 编号语义的说明。常量而非拼接，它必须逐字节稳定。
const NUMBERING_NOTE: &str =
    "编号是每条记忆的固定标识，不随列表变化，删掉的号也不会补位——不连续是正常的。\
需要某条正文时按这个号读取。";

/// 渲染索引段落。
///
/// `memories` 需按 id 升序排好，渲染出来就是这个顺序。
///
/// 超预算时丢的是**最久没更新**的那几条，但留下来的仍按 id 升序列出：编号是身份，
/// 位置不该随更新时间跳来跳去，否则模型会把「排在前面」读成「更重要」。
///
/// 无记忆时返回一句「当前没有任何长期记忆」，而不是空串——明确告诉模型没东西
/// 可查，省掉一次无谓的读取。
pub fn render_index(memories: &[Memory], budget: &IndexBudget) -> String {
    if memories.is_empty() {
        return format!("{MEMORY_SECTION_TITLE}\n当前没有任何长期记忆。");
    }

    let rendered: Vec<String> = memories
        .iter()
        .map(|memory| render_entry(memory, budget.summary_chars))
        .collect();

    let mut by_recency: Vec<usize> = (0..memories.len()).collect();
    by_recency.sort_by(|&a, &b| memories[b].updated_at.cmp(&memories[a].updated_at));

    let mut keep = vec![false; memories.len()];
    let mut used = 0usize;
    for (nth, &i) in by_recency.iter().take(budget.max_entries).enumerate() {
        let cost = rendered[i].chars().count() + 1;
        // 至少留一条：预算被调得极小时整段空掉，模型会以为根本没有记忆
        if nth > 0 && used + cost > budget.max_chars {
            break;
        }
        used += cost;
        keep[i] = true;
    }

    let picked: Vec<&String> = rendered
        .iter()
        .zip(&keep)
        .filter_map(|(entry, &keep)| keep.then_some(entry))
        .collect();

    let total = memories.len();
    let shown = picked.len();
    let mut out = String::from(MEMORY_SECTION_TITLE);
    out.push('\n');
    if shown == total {
        out.push_str(&format!("以下是你全部的长期记忆索引（共 {total} 条）。"));
    } else {
        out.push_str(&format!(
            "以下是你最近更新的 {shown} 条长期记忆索引（共 {total} 条）。"
        ));
    }
    out.push_str(NUMBERING_NOTE);
    out.push('\n');

    for entry in picked {
        out.push('\n');
        out.push_str(entry);
    }

    if shown < total {
        out.push_str(&format!("\n\n另有 {} 条较早的记忆未列出。", total - shown));
    }
    out
}

/// 渲染单条：编号 + 标题 / 标签 / 摘要。
fn render_entry(memory: &Memory, summary_chars: usize) -> String {
    let mut entry = format!("#{} {}\n", memory.id, memory.title);
    if !memory.tags.is_empty() {
        entry.push_str(&format!("   {}\n", memory.tags.join(", ")));
    }
    let summary = memory.summary.trim();
    if !summary.is_empty() {
        entry.push_str(&format!("   {}\n", truncate(summary, summary_chars)));
    }
    entry
}

/// 按字符截断，超出时补省略号。
fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let cut: String = text.chars().take(max_chars).collect();
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn memory(id: i64, title: &str, updated_secs: i64) -> Memory {
        Memory {
            id,
            title: title.into(),
            summary: format!("{title} 的摘要"),
            body: "正文".into(),
            tags: vec!["tag_a".into(), "tag_b".into()],
            source_session_id: None,
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
            updated_at: Utc.timestamp_opt(updated_secs, 0).unwrap(),
        }
    }

    #[test]
    fn empty_says_so_explicitly() {
        let text = render_index(&[], &IndexBudget::default());
        assert!(text.contains("当前没有任何长期记忆"));
    }

    #[test]
    fn numbers_are_db_ids_and_gaps_are_explained() {
        let items = vec![memory(3, "旧的", 100), memory(21, "新的", 300)];
        let text = render_index(&items, &IndexBudget::default());
        assert!(text.contains("共 2 条"));
        assert!(!text.contains("未列出"));
        assert!(text.contains("#3 旧的"));
        assert!(text.contains("#21 新的"));
        assert!(text.contains("删掉的号也不会补位"), "断号要有一句交代");
    }

    #[test]
    fn order_is_id_ascending_regardless_of_update_time() {
        let items = vec![
            memory(3, "最早建的", 999),
            memory(13, "中间的", 100),
            memory(21, "最后建的", 500),
        ];
        let text = render_index(&items, &IndexBudget::default());
        let positions: Vec<usize> = ["#3 最早建的", "#13 中间的", "#21 最后建的"]
            .iter()
            .map(|needle| text.find(needle).expect("每条都该在"))
            .collect();
        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "刚更新过的不该插队到前面：{text}"
        );
    }

    #[test]
    fn entry_cap_drops_the_stalest_but_keeps_id_order() {
        let items = vec![
            memory(1, "最旧", 100),
            memory(2, "最新", 300),
            memory(3, "次新", 200),
        ];
        let budget = IndexBudget {
            max_entries: 2,
            ..Default::default()
        };
        let text = render_index(&items, &budget);
        assert!(!text.contains("#1 最旧"), "丢的该是最久没更新的那条");
        assert!(text.contains("#2 最新") && text.contains("#3 次新"));
        assert!(text.find("#2 最新").unwrap() < text.find("#3 次新").unwrap());
        assert!(text.contains("另有 1 条较早的记忆未列出"));
    }

    #[test]
    fn char_cap_still_keeps_at_least_one() {
        let items = vec![memory(1, "最旧", 100), memory(2, "最新", 300)];
        let budget = IndexBudget {
            max_chars: 1,
            ..Default::default()
        };
        let text = render_index(&items, &budget);
        assert!(text.contains("#2 最新"));
        assert!(text.contains("另有 1 条较早的记忆未列出"));
    }

    #[test]
    fn long_summary_is_truncated() {
        let mut item = memory(1, "长摘要", 100);
        item.summary = "字".repeat(200);
        let budget = IndexBudget {
            summary_chars: 10,
            ..Default::default()
        };
        let text = render_index(&[item], &budget);
        assert!(text.contains(&format!("{}…", "字".repeat(10))));
    }
}
