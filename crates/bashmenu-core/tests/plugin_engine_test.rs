//! Unit and integration tests for the PluginEngine.

use bashmenu_core::config::{BashMenuConfig, Settings};
use bashmenu_core::plugin::PluginEngine;
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn test_plugin_sanitize_lines() {
    let raw = "Line 1\twith\ttab\r\n\0Line 2 with \u{fe0f}selector\r\n\n\r\n";
    let sanitized = PluginEngine::sanitize_lines(raw);
    assert_eq!(sanitized.len(), 2);
    assert_eq!(sanitized[0], "Line 1    with    tab");
    assert_eq!(sanitized[1], "Line 2 with selector");
}

#[test]
fn test_parse_plugins_from_config() {
    let yaml_str = r#"
type: table
width: 100%
entries: 4
divider: true
weather:
  script: "curl -s 'wttr.in/49079?format=4'"
  sleep: 10000
alienware:
  sleep: 15000
"#;
    let plugins_val: serde_yml::Value = serde_yml::from_str(yaml_str).unwrap();
    let config = BashMenuConfig {
        version: "1.0".to_string(),
        theme: "dracula".to_string(),
        user: Default::default(),
        settings: Settings {
            plugins: Some(plugins_val),
            ..Default::default()
        },
        extra: Default::default(),
    };

    let parsed = PluginEngine::parse_plugins(&config);
    assert_eq!(parsed.layout.layout_type, "table");
    assert_eq!(parsed.layout.entries, 4);
    assert_eq!(parsed.layout.divider, Some("true".to_string()));
    assert_eq!(parsed.order, vec!["weather", "alienware"]);

    let weather = parsed.plugins.get("weather").unwrap();
    assert_eq!(
        weather.script.as_deref(),
        Some("curl -s 'wttr.in/49079?format=4'")
    );
    assert_eq!(weather.sleep, 10000);

    let alienware = parsed.plugins.get("alienware").unwrap();
    assert_eq!(alienware.script, None);
    assert_eq!(alienware.sleep, 15000);
}

#[test]
fn test_fetch_plugin_sync_echo() {
    let lines = PluginEngine::fetch_plugin_sync("echo 'Hello World'", None, 80);
    assert_eq!(lines, vec!["Hello World"]);
}

#[test]
fn test_plugin_cache_and_table_formatting() {
    let engine = PluginEngine::new();
    engine.set_cached("weather", vec!["72°F Sunny".to_string()]);
    engine.set_cached("system", vec!["CPU: 12%".to_string()]);

    let mut plugins_map = serde_yml::Mapping::new();
    plugins_map.insert(
        serde_yml::Value::String("type".to_string()),
        serde_yml::Value::String("table".to_string()),
    );
    plugins_map.insert(
        serde_yml::Value::String("entries".to_string()),
        serde_yml::Value::Number(serde_yml::Number::from(2)),
    );

    let mut w_map = serde_yml::Mapping::new();
    w_map.insert(
        serde_yml::Value::String("script".to_string()),
        serde_yml::Value::String("dummy".to_string()),
    );
    plugins_map.insert(
        serde_yml::Value::String("weather".to_string()),
        serde_yml::Value::Mapping(w_map),
    );

    let mut s_map = serde_yml::Mapping::new();
    s_map.insert(
        serde_yml::Value::String("script".to_string()),
        serde_yml::Value::String("dummy".to_string()),
    );
    plugins_map.insert(
        serde_yml::Value::String("system".to_string()),
        serde_yml::Value::Mapping(s_map),
    );

    let config = BashMenuConfig {
        version: "1.0".to_string(),
        theme: "dracula".to_string(),
        user: Default::default(),
        settings: Settings {
            plugins: Some(serde_yml::Value::Mapping(plugins_map)),
            ..Default::default()
        },
        extra: Default::default(),
    };

    let rows = engine.format_display_rows(&config, 40, '═');
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].is_divider);
    // Both entries should appear formatted into the row
    assert!(rows[0].text.contains("72°F Sunny"));
    assert!(rows[0].text.contains("CPU: 12%"));
}

#[test]
fn test_plugin_background_trigger_and_mpsc_notification() {
    let (tx, rx) = mpsc::channel();
    let engine = PluginEngine::new().with_sender(tx);

    let mut plugins_map = serde_yml::Mapping::new();
    let mut echo_map = serde_yml::Mapping::new();
    echo_map.insert(
        serde_yml::Value::String("script".to_string()),
        serde_yml::Value::String("echo 'Async Output'".to_string()),
    );
    echo_map.insert(
        serde_yml::Value::String("sleep".to_string()),
        serde_yml::Value::Number(serde_yml::Number::from(10)),
    );
    plugins_map.insert(
        serde_yml::Value::String("async_plugin".to_string()),
        serde_yml::Value::Mapping(echo_map),
    );

    let config = BashMenuConfig {
        version: "1.0".to_string(),
        theme: "dracula".to_string(),
        user: Default::default(),
        settings: Settings {
            plugins: Some(serde_yml::Value::Mapping(plugins_map)),
            ..Default::default()
        },
        extra: Default::default(),
    };

    engine.trigger_refresh(&config, true);

    // Wait for the background worker to deliver completion notification
    let updated_name = rx
        .recv_timeout(Duration::from_secs(3))
        .expect("Plugin did not complete within 3 seconds");
    assert_eq!(updated_name, "async_plugin");

    let cached = engine.get_cached("async_plugin");
    assert_eq!(cached, Some(vec!["Async Output".to_string()]));
}
