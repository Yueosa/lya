//! [`SessionStore`]：会话 CRUD 与消息树操作。
//!
//! 所有写操作都跑在 [`lya_db::Db::write`] 的事务里，读操作走 [`lya_db::Db::read`]。
//! 一次调用 = 一个事务，调用方不需要自己管理原子性。

use std::sync::Arc;

use chrono::{DateTime, Utc};
use lya_db::Db;
use lya_mode::Mode;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use uuid::Uuid;

use crate::MIGRATION_SQL;
use crate::error::SessionError;
use crate::message::{MessagePayload, MessageRole, MessageStatus};
use crate::types::{CreateSession, MessageRecord, SessionMeta, SessionStatus};

/// 会话存储：会话元数据 + 消息树。
pub struct SessionStore {
    /// 共享数据库句柄。
    db: Arc<Db>,
}

impl SessionStore {
    /// 用已打开的 [`Db`] 构造，并把 session 迁移登记进去。
    ///
    /// 只登记不执行，调用方需要再调 [`SessionStore::migrate`]；
    /// 这样多个领域 crate 可以先各自登记迁移，最后统一执行。
    pub fn new(db: Db) -> Self {
        Self {
            db: Arc::new(db.with_migration(MIGRATION_SQL)),
        }
    }

    /// 复用别处已经建好的 [`Db`]。
    ///
    /// 与 [`SessionStore::new`] 的区别是**不登记迁移**——调用方要自己先
    /// `with_migration(lya_session::MIGRATION_SQL)` 并 `migrate()`。
    /// 多个领域仓储共享同一个库文件时用这个，这样写入仍走同一把锁，
    /// 不会出现两个连接互相抢。
    pub fn with_db(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// 打开默认库 `~/.lya/lya.db` 并立即迁移。
    pub fn open_default() -> Result<Self, SessionError> {
        let store = Self::new(Db::open_default()?);
        store.migrate()?;
        Ok(store)
    }

    /// 打开指定库文件并立即迁移。
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, SessionError> {
        let store = Self::new(Db::open(path)?);
        store.migrate()?;
        Ok(store)
    }

    /// 执行已登记的迁移。
    pub fn migrate(&self) -> Result<(), SessionError> {
        self.db.migrate()?;
        Ok(())
    }

    /// 底层数据库，供共享同一文件的其它领域 crate 复用。
    pub fn db(&self) -> &Db {
        &self.db
    }

    // ── 会话元数据 ────────────────────────────────────────────────

    /// 创建会话，返回新会话的元数据快照。
    pub fn create_session(&self, req: CreateSession) -> Result<SessionMeta, SessionError> {
        let meta = SessionMeta {
            id: Uuid::new_v4().to_string(),
            title: req.title,
            status: SessionStatus::Active,
            active_leaf_id: None,
            work_mode: req.work_mode,
            persona: req.persona,
            model_id: req.model_id,
            enabled_tools: req.enabled_tools,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let tools_json = meta
            .enabled_tools
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        self.db.write(|conn| -> Result<(), SessionError> {
            conn.execute(
                "INSERT INTO sessions (
                     id, title, status, active_leaf_id, work_mode, persona, model_id,
                     enabled_tools_json, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    meta.id,
                    meta.title,
                    meta.status.as_str(),
                    meta.work_mode.as_str(),
                    meta.persona,
                    meta.model_id,
                    tools_json,
                    meta.created_at.to_rfc3339(),
                    meta.updated_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })?;

        Ok(meta)
    }

    /// 读取会话；不存在返回 `Ok(None)`。
    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionMeta>, SessionError> {
        self.db.read(|conn| load_session(conn, session_id))
    }

    /// 列出活跃会话，按更新时间倒序。
    pub fn list_sessions(&self) -> Result<Vec<SessionMeta>, SessionError> {
        self.db.read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, title, status, active_leaf_id, work_mode, persona, model_id,
                        enabled_tools_json, created_at, updated_at
                 FROM sessions
                 WHERE status = 'active'
                 ORDER BY updated_at DESC",
            )?;
            let rows = stmt
                .query_map([], RawSession::from_row)?
                .collect::<Result<Vec<_>, _>>()?;
            rows.into_iter().map(RawSession::into_meta).collect()
        })
    }

