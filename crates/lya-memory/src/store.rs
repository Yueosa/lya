//! [`MemoryStore`]：记忆的仓储实现。
//!
//! 只做存取与索引渲染，不定义暴露给模型的 action/tool——那属于 `lya-action`，
//! 它才决定哪些操作给 LLM（删除就不给）以及配套的提示词。

use std::sync::Arc;

use chrono::{DateTime, Utc};
use lya_db::Db;
use rusqlite::{Connection, OptionalExtension, params};

use crate::MIGRATION_SQL;
use crate::error::MemoryError;
use crate::index::{IndexBudget, render_index};
use crate::types::{MatchField, Memory, MemoryHit, MemoryLimits, MemoryPatch, NewMemory};

/// 长期记忆仓储。
pub struct MemoryStore {
    /// 共享数据库句柄。
    db: Arc<Db>,
    /// 写入长度上限。
    limits: MemoryLimits,
    /// 常驻索引的体积上限。
    budget: IndexBudget,
}

impl MemoryStore {
    /// 用已打开的 [`Db`] 构造，并登记 memory 迁移（不执行）。
    pub fn new(db: Db) -> Self {
        Self {
            db: Arc::new(db.with_migration(MIGRATION_SQL)),
            limits: MemoryLimits::default(),
            budget: IndexBudget::default(),
        }
    }

    /// 复用别处已经建好的 [`Db`]。
    ///
    /// **不登记迁移**——调用方要自己先 `with_migration(lya_memory::MIGRATION_SQL)`
    /// 并 `migrate()`。与 `lya-session` 共享同一个库文件时用这个。
    pub fn with_db(db: Arc<Db>) -> Self {
        Self {
            db,
            limits: MemoryLimits::default(),
            budget: IndexBudget::default(),
        }
    }

    /// 打开默认库 `~/.lya/lya.db` 并立即迁移。
    pub fn open_default() -> Result<Self, MemoryError> {
        let store = Self::new(Db::open_default()?);
        store.migrate()?;
        Ok(store)
    }

    /// 打开指定库文件并立即迁移。
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, MemoryError> {
        let store = Self::new(Db::open(path)?);
        store.migrate()?;
        Ok(store)
    }

    /// 覆盖写入长度上限。
    pub fn with_limits(mut self, limits: MemoryLimits) -> Self {
        self.limits = limits;
        self
    }

    /// 覆盖常驻索引的体积上限。
    pub fn with_budget(mut self, budget: IndexBudget) -> Self {
        self.budget = budget;
        self
    }

    /// 执行已登记的迁移。
    pub fn migrate(&self) -> Result<(), MemoryError> {
        self.db.migrate()?;
        Ok(())
    }

    /// 底层数据库，供共享同一文件的其它领域 crate 复用。
    pub fn db(&self) -> &Db {
        &self.db
    }

    // ── 写 ───────────────────────────────────────────────────────

    /// 新建记忆；标题已存在则报 [`MemoryError::DuplicateTitle`]。
    pub fn create(&self, new: NewMemory) -> Result<Memory, MemoryError> {
        let fields = self.normalize(new)?;
        self.db.write(|conn| {
            if find_id_by_title(conn, &fields.title)?.is_some() {
                return Err(MemoryError::DuplicateTitle(fields.title));
            }
            let id = insert(conn, &fields, Utc::now())?;
            load(conn, id)
        })
    }

    /// 按标题写入：不存在就新建，已存在就整体覆盖。
    ///
    /// 这是给模型用的 `write` 语义——它不需要先查一次再决定新建还是更新。
    /// 覆盖是**整体替换**，摘要与标签留空就会被清掉，想局部改用
    /// [`MemoryStore::update`]。
    pub fn upsert_by_title(&self, new: NewMemory) -> Result<Memory, MemoryError> {
        let fields = self.normalize(new)?;
        self.db.write(|conn| {
            let now = Utc::now();
            let id = match find_id_by_title(conn, &fields.title)? {
                Some(id) => {
                    conn.execute(
                        "UPDATE memories
                         SET summary = ?1, body = ?2, source_session_id = ?3, updated_at = ?4
                         WHERE id = ?5",
                        params![
                            fields.summary,
                            fields.body,
                            fields.source_session_id,
                            now.to_rfc3339(),
                            id,
                        ],
                    )?;
                    replace_tags(conn, id, &fields.tags)?;
                    id
                }
                None => insert(conn, &fields, now)?,
            };
            load(conn, id)
        })
    }

