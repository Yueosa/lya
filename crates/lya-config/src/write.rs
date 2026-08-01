//! 把改动写回配置文件。
//!
//! **用 `toml_edit` 而不是重新序列化整个结构体。** 模板里那些注释是在逐字段解释
//! 自己在干什么，而本地优先的工具用户是会手改这些文件的——把结构体序列化回去会
//! 让注释、字段顺序、空行全部消失，等于每次从界面改一下就把文件洗一遍。
//! `toml_edit` 只替换对应的值，其余原样保留。

use std::fs;
use std::path::Path;

use crate::error::ConfigError;

/// 改 `runtime.toml` 里若干张表。
///
/// 入参形如 `{"agent": {"max_tool_rounds": 8}, "shell": {"confirm": "always"}}`，
/// 只覆盖提到的键。
pub fn write_runtime(
    dir: &Path,
    tables: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), ConfigError> {
    edit_file(&dir.join(crate::RUNTIME_FILE), |document| {
        for (table, values) in tables {
            let Some(values) = values.as_object() else {
                return Err(ConfigError::Invalid(format!("{table} 应当是一张表")));
            };
            merge_table(document, table, values)?;
        }
        Ok(())
    })
}

/// 写入全局人设。
pub fn write_persona(dir: &Path, text: &str) -> Result<(), ConfigError> {
    edit_file(&dir.join(crate::PERSONA_FILE), |document| {
        document["text"] = toml_edit::value(text);
        Ok(())
    })
}

/// 用 `toml_edit` 就地改一个文件。
///
/// 文件不存在时从空文档开始，这样第一次写也能成功。
pub fn edit_file(
    path: &Path,
    apply: impl FnOnce(&mut toml_edit::DocumentMut) -> Result<(), ConfigError>,
) -> Result<(), ConfigError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(source) => {
            return Err(ConfigError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    let mut document: toml_edit::DocumentMut =
        text.parse().map_err(|err: toml_edit::TomlError| {
            ConfigError::Invalid(format!("{} 不是合法 TOML：{err}", path.display()))
        })?;
    apply(&mut document)?;

    fs::write(path, document.to_string()).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// 把一份 JSON 值写进文档的某个顶层表。
///
/// 只覆盖给出的键，没提到的原样留着——界面上改一个开关不该顺手重置别的字段。
pub fn merge_table(
    document: &mut toml_edit::DocumentMut,
    table: &str,
    values: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), ConfigError> {
    let entry = document
        .entry(table)
        .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
    let Some(target) = entry.as_table_mut() else {
        return Err(ConfigError::Invalid(format!("{table} 不是一张表")));
    };
    for (key, value) in values {
        set_preserving_decor(target, key, to_item(value)?);
    }
    Ok(())
}

/// 只换掉值，键和它前后的注释原封不动。
///
/// 直接 `insert` 会把整个条目连同装饰一起替换，字段旁的注释就没了。
fn set_preserving_decor(table: &mut toml_edit::Table, key: &str, item: toml_edit::Item) {
    match table.get_mut(key) {
        Some(slot) => {
            let prefix = slot
                .as_value()
                .and_then(|value| value.decor().prefix().cloned());
            let suffix = slot
                .as_value()
                .and_then(|value| value.decor().suffix().cloned());
            *slot = item;
            if let Some(value) = slot.as_value_mut() {
                if let Some(prefix) = prefix {
                    value.decor_mut().set_prefix(prefix);
                }
                if let Some(suffix) = suffix {
                    value.decor_mut().set_suffix(suffix);
                }
            }
        }
        None => {
            table.insert(key, item);
        }
    }
}

/// JSON 值转成 TOML 项。
pub fn to_item(value: &serde_json::Value) -> Result<toml_edit::Item, ConfigError> {
    use serde_json::Value;
    let item = match value {
        Value::Null => {
            return Err(ConfigError::Invalid(
                "TOML 没有 null；要清空请给出空字符串或空数组".into(),
            ));
        }
        Value::Bool(b) => toml_edit::value(*b),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => toml_edit::value(i),
            (_, Some(f)) => toml_edit::value(f),
            _ => return Err(ConfigError::Invalid(format!("无法表示的数字：{n}"))),
        },
        Value::String(s) => toml_edit::value(s.as_str()),
        Value::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(to_value(item)?);
            }
            toml_edit::value(array)
        }
        Value::Object(map) => {
            let mut table = toml_edit::Table::new();
            for (key, value) in map {
                table.insert(key, to_item(value)?);
            }
            toml_edit::Item::Table(table)
        }
    };
    Ok(item)
}

