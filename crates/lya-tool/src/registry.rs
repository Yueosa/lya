//! 工具注册中心。
//!
//! 启动时 [`ToolRegistry::register`] 全部工具；运行时按
//! **名字列表 ∩ RWX 上限** 导出 [`ToolBundle`]（提示词 + OpenAI schemas）。

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::error::ToolError;
use crate::meta::ToolResult;
use crate::permission::Permission;
use crate::traits::Tool;

/// 一次筛选导出的结果：提示词段 + 供 `chat/completions` 使用的 `tools` 数组。
///
/// 两者由同一筛选结果生成，避免「prompt 里有、tools[] 没有」的不一致。
#[derive(Debug, Clone, PartialEq)]
pub struct ToolBundle {
    /// 拼进 system prompt 的工具说明段；无匹配工具时为空串。
    pub prompt: String,
    /// OpenAI 兼容的 `tools` 数组元素列表。
    pub schemas: Vec<Value>,
    /// 本次筛选出的工具内部名。
    ///
    /// 执行前的二次拦截要用**这一份**，而不是自己再算一遍筛选条件——
    /// 两处逻辑一旦漂移，就会出现「没提供给模型却能执行」的漏洞。
    pub names: Vec<String>,
}

impl ToolBundle {
    /// 空结果。
    pub fn empty() -> Self {
        Self {
            prompt: String::new(),
            schemas: Vec::new(),
            names: Vec::new(),
        }
    }

    /// 该工具是否在本次筛选结果内。
    pub fn allows(&self, name: &str) -> bool {
        self.names.iter().any(|n| n == name)
    }

    /// 是否没有任何工具。
    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }
}

/// 进程内工具目录：`name → Tool`。
///
/// 不持有会话启用状态；调用方（session / agent）传入筛选条件即可。
#[derive(Default)]
pub struct ToolRegistry {
    /// 按 name 排序存储，保证导出顺序稳定。
    tools: BTreeMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个工具；`meta.name` 重复则报错。
    pub fn register(&mut self, tool: Arc<dyn Tool>) -> Result<(), ToolError> {
        let name = tool.meta().name.clone();
        if self.tools.contains_key(&name) {
            return Err(ToolError::DuplicateName(name));
        }
        self.tools.insert(name, tool);
        Ok(())
    }

    /// 已注册数量。
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// 按内部名查找。
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// 全部工具（按 name 排序）。
    pub fn list(&self) -> impl Iterator<Item = &Arc<dyn Tool>> {
        self.tools.values()
    }

    /// 全部内部名（排序后）。
    pub fn names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// 按条件筛选并导出 [`ToolBundle`]。
    ///
    /// - `names`：`None` 表示不按名过滤（全集）；`Some(list)` 只保留列表中的名字
    ///   （未知名字静默忽略，便于 session 配置里残留旧名）
    /// - `allowed`：权限上限；仅 `tool.prmt ⊆ allowed` 的工具入选
    pub fn bundle(&self, names: Option<&[&str]>, allowed: Permission) -> ToolBundle {
        let selected = self.select(names, allowed);
        if selected.is_empty() {
            return ToolBundle::empty();
        }

        let mut prompt = String::from("## Tools\n\n");
        let mut schemas = Vec::with_capacity(selected.len());
        let mut names = Vec::with_capacity(selected.len());

        for tool in &selected {
            let meta = tool.meta();
            names.push(meta.name.clone());
            prompt.push_str(&format!(
                "### {} ({}) [{}]\n{}\n\n{}\n\n",
                meta.name,
                meta.raw_name,
                meta.prmt,
                meta.desc,
                tool.prompt_hint().trim()
            ));
            schemas.push(openai_tool_schema(tool.as_ref()));
        }

        ToolBundle {
            prompt,
            schemas,
            names,
        }
    }

    /// 只导出提示词段（等价于 [`ToolRegistry::bundle`] 的 `.prompt`）。
    pub fn prompt_section(&self, names: Option<&[&str]>, allowed: Permission) -> String {
        self.bundle(names, allowed).prompt
    }

    /// 只导出 OpenAI `tools` schemas。
    pub fn openai_tools(&self, names: Option<&[&str]>, allowed: Permission) -> Vec<Value> {
        self.bundle(names, allowed).schemas
    }

