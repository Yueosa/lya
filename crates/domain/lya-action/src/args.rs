//! 参数取值辅助。
//!
//! 失败返回的 `String` 是**直接给模型看的**错误说明，所以要写清楚缺了什么、
//! 期望什么类型，模型才能自己改对。

use serde_json::Value;

/// 取必填字符串（去首尾空白，不允许空串）。
pub(crate) fn req_str(args: &Value, key: &str) -> Result<String, String> {
    match args.get(key) {
        None | Some(Value::Null) => Err(format!("缺少必填参数 `{key}`")),
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.trim().to_string()),
        Some(Value::String(_)) => Err(format!("参数 `{key}` 不能为空")),
        Some(_) => Err(format!("参数 `{key}` 应为字符串")),
    }
}

/// 取可选字符串；缺省或 null 返回 `None`。
pub(crate) fn opt_str(args: &Value, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.trim().to_string())),
        Some(_) => Err(format!("参数 `{key}` 应为字符串")),
    }
}

/// 取必填整数。
pub(crate) fn req_i64(args: &Value, key: &str) -> Result<i64, String> {
    match args.get(key) {
        None | Some(Value::Null) => Err(format!("缺少必填参数 `{key}`")),
        Some(Value::Number(n)) => n.as_i64().ok_or_else(|| format!("参数 `{key}` 应为整数")),
        // 模型偶尔会把数字写成字符串，这种能救就救，不必让它多跑一轮
        Some(Value::String(s)) => s
            .trim()
            .parse::<i64>()
            .map_err(|_| format!("参数 `{key}` 应为整数，收到 {s:?}")),
        Some(_) => Err(format!("参数 `{key}` 应为整数")),
    }
}

/// 取可选布尔；缺省返回 `false`。
pub(crate) fn opt_bool(args: &Value, key: &str) -> Result<bool, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(format!("参数 `{key}` 应为布尔值")),
    }
}

/// 取可选字符串数组；缺省返回空 `Vec`。
pub(crate) fn opt_str_array(args: &Value, key: &str) -> Result<Vec<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| match item {
                Value::String(s) => Ok(s.trim().to_string()),
                _ => Err(format!("参数 `{key}` 的元素应为字符串")),
            })
            .collect(),
        Some(_) => Err(format!("参数 `{key}` 应为字符串数组")),
    }
}

/// 取必填数组。
pub(crate) fn req_array<'a>(args: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Err(format!("缺少必填参数 `{key}`")),
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(format!("参数 `{key}` 应为数组")),
    }
}