    /// 局部更新；`patch` 里为 `None` 的字段保持原值。
    pub fn update(&self, id: i64, patch: MemoryPatch) -> Result<Memory, MemoryError> {
        if patch.is_empty() {
            return self.get(id);
        }
        let title = patch
            .title
            .as_deref()
            .map(|t| self.limits.check_title(t))
            .transpose()?;
        let summary = patch
            .summary
            .as_deref()
            .map(|s| self.limits.check_summary(s))
            .transpose()?;
        let body = patch
            .body
            .as_deref()
            .map(|b| self.limits.check_body(b))
            .transpose()?;
        let tags = patch
            .tags
            .as_deref()
            .map(|t| self.limits.check_tags(t))
            .transpose()?;

        self.db.write(|conn| {
            ensure_exists(conn, id)?;
            if let Some(title) = &title {
                match find_id_by_title(conn, title)? {
                    Some(other) if other != id => {
                        return Err(MemoryError::DuplicateTitle(title.clone()));
                    }
                    _ => {}
                }
                conn.execute(
                    "UPDATE memories SET title = ?1 WHERE id = ?2",
                    params![title, id],
                )?;
            }
            if let Some(summary) = &summary {
                conn.execute(
                    "UPDATE memories SET summary = ?1 WHERE id = ?2",
                    params![summary, id],
                )?;
            }
            if let Some(body) = &body {
                conn.execute(
                    "UPDATE memories SET body = ?1 WHERE id = ?2",
                    params![body, id],
                )?;
            }
            if let Some(tags) = &tags {
                replace_tags(conn, id, tags)?;
            }
            conn.execute(
                "UPDATE memories SET updated_at = ?1 WHERE id = ?2",
                params![Utc::now().to_rfc3339(), id],
            )?;
            load(conn, id)
        })
    }

    /// 删除记忆（标签由外键级联删除）。
    ///
    /// 仓储层提供，但不打算暴露给模型：破坏性操作只走用户界面。
    pub fn delete(&self, id: i64) -> Result<(), MemoryError> {
        self.db.write(|conn| {
            let n = conn.execute("DELETE FROM memories WHERE id = ?1", [id])?;
            if n == 0 {
                return Err(MemoryError::NotFound(id));
            }
            Ok(())
        })
    }

    // ── 读 ───────────────────────────────────────────────────────

    /// 按 id 读取完整记忆（含正文）。
    pub fn get(&self, id: i64) -> Result<Memory, MemoryError> {
        self.db.read(|conn| load(conn, id))
    }

    /// 按标题精确查找。
    pub fn find_by_title(&self, title: &str) -> Result<Option<Memory>, MemoryError> {
        let title = title.trim();
        self.db.read(|conn| match find_id_by_title(conn, title)? {
            Some(id) => load(conn, id).map(Some),
            None => Ok(None),
        })
    }

    /// 全部记忆，按 `updated_at` 倒序。
    pub fn list(&self) -> Result<Vec<Memory>, MemoryError> {
        self.db.read(load_all)
    }