    /// 归档会话（不删除消息）。
    pub fn archive_session(&self, session_id: &str) -> Result<(), SessionError> {
        self.set_field(session_id, |conn, now| {
            conn.execute(
                "UPDATE sessions SET status = 'archived', updated_at = ?1 WHERE id = ?2",
                params![now, session_id],
            )?;
            Ok(())
        })
    }

    /// 设置工作模式。
    pub fn set_work_mode(&self, session_id: &str, mode: Mode) -> Result<(), SessionError> {
        self.set_field(session_id, |conn, now| {
            conn.execute(
                "UPDATE sessions SET work_mode = ?1, updated_at = ?2 WHERE id = ?3",
                params![mode.as_str(), now, session_id],
            )?;
            Ok(())
        })
    }

    /// 设置用户启用的工具列表（内部名）；`None` 表示启用全部。
    ///
    /// 这里只存用户意愿，不做 RWX 校验；实际可见工具由
    /// [`lya_mode::Mode::resolve`] 与本列表取交集决定。
    pub fn set_enabled_tools(
        &self,
        session_id: &str,
        tools: Option<&[String]>,
    ) -> Result<(), SessionError> {
        let json = tools.map(serde_json::to_string).transpose()?;
        self.set_field(session_id, |conn, now| {
            conn.execute(
                "UPDATE sessions SET enabled_tools_json = ?1, updated_at = ?2 WHERE id = ?3",
                params![json, now, session_id],
            )?;
            Ok(())
        })
    }

    /// 设置标题。
    pub fn set_title(&self, session_id: &str, title: impl AsRef<str>) -> Result<(), SessionError> {
        let title = title.as_ref();
        self.set_field(session_id, |conn, now| {
            conn.execute(
                "UPDATE sessions SET title = ?1, updated_at = ?2 WHERE id = ?3",
                params![title, now, session_id],
            )?;
            Ok(())
        })
    }

    /// 设置会话使用的模型；`None` 表示回退到配置里的默认模型。
    ///
    /// 只存 id，不校验它是否真的存在——模型清单归 `lya-config`，会话层不认识它。
    /// 校验放在写入接口（HTTP 层）和取用处。
    pub fn set_model(&self, session_id: &str, model_id: Option<&str>) -> Result<(), SessionError> {
        self.set_field(session_id, |conn, now| {
            conn.execute(
                "UPDATE sessions SET model_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![model_id, now, session_id],
            )?;
            Ok(())
        })
    }

    /// 设置会话级人设覆盖；`None` 表示回退到全局人设。
    pub fn set_persona(&self, session_id: &str, persona: Option<&str>) -> Result<(), SessionError> {
        self.set_field(session_id, |conn, now| {
            conn.execute(
                "UPDATE sessions SET persona = ?1, updated_at = ?2 WHERE id = ?3",
                params![persona, now, session_id],
            )?;
            Ok(())
        })
    }

    // ── 消息树 ────────────────────────────────────────────────────

    /// 在当前 `active_leaf` 下追加一条消息，并把 leaf 指向新节点。
    ///
    /// 当前 leaf 是未决 HITL 时默认拒绝追加（返回 [`SessionError::PendingHitl`]），
    /// 避免绕过用户确认继续跑；写入 HITL 应答时把 `allow_while_hitl` 置 true。
    pub fn append(
        &self,
        session_id: &str,
        payload: MessagePayload,
        allow_while_hitl: bool,
    ) -> Result<MessageRecord, SessionError> {
        let json = serde_json::to_string(&payload)?;

        self.db.write(|conn| {
            let meta = load_session(conn, session_id)?
                .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

            if !allow_while_hitl && let Some(leaf_id) = meta.active_leaf_id {
                let leaf = load_message(conn, session_id, leaf_id)?;
                if leaf.payload.is_pending_hitl() {
                    return Err(SessionError::PendingHitl(leaf_id));
                }
            }

            // sort_key 只保证会话内单调递增，用来给整棵树一个稳定的时间序。
            let sort_key: i64 = conn.query_row(
                "SELECT COALESCE(MAX(sort_key), -1) + 1 FROM messages WHERE session_id = ?1",
                [session_id],
                |row| row.get(0),
            )?;
            let parent_id = meta.active_leaf_id;
            let now = Utc::now();

            conn.execute(
                "INSERT INTO messages (session_id, parent_id, sort_key, message_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![session_id, parent_id, sort_key, json, now.to_rfc3339()],
            )?;
            let id = conn.last_insert_rowid();

            conn.execute(
                "UPDATE sessions SET active_leaf_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![id, now.to_rfc3339(), session_id],
            )?;

            Ok(MessageRecord {
                id,
                session_id: session_id.to_string(),
                parent_id,
                sort_key,
                payload,
                created_at: now,
            })
        })
    }

