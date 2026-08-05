//! `request_mode_change`：请求切换工作模式，需用户确认。
//!
//! 上一代没有这个动作——模式只能用户从界面改，模型撞到权限墙时只会干巴巴地
//! 说一句「请切换到 agent 模式」，用户还得自己去点。这里让模型能带着理由发起
//! 请求，用户一次确认即可放行。

use lya_base::Mode;
use lya_session::HitlBlock;
use serde_json::{Value, json};

use crate::args::req_str;
use crate::meta::{ActionFlow, ActionMeta, ActionOutcome};
use crate::traits::{Action, ActionCallFuture, ActionCtx};

const HINT: &str = "\
当你确实需要当前模式不允许的能力时才发起，并在 `reason` 里说清**为什么需要**\
以及**打算做什么**——用户是看着这句话决定放不放行的，写「需要更高权限」\
等于没写。

模式与权限：ask 只读；edit 可读写文件；agent 可读写并执行命令。
请求会挂起本轮，等用户确认后再继续；被拒绝就在现有权限内想办法，或者告诉\
用户你做不到，不要反复请求。";

/// `request_mode_change` 动作。
pub struct RequestModeChangeAction {
    meta: ActionMeta,
    params: Value,
}

impl Default for RequestModeChangeAction {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestModeChangeAction {
    /// 构造。
    pub fn new() -> Self {
        Self {
            meta: ActionMeta::new(
                "request_mode_change",
                "请求切换模式",
                "请求用户把会话切换到另一个工作模式",
                ActionFlow::AwaitHuman,
            ),
            params: json!({
                "type": "object",
                "properties": {
                    "to_mode": {
                        "type": "string",
                        "enum": ["ask", "edit", "agent"],
                        "description": "目标模式：ask 只读 / edit 可写 / agent 可执行"
                    },
                    "reason": {
                        "type": "string",
                        "description": "为什么需要，以及打算做什么；用户据此决定是否放行"
                    }
                },
                "required": ["to_mode", "reason"]
            }),
        }
    }
}

impl Action for RequestModeChangeAction {
    fn meta(&self) -> &ActionMeta {
        &self.meta
    }

    fn parameters(&self) -> &Value {
        &self.params
    }

    fn prompt_hint(&self) -> &str {
        HINT
    }

    /// agent 已是最高权限，没有可请求的目标，直接不暴露。
    fn visible_in(&self, mode: Mode) -> bool {
        mode != Mode::Agent
    }

    fn call<'a>(&'a self, ctx: ActionCtx<'a>, args: Value) -> ActionCallFuture<'a> {
        Box::pin(async move {
            let to_mode = match req_str(&args, "to_mode") {
                Ok(raw) => match raw.parse::<Mode>() {
                    Ok(mode) => mode,
                    Err(err) => return ActionOutcome::err(err.to_string()),
                },
                Err(msg) => return ActionOutcome::err(msg),
            };
            let reason = match req_str(&args, "reason") {
                Ok(reason) => reason,
                Err(msg) => return ActionOutcome::err(msg),
            };

            if to_mode == ctx.mode {
                return ActionOutcome::err(format!("当前已经是 {} 模式了。", to_mode.as_str()));
            }

            ActionOutcome::await_human(HitlBlock::ModeChange {
                to_mode: to_mode.as_str().to_string(),
                reason,
            })
        })
    }
}
