use chrono::Utc;
use rusqlite::params;

use crate::error::SessionError;
use crate::message::{MessagePayload, MessageStatus};
use crate::types::{MessageRecord, SessionStatus};

use super::SessionStore;
use super::helpers::*;

impl SessionStore {
    // ── 消息树 ────────────────────────────────────────────────────

    /// 在当前 `active_leaf` 下追加一条消息，并把 leaf 指向新节点。
    ///
    /// 当前 leaf 是未决 HITL 时默认拒绝追加（返回 [`SessionError::PendingHitl`]），
    /// 避免绕过用户确认继续跑；写入 HITL 应答时把 `allow_while_hitl` 置 true。
    ///
    /// 已归档的会话一律拒绝。**只读必须在这里保证**——界面藏掉输入框只能挡住
    /// 走界面的人，绕过去直接调接口照样能写。所有新内容都从这个口进来，
    /// 守住这一个就够了。
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

            if meta.status == SessionStatus::Archived {
                return Err(SessionError::Archived(session_id.to_string()));
            }

            if !allow_while_hitl && let Some(leaf_id) = meta.active_leaf_id {
                let path = walk_path(conn, session_id, leaf_id)?;
                if let Some(hitl_id) = find_pending_hitl_in_path(&path) {
                    return Err(SessionError::PendingHitl(hitl_id));
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
    ///
    /// 归档会话也允许切换：支线同样是这段对话的一部分，挡住就等于把归档里的一半
    /// 内容变成看不到的。但这只是挪一下回看位置，不该让它显示成「刚更新过」，
    /// 所以归档时保留原本的 `updated_at`。
    pub fn switch_leaf(&self, session_id: &str, leaf_msg_id: i64) -> Result<(), SessionError> {
        self.db.write(|conn| {
            let meta = load_session(conn, session_id)?
                .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;
            load_message(conn, session_id, leaf_msg_id)?;
            if has_children(conn, leaf_msg_id)? {
                return Err(SessionError::Invalid(format!(
                    "message {leaf_msg_id} is not a leaf; use fork_at to branch from it"
                )));
            }
            let updated_at = if meta.status == SessionStatus::Archived {
                meta.updated_at
            } else {
                Utc::now()
            };
            conn.execute(
                "UPDATE sessions SET active_leaf_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![leaf_msg_id, updated_at.to_rfc3339(), session_id],
            )?;
            Ok(())
        })
    }

    /// 把 `active_leaf` 指到任意节点（`None` 表示回到空根），之后 `append` 即从此分叉。
    ///
    /// 「编辑并重发」= `fork_at(父节点)` 再 `append(新内容)`：旧分支原样保留。
    ///
    /// 已归档的会话一律拒绝。它虽然只挪指针，却总是「分叉后再写」的前半步：
    /// 后半步撞上只读失败时，指针已经退回到父节点了，那段对话从此显示成截断的
    /// ——内容还在库里，界面上却再也走不回去。要挡就得挡在这里。
    /// 想在归档里换分支看请用 [`SessionStore::switch_leaf`]，那个是纯回看。
    pub fn fork_at(
        &self,
        session_id: &str,
        parent_msg_id: Option<i64>,
    ) -> Result<(), SessionError> {
        self.db.write(|conn| {
            let meta = load_session(conn, session_id)?
                .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;
            if meta.status == SessionStatus::Archived {
                return Err(SessionError::Archived(session_id.to_string()));
            }
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
    ///
    /// 已归档的会话一律拒绝。归档承诺的是「只能回看」，而这个口会真的抹掉内容——
    /// 界面藏掉删除按钮只挡得住走界面的人，守在这里才算数。
    pub fn delete_leaf(&self, session_id: &str, msg_id: i64) -> Result<(), SessionError> {
        self.db.write(|conn| {
            let meta = load_session(conn, session_id)?
                .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;
            if meta.status == SessionStatus::Archived {
                return Err(SessionError::Archived(session_id.to_string()));
            }
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

}