    /// 全文检索。
    ///
    /// 常驻索引只有标题、摘要和标签，**搜不到正文**；而且索引有体积上限，
    /// 超出的条目模型根本看不见——索引会说「另有 N 条未列出」，却没有任何手段
    /// 够到它们。这两点让检索成为必需，而不是「以后可能要」。
    ///
    /// 用 `LIKE` 而不是 FTS：这个量级下没必要，也免了维护索引表。
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryHit>, MemoryError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(MemoryError::Invalid("检索词不能为空".into()));
        }
        let pattern = format!("%{}%", escape_like(query));

        let ids: Vec<i64> = self.db.read(|conn| -> Result<Vec<i64>, MemoryError> {
            let mut stmt = conn.prepare(
                "SELECT m.id FROM memories m
                 LEFT JOIN memory_tags t ON t.memory_id = m.id
                 WHERE m.title LIKE ?1 ESCAPE '\\'
                    OR m.summary LIKE ?1 ESCAPE '\\'
                    OR m.body LIKE ?1 ESCAPE '\\'
                    OR t.tag LIKE ?1 ESCAPE '\\'
                 GROUP BY m.id
                 ORDER BY m.updated_at DESC
                 LIMIT ?2",
            )?;
            let ids = stmt
                .query_map(params![pattern, limit as i64], |row| row.get(0))?
                .collect::<Result<Vec<i64>, _>>()?;
            Ok(ids)
        })?;

        let mut hits = Vec::with_capacity(ids.len());
        for id in ids {
            let memory = self.get(id)?;
            let (matched_in, snippet) = locate(&memory, query);
            hits.push(MemoryHit {
                id: memory.id,
                title: memory.title,
                summary: memory.summary,
                tags: memory.tags,
                matched_in,
                snippet,
            });
        }
        Ok(hits)
    }

    /// 记忆条数。
    pub fn count(&self) -> Result<i64, MemoryError> {
        self.db
            .read(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))?))
    }

    /// 渲染常驻索引段落，直接塞进 `lya_prompt::PromptInput::memory_section`。
    pub fn index_section(&self) -> Result<String, MemoryError> {
        let all = self.list()?;
        Ok(render_index(&all, &self.budget))
    }

    /// 校验并规整一次写入的全部字段。
    fn normalize(&self, new: NewMemory) -> Result<Fields, MemoryError> {
        Ok(Fields {
            title: self.limits.check_title(&new.title)?,
            summary: self.limits.check_summary(&new.summary)?,
            body: self.limits.check_body(&new.body)?,
            tags: self.limits.check_tags(&new.tags)?,
            source_session_id: new.source_session_id,
        })
    }
}

/// 校验后的写入字段。
struct Fields {
    title: String,
    summary: String,
    body: String,
    tags: Vec<String>,
    source_session_id: Option<String>,
}

fn insert(conn: &Connection, fields: &Fields, now: DateTime<Utc>) -> Result<i64, MemoryError> {
    conn.execute(
        "INSERT INTO memories (title, summary, body, source_session_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            fields.title,
            fields.summary,
            fields.body,
            fields.source_session_id,
            now.to_rfc3339(),
            now.to_rfc3339(),
        ],
    )?;
    let id = conn.last_insert_rowid();
    replace_tags(conn, id, &fields.tags)?;
    Ok(id)
}

/// 标签整体替换：先清空再写入，避免逐个 diff。写入顺序记进 `ord`。
fn replace_tags(conn: &Connection, id: i64, tags: &[String]) -> Result<(), MemoryError> {
    conn.execute("DELETE FROM memory_tags WHERE memory_id = ?1", [id])?;
    for (ord, tag) in tags.iter().enumerate() {
        conn.execute(
            "INSERT INTO memory_tags (memory_id, tag, ord) VALUES (?1, ?2, ?3)",
            params![id, tag, ord as i64],
        )?;
    }
    Ok(())
}

fn ensure_exists(conn: &Connection, id: i64) -> Result<(), MemoryError> {
    let exists = conn
        .query_row("SELECT 1 FROM memories WHERE id = ?1", [id], |_| Ok(()))
        .optional()?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(MemoryError::NotFound(id))
    }
}