    /// 切换当前分支到某个叶节点。
    ///
    /// 目标必须是叶（没有子节点）；想从中间节点开新分支请用 [`SessionStore::fork_at`]。
    pub fn switch_leaf(&self, session_id: &str, leaf_msg_id: i64) -> Result<(), SessionError> {
        self.db.write(|conn| {
            ensure_session(conn, session_id)?;
            load_message(conn, session_id, leaf_msg_id)?;
            if has_children(conn, leaf_msg_id)? {
                return Err(SessionError::Invalid(format!(
                    "message {leaf_msg_id} is not a leaf; use fork_at to branch from it"
                )));
            }
            conn.execute(
                "UPDATE sessions SET active_leaf_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![leaf_msg_id, Utc::now().to_rfc3339(), session_id],
            )?;
            Ok(())
        })
    }

    /// 把 `active_leaf` 指到任意节点（`None` 表示回到空根），之后 `append` 即从此分叉。
    ///
    /// 「编辑并重发」= `fork_at(父节点)` 再 `append(新内容)`：旧分支原样保留。
    pub fn fork_at(
        &self,
        session_id: &str,
        parent_msg_id: Option<i64>,
    ) -> Result<(), SessionError> {
        self.db.write(|conn| {
            ensure_session(conn, session_id)?;
            if let Some(pid) = parent_msg_id {
                load_message(conn, session_id, pid)?;
            }
            conn.execute(
                "UPDATE sessions SET active_leaf_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![parent_msg_id, Utc::now().to_rfc3339(), session_id],
            )?;
            Ok(())
        })
    }

    /// 删除一个叶节点；若它正是当前 leaf，指针回退到父节点。
    ///
    /// 只允许删叶子，否则会把子树变成孤儿。
    pub fn delete_leaf(&self, session_id: &str, msg_id: i64) -> Result<(), SessionError> {
        self.db.write(|conn| {
            let meta = load_session(conn, session_id)?
                .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;
            let msg = load_message(conn, session_id, msg_id)?;
            if has_children(conn, msg_id)? {
                return Err(SessionError::NotLeaf(msg_id));
            }

            conn.execute("DELETE FROM messages WHERE id = ?1", [msg_id])?;
            if meta.active_leaf_id == Some(msg_id) {
                conn.execute(
                    "UPDATE sessions SET active_leaf_id = ?1, updated_at = ?2 WHERE id = ?3",
                    params![msg.parent_id, Utc::now().to_rfc3339(), session_id],
                )?;
            } else {
                touch_session(conn, session_id)?;
            }
            Ok(())
        })
    }

    /// 当前分支从根到 `active_leaf` 的完整路径（时间正序）。
    ///
    /// 这就是要喂给 LLM 的对话历史；空会话返回空 `Vec`。
    pub fn path_to_active_leaf(
        &self,
        session_id: &str,
    ) -> Result<Vec<MessageRecord>, SessionError> {
        self.db.read(|conn| {
            let meta = load_session(conn, session_id)?
                .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;
            match meta.active_leaf_id {
                Some(leaf) => walk_path(conn, session_id, leaf),
                None => Ok(Vec::new()),
            }
        })
    }

    /// 指定叶节点的根到叶路径（时间正序），用于预览其它分支。
    pub fn path_to_leaf(
        &self,
        session_id: &str,
        leaf_msg_id: i64,
    ) -> Result<Vec<MessageRecord>, SessionError> {
        self.db
            .read(|conn| walk_path(conn, session_id, leaf_msg_id))
    }

