//! `lya-config` 的加载与校验测试。

use std::fs;
use std::path::Path;

use lya_config::{
    ApiMode, Config, ConfigError, LogLevel, MODELS_FILE, PROMPT_FILE, validate_session_binding,
};
use lya_base::Mode;

fn write(dir: &Path, name: &str, text: &str) {
    fs::write(dir.join(name), text).unwrap();
}

fn sample_model(id: &str, with_responses: bool) -> String {
    let responses = if with_responses {
        r#"
[models.modes.responses]
capabilities = ["text", "web_search"]
params = { model = "x-flash", max_output_tokens = 4096 }
"#
    } else {
        ""
    };
    format!(
        r#"
[[models]]
id = "{id}"
name = "Name {id}"
base_url = "https://api.example.com"
api_key = "sk-real"
context_window = 1048576

[models.modes.completions]
capabilities = ["text"]
params = {{ model = "{id}", max_tokens = 4096, reasoning_effort = "high" }}
{responses}"#
    )
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
    assert_eq!(config.runtime.agent.default_api_mode, ApiMode::Completions);
    assert_eq!(config.runtime.media.image.max_bytes, 32 * 1024 * 1024);
    assert!(config.runtime.media.image.retain_local);
    assert!(config.runtime.media.image.retain_web);
    assert_eq!(config.runtime.media.video.max_bytes, 512 * 1024 * 1024);
    assert!(config.runtime.media.video.retain_local);
    assert_eq!(config.runtime.media.audio.max_bytes, 128 * 1024 * 1024);
    assert_eq!(config.runtime.tools.enabled, None, "键缺省表示启用全部工具");
    assert!(config.models.is_empty());
    assert_eq!(config.prompt.section_text(lya_config::PromptSectionKey::Environment), None);
}

#[test]
fn generated_templates_parse_and_are_consistent() {
    let dir = tempfile::tempdir().unwrap();
    let created = Config::init_missing(dir.path()).unwrap();
    assert_eq!(created.len(), 4);

    let config = Config::load_from(dir.path()).unwrap();
    assert_eq!(config.models.models.len(), 2);
    let flash = config.models.get("deepseek-v4-flash").unwrap();
    assert!(flash.supports(ApiMode::Completions));
    assert!(flash.supports(ApiMode::Responses));
    assert!(flash.can(ApiMode::Responses, "web_search"));
    let pro = config.models.get("deepseek-v4-pro").unwrap();
    assert!(pro.supports(ApiMode::Completions));
    assert!(!pro.supports(ApiMode::Responses));
    assert!(config.prompt.identity.text.contains("普拉娜"));
    assert_eq!(
        config.runtime.agent.default_model.as_deref(),
        Some("deepseek-v4-flash")
    );

    assert!(matches!(
        config.check_ready(),
        Err(ConfigError::NotReady(_))
    ));

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
fn legacy_flat_model_format_is_rejected() {
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
capabilities = ["text"]
model = "deepseek-v4-flash"
max_tokens = 4096
"#,
    );
    assert!(matches!(
        Config::load_from(dir.path()),
        Err(ConfigError::Parse { .. })
    ));
}

#[test]
fn mode_params_are_scoped_not_merged_at_top_level() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), MODELS_FILE, &sample_model("ds", true));
    let config = Config::load_from(dir.path()).unwrap();
    let entry = config.models.get("ds").unwrap();
    assert_eq!(
        entry.params_for(ApiMode::Completions)["max_tokens"],
        4096
    );
    assert_eq!(
        entry.params_for(ApiMode::Responses)["max_output_tokens"],
        4096
    );
    assert!(!entry.params_for(ApiMode::Completions).contains_key("max_output_tokens"));
    config.check_ready().unwrap();
}

#[test]
fn context_window_is_lya_metadata_not_in_params() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), MODELS_FILE, &sample_model("ds", false));
    let config = Config::load_from(dir.path()).unwrap();
    let entry = config.models.get("ds").unwrap();
    assert_eq!(entry.context_window, Some(1_048_576));
    assert!(!entry.params_for(ApiMode::Completions).contains_key("context_window"));
}

#[test]
fn capabilities_are_per_mode() {
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

[models.modes.completions]
capabilities = ["text"]
params = { model = "plain" }

[[models]]
id = "eyes"
name = "会看图的"
base_url = "https://b"
api_key = "k"

[models.modes.completions]
capabilities = ["text", "vision"]
params = { model = "eyes" }
"#,
    );

    let config = Config::load_from(dir.path()).unwrap();
    let plain = config.models.get("plain").unwrap();
    assert!(plain.can(ApiMode::Completions, "text"));
    assert!(!plain.can(ApiMode::Completions, "vision"));

    let eyes = config.models.get("eyes").unwrap();
    assert!(eyes.can(ApiMode::Completions, "vision"));
    assert_eq!(
        config.models.first_with(ApiMode::Completions, "vision").unwrap().id,
        "eyes"
    );
    assert!(config.models.first_with(ApiMode::Completions, "video").is_none());
}

#[test]
fn validate_session_binding_checks_api_mode() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        MODELS_FILE,
        &format!(
            "{}{}",
            sample_model("flash", true),
            sample_model("pro", false).replace("flash", "pro")
        ),
    );
    let config = Config::load_from(dir.path()).unwrap();

    validate_session_binding(
        &config.models,
        Some("flash"),
        "flash",
        ApiMode::Responses,
    )
    .unwrap();
    assert!(validate_session_binding(
        &config.models,
        Some("pro"),
        "flash",
        ApiMode::Responses,
    )
    .is_err());
}

#[test]
fn dangling_default_model_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), MODELS_FILE, &sample_model("ds", false));
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

[models.modes.completions]
capabilities = ["text"]
params = { model = "same" }

[[models]]
id = "same"
name = "B"
base_url = "https://b"
api_key = "k"

[models.modes.completions]
capabilities = ["text"]
params = { model = "same" }
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
        panic!("拼写错误应当解析失败");
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

[models.modes.completions]
capabilities = ["text"]
params = { model = "first" }

[[models]]
id = "second"
name = "B"
base_url = "https://b"
api_key = "k"

[models.modes.completions]
capabilities = ["text"]
params = { model = "second" }
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
fn empty_prompt_section_is_treated_as_unset() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        PROMPT_FILE,
        "[identity]\ntext = \"   \"\n",
    );
    let config = Config::load_from(dir.path()).unwrap();
    assert_eq!(
        config.prompt.section_text(lya_config::PromptSectionKey::Identity),
        None,
        "空段应回退到 lya-prompt 的内置默认"
    );
}