fn find_id_by_title(conn: &Connection, title: &str) -> Result<Option<i64>, MemoryError> {
    Ok(conn
        .query_row("SELECT id FROM memories WHERE title = ?1", [title], |row| {
            row.get(0)
        })
        .optional()?)
}

fn load(conn: &Connection, id: i64) -> Result<Memory, MemoryError> {
    let mut memory = conn
        .query_row(
            "SELECT id, title, summary, body, source_session_id, created_at, updated_at
             FROM memories WHERE id = ?1",
            [id],
            row_to_memory,
        )
        .optional()?
        .ok_or(MemoryError::NotFound(id))??;
    memory.tags = load_tags(conn, id)?;
    Ok(memory)
}

/// 一次取全量。记忆是稀疏数据（旧实现三个月只攒了 8 条），没必要分页。
fn load_all(conn: &Connection) -> Result<Vec<Memory>, MemoryError> {
    let mut stmt = conn.prepare(
        "SELECT id, title, summary, body, source_session_id, created_at, updated_at
         FROM memories ORDER BY updated_at DESC, id DESC",
    )?;
    let rows = stmt
        .query_map([], row_to_memory)?
        .collect::<Result<Vec<_>, _>>()?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut memory = row?;
        memory.tags = load_tags(conn, memory.id)?;
        out.push(memory);
    }
    Ok(out)
}

