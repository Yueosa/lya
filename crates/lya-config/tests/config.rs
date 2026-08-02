//! `lya-config` 的加载与校验测试。

use std::fs;
use std::path::Path;

use lya_config::{Config, ConfigError, LogLevel, MODELS_FILE};
use lya_mode::Mode;

fn write(dir: &Path, name: &str, text: &str) {
    fs::write(dir.join(name), text).unwrap();
}

#[test]
fn missing_files_fall_back_to_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::load_from(dir.path()).unwrap();

    assert_eq!(config.core.server.port, 51616);
    assert_eq!(config.core.log.level, LogLevel::Info);
    assert_eq!(config.core.http.timeout_secs, 120);
    assert_eq!(config.runtime.agent.max_tool_rounds, 32);
    assert_eq!(config.runtime.agent.default_work_mode, Mode::Agent);
    assert_eq!(config.runtime.tools.enabled, None, "键缺省表示启用全部工具");
    assert!(config.models.is_empty());
    assert_eq!(config.persona, None);
}

#[test]
fn generated_templates_parse_and_are_consistent() {
    let dir = tempfile::tempdir().unwrap();
    let created = Config::init_missing(dir.path()).unwrap();
    assert_eq!(created.len(), 4);

    // 模板必须自洽：default_model 指向的 id 确实在清单里，否则校验会失败
    let config = Config::load_from(dir.path()).unwrap();
    assert_eq!(config.models.models.len(), 2);
    assert!(config.persona.unwrap().contains("小恋恋"));
    assert_eq!(
        config.runtime.agent.default_model.as_deref(),
        Some("deepseek-v4-flash")
    );

    // 但密钥还是占位符，不该被当成可用
    let config = Config::load_from(dir.path()).unwrap();
    assert!(matches!(
        config.check_ready(),
        Err(ConfigError::NotReady(_))
    ));

    // 再跑一次不覆盖已有文件
    assert!(Config::init_missing(dir.path()).unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn models_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    Config::init_missing(dir.path()).unwrap();
    let mode = fs::metadata(dir.path().join(MODELS_FILE))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600, "含密钥的文件不该让别人读到");
}

#[test]
fn extra_model_fields_pass_through() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        MODELS_FILE,
        r#"
[[models]]
id = "ds"
name = "DeepSeek"
base_url = "https://api.deepseek.com"
api_key = "sk-real"
model = "deepseek-v4-flash"
reasoning_effort = "high"
thinking = { type = "enabled" }
max_tokens = 4096
"#,
    );

    let config = Config::load_from(dir.path()).unwrap();
    let entry = config.models.get("ds").unwrap();
    assert_eq!(entry.params["model"], "deepseek-v4-flash");
    assert_eq!(entry.params["reasoning_effort"], "high");
    assert_eq!(entry.params["max_tokens"], 4096);
    // 嵌套表也要原样带过去
    assert_eq!(entry.params["thinking"]["type"], "enabled");
    // 固定字段不该混进透传参数
    for fixed in ["id", "name", "base_url", "api_key", "context_window"] {
        assert!(!entry.params.contains_key(fixed), "{fixed} 不该被透传");
    }

    config.check_ready().unwrap();
}

#[test]
fn context_window_is_lya_metadata_not_api_param() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        MODELS_FILE,
        r#"
[[models]]
id = "ds"
name = "DeepSeek"
base_url = "https://api.deepseek.com"
api_key = "sk-real"
context_window = 1048576
model = "deepseek-v4-flash"
max_tokens = 8192
"#,
    );

    let config = Config::load_from(dir.path()).unwrap();
    let entry = config.models.get("ds").unwrap();
    assert_eq!(entry.context_window, Some(1_048_576));
    assert_eq!(entry.params["max_tokens"], 8192);
    assert!(!entry.params.contains_key("context_window"));
}

#[test]
fn capabilities_default_to_text_only() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        MODELS_FILE,
        r#"
[[models]]
id = "plain"
name = "只会说话的"
base_url = "https://a"
api_key = "k"