    /// 调用已注册工具（不做会话白名单；调用方先自己筛）。
    pub async fn invoke(&self, name: &str, args: Value) -> Result<ToolResult, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;
        Ok(tool.call(args).await)
    }

    /// 内部筛选：名字 ∩ 权限。
    fn select(&self, names: Option<&[&str]>, allowed: Permission) -> Vec<Arc<dyn Tool>> {
        self.tools
            .values()
            .filter(|tool| {
                let meta = tool.meta();
                if !meta.prmt.is_subset_of(allowed) {
                    return false;
                }
                match names {
                    None => true,
                    Some(list) => list.iter().any(|n| *n == meta.name),
                }
            })
            .cloned()
            .collect()
    }
}

/// 生成单条 OpenAI function 定义。
///
/// 工具与 action 最终发给的是同一个 `tools[]`，schema 形状必须完全一致，
/// 所以两边共用这一个函数，而不是各写各的。
pub fn openai_function_schema(name: &str, description: &str, parameters: &Value) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": parameters,
        }
    })
}

/// 从工具生成单条 OpenAI function tool 定义。
///
/// - `function.name` ← `meta.name`
/// - `function.description` ← `meta.desc`
/// - `function.parameters` ← `parameters()`
///
/// `raw_name` / `prmt` / `prompt_hint` **不**进入该 JSON。
pub fn openai_tool_schema(tool: &dyn Tool) -> Value {
    let meta = tool.meta();
    openai_function_schema(&meta.name, &meta.desc, tool.parameters())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::ToolMeta;
    use crate::traits::ToolCallFuture;
    use serde_json::json;
    use std::sync::Arc;

    struct DummyTool {
        meta: ToolMeta,
        params: Value,
        hint: &'static str,
    }

    impl Tool for DummyTool {
        fn meta(&self) -> &ToolMeta {
            &self.meta
        }
        fn parameters(&self) -> &Value {
            &self.params
        }
        fn prompt_hint(&self) -> &str {
            self.hint
        }
        fn call(&self, _args: Value) -> ToolCallFuture<'_> {
            Box::pin(async { ToolResult::ok("ok") })
        }
    }

    fn tool(name: &str, prmt: Permission) -> Arc<dyn Tool> {
        Arc::new(DummyTool {
            meta: ToolMeta::new(name, format!("{name}_display"), format!("{name} desc"), prmt),
            params: json!({
                "type": "object",
                "properties": { "q": { "type": "string" } },
                "required": ["q"]
            }),
            hint: "usage hint",
        })
    }

    #[test]
    fn register_and_filter_by_permission() {
        let mut reg = ToolRegistry::new();
        reg.register(tool("read_only", Permission::READ)).unwrap();
        reg.register(tool("writer", Permission::READ_WRITE)).unwrap();
        reg.register(tool("shell", Permission::READ_WRITE_EXEC))
            .unwrap();

        let ask = reg.bundle(None, Permission::READ);
        assert_eq!(ask.schemas.len(), 1);
        assert_eq!(ask.schemas[0]["function"]["name"], "read_only");
        assert!(ask.prompt.contains("read_only"));
        assert!(!ask.prompt.contains("writer"));

        let edit = reg.bundle(None, Permission::READ_WRITE);
        let names: Vec<_> = edit.schemas
            .iter()
            .map(|s| s["function"]["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(names, vec!["read_only", "writer"]);

        let agent = reg.bundle(None, Permission::READ_WRITE_EXEC);
        assert_eq!(agent.schemas.len(), 3);
    }

    #[test]
    fn filter_by_name_list() {
        let mut reg = ToolRegistry::new();
        reg.register(tool("a", Permission::READ)).unwrap();
        reg.register(tool("b", Permission::READ)).unwrap();
        reg.register(tool("c", Permission::READ)).unwrap();

        let bundle = reg.bundle(Some(&["c", "a", "missing"]), Permission::READ_WRITE_EXEC);
        let names: Vec<_> = bundle
            .schemas
            .iter()
            .map(|s| s["function"]["name"].as_str().unwrap().to_string())
            .collect();
        // BTreeMap 顺序：a, c
        assert_eq!(names, vec!["a", "c"]);
        assert!(bundle.prompt.contains("usage hint"));
        assert_eq!(bundle.schemas[0]["function"]["description"], "a desc");
    }

    #[test]
    fn duplicate_register_fails() {
        let mut reg = ToolRegistry::new();
        reg.register(tool("x", Permission::READ)).unwrap();
        let err = reg.register(tool("x", Permission::READ)).unwrap_err();
        assert_eq!(err, ToolError::DuplicateName("x".into()));
    }

    #[tokio::test]
    async fn invoke_unknown() {
        let reg = ToolRegistry::new();
        let err = reg.invoke("nope", json!({})).await.unwrap_err();
        assert_eq!(err, ToolError::NotFound("nope".into()));
    }
}
