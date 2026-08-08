use tempfile::TempDir;

use super::SessionStore;
use crate::error::SessionError;
use crate::message::{HitlBlock, MessageKind, MessagePayload, MessageStatus};
use crate::types::{CreateSession, SessionStatus};
use lya_base::Mode;

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
    fn archived_sessions_reject_writes() {
        let (_dir, store) = store();
        let id = new_session(&store);
        store
            .append(&id, MessagePayload::user_text("归档前"), false)
            .unwrap();

        store.archive_session(&id).unwrap();

        // 只读要在这里保证：界面藏掉输入框只挡得住走界面的人，
        // 绕过去直接调接口照样能写
        let err = store
            .append(&id, MessagePayload::user_text("归档后"), false)
            .unwrap_err();
        assert!(matches!(err, SessionError::Archived(_)));

        // 删除同理——它会真的抹掉内容，只靠界面藏按钮不算数
        let leaf = store.list_leaves(&id).unwrap()[0];
        let err = store.delete_leaf(&id, leaf).unwrap_err();
        assert!(matches!(err, SessionError::Archived(_)));
        assert_eq!(store.list_messages(&id).unwrap().len(), 1);

        // 分叉是「改写后重发」的前半步。放它过去的话，后半步撞上只读失败时
        // 指针已经退回去了，这段归档从此显示成截断的
        let err = store.fork_at(&id, None).unwrap_err();
        assert!(matches!(err, SessionError::Archived(_)));
        assert_eq!(store.get_session(&id).unwrap().unwrap().active_leaf_id, Some(leaf));

        // 但回看不受影响
        assert_eq!(store.path_to_active_leaf(&id).unwrap().len(), 1);
        assert_eq!(store.get_session(&id).unwrap().unwrap().status, SessionStatus::Archived);
    }

    #[test]
    fn archived_sessions_can_still_switch_branches() {
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
        store
            .append(
                &id,
                MessagePayload::assistant_text("second", MessageStatus::Complete),
                false,
            )
            .unwrap();

        store.archive_session(&id).unwrap();
        let before = store.get_session(&id).unwrap().unwrap().updated_at;

        // 支线也是这段对话的一部分，挡住切换等于把归档里的一半内容变成看不到的
        store.switch_leaf(&id, a1.id).unwrap();
        let meta = store.get_session(&id).unwrap().unwrap();
        assert_eq!(meta.active_leaf_id, Some(a1.id));
        assert_eq!(
            store
                .path_to_active_leaf(&id)
                .unwrap()
                .iter()
                .map(|m| m.id)
                .collect::<Vec<_>>(),
            vec![u1.id, a1.id]
        );

        // 只是挪了回看位置，不该让归档显示成「刚更新过」而窜到列表最前面
        assert_eq!(meta.updated_at, before);
    }

    #[test]
    fn archiving_is_reversible() {
        let (_dir, store) = store();
        let id = new_session(&store);

        store.archive_session(&id).unwrap();
        assert!(store.list_sessions().unwrap().is_empty());
        assert_eq!(store.list_archived().unwrap().len(), 1);

        // 取不回来的话，误点一下会话就凭空消失——比删除还糟，
        // 因为删除至少你知道它没了
        store.unarchive_session(&id).unwrap();
        assert_eq!(store.list_sessions().unwrap().len(), 1);
        assert!(store.list_archived().unwrap().is_empty());
        store
            .append(&id, MessagePayload::user_text("又能说话了"), false)
            .unwrap();
    }

    #[test]
    fn delete_removes_session_and_its_messages() {
        let (_dir, store) = store();
        let id = new_session(&store);
        let msg = store
            .append(&id, MessagePayload::user_text("待删"), false)
            .unwrap();

        // 再追两条，构成一条父子链。messages.parent_id 上挂的是 ON DELETE
        // RESTRICT，删父节点时若还有子节点引用它就会被拦——真删整个会话时
        // 必须确认这条链能一起清掉，否则会话删了、消息还在库里够不着
        store
            .append(&id, MessagePayload::assistant_text("回", MessageStatus::Complete), false)
            .unwrap();
        store
            .append(&id, MessagePayload::user_text("再问"), false)
            .unwrap();

        store.delete_session(&id).unwrap();

        assert!(store.get_session(&id).unwrap().is_none());
        // 消息靠外键级联清掉，别在库里留一堆够不着的行
        assert!(matches!(
            store.get_message(&id, msg.id),
            Err(SessionError::NotFound(_) | SessionError::MessageNotFound(_))
        ));
        assert!(matches!(
            store.delete_session(&id),
            Err(SessionError::NotFound(_))
        ));
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
    fn pending_hitl_finds_hitl_before_trailing_tool_result() {
        let (_dir, store) = store();
        let id = new_session(&store);

        store
            .append(&id, MessagePayload::user_text("跑两个工具"), false)
            .unwrap();
        let hitl = store
            .append(
                &id,
                MessagePayload::hitl_pending(
                    MessageKind::ToolConfirm,
                    HitlBlock::ToolConfirm {
                        tool_call_id: "call_bash".into(),
                        tool_name: "bash".into(),
                        arguments: serde_json::json!({ "command": "echo hi" }),
                        summary: "执行：echo hi".into(),
                        steps: Vec::new(),
                        reasons: Vec::new(),
                    },
                ),
                false,
            )
            .unwrap();
        // 同批里另一个 tool 的结果可能挂在 HITL 后面（allow_while_hitl）
        store
            .append(
                &id,
                MessagePayload::tool_result("call_list", "[]"),
                true,
            )
            .unwrap();

        assert_eq!(store.pending_hitl(&id).unwrap(), Some(hitl.id));
        assert!(matches!(
            store.append(&id, MessagePayload::user_text("别插队"), false),
            Err(SessionError::PendingHitl(pending)) if pending == hitl.id
        ));
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