[[models]]
id = "eyes"
name = "会看图的"
base_url = "https://b"
api_key = "k"
capabilities = ["text", "vision"]
"#,
    );

    let config = Config::load_from(dir.path()).unwrap();
    let plain = config.models.get("plain").unwrap();
    // 没写 capabilities 就按纯文本算，老配置不用改也能跑
    assert!(plain.can("text"));
    assert!(!plain.can("vision"));
    assert_eq!(plain.effective_capabilities(), vec!["text".to_string()]);

    let eyes = config.models.get("eyes").unwrap();
    assert!(eyes.can("vision"));

    // 视觉工具靠它挑「谁能看图」，不用让用户再配一遍
    assert_eq!(config.models.first_with("vision").unwrap().id, "eyes");
    assert!(config.models.first_with("video").is_none());
}

#[test]
fn dangling_default_model_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        MODELS_FILE,
        r#"
[[models]]
id = "ds"
name = "DeepSeek"
base_url = "https://api.deepseek.com"
api_key = "sk-real"
model = "x"
"#,
    );
    write(
        dir.path(),
        "runtime.toml",
        "[agent]\ndefault_model = \"nope\"\n",
    );

    let err = Config::load_from(dir.path()).unwrap_err();
    let ConfigError::Invalid(msg) = err else {
        panic!("应当是配置自相矛盾");
    };
    assert!(msg.contains("nope"));
    assert!(msg.contains("ds"), "报错要顺便列出可用的 id");
}

#[test]
fn duplicate_model_id_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        MODELS_FILE,
        r#"
[[models]]
id = "same"
name = "A"
base_url = "https://a"
api_key = "k"

[[models]]
id = "same"
name = "B"
base_url = "https://b"
api_key = "k"
"#,
    );
    assert!(matches!(
        Config::load_from(dir.path()),
        Err(ConfigError::Invalid(_))
    ));
}

#[test]
fn typo_in_field_name_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "core.toml", "[server]\nprot = 8080\n");

    let err = Config::load_from(dir.path()).unwrap_err();
    let ConfigError::Parse { path, .. } = err else {
        panic!("拼错字段名应当解析失败");
    };
    assert!(path.ends_with("core.toml"));
}

#[test]
fn bad_work_mode_fails_at_load() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "runtime.toml",
        "[agent]\ndefault_work_mode = \"asdf\"\n",
    );
    assert!(matches!(
        Config::load_from(dir.path()),
        Err(ConfigError::Parse { .. })
    ));
}

#[test]
fn empty_tool_list_means_none_enabled() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "runtime.toml", "[tools]\nenabled = []\n");
    let config = Config::load_from(dir.path()).unwrap();
    assert_eq!(config.runtime.tools.enabled, Some(Vec::new()));
}

#[test]
fn db_path_resolves_relative_and_absolute() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::load_from(dir.path()).unwrap();
    assert_eq!(config.db_path(), dir.path().join("lya.db"));

    write(
        dir.path(),
        "core.toml",
        "[db]\npath = \"/srv/lya/main.db\"\n",
    );
    let config = Config::load_from(dir.path()).unwrap();
    assert_eq!(config.db_path(), Path::new("/srv/lya/main.db"));
}

#[test]
fn default_model_falls_back_to_first_entry() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        MODELS_FILE,
        r#"
[[models]]
id = "first"
name = "A"
base_url = "https://a"
api_key = "k"

[[models]]
id = "second"
name = "B"
base_url = "https://b"
api_key = "k"
"#,
    );
    let config = Config::load_from(dir.path()).unwrap();
    assert_eq!(config.default_model().unwrap().id, "first");
}

#[test]
fn port_backoff_yields_candidate_range() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::load_from(dir.path()).unwrap();
    let ports: Vec<u16> = config.core.server.candidate_ports().collect();
    assert_eq!(ports.len(), 51);
    assert_eq!(ports.first(), Some(&51616));
    assert_eq!(ports.last(), Some(&51666));
}

#[test]
fn empty_persona_is_treated_as_unset() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "persona.toml", "text = \"   \"\n");
    let config = Config::load_from(dir.path()).unwrap();
    assert_eq!(config.persona, None, "空人设应回退到 lya-prompt 的内置默认");
}