    /// 会话内所有叶节点 id（按 `sort_key` 正序），即所有可切换的分支端点。
    pub fn list_leaves(&self, session_id: &str) -> Result<Vec<i64>, SessionError> {
        self.db.read(|conn| {
            ensure_session(conn, session_id)?;
            let mut stmt = conn.prepare(
                "SELECT m.id FROM messages m
                 WHERE m.session_id = ?1
                   AND NOT EXISTS (SELECT 1 FROM messages c WHERE c.parent_id = m.id)
                 ORDER BY m.sort_key ASC",
            )?;
            let ids = stmt
                .query_map([session_id], |row| row.get(0))?
                .collect::<Result<Vec<i64>, _>>()?;
            Ok(ids)
        })
    }

    /// 把残留的「流式中」消息标成中断，返回改了几条。
    ///
    /// 进程崩在生成中途会留下 `status=Streaming` 的记录。启动时不扫一遍的话，
    /// 界面会把它渲染成一条永远转圈的消息，而它其实早就不会再更新了。
    pub fn mark_stale_streaming(&self) -> Result<usize, SessionError> {
        self.db.write(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, parent_id, sort_key, message_json, created_at
                 FROM messages WHERE message_json LIKE '%\"status\":\"streaming\"%'",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(4)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            drop(stmt);

            let mut changed = 0;
            for (id, _session, json) in rows {
                let mut payload: MessagePayload = serde_json::from_str(&json)?;
                if payload.status != MessageStatus::Streaming {
                    continue;
                }
                payload.status = MessageStatus::Interrupted;
                let updated = serde_json::to_string(&payload)?;
                conn.execute(
                    "UPDATE messages SET message_json = ?1 WHERE id = ?2",
                    params![updated, id],
                )?;
                changed += 1;
            }
            Ok(changed)
        })
    }

    /// 会话内**全部**消息，按 `sort_key` 正序。
    ///
    /// 与 `path_to_active_leaf` 不同：那个只给当前分支，这个给整棵树，
    /// 用于画分叉图与逐节点回看（每条消息本身就带着思考、工具调用、耗时，
    /// 所以「调用追踪」不需要另建一套记录）。
    pub fn list_messages(&self, session_id: &str) -> Result<Vec<MessageRecord>, SessionError> {
        self.db.read(|conn| {
            ensure_session(conn, session_id)?;
            let mut stmt = conn.prepare(
                "SELECT id, parent_id, sort_key, message_json, created_at
                 FROM messages WHERE session_id = ?1 ORDER BY sort_key ASC",
            )?;
            let raws = stmt
                .query_map([session_id], RawMessage::from_row)?
                .collect::<Result<Vec<_>, _>>()?;
            raws.into_iter()
                .map(|raw| raw.into_record(session_id))
                .collect()
        })
    }

    /// 读取单条消息。
    pub fn get_message(
        &self,
        session_id: &str,
        msg_id: i64,
    ) -> Result<MessageRecord, SessionError> {
        self.db.read(|conn| load_message(conn, session_id, msg_id))
    }

    /// 整体替换某条消息的 payload，返回改写后的记录。
    ///
    /// 流式生成时用它把 `streaming` 的助手消息落成 `complete`。返回记录是为了
    /// 让调用方能把「消息变了」这件事原样推给订阅者，不必再回查一次。
    pub fn update_payload(
        &self,
        session_id: &str,
        msg_id: i64,
        payload: &MessagePayload,
    ) -> Result<MessageRecord, SessionError> {
        let json = serde_json::to_string(payload)?;
        self.db.write(|conn| {
            let existing = load_message(conn, session_id, msg_id)?;
            conn.execute(
                "UPDATE messages SET message_json = ?1 WHERE id = ?2",
                params![json, msg_id],
            )?;
            touch_session(conn, session_id)?;
            Ok(MessageRecord {
                payload: payload.clone(),
                ..existing
            })
        })
    }

    // ── HITL ─────────────────────────────────────────────────────

    /// 把某个 HITL 节点标记为已解决，并留下用户的原始答复。
    ///
    /// 用户的答复本身是另一个独立节点（`append(..., allow_while_hitl = true)`），
    /// 但那条只有渲染后的文本。`answer` 把结构化的原始答复存进 `lya.meta`，
    /// 界面回看这段对话时才能把当时勾选的选项**原样回显**，而不是从一段中文里
    /// 反解。传 `None` 表示没有可留档的答复（如模式切换只有是/否）。
    pub fn resolve_hitl(
        &self,
        session_id: &str,
        hitl_msg_id: i64,
        answer: Option<Value>,
    ) -> Result<(), SessionError> {
        self.db.write(|conn| {
            let mut msg = load_message(conn, session_id, hitl_msg_id)?;
            if msg.payload.role != MessageRole::Hitl {
                return Err(SessionError::Invalid(format!(
                    "message {hitl_msg_id} is not a hitl node"
                )));
            }
            msg.payload.status = MessageStatus::Resolved;
            if let Some(answer) = answer {
                let meta = msg
                    .payload
                    .lya
                    .meta
                    .get_or_insert_with(|| Value::Object(Default::default()));
                if let Some(object) = meta.as_object_mut() {
                    object.insert("answer".into(), answer);
                }
            }
            let json = serde_json::to_string(&msg.payload)?;
            conn.execute(
                "UPDATE messages SET message_json = ?1 WHERE id = ?2",
                params![json, hitl_msg_id],
            )?;
            touch_session(conn, session_id)?;
            Ok(())
        })
    }

    /// 当前 leaf 若是未决 HITL 则返回其 id。
    ///
    /// 进程重启后靠它恢复「正等用户回答」的状态。
    pub fn pending_hitl(&self, session_id: &str) -> Result<Option<i64>, SessionError> {
        self.db.read(|conn| {
            let meta = load_session(conn, session_id)?
                .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;
            let Some(leaf) = meta.active_leaf_id else {
                return Ok(None);
            };
            let msg = load_message(conn, session_id, leaf)?;
            Ok(msg.payload.is_pending_hitl().then_some(leaf))
        })
    }

    /// 会话字段更新的公共外壳：先确认会话存在，再执行具体 UPDATE。
    fn set_field(
        &self,
        session_id: &str,
        f: impl FnOnce(&Connection, String) -> Result<(), SessionError>,
    ) -> Result<(), SessionError> {
        self.db.write(|conn| {
            ensure_session(conn, session_id)?;
            f(conn, Utc::now().to_rfc3339())
        })
    }
}

