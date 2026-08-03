//! 常驻索引：把全部记忆的标题/标签/摘要渲染成一段提示词。
//!
//! 记忆量小的时候，把索引整个放进 system prompt 比任何检索都准——模型看得见
//! 全部条目，要正文再按编号读。代价是索引会随条数增长，所以有 [`IndexBudget`]
//! 兜底：超预算就只留最近更新的若干条，并明确告知还有多少没列出。
//!
//! 模型看到的 `#N` 是**展示编号**（1 起连续），不是 SQLite 自增 id。排序规则：
//! `#1` 永远是置顶记忆，其余按 `updated_at` 倒序为 `#2`、`#3`…

use crate::types::Memory;

/// 索引段落标题。
pub const MEMORY_SECTION_TITLE: &str = "=== [记忆] Memory ===";

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

/// 渲染索引段落。
///
/// `entries` 需已按展示顺序排好：`(slot, memory)`，slot 从 1 连续编号。
/// 预算不够时丢弃的是**靠后**的条目；置顶（`pinned`）条目始终保留在 `#1`。
///
/// 无记忆时返回一句「当前没有任何长期记忆」，而不是空串——明确告诉模型没东西
/// 可查，省掉一次无谓的读取。
pub fn render_index(entries: &[(i64, Memory)], budget: &IndexBudget) -> String {
    if entries.is_empty() {
        return format!("{MEMORY_SECTION_TITLE}\n当前没有任何长期记忆。");
    }

    let has_pinned = entries.first().is_some_and(|(_, m)| m.pinned);
    let mut picked: Vec<String> = Vec::new();
    let mut used = 0usize;
    let mut rest = entries.iter();

    if has_pinned {
        if let Some((slot, memory)) = rest.next() {
            let entry = render_entry(*slot, memory, budget.summary_chars);
            used += entry.chars().count() + 1;
            picked.push(entry);
        }
    }

    let rest_cap = budget.max_entries.saturating_sub(picked.len());
    for (slot, memory) in rest.take(rest_cap) {
        let entry = render_entry(*slot, memory, budget.summary_chars);
        let cost = entry.chars().count() + 1;
        if !picked.is_empty() && used + cost > budget.max_chars {
            break;
        }
        used += cost;
        picked.push(entry);
    }

    if picked.is_empty() {
        if let Some((slot, memory)) = entries.first() {
            picked.push(render_entry(*slot, memory, budget.summary_chars));
        }
    }

    let total = entries.len();
    let shown = picked.len();
    let mut out = String::from(MEMORY_SECTION_TITLE);
    out.push('\n');
    if shown == total {
        out.push_str(&format!(
            "以下是你全部的长期记忆索引（共 {total} 条）。需要某条正文时按编号读取。\n"
        ));
    } else {
        out.push_str(&format!(
            "以下是你最近更新的 {shown} 条长期记忆索引（共 {total} 条）。需要某条正文时按编号读取。\n"
        ));
    }

    for entry in &picked {
        out.push('\n');
        out.push_str(entry);
    }

    if shown < total {
        out.push_str(&format!("\n\n另有 {} 条较早的记忆未列出。", total - shown));
    }
    out
}

/// 渲染单条：展示编号 + 标题 / 标签 / 摘要。
fn render_entry(slot: i64, memory: &Memory, summary_chars: usize) -> String {
    let mut entry = format!("#{} {}\n", slot, memory.title);
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

    fn memory(id: i64, title: &str, updated_secs: i64, pinned: bool) -> Memory {
        Memory {
            id,
            title: title.into(),
            summary: format!("{title} 的摘要"),
            body: "正文".into(),
            tags: vec!["tag_a".into(), "tag_b".into()],
            pinned,
            source_session_id: None,
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
            updated_at: Utc.timestamp_opt(updated_secs, 0).unwrap(),
        }
    }

    fn slotted(items: Vec<Memory>) -> Vec<(i64, Memory)> {
        items
            .into_iter()
            .enumerate()
            .map(|(i, m)| ((i + 1) as i64, m))
            .collect()
    }

    #[test]
    fn empty_says_so_explicitly() {
        let text = render_index(&[], &IndexBudget::default());
        assert!(text.contains("当前没有任何长期记忆"));
    }

    #[test]
    fn uses_sequential_slots_not_db_ids() {
        let items = slotted(vec![memory(7, "新的", 300, false), memory(2, "旧的", 100, false)]);
        let text = render_index(&items, &IndexBudget::default());
        assert!(text.contains("共 2 条"));
        assert!(!text.contains("未列出"));
        assert!(text.contains("#1 新的"));
        assert!(text.contains("#2 旧的"));
        assert!(!text.contains("#7"));
    }

    #[test]
    fn pinned_stays_at_slot_one() {
        let items = slotted(vec![
            memory(1, "置顶", 50, true),
            memory(5, "最新", 300, false),
            memory(3, "次新", 200, false),
        ]);
        let text = render_index(&items, &IndexBudget::default());
        assert!(text.contains("#1 置顶"));
        assert!(text.contains("#2 最新"));
        assert!(text.contains("#3 次新"));
        let first = text.find("#1 置顶").unwrap();
        let second = text.find("#2 最新").unwrap();
        assert!(first < second);
    }

    #[test]
    fn entry_cap_keeps_most_recent_after_pinned() {
        let items = slotted(vec![
            memory(1, "置顶", 10, true),
            memory(2, "最新", 300, false),
            memory(3, "次新", 200, false),
            memory(4, "最旧", 100, false),
        ]);
        let budget = IndexBudget {
            max_entries: 3,
            ..Default::default()
        };
        let text = render_index(&items, &budget);
        assert!(text.contains("#1 置顶"));
        assert!(text.contains("#2 最新") && text.contains("#3 次新"));
        assert!(!text.contains("#4 最旧"));
        assert!(text.contains("另有 1 条较早的记忆未列出"));
    }

    #[test]
    fn char_cap_keeps_pinned_and_at_least_one_more() {
        let items = slotted(vec![
            memory(1, "置顶", 10, true),
            memory(2, "最新", 300, false),
        ]);
        let budget = IndexBudget {
            max_chars: 1,
            ..Default::default()
        };
        let text = render_index(&items, &budget);
        assert!(text.contains("#1 置顶"));
        assert!(text.contains("另有 1 条较早的记忆未列出"));
    }

    #[test]
    fn long_summary_is_truncated() {
        let mut item = memory(1, "长摘要", 100, false);
        item.summary = "字".repeat(200);
        let budget = IndexBudget {
            summary_chars: 10,
            ..Default::default()
        };
        let text = render_index(&slotted(vec![item]), &budget);
        assert!(text.contains(&format!("{}…", "字".repeat(10))));
    }
}