fn load_tags(conn: &Connection, id: i64) -> Result<Vec<String>, MemoryError> {
    let mut stmt =
        conn.prepare("SELECT tag FROM memory_tags WHERE memory_id = ?1 ORDER BY ord ASC")?;
    let tags = stmt
        .query_map([id], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(tags)
}

/// 行映射；时间解析可能失败，所以返回嵌套 `Result`。
fn row_to_memory(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<Memory, MemoryError>> {
    let created: String = row.get(5)?;
    let updated: String = row.get(6)?;
    Ok((|| {
        Ok(Memory {
            id: row.get(0)?,
            title: row.get(1)?,
            summary: row.get(2)?,
            body: row.get(3)?,
            tags: Vec::new(),
            source_session_id: row.get(4)?,
            created_at: parse_time(&created)?,
            updated_at: parse_time(&updated)?,
        })
    })())
}

/// 转义 `LIKE` 的通配符，免得用户搜 `%` 时匹配到全部。
fn escape_like(query: &str) -> String {
    query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// 找出命中在哪个字段，并摘一段上下文。
fn locate(memory: &Memory, query: &str) -> (MatchField, String) {
    let needle = query.to_lowercase();
    if memory.title.to_lowercase().contains(&needle) {
        return (MatchField::Title, memory.title.clone());
    }
    if let Some(tag) = memory
        .tags
        .iter()
        .find(|tag| tag.to_lowercase().contains(&needle))
    {
        return (MatchField::Tag, tag.clone());
    }
    if memory.summary.to_lowercase().contains(&needle) {
        return (MatchField::Summary, memory.summary.clone());
    }
    (MatchField::Body, excerpt(&memory.body, query))
}

/// 摘出正文里命中处前后一小段。
fn excerpt(body: &str, query: &str) -> String {
    const CONTEXT: usize = 60;
    let chars: Vec<char> = body.chars().collect();
    let lower: String = body.to_lowercase();
    let needle = query.to_lowercase();

    let Some(byte_pos) = lower.find(&needle) else {
        return chars.iter().take(CONTEXT * 2).collect();
    };
    // 字节位置换成字符位置，中文才不会切碎
    let hit = lower[..byte_pos].chars().count();
    let start = hit.saturating_sub(CONTEXT);
    let end = (hit + needle.chars().count() + CONTEXT).min(chars.len());

    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.extend(&chars[start..end]);
    if end < chars.len() {
        out.push('…');
    }
    out
}

fn parse_time(s: &str) -> Result<DateTime<Utc>, MemoryError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| MemoryError::Invalid(format!("bad timestamp {s}: {err}")))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn store() -> (TempDir, MemoryStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::open(dir.path().join("lya.db")).unwrap();
        (dir, store)
    }

    #[test]
    fn create_and_get_roundtrip() {
        let (_dir, store) = store();
        let created = store
            .create(
                NewMemory::new("Hyprland 崩溃", "换 -git 包可绕过")
                    .with_summary("多显示器下 DRM page-flip 崩溃")
                    .with_tags(["hyprland", "drm", "hyprland"])
                    .from_session("s1"),
            )
            .unwrap();

        assert_eq!(created.id, 1);
        // 标签去重且排序
        assert_eq!(
            created.tags,
            vec!["hyprland".to_string(), "drm".to_string()],
            "去重但保持写入顺序"
        );
        assert_eq!(created.source_session_id.as_deref(), Some("s1"));
        assert_eq!(store.get(created.id).unwrap(), created);
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn duplicate_title_rejected_but_upsert_overwrites() {
        let (_dir, store) = store();
        store.create(NewMemory::new("同名", "第一版")).unwrap();

        assert!(matches!(
            store.create(NewMemory::new("同名", "第二版")),
            Err(MemoryError::DuplicateTitle(_))
        ));

        let updated = store
            .upsert_by_title(NewMemory::new("同名", "第二版").with_tags(["新标签"]))
            .unwrap();
        assert_eq!(updated.id, 1, "同名应更新原记录而不是新建");
        assert_eq!(updated.body, "第二版");
        assert_eq!(updated.tags, vec!["新标签".to_string()]);
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn upsert_creates_when_absent() {
        let (_dir, store) = store();
        let created = store
            .upsert_by_title(NewMemory::new("新的", "正文"))
            .unwrap();
        assert_eq!(created.id, 1);
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn patch_only_touches_given_fields() {
        let (_dir, store) = store();
        let created = store
            .create(
                NewMemory::new("原标题", "原正文")
                    .with_summary("原摘要")
                    .with_tags(["a"]),
            )
            .unwrap();

        let patched = store
            .update(
                created.id,
                MemoryPatch {
                    body: Some("新正文".into()),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(patched.body, "新正文");
        assert_eq!(patched.title, "原标题");
        assert_eq!(patched.summary, "原摘要");
        assert_eq!(patched.tags, vec!["a".to_string()]);
        assert!(patched.updated_at >= created.updated_at);
    }

    #[test]
    fn rename_into_existing_title_rejected() {
        let (_dir, store) = store();
        store.create(NewMemory::new("甲", "x")).unwrap();
        let second = store.create(NewMemory::new("乙", "y")).unwrap();

        assert!(matches!(
            store.update(
                second.id,
                MemoryPatch {
                    title: Some("甲".into()),
                    ..Default::default()
                }
            ),
            Err(MemoryError::DuplicateTitle(_))
        ));

        // 改成自己的标题不算冲突
        store
            .update(
                second.id,
                MemoryPatch {
                    title: Some("乙".into()),
                    ..Default::default()
                },
            )
            .unwrap();
    }

    #[test]
    fn delete_cascades_tags() {
        let (_dir, store) = store();
        let created = store
            .create(NewMemory::new("要删的", "x").with_tags(["t1", "t2"]))
            .unwrap();

        store.delete(created.id).unwrap();
        assert!(matches!(
            store.get(created.id),
            Err(MemoryError::NotFound(_))
        ));
        assert!(matches!(
            store.delete(created.id),
            Err(MemoryError::NotFound(_))
        ));

        let orphans: i64 = store
            .db()
            .read::<_, MemoryError>(|conn| {
                Ok(conn.query_row("SELECT COUNT(*) FROM memory_tags", [], |row| row.get(0))?)
            })
            .unwrap();
        assert_eq!(orphans, 0);
    }

    #[test]
    fn list_is_newest_first_and_index_renders() {
        let (_dir, store) = store();
        store.create(NewMemory::new("先写的", "x")).unwrap();
        let second = store.create(NewMemory::new("后写的", "y")).unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.first().unwrap().id, second.id);

        let section = store.index_section().unwrap();
        assert!(section.contains("共 2 条"));
        assert!(section.contains("#1 先写的"));
        assert!(section.contains("#2 后写的"));
    }

    #[test]
    fn empty_title_and_overlong_body_rejected() {
        let (_dir, store) = store();
        assert!(matches!(
            store.create(NewMemory::new("   ", "x")),
            Err(MemoryError::Invalid(_))
        ));

        let store = store.with_limits(MemoryLimits {
            max_body_chars: 4,
            ..Default::default()
        });
        assert!(matches!(
            store.create(NewMemory::new("标题", "太长了一点")),
            Err(MemoryError::Invalid(_))
        ));
    }

    #[test]
    fn search_reaches_into_the_body() {
        let (_dir, store) = store();
        store
            .create(
                NewMemory::new("NetworkManager 高 CPU", "根因是 qshell 泄漏了 nmcli monitor 进程")
                    .with_summary("NM 吃满 CPU")
                    .with_tags(["NetworkManager", "bug"]),
            )
            .unwrap();

        // 常驻索引里没有 nmcli，只有正文里有——这正是检索存在的理由
        let hits = store.search("nmcli", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].matched_in, MatchField::Body);
        assert!(hits[0].snippet.contains("nmcli"));
        assert_eq!(hits[0].title, "NetworkManager 高 CPU");
    }

    #[test]
    fn search_reports_which_field_matched() {
        let (_dir, store) = store();
        store
            .create(
                NewMemory::new("Hyprland 崩溃", "正文提到 aquamarine")
                    .with_summary("多显示器下崩溃")
                    .with_tags(["drm", "troubleshooting"]),
            )
            .unwrap();

        assert_eq!(
            store.search("Hyprland", 10).unwrap()[0].matched_in,
            MatchField::Title
        );
        assert_eq!(store.search("drm", 10).unwrap()[0].matched_in, MatchField::Tag);
        assert_eq!(
            store.search("多显示器", 10).unwrap()[0].matched_in,
            MatchField::Summary
        );
        assert_eq!(
            store.search("aquamarine", 10).unwrap()[0].matched_in,
            MatchField::Body
        );
    }

    #[test]
    fn search_respects_limit_and_rejects_empty() {
        let (_dir, store) = store();
        for i in 0..5 {
            store
                .create(NewMemory::new(format!("条目 {i}"), "共同关键词 zebra"))
                .unwrap();
        }
        assert_eq!(store.search("zebra", 3).unwrap().len(), 3);
        assert!(store.search("   ", 3).is_err());
        assert!(store.search("查不到的词", 3).unwrap().is_empty());
    }

    #[test]
    fn wildcards_do_not_match_everything() {
        let (_dir, store) = store();
        store.create(NewMemory::new("普通条目", "正文")).unwrap();
        // 不转义的话 % 会匹配到所有记录
        assert!(store.search("%", 10).unwrap().is_empty());
        assert!(store.search("_", 10).unwrap().is_empty());
    }

    #[test]
    fn body_excerpt_is_trimmed_around_the_hit() {
        let (_dir, store) = store();
        let long = format!("{}关键词{}", "前".repeat(200), "后".repeat(200));
        store.create(NewMemory::new("长正文", long)).unwrap();

        let snippet = &store.search("关键词", 10).unwrap()[0].snippet;
        assert!(snippet.contains("关键词"));
        assert!(snippet.starts_with('…') && snippet.ends_with('…'));
        assert!(snippet.chars().count() < 200, "只给命中处附近，不是整篇");
    }

    #[test]
    fn find_by_title_matches_exactly() {
        let (_dir, store) = store();
        store.create(NewMemory::new("环境操作偏好", "x")).unwrap();
        assert!(store.find_by_title("环境操作偏好").unwrap().is_some());
        assert!(store.find_by_title("环境").unwrap().is_none());
    }
}