// ── 行映射与内部工具 ─────────────────────────────────────────────

/// `sessions` 的原始行；先取出字符串再解析，避免在 rusqlite 回调里返回自定义错误。
struct RawSession {
    id: String,
    title: String,
    status: String,
    active_leaf_id: Option<i64>,
    work_mode: String,
    persona: Option<String>,
    model_id: Option<String>,
    enabled_tools_json: Option<String>,
    created_at: String,
    updated_at: String,
}

impl RawSession {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            title: row.get(1)?,
            status: row.get(2)?,
            active_leaf_id: row.get(3)?,
            work_mode: row.get(4)?,
            persona: row.get(5)?,
            model_id: row.get(6)?,
            enabled_tools_json: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        })
    }

    fn into_meta(self) -> Result<SessionMeta, SessionError> {
        Ok(SessionMeta {
            status: SessionStatus::parse(&self.status).ok_or_else(|| {
                SessionError::Invalid(format!("bad session status: {}", self.status))
            })?,
            work_mode: self
                .work_mode
                .parse()
                .map_err(|err: lya_mode::ModeParseError| SessionError::Invalid(err.to_string()))?,
            enabled_tools: self
                .enabled_tools_json
                .as_deref()
                .map(serde_json::from_str)
                .transpose()?,
            created_at: parse_time(&self.created_at)?,
            updated_at: parse_time(&self.updated_at)?,
            id: self.id,
            title: self.title,
            active_leaf_id: self.active_leaf_id,
            persona: self.persona,
            model_id: self.model_id,
        })
    }
}

/// `messages` 的原始行。
struct RawMessage {
    id: i64,
    parent_id: Option<i64>,
    sort_key: i64,
    message_json: String,
    created_at: String,
}

impl RawMessage {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            parent_id: row.get(1)?,
            sort_key: row.get(2)?,
            message_json: row.get(3)?,
            created_at: row.get(4)?,
        })
    }

    fn into_record(self, session_id: &str) -> Result<MessageRecord, SessionError> {
        Ok(MessageRecord {
            id: self.id,
            session_id: session_id.to_string(),
            parent_id: self.parent_id,
            sort_key: self.sort_key,
            payload: serde_json::from_str(&self.message_json)?,
            created_at: parse_time(&self.created_at)?,
        })
    }
}

