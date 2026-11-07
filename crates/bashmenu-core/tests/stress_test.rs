//! Comprehensive stress tests for `bashmenu-core` logic, macro engine,
//! configuration serialization, and notebook 3-way conflict resolution.

use bashmenu_core::config::BashMenuConfig;
use bashmenu_core::macro_engine::{
    clear_command_cache, extract_toasts, interpolate_placeholders, MacroContext,
};
use bashmenu_core::menu::{MenuItem, MenuItemType, MenuSchema};
use bashmenu_core::notebook::{merge_note_content, NoteSwapFile};
use std::fs;
use std::sync::Arc;
use std::thread;

#[test]
fn test_stress_macro_engine_deep_nesting() {
    let config = BashMenuConfig::default();
    let ctx = MacroContext::new(&config);

    // Deeply nested curly braces: 100 levels
    let mut nested = String::new();
    for _ in 0..100 {
        nested.push('{');
    }
    nested.push_str("home");
    for _ in 0..100 {
        nested.push('}');
    }

    // Must not panic or stack overflow
    let result = interpolate_placeholders(&nested, &ctx);
    assert!(!result.is_empty());
}

#[test]
fn test_stress_macro_engine_massive_string() {
    let mut config = BashMenuConfig::default();
    config.user.insert(
        "token".to_string(),
        serde_yml::Value::String("TEST_VALUE".to_string()),
    );
    let ctx = MacroContext::new(&config);

    // 1 MB string with repetitive placeholders
    let chunk = "Text segment with {user.token} and {home} and {dev_dir}. ";
    let massive_input = chunk.repeat(10_000);
    assert!(massive_input.len() > 500_000);

    let output = interpolate_placeholders(&massive_input, &ctx);
    assert!(!output.contains("{user.token}"));
    assert!(output.contains("TEST_VALUE"));
}

#[test]
fn test_stress_macro_engine_malformed_and_binary() {
    let config = BashMenuConfig::default();
    let ctx = MacroContext::new(&config);

    let malformed_inputs = vec![
        "",
        "{",
        "}",
        "}{",
        "{{{",
        "}}}",
        "{user.",
        "{env:}",
        "{command:}",
        "{toast:}",
        "{toast:abc:}",
        "{nf:}",
        "{nf:a:b:c:d:e}",
        "{ascii:}",
        "{ascii:99999999999999999999999999999}",
        "{ascii:-1}",
        "\x00\x01\x02\x03{home}\x00",
        "{command:echo 'hello' && invalid_syntax ;;;}",
        "{{{{{{{{{{user.unknown}}}}}}}}}}",
    ];

    for input in malformed_inputs {
        // Must never panic on any malformed or binary inputs
        let out = interpolate_placeholders(input, &ctx);
        let _ = extract_toasts(input, &ctx);
        let _ = out.len();
    }
}

#[test]
fn test_stress_macro_engine_concurrent_threads() {
    let config = Arc::new(BashMenuConfig::default());
    let mut handles = vec![];

    for t in 0..20 {
        let cfg = Arc::clone(&config);
        handles.push(thread::spawn(move || {
            let ctx = MacroContext::new(&cfg);
            for i in 0..500 {
                let s = format!(
                    "Thread {} iter {} {{home}} {{scripts_dir}} {{date_time_24}}",
                    t, i
                );
                let res = interpolate_placeholders(&s, &ctx);
                assert!(!res.contains("{home}"));
            }
        }));
    }

    for h in handles {
        h.join().expect("Thread should not panic");
    }
}

#[test]
fn test_stress_command_cache_and_clearing() {
    let config = BashMenuConfig::default();
    let ctx = MacroContext::new(&config);

    // Run command substitution
    let res1 = interpolate_placeholders("{command:echo 'cache_stress'}", &ctx);
    assert_eq!(res1.trim(), "cache_stress");

    // Second run hits cache
    let res2 = interpolate_placeholders("{command:echo 'cache_stress'}", &ctx);
    assert_eq!(res2.trim(), "cache_stress");

    // Clear cache
    clear_command_cache();
    let res3 = interpolate_placeholders("{command:echo 'cache_stress'}", &ctx);
    assert_eq!(res3.trim(), "cache_stress");
}

