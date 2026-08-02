use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use crate::error::SessionError;
use crate::types::{MessageRecord, SessionMeta, SessionStatus};

pub(super) fn ensure_session(conn: &Connection, session_id: &str) -> Result<(), SessionError> {
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
pub(super) fn touch_session(conn: &Connection, session_id: &str) -> Result<(), SessionError> {
    conn.execute(
        "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), session_id],
    )?;
    Ok(())
}

pub(super) fn load_session(
    conn: &Connection,
    session_id: &str,
) -> Result<Option<SessionMeta>, SessionError> {
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
pub(super) fn load_message(
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

pub(super) fn has_children(conn: &Connection, msg_id: i64) -> Result<bool, SessionError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE parent_id = ?1",
        [msg_id],
        |row| row.get(0),
    )?;
    Ok(n > 0)
}

/// 从根往叶找第一条未决 HITL（同批按 call 顺序审；后面可能挂了 tool 结果）。
pub(super) fn find_pending_hitl_in_path(path: &[MessageRecord]) -> Option<i64> {
    path.iter()
        .find(|msg| msg.payload.is_pending_hitl())
        .map(|msg| msg.id)
}

/// 路径上所有未决 HITL，按出现顺序。
pub(super) fn find_all_pending_hitl_in_path(path: &[MessageRecord]) -> Vec<i64> {
    path.iter()
        .filter(|msg| msg.payload.is_pending_hitl())
        .map(|msg| msg.id)
        .collect()
}

/// 沿 `parent_id` 从叶回溯到根，再按 `sort_key` 正序输出。
pub(super) fn walk_path(
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

/// `sessions` 的原始行；先取出字符串再解析，避免在 rusqlite 回调里返回自定义错误。
pub(super) struct RawSession {
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
    pub(super) fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
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

    pub(super) fn into_meta(self) -> Result<SessionMeta, SessionError> {
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
pub(super) struct RawMessage {
    id: i64,
    parent_id: Option<i64>,
    sort_key: i64,
    message_json: String,
    created_at: String,
}

impl RawMessage {
    pub(super) fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            parent_id: row.get(1)?,
            sort_key: row.get(2)?,
            message_json: row.get(3)?,
            created_at: row.get(4)?,
        })
    }

    pub(super) fn into_record(self, session_id: &str) -> Result<MessageRecord, SessionError> {
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
