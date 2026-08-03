//! `lya-action` 的行为测试。
//!
//! 重点覆盖两件事：参数出错时是**回灌可读说明**而不是报 `Err`（模型要能自己
//! 改），以及需要人介入的动作只产出 HITL 意图、不碰会话。

use std::sync::Arc;

use lya_action::{
    Action, ActionCtx, ActionFlow, ActionOutcome, ActionRegistry, FormAction, FormAnswer,
    FormAnswerItem, MemoryReadAction, MemoryWriteAction, RequestModeChangeAction,
    register_builtins, render_form_answer,
};
use lya_memory::MemoryStore;
use lya_mode::Mode;
use lya_session::{FormQuestionKind, HitlBlock};
use serde_json::json;
use tempfile::TempDir;

fn memory() -> (TempDir, Arc<MemoryStore>) {
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::open(dir.path().join("lya.db")).unwrap();
    (dir, Arc::new(store))
}

fn ctx(mode: Mode) -> ActionCtx<'static> {
    ActionCtx::new("session-1", mode)
}

fn content(outcome: &ActionOutcome) -> &str {
    match outcome {
        ActionOutcome::Continue(result) => &result.content,
        ActionOutcome::AwaitHuman(_) => panic!("期望 Continue，实际要求人工介入"),
    }
}

fn is_err(outcome: &ActionOutcome) -> bool {
    matches!(outcome, ActionOutcome::Continue(result) if result.is_error())
}

// ── 记忆动作 ─────────────────────────────────────────────────

#[tokio::test]
async fn memory_write_then_read_roundtrip() {
    let (_dir, store) = memory();
    let write = MemoryWriteAction::new(Arc::clone(&store));
    let read = MemoryReadAction::new(Arc::clone(&store));

    let written = write
        .call(
            ctx(Mode::Agent),
            json!({
                "title": "Hyprland 崩溃",
                "body": "换 -git 包可绕过",
                "summary": "多显示器 DRM page-flip",
                "tags": ["hyprland", "drm"]
            }),
        )
        .await;
    assert!(!is_err(&written), "{}", content(&written));
    assert!(content(&written).contains("#2"));

    // 写入时自动记录来源会话
    let stored = store.get(2).unwrap();
    assert_eq!(stored.source_session_id.as_deref(), Some("session-1"));

    let got = read.call(ctx(Mode::Agent), json!({ "id": 2 })).await;
    let text = content(&got);
    assert!(text.contains("Hyprland 崩溃"));
    assert!(text.contains("换 -git 包可绕过"));
    assert!(text.contains("hyprland, drm"));
}

#[tokio::test]
async fn memory_write_same_title_updates_in_place() {
    let (_dir, store) = memory();
    let write = MemoryWriteAction::new(Arc::clone(&store));

    for body in ["第一版", "第二版"] {
        let outcome = write
            .call(ctx(Mode::Agent), json!({ "title": "同名", "body": body }))
            .await;
        assert!(!is_err(&outcome));
    }

    assert_eq!(store.count().unwrap(), 2, "置顶 + 一条用户记忆");
    let user = store.find_by_title("同名").unwrap().unwrap();
    assert_eq!(user.body, "第二版");
}

#[tokio::test]
async fn bad_args_are_fed_back_not_raised() {
    let (_dir, store) = memory();
    let write = MemoryWriteAction::new(Arc::clone(&store));
    let read = MemoryReadAction::new(store);

    let missing = write
        .call(ctx(Mode::Agent), json!({ "title": "只有标题" }))
        .await;
    assert!(is_err(&missing));
    assert!(content(&missing).contains("body"));

    let wrong_type = write
        .call(
            ctx(Mode::Agent),
            json!({ "title": "x", "body": "y", "tags": "不是数组" }),
        )
        .await;
    assert!(is_err(&wrong_type));

    let absent = read.call(ctx(Mode::Agent), json!({ "id": 999 })).await;
    assert!(is_err(&absent));
    assert!(content(&absent).contains("999"));
}

#[tokio::test]
async fn memory_read_accepts_stringified_id() {
    let (_dir, store) = memory();
    let write = MemoryWriteAction::new(Arc::clone(&store));
    let read = MemoryReadAction::new(store);
    write
        .call(ctx(Mode::Agent), json!({ "title": "t", "body": "b" }))
        .await;

    // 模型偶尔把数字写成字符串，能救则救
    let got = read.call(ctx(Mode::Agent), json!({ "id": "2" })).await;
    assert!(!is_err(&got));
}