/// 数组元素只能是标量或内联表。
fn to_value(value: &serde_json::Value) -> Result<toml_edit::Value, ConfigError> {
    use serde_json::Value;
    let value = match value {
        Value::Bool(b) => toml_edit::Value::from(*b),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => toml_edit::Value::from(i),
            (_, Some(f)) => toml_edit::Value::from(f),
            _ => return Err(ConfigError::Invalid(format!("无法表示的数字：{n}"))),
        },
        Value::String(s) => toml_edit::Value::from(s.as_str()),
        Value::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(to_value(item)?);
            }
            toml_edit::Value::Array(array)
        }
        Value::Object(map) => {
            let mut inline = toml_edit::InlineTable::new();
            for (key, value) in map {
                inline.insert(key, to_value(value)?);
            }
            toml_edit::Value::InlineTable(inline)
        }
        Value::Null => {
            return Err(ConfigError::Invalid("数组里不能有 null".into()));
        }
    };
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn comments_and_untouched_fields_survive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("runtime.toml");
        fs::write(
            &path,
            "# 顶部说明\n\n[agent]\n# 这一行解释 max_tool_rounds 是干嘛的\nmax_tool_rounds = 32\ndefault_work_mode = \"agent\"\n",
        )
        .unwrap();

        edit_file(&path, |doc| {
            let values = json!({ "max_tool_rounds": 8 });
            merge_table(doc, "agent", values.as_object().unwrap())
        })
        .unwrap();

        let after = fs::read_to_string(&path).unwrap();
        assert!(after.contains("max_tool_rounds = 8"));
        assert!(after.contains("# 顶部说明"), "注释不能被洗掉");
        assert!(after.contains("# 这一行解释"), "字段旁的注释也要留着");
        assert!(
            after.contains("default_work_mode = \"agent\""),
            "没提到的字段不该被动"
        );
    }

    #[test]
    fn creates_the_table_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("runtime.toml");
        fs::write(&path, "[agent]\nmax_tool_rounds = 32\n").unwrap();

        edit_file(&path, |doc| {
            let values = json!({ "confirm": "always" });
            merge_table(doc, "shell", values.as_object().unwrap())
        })
        .unwrap();

        let after = fs::read_to_string(&path).unwrap();
        assert!(after.contains("[shell]"));
        assert!(after.contains("confirm = \"always\""));
    }

    #[test]
    fn arrays_and_nested_tables_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("runtime.toml");

        edit_file(&path, |doc| {
            let values = json!({
                "enabled": ["file_read", "bash"],
                "nested": { "a": 1, "b": true }
            });
            merge_table(doc, "tools", values.as_object().unwrap())
        })
        .unwrap();

        let after = fs::read_to_string(&path).unwrap();
        let parsed: toml::Value = toml::from_str(&after).unwrap();
        assert_eq!(parsed["tools"]["enabled"][1].as_str(), Some("bash"));
        assert_eq!(parsed["tools"]["nested"]["a"].as_integer(), Some(1));
    }

    #[test]
    fn null_is_rejected_with_a_hint() {
        let err = to_item(&json!(null)).unwrap_err();
        assert!(err.to_string().contains("null"));
    }
}