#[test]
fn test_stress_notebook_3way_merge_large_content() {
    // Generate 2,000 lines of base content
    let mut base_lines = Vec::new();
    for i in 0..2000 {
        base_lines.push(format!("Line #{}: Original base content here.", i));
    }
    let base = base_lines.join("\n");

    // User modified lines 10..20
    let mut local_lines = base_lines.clone();
    for (i, line) in local_lines.iter_mut().enumerate().take(20).skip(10) {
        *line = format!("Line #{}: Modified by user locally.", i);
    }
    let local = local_lines.join("\n");

    // Agent / remote modified lines 1000..1010
    let mut remote_lines = base_lines.clone();
    for (i, line) in remote_lines.iter_mut().enumerate().take(1010).skip(1000) {
        *line = format!("Line #{}: Modified by remote agent.", i);
    }
    let remote = remote_lines.join("\n");

    // Merge should cleanly succeed with 0 conflicts
    let res = merge_note_content(&base, &remote, &local);
    assert!(
        !res.has_conflicts,
        "Non-overlapping edits must merge cleanly"
    );
    assert!(res.content.contains("Modified by user locally."));
    assert!(res.content.contains("Modified by remote agent."));

    // Now introduce an overlapping conflict at line 500
    let mut conflict_local = base_lines.clone();
    conflict_local[500] = "Line #500: User conflicting modification.".to_string();
    let mut conflict_remote = base_lines.clone();
    conflict_remote[500] = "Line #500: Remote conflicting modification.".to_string();

    let conflict_res = merge_note_content(
        &base,
        &conflict_remote.join("\n"),
        &conflict_local.join("\n"),
    );
    assert!(
        conflict_res.has_conflicts,
        "Overlapping edits must flag conflict"
    );
    assert!(conflict_res.content.contains("<<<<<<< Local Edits"));
    assert!(conflict_res.content.contains(">>>>>>> Disk Version"));
}

#[test]
fn test_stress_notebook_swap_file_lifecycle() {
    let tmp_dir = std::env::temp_dir().join(format!("bm_stress_swap_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);
    let nb_path = tmp_dir.join("test_nb.yml");

    // Rapid write of swap file
    for i in 0..100 {
        let content = format!("Draft content iteration #{}", i);
        let created = NoteSwapFile::create(&nb_path, "StressNote", &content);
        assert!(created.is_some());
        let swp = NoteSwapFile::load(&nb_path, "StressNote");
        assert!(swp.is_some());
        let loaded = swp.unwrap();
        assert_eq!(loaded.base_content, content);
    }

    // Clean swap file
    NoteSwapFile::remove(&nb_path, "StressNote");
    assert!(NoteSwapFile::load(&nb_path, "StressNote").is_none());

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_stress_menu_schema_scale() {
    let mut schema = MenuSchema {
        version: "1.0".to_string(),
        title: "Scale Stress Test Banner".to_string(),
        items: Vec::new(),
        extra: Default::default(),
    };

    // Populate with 2,000 menu items across various types
    for i in 0..2000 {
        let mut item = MenuItem {
            label: format!("Item #{:04}", i),
            command: Some(format!("echo 'item {}'", i)),
            icon: Some("{nf::#f015}".to_string()),
            ..Default::default()
        };
        if i % 5 == 0 {
            item.r#type = MenuItemType::Radio;
            item.action = Some("settings.theme".to_string());
        } else if i % 7 == 0 {
            item.r#type = MenuItemType::Divider;
        }
        schema.items.push(item);
    }

    // Serialize to YAML
    let yml_str = serde_yml::to_string(&schema).expect("Serialization must succeed");
    assert!(yml_str.len() > 50_000);

    // Deserialize back
    let roundtrip: MenuSchema =
        serde_yml::from_str(&yml_str).expect("Deserialization must succeed");
    assert_eq!(roundtrip.items.len(), 2000);
}