// ── 表单 ─────────────────────────────────────────────────────

#[tokio::test]
async fn form_produces_hitl_block() {
    let form = FormAction::new();
    let outcome = form
        .call(
            ctx(Mode::Agent),
            json!({
                "form_id": "deploy",
                "title": "部署方式",
                "questions": [
                    {
                        "id": "svc", "text": "用哪种服务管理？", "kind": "single",
                        "options": [
                            { "key": "systemd", "label": "systemd --user" },
                            { "key": "execonce", "label": "exec-once" }
                        ],
                        "allow_note": true
                    },
                    { "id": "path", "text": "配置放在哪个目录？", "kind": "text" }
                ]
            }),
        )
        .await;

    let ActionOutcome::AwaitHuman(block) = outcome else {
        panic!("表单应当要求人工介入");
    };
    let HitlBlock::Form {
        form_id,
        title,
        questions,
    } = *block
    else {
        panic!("应当是 Form 块");
    };
    assert_eq!(form_id, "deploy");
    assert_eq!(title, "部署方式");
    assert_eq!(questions.len(), 2);
    assert_eq!(questions[0].kind, FormQuestionKind::Single);
    assert!(questions[0].allow_note);
    assert_eq!(questions[1].kind, FormQuestionKind::Text);
    assert!(questions[1].options.is_empty());
}

#[tokio::test]
async fn form_validation_rejects_mismatched_shapes() {
    let form = FormAction::new();

    let cases = vec![
        (
            "选项题没给选项",
            json!({ "form_id": "f", "title": "t", "questions": [
                { "id": "q", "text": "选一个", "kind": "single" }
            ]}),
        ),
        (
            "文本题却带了选项",
            json!({ "form_id": "f", "title": "t", "questions": [
                { "id": "q", "text": "填一个", "kind": "text",
                  "options": [{ "key": "a", "label": "A" }] }
            ]}),
        ),
        (
            "题目 id 重复",
            json!({ "form_id": "f", "title": "t", "questions": [
                { "id": "q", "text": "一", "kind": "text" },
                { "id": "q", "text": "二", "kind": "text" }
            ]}),
        ),
        (
            "未知题型",
            json!({ "form_id": "f", "title": "t", "questions": [
                { "id": "q", "text": "?", "kind": "ranking" }
            ]}),
        ),
        (
            "空表单",
            json!({ "form_id": "f", "title": "t", "questions": [] }),
        ),
    ];

    for (label, args) in cases {
        let outcome = form.call(ctx(Mode::Agent), args).await;
        assert!(is_err(&outcome), "{label} 应当被拒绝");
    }
}

#[tokio::test]
async fn form_rejects_oversized_form() {
    let form = FormAction::new();
    let questions: Vec<_> = (0..11)
        .map(|i| json!({ "id": format!("q{i}"), "text": "?", "kind": "text" }))
        .collect();
    let outcome = form
        .call(
            ctx(Mode::Agent),
            json!({ "form_id": "f", "title": "t", "questions": questions }),
        )
        .await;
    assert!(is_err(&outcome));
    assert!(content(&outcome).contains("最多"));
}

#[test]
fn answer_renders_labels_not_keys() {
    let questions = vec![
        lya_session::FormQuestion {
            id: "svc".into(),
            text: "用哪种服务管理？".into(),
            kind: FormQuestionKind::Single,
            options: vec![lya_session::FormOption {
                key: "systemd".into(),
                label: "systemd --user".into(),
            }],
            allow_note: true,
        },
        lya_session::FormQuestion {
            id: "path".into(),
            text: "配置放在哪个目录？".into(),
            kind: FormQuestionKind::Text,
            options: Vec::new(),
            allow_note: false,
        },
        lya_session::FormQuestion {
            id: "skipped".into(),
            text: "要顺便重启吗？".into(),
            kind: FormQuestionKind::Single,
            options: vec![lya_session::FormOption {
                key: "yes".into(),
                label: "重启".into(),
            }],
            allow_note: false,
        },
    ];

    let answer = FormAnswer {
        form_id: "deploy".into(),
        items: vec![
            FormAnswerItem {
                question_id: "svc".into(),
                values: vec!["systemd".into()],
                note: Some("想要开机自启".into()),
            },
            FormAnswerItem {
                question_id: "path".into(),
                values: vec!["~/.config/lya".into()],
                note: None,
            },
        ],
        freetext: Some("顺便看下日志".into()),
    };

    let text = render_form_answer("部署方式", &questions, &answer);
    assert_eq!(
        text,
        "[表单回答: 部署方式]\n\
         - 用哪种服务管理？: systemd --user（备注: 想要开机自启）\n\
         - 配置放在哪个目录？: ~/.config/lya\n\
         - 要顺便重启吗？: （未回答）\n\
         - 补充说明: 顺便看下日志"
    );
}

