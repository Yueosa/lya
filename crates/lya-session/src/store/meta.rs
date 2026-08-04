use chrono::Utc;
use lya_mode::Mode;
use rusqlite::params;
use uuid::Uuid;

use crate::error::SessionError;
use crate::types::{CreateSession, SessionMeta, SessionStatus};

use super::SessionStore;
use super::helpers::*;

impl SessionStore {
    // ── 会话元数据 ────────────────────────────────────────────────

    /// 创建会话，返回新会话的元数据快照。
    pub fn create_session(&self, req: CreateSession) -> Result<SessionMeta, SessionError> {
        let api_mode = req
            .api_mode
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "completions".into());
        let meta = SessionMeta {
            id: Uuid::new_v4().to_string(),
            title: req.title,
            status: SessionStatus::Active,
            active_leaf_id: None,
            work_mode: req.work_mode,
            persona: req.persona,
            model_id: req.model_id,
            api_mode,
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
                     api_mode, enabled_tools_json, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    meta.id,
                    meta.title,
                    meta.status.as_str(),
                    meta.work_mode.as_str(),
                    meta.persona,
                    meta.model_id,
                    meta.api_mode,
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
        self.list_by_status(SessionStatus::Active)
    }

    fn list_by_status(&self, status: SessionStatus) -> Result<Vec<SessionMeta>, SessionError> {
        self.db.read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, title, status, active_leaf_id, work_mode, persona, model_id,
                        api_mode, enabled_tools_json, created_at, updated_at
                 FROM sessions
                 WHERE status = ?1
                 ORDER BY updated_at DESC",
            )?;
            let rows = stmt
                .query_map([status.as_str()], RawSession::from_row)?
                .collect::<Result<Vec<_>, _>>()?;
            rows.into_iter().map(RawSession::into_meta).collect()
        })
    }

    /// 归档会话（不删除消息）。
    pub fn archive_session(&self, session_id: &str) -> Result<(), SessionError> {
        self.set_status(session_id, SessionStatus::Archived)
    }

    /// 取消归档，让会话回到可继续对话的状态。
    pub fn unarchive_session(&self, session_id: &str) -> Result<(), SessionError> {
        self.set_status(session_id, SessionStatus::Active)
    }

    fn set_status(&self, session_id: &str, status: SessionStatus) -> Result<(), SessionError> {
        let status = status.as_str();
        self.set_field(session_id, move |conn, now| {
            conn.execute(
                "UPDATE sessions SET status = ?1, updated_at = ?2 WHERE id = ?3",
                params![status, now, session_id],
            )?;
            Ok(())
        })
    }

    /// 真删：会话连同它的消息一起从库里去掉，不可恢复。
    ///
    /// 和归档是两回事——归档只是收起来、仍可回看，删除是真没了。消息靠外键
    /// 级联清理，所以这里只删会话行。
    pub fn delete_session(&self, session_id: &str) -> Result<(), SessionError> {
        self.db.write(|conn| {
            ensure_session(conn, session_id)?;

            // 光靠 sessions → messages 的级联删不掉：messages.parent_id 上挂着
            // ON DELETE RESTRICT，删到一个还有子节点的消息就会被拦，于是只有
            // 「只有一条消息」的会话删得动。
            //
            // 想靠「先删叶子再往上」绕过去也不行——一条 DELETE 语句里的删除顺序
            // 不受 ORDER BY 控制。正解是把外键检查推迟到提交时统一做：中间态
            // 允许暂时不一致，只要提交那一刻整棵树都没了就行。这个 pragma 只在
            // 当前事务内有效。
            conn.execute_batch("PRAGMA defer_foreign_keys = ON;")?;
            conn.execute("DELETE FROM messages WHERE session_id = ?1", [session_id])?;
            conn.execute("DELETE FROM sessions WHERE id = ?1", [session_id])?;
            Ok(())
        })
    }

    /// 已归档的会话。
    pub fn list_archived(&self) -> Result<Vec<SessionMeta>, SessionError> {
        self.list_by_status(SessionStatus::Archived)
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

    /// 会话是否还没有任何消息（此时允许改 API 栈）。
    pub fn session_is_empty(&self, session_id: &str) -> Result<bool, SessionError> {
        self.db.read(|conn| {
            ensure_session(conn, session_id)?;
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id = ?1",
                [session_id],
                |row| row.get(0),
            )?;
            Ok(count == 0)
        })
    }

    /// 设置 API 栈（仅空会话；校验在 HTTP 层）。
    pub fn set_api_mode(&self, session_id: &str, api_mode: &str) -> Result<(), SessionError> {
        self.set_field(session_id, |conn, now| {
            conn.execute(
                "UPDATE sessions SET api_mode = ?1, updated_at = ?2 WHERE id = ?3",
                params![api_mode, now, session_id],
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

    /// 会话字段更新的公共外壳：先确认会话存在，再执行具体 UPDATE。
    fn set_field(
        &self,
        session_id: &str,
        f: impl FnOnce(&rusqlite::Connection, String) -> Result<(), SessionError>,
    ) -> Result<(), SessionError> {
        self.db.write(|conn| {
            ensure_session(conn, session_id)?;
            f(conn, Utc::now().to_rfc3339())
        })
    }
}
