//! 内置动作。

pub mod form;
pub mod memory;
pub mod mode_change;

use std::sync::Arc;

use lya_memory::MemoryStore;

use crate::error::ActionError;
use crate::registry::ActionRegistry;

pub use form::{FormAction, FormAnswer, FormAnswerItem, render_form_answer};
pub use memory::{MemoryReadAction, MemoryWriteAction};
pub use mode_change::RequestModeChangeAction;

/// 注册全部内置动作。
///
/// 记忆动作需要仓储，所以要求调用方先把 [`MemoryStore`] 建好传进来。
pub fn register_builtins(
    registry: &mut ActionRegistry,
    memory: Arc<MemoryStore>,
) -> Result<(), ActionError> {
    registry.register(Arc::new(MemoryReadAction::new(Arc::clone(&memory))))?;
    registry.register(Arc::new(MemoryWriteAction::new(memory)))?;
    registry.register(Arc::new(FormAction::new()))?;
    registry.register(Arc::new(RequestModeChangeAction::new()))?;
    Ok(())
}