// ── 模式切换 ─────────────────────────────────────────────────

#[tokio::test]
async fn mode_change_requests_confirmation() {
    let action = RequestModeChangeAction::new();
    let outcome = action
        .call(
            ctx(Mode::Ask),
            json!({ "to_mode": "edit", "reason": "要改 hyprland.conf 才能修好" }),
        )
        .await;

    let ActionOutcome::AwaitHuman(block) = outcome else {
        panic!("切换模式应当要用户确认");
    };
    assert_eq!(
        *block,
        HitlBlock::ModeChange {
            to_mode: "edit".into(),
            reason: "要改 hyprland.conf 才能修好".into(),
        }
    );
}

#[tokio::test]
async fn mode_change_rejects_noop_and_hides_in_agent() {
    let action = RequestModeChangeAction::new();

    let same = action
        .call(
            ctx(Mode::Edit),
            json!({ "to_mode": "edit", "reason": "因为" }),
        )
        .await;
    assert!(is_err(&same));

    let unknown = action
        .call(
            ctx(Mode::Ask),
            json!({ "to_mode": "godmode", "reason": "因为" }),
        )
        .await;
    assert!(is_err(&unknown));

    assert!(action.visible_in(Mode::Ask));
    assert!(action.visible_in(Mode::Edit));
    assert!(!action.visible_in(Mode::Agent), "agent 已是最高权限");
}

// ── 注册中心 ─────────────────────────────────────────────────

#[test]
fn registry_filters_by_mode_and_exports_consistently() {
    let (_dir, store) = memory();
    let mut registry = ActionRegistry::new();
    register_builtins(&mut registry, store).unwrap();
    assert_eq!(registry.len(), 5);

    let ask = registry.bundle(Mode::Ask);
    let agent = registry.bundle(Mode::Agent);
    assert_eq!(ask.schemas.len(), 5);
    assert_eq!(
        agent.schemas.len(),
        4,
        "agent 模式下隐藏 request_mode_change"
    );
    assert!(ask.prompt.contains("request_mode_change"));
    assert!(!agent.prompt.contains("request_mode_change"));

    // 提示词与 schemas 出自同一次筛选，不会出现「prompt 里有、tools[] 没有」
    for schema in &agent.schemas {
        let name = schema["function"]["name"].as_str().unwrap();
        assert!(agent.prompt.contains(name), "{name} 应同时出现在提示词里");
        let desc = schema["function"]["description"].as_str().unwrap();
        assert!(
            desc.starts_with("[action] "),
            "{name} 的描述应标记为 action"
        );
    }

    // 流转标注要进提示词，模型才知道哪个会挂起
    assert!(agent.prompt.contains(ActionFlow::AwaitHuman.label()));
    assert!(agent.prompt.contains(ActionFlow::Continue.label()));
}

#[tokio::test]
async fn registry_invoke_reports_unknown_action() {
    let (_dir, store) = memory();
    let mut registry = ActionRegistry::new();
    register_builtins(&mut registry, store).unwrap();

    let ok = registry
        .invoke(
            "memory_write",
            ctx(Mode::Agent),
            json!({ "title": "t", "body": "b" }),
        )
        .await
        .unwrap();
    assert!(!is_err(&ok));

    let err = registry
        .invoke("nope", ctx(Mode::Agent), json!({}))
        .await
        .unwrap_err();
    assert!(matches!(err, lya_action::ActionError::NotFound(_)));
}

#[test]
fn duplicate_registration_is_rejected() {
    let mut registry = ActionRegistry::new();
    registry.register(Arc::new(FormAction::new())).unwrap();
    let err = registry.register(Arc::new(FormAction::new())).unwrap_err();
    assert!(matches!(err, lya_action::ActionError::DuplicateName(_)));
}