/// 会话不存在时直接报错。
fn ensure_session(conn: &Connection, session_id: &str) -> Result<(), SessionError> {
    let exists = conn
        .query_row("SELECT 1 FROM sessions WHERE id = ?1", [session_id], |_| {
            Ok(())
        })
        .optional()?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(SessionError::NotFound(session_id.to_string()))
    }
}

/// 刷新 `updated_at`，让会话列表的排序反映最近活动。
fn touch_session(conn: &Connection, session_id: &str) -> Result<(), SessionError> {
    conn.execute(
        "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), session_id],
    )?;
    Ok(())
}

fn load_session(conn: &Connection, session_id: &str) -> Result<Option<SessionMeta>, SessionError> {
    let raw = conn
        .query_row(
            "SELECT id, title, status, active_leaf_id, work_mode, persona, model_id,
                    enabled_tools_json, created_at, updated_at
             FROM sessions WHERE id = ?1",
            [session_id],
            RawSession::from_row,
        )
        .optional()?;
    raw.map(RawSession::into_meta).transpose()
}

/// 读取消息，并顺带校验它确实属于该会话。
fn load_message(
    conn: &Connection,
    session_id: &str,
    msg_id: i64,
) -> Result<MessageRecord, SessionError> {
    let raw = conn
        .query_row(
            "SELECT id, parent_id, sort_key, message_json, created_at
             FROM messages WHERE id = ?1 AND session_id = ?2",
            params![msg_id, session_id],
            RawMessage::from_row,
        )
        .optional()?
        .ok_or(SessionError::MessageNotFound(msg_id))?;
    raw.into_record(session_id)
}

fn has_children(conn: &Connection, msg_id: i64) -> Result<bool, SessionError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE parent_id = ?1",
        [msg_id],
        |row| row.get(0),
    )?;
    Ok(n > 0)
}

/// 沿 `parent_id` 从叶回溯到根，再按 `sort_key` 正序输出。
fn walk_path(
    conn: &Connection,
    session_id: &str,
    leaf_msg_id: i64,
) -> Result<Vec<MessageRecord>, SessionError> {
    load_message(conn, session_id, leaf_msg_id)?;

    let mut stmt = conn.prepare(
        "WITH RECURSIVE path(id, parent_id, sort_key, message_json, created_at) AS (
             SELECT id, parent_id, sort_key, message_json, created_at
             FROM messages WHERE id = ?1
             UNION ALL
             SELECT m.id, m.parent_id, m.sort_key, m.message_json, m.created_at
             FROM messages m JOIN path p ON m.id = p.parent_id
         )
         SELECT id, parent_id, sort_key, message_json, created_at
         FROM path ORDER BY sort_key ASC",
    )?;
    let raws = stmt
        .query_map([leaf_msg_id], RawMessage::from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    raws.into_iter()
        .map(|raw| raw.into_record(session_id))
        .collect()
}

