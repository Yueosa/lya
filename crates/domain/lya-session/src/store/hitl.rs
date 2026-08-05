use serde_json::Value;
use rusqlite::params;

use crate::error::SessionError;
use crate::message::{MessageRole, MessageStatus};

use super::SessionStore;
use super::helpers::*;

impl SessionStore {
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

    /// 当前分支上若有未决 HITL 则返回其 id。
    ///
    /// 同批工具里先挂起确认、后面又追加了其它 tool 结果时，leaf 可能已经不是
    /// HITL 节点本身——要从 active 路径上往回找，不能只看 leaf。
    /// 进程重启后靠它恢复「正等用户回答」的状态。
    pub fn pending_hitl(&self, session_id: &str) -> Result<Option<i64>, SessionError> {
        self.db.read(|conn| {
            let meta = load_session(conn, session_id)?
                .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;
            let Some(leaf) = meta.active_leaf_id else {
                return Ok(None);
            };
            let path = walk_path(conn, session_id, leaf)?;
            Ok(find_pending_hitl_in_path(&path))
        })
    }

    /// 路径上全部未决 HITL id（按出现顺序）。
    pub fn pending_hitl_all(&self, session_id: &str) -> Result<Vec<i64>, SessionError> {
        self.db.read(|conn| {
            let meta = load_session(conn, session_id)?
                .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;
            let Some(leaf) = meta.active_leaf_id else {
                return Ok(Vec::new());
            };
            let path = walk_path(conn, session_id, leaf)?;
            Ok(find_all_pending_hitl_in_path(&path))
        })
    }
}
