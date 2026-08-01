//! 把各种消息节点序列化成 JSON 打出来，作为前端类型的依据。
//!
//! 前端类型如果照着 Rust 结构体手抄，serde 的重命名、`skip_serializing_if`、
//! 枚举的 tag 形式都可能对不上，而这类偏差要等联调时才炸。
//!
//! ```bash
//! cargo run -p lya-core --example wire
//! ```

use lya_session::{
    ConfirmStepBlock, FormOption, FormQuestion, FormQuestionKind, HitlBlock, MessageKind,
    MessagePayload, MessageStatus, OpenAiFunction, OpenAiToolCall,
};

fn dump(label: &str, payload: &MessagePayload) {
    println!("\n// {label}");
    println!("{}", serde_json::to_string_pretty(payload).unwrap());
}

fn main() {
    dump("用户消息", &MessagePayload::user_text("帮我看看家目录有多少图片"));

    dump(
        "助手正文（流式中）",
        &MessagePayload::assistant_text("我来看看。", MessageStatus::Streaming),
    );

    let mut with_reasoning =
        MessagePayload::assistant_text("我来看看。", MessageStatus::Complete);
    with_reasoning.lya.reasoning = Some("用户想知道图片数量，先扫一下目录".into());
    dump("助手正文 + 思考", &with_reasoning);

    // 没有现成的构造器：助手带调用的消息由 agent 从 ChatCompletion 拼出来，
    // 这里照它的形状手搓一个
    let mut calling = MessagePayload::assistant_text("先扫一下你的图片目录。", MessageStatus::Complete);
    calling.kind = MessageKind::ToolCall;
    if let Some(openai) = calling.openai.as_mut() {
        openai.tool_calls = Some(vec![OpenAiToolCall {
            id: "call_1".into(),
            kind: "function".into(),
            function: OpenAiFunction {
                name: "image_scan".into(),
                arguments: r#"{"path":"~/图片","recursive":true}"#.into(),
            },
        }]);
    }
    dump("助手发起工具调用（正文与调用可以同时出现）", &calling);

    dump(
        "工具结果（独立节点，靠 tool_call_id 与调用配对）",
        &MessagePayload::tool_result("call_1", "~/图片（共 12 张，列出 12 张）\n…"),
    );

    dump(
        "HITL：表单",
        &MessagePayload::hitl_pending(
            MessageKind::Form,
            HitlBlock::Form {
                form_id: "f_1".into(),
                title: "要删掉这些重复图吗".into(),
                questions: vec![FormQuestion {
                    id: "q1".into(),
                    text: "选择要删除的".into(),
                    kind: FormQuestionKind::Multi,
                    options: vec![FormOption {
                        key: "a".into(),
                        label: "猫副本.png".into(),
                    }],
                    allow_note: true,
                }],
            },
        ),
    );

    dump(
        "HITL：工具确认",
        &MessagePayload::hitl_pending(
            MessageKind::ToolConfirm,
            HitlBlock::ToolConfirm {
                tool_call_id: "call_2".into(),
                tool_name: "bash".into(),
                arguments: serde_json::json!({ "command": "rm ~/图片/猫副本.png" }),
                summary: "执行：rm ~/图片/猫副本.png".into(),
                steps: vec![ConfirmStepBlock {
                    raw: "rm ~/图片/猫副本.png".into(),
                    explain: "删除文件".into(),
                    risk: Some("不可恢复".into()),
                    connector: String::new(),
                }],
                reasons: vec!["会删文件".into()],
            },
        ),
    );

    dump(
        "HITL：模式切换",
        &MessagePayload::hitl_pending(
            MessageKind::ModeChange,
            HitlBlock::ModeChange {
                to_mode: "agent".into(),
                reason: "需要执行命令才能继续".into(),
            },
        ),
    );

    dump(
        "系统标记（用户手动切模式时入树）",
        &MessagePayload::system_text("用户把工作模式切换为 agent。"),
    );

    let mut interrupted =
        MessagePayload::assistant_text("说到一半就被", MessageStatus::Interrupted);
    interrupted.lya.meta = Some(serde_json::json!({ "reason": "cancelled" }));
    dump("被中断的助手消息", &interrupted);
}