fn parse_time(s: &str) -> Result<DateTime<Utc>, SessionError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| SessionError::Invalid(format!("bad timestamp {s}: {err}")))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::message::{HitlBlock, MessageKind};

    fn store() -> (TempDir, SessionStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::open(dir.path().join("lya.db")).unwrap();
        (dir, store)
    }

    fn new_session(store: &SessionStore) -> String {
        store
            .create_session(CreateSession {
                title: "t".into(),
                work_mode: Mode::Agent,
                enabled_tools: Some(vec!["file_read".into()]),
                ..Default::default()
            })
            .unwrap()
            .id
    }

    #[test]
    fn session_roundtrip() {
        let (_dir, store) = store();
        let id = new_session(&store);

        let meta = store.get_session(&id).unwrap().unwrap();
        assert_eq!(meta.work_mode, Mode::Agent);
        assert_eq!(meta.enabled_tools, Some(vec!["file_read".to_string()]));
        assert_eq!(meta.active_leaf_id, None);

        store.set_work_mode(&id, Mode::Ask).unwrap();
        store
            .set_enabled_tools(&id, Some(&["file_read".to_string()]))
            .unwrap();
        store.set_title(&id, "renamed").unwrap();
        store.set_persona(&id, Some("小恋恋")).unwrap();

        let meta = store.get_session(&id).unwrap().unwrap();
        assert_eq!(meta.work_mode, Mode::Ask);
        assert_eq!(meta.title, "renamed");
        assert_eq!(meta.persona.as_deref(), Some("小恋恋"));

        assert_eq!(store.list_sessions().unwrap().len(), 1);
        store.archive_session(&id).unwrap();
        assert!(store.list_sessions().unwrap().is_empty());
    }

    #[test]
    fn missing_session_is_reported() {
        let (_dir, store) = store();
        assert!(store.get_session("nope").unwrap().is_none());
        assert!(matches!(
            store.set_title("nope", "x"),
            Err(SessionError::NotFound(_))
        ));
    }

    #[test]
    fn append_builds_linear_path() {
        let (_dir, store) = store();
        let id = new_session(&store);

        let u1 = store
            .append(&id, MessagePayload::user_text("hi"), false)
            .unwrap();
        let a1 = store
            .append(
                &id,
                MessagePayload::assistant_text("yo", MessageStatus::Complete),
                false,
            )
            .unwrap();

        assert_eq!(u1.parent_id, None);
        assert_eq!(a1.parent_id, Some(u1.id));

        let path = store.path_to_active_leaf(&id).unwrap();
        assert_eq!(
            path.iter().map(|m| m.id).collect::<Vec<_>>(),
            vec![u1.id, a1.id]
        );
    }

    #[test]
    fn fork_creates_sibling_branch() {
        let (_dir, store) = store();
        let id = new_session(&store);

        let u1 = store
            .append(&id, MessagePayload::user_text("hi"), false)
            .unwrap();
        let a1 = store
            .append(
                &id,
                MessagePayload::assistant_text("first", MessageStatus::Complete),
                false,
            )
            .unwrap();

        store.fork_at(&id, Some(u1.id)).unwrap();
        let a2 = store
            .append(
                &id,
                MessagePayload::assistant_text("second", MessageStatus::Complete),
                false,
            )
            .unwrap();
        assert_eq!(a2.parent_id, Some(u1.id));

        let mut leaves = store.list_leaves(&id).unwrap();
        leaves.sort_unstable();
        let mut expected = vec![a1.id, a2.id];
        expected.sort_unstable();
        assert_eq!(leaves, expected);

        // 新分支不包含旧分支的助手回复
        let path = store.path_to_active_leaf(&id).unwrap();
        assert_eq!(
            path.iter().map(|m| m.id).collect::<Vec<_>>(),
            vec![u1.id, a2.id]
        );

        store.switch_leaf(&id, a1.id).unwrap();
        assert_eq!(
            store.get_session(&id).unwrap().unwrap().active_leaf_id,
            Some(a1.id)
        );
        // 中间节点不能当作叶来切换
        assert!(matches!(
            store.switch_leaf(&id, u1.id),
            Err(SessionError::Invalid(_))
        ));
    }

    #[test]
    fn delete_leaf_rewinds_pointer() {
        let (_dir, store) = store();
        let id = new_session(&store);

        let u1 = store
            .append(&id, MessagePayload::user_text("hi"), false)
            .unwrap();
        let a1 = store
            .append(
                &id,
                MessagePayload::assistant_text("yo", MessageStatus::Complete),
                false,
            )
            .unwrap();

        assert!(matches!(
            store.delete_leaf(&id, u1.id),
            Err(SessionError::NotLeaf(_))
        ));

        store.delete_leaf(&id, a1.id).unwrap();
        assert_eq!(
            store.get_session(&id).unwrap().unwrap().active_leaf_id,
            Some(u1.id)
        );
        assert!(matches!(
            store.get_message(&id, a1.id),
            Err(SessionError::MessageNotFound(_))
        ));
    }

    #[test]
    fn update_payload_finalizes_streaming_message() {
        let (_dir, store) = store();
        let id = new_session(&store);

        store
            .append(&id, MessagePayload::user_text("hi"), false)
            .unwrap();
        let draft = store
            .append(
                &id,
                MessagePayload::assistant_text("", MessageStatus::Streaming),
                false,
            )
            .unwrap();

        let final_payload = MessagePayload::assistant_text("done", MessageStatus::Complete);
        store.update_payload(&id, draft.id, &final_payload).unwrap();

        let stored = store.get_message(&id, draft.id).unwrap();
        assert_eq!(stored.payload.status, MessageStatus::Complete);
        assert_eq!(stored.payload.openai.unwrap().content, "done");
    }

    #[test]
    fn pending_hitl_blocks_plain_append() {
        let (_dir, store) = store();
        let id = new_session(&store);

        store
            .append(&id, MessagePayload::user_text("删这个文件"), false)
            .unwrap();
        let hitl = store
            .append(
                &id,
                MessagePayload::hitl_pending(
                    MessageKind::ToolConfirm,
                    HitlBlock::ToolConfirm {
                        tool_call_id: "call_1".into(),
                        tool_name: "bash".into(),
                        arguments: serde_json::json!({ "command": "rm a.txt" }),
                        summary: "执行：rm a.txt".into(),
                        steps: Vec::new(),
                        reasons: vec!["会删文件".into()],
                    },
                ),
                false,
            )
            .unwrap();

        assert_eq!(store.pending_hitl(&id).unwrap(), Some(hitl.id));
        assert!(matches!(
            store.append(&id, MessagePayload::user_text("继续"), false),
            Err(SessionError::PendingHitl(_))
        ));

        // 答复节点可以强制写入，随后 HITL 结清、追加恢复正常
        store
            .append(&id, MessagePayload::user_text("同意"), true)
            .unwrap();
        store.resolve_hitl(&id, hitl.id, None).unwrap();
        assert_eq!(store.pending_hitl(&id).unwrap(), None);
        assert_eq!(
            store.get_message(&id, hitl.id).unwrap().payload.status,
            MessageStatus::Resolved
        );
        store
            .append(&id, MessagePayload::user_text("继续"), false)
            .unwrap();
    }

    #[test]
    fn stale_streaming_messages_are_marked_interrupted() {
        let (_dir, store) = store();
        let id = new_session(&store);
        store
            .append(&id, MessagePayload::user_text("你好"), false)
            .unwrap();
        let draft = store
            .append(
                &id,
                MessagePayload::assistant_text("说到一半", MessageStatus::Streaming),
                false,
            )
            .unwrap();

        assert_eq!(store.mark_stale_streaming().unwrap(), 1);
        assert_eq!(
            store.get_message(&id, draft.id).unwrap().payload.status,
            MessageStatus::Interrupted,
            "崩溃留下的残留不清理，界面会渲染成一条永远转圈的消息"
        );
        // 已经清过的不会重复计数
        assert_eq!(store.mark_stale_streaming().unwrap(), 0);
    }

    #[test]
    fn resolving_hitl_can_archive_the_raw_answer() {
        let (_dir, store) = store();
        let id = new_session(&store);
        let hitl = store
            .append(
                &id,
                MessagePayload::hitl_pending(
                    MessageKind::Form,
                    HitlBlock::Form {
                        form_id: "f".into(),
                        title: "t".into(),
                        questions: Vec::new(),
                    },
                ),
                false,
            )
            .unwrap();

        store
            .resolve_hitl(
                &id,
                hitl.id,
                Some(serde_json::json!({ "items": [{ "question_id": "q", "values": ["a"] }] })),
            )
            .unwrap();

        let record = store.get_message(&id, hitl.id).unwrap();
        assert_eq!(record.payload.status, MessageStatus::Resolved);
        // 界面回看时要能原样回显当时勾了什么，而不是从渲染后的中文里反解
        let answer = &record.payload.lya.meta.unwrap()["answer"];
        assert_eq!(answer["items"][0]["values"][0], "a");
    }

    #[test]
    fn resolve_hitl_rejects_non_hitl_node() {
        let (_dir, store) = store();
        let id = new_session(&store);
        let u1 = store
            .append(&id, MessagePayload::user_text("hi"), false)
            .unwrap();
        assert!(matches!(
            store.resolve_hitl(&id, u1.id, None),
            Err(SessionError::Invalid(_))
        ));
    }
}
