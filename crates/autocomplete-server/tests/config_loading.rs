use autocomplete_server::{BrokerConfig, ProviderKind, TriggerMode};
use std::fs;

#[test]
fn config_loading_applies_defaults_and_privacy_overrides() {
    let file = tempfile::NamedTempFile::new().expect("config file");
    fs::write(
        file.path(),
        r#"
[provider]
kind = "pi"

[pi]
model = "openai/example"

[privacy]
remote_context_byte_limit = 4096
excluded_globs = ["**/.env*", "**/private/**"]
"#,
    )
    .expect("write config");

    let config = BrokerConfig::load(file.path()).expect("load config");

    assert_eq!(config.provider.kind, ProviderKind::Pi);
    assert_eq!(config.pi.model, "openai/example");
    assert_eq!(config.privacy.remote_context_byte_limit, 4096);
    assert!(
        config
            .privacy
            .excluded_globs
            .contains(&"**/private/**".to_owned())
    );
    assert_eq!(config.trigger.mode, TriggerMode::IdleOrManual);
    assert_eq!(config.trigger.idle_delay_ms, 500);
    assert_eq!(config.trigger.min_prefix_chars, 8);
    assert!(!config.context.include_open_files);
    assert!(!config.context.include_workspace_symbols);
}
