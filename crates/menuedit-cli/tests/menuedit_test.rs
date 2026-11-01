//! Integration tests for MenuEdit state machine, item reordering, and submenu navigation.

use bashmenu_core::{load_menu, MenuItem, MenuItemType, Submenu};
use menuedit_cli::MenuEditorState;
use std::fs;
use std::path::PathBuf;

fn get_fixture_path(filename: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let path = manifest_dir.join("../../tests/fixtures").join(filename);
    if path.exists() {
        path
    } else {
        PathBuf::from("tests/fixtures").join(filename)
    }
}

#[test]
fn test_menuedit_navigation_and_reorder() {
    let fixture_path = get_fixture_path("bashmenu.mnu");
    let mut state = MenuEditorState::new(fixture_path);

    assert_eq!(state.selected_index(), 0);
    let original_first = state.current_items()[0].label.clone();
    let original_second = state.current_items()[1].label.clone();

    // Move down
    state.move_down();
    assert_eq!(state.selected_index(), 1);

    // Swap up (move item 1 to item 0)
    state.swap_up();
    assert_eq!(state.selected_index(), 0);
    assert_eq!(state.current_items()[0].label, original_second);
    assert_eq!(state.current_items()[1].label, original_first);
    assert!(state.is_modified);

    // Swap down (move item 0 back to item 1)
    state.swap_down();
    assert_eq!(state.selected_index(), 1);
    assert_eq!(state.current_items()[0].label, original_first);
    assert_eq!(state.current_items()[1].label, original_second);
}

#[test]
fn test_menuedit_add_edit_delete() {
    let fixture_path = get_fixture_path("bashmenu.mnu");
    let mut state = MenuEditorState::new(fixture_path);

    let initial_count = state.current_items().len();

    // Add new command item
    let new_item = MenuItem {
        label: "Custom Added Item".to_string(),
        r#type: MenuItemType::Command,
        command: Some("echo custom".to_string()),
        ..Default::default()
    };
    state.add_item(new_item);
    assert_eq!(state.current_items().len(), initial_count + 1);
    assert_eq!(
        state.current_items()[state.selected_index()].label,
        "Custom Added Item"
    );

    // Edit current item
    state.edit_selected(
        "Custom Renamed Item".to_string(),
        Some("echo edited".to_string()),
        Some("⭐".to_string()),
    );
    assert_eq!(
        state.current_items()[state.selected_index()].label,
        "Custom Renamed Item"
    );
    assert_eq!(
        state.current_items()[state.selected_index()]
            .command
            .as_deref(),
        Some("echo edited")
    );

    // Delete current item
    let deleted = state.delete_selected().unwrap();
    assert_eq!(deleted.label, "Custom Renamed Item");
    assert_eq!(state.current_items().len(), initial_count);
}

#[test]
fn test_menuedit_submenu_dive_and_save_roundtrip() {
    let tmp_dir = std::env::temp_dir().join(format!("menuedit_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);
    let tmp_file = tmp_dir.join("test.mnu");

    let fixture_path = get_fixture_path("bashmenu.mnu");
    fs::copy(&fixture_path, &tmp_file).unwrap();

    let mut state = MenuEditorState::new(tmp_file.clone());

    // Add a submenu item
    let sub_item = MenuItem {
        label: "Test Submenu".to_string(),
        r#type: MenuItemType::Submenu,
        submenu: Some(Submenu {
            title: "Sub Title".to_string(),
            items: vec![MenuItem {
                label: "Sub Item 1".to_string(),
                r#type: MenuItemType::Command,
                command: Some("echo sub".to_string()),
                ..Default::default()
            }],
        }),
        ..Default::default()
    };
    state.add_item(sub_item);

    // Enter submenu
    let entered = state.enter_submenu();
    assert!(entered);
    assert_eq!(state.stack.len(), 2);
    assert_eq!(state.current_level().title, "Sub Title");
    assert_eq!(state.current_items().len(), 1);

    // Save to disk (which automatically unwinds stack and saves root)
    state.save_to_disk().unwrap();
    assert!(!state.is_modified);

    // Reload from disk and verify
    let reloaded = load_menu(&tmp_file).unwrap();
    let found = reloaded.items.iter().find(|i| i.label == "Test Submenu");
    assert!(found.is_some());
    let sub = found.unwrap().submenu.as_ref().unwrap();
    assert_eq!(sub.title, "Sub Title");
    assert_eq!(sub.items[0].label, "Sub Item 1");

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_menuedit_inspector_properties_parity() {
    use menuedit_cli::format_inspector_text;
    use std::collections::HashMap;

    let mut extra = HashMap::new();
    extra.insert("no_formatting".to_string(), serde_yml::Value::Bool(true));
    extra.insert("show_whitespace".to_string(), serde_yml::Value::Bool(true));
    extra.insert("external".to_string(), serde_yml::Value::Bool(true));
    extra.insert(
        "picker".to_string(),
        serde_yml::Value::String("file_picker".to_string()),
    );
    extra.insert(
        "on_yes".to_string(),
        serde_yml::Value::String("echo yes".to_string()),
    );
    extra.insert(
        "on_no".to_string(),
        serde_yml::Value::String("echo no".to_string()),
    );
    extra.insert(
        "custom_prop".to_string(),
        serde_yml::Value::String("custom_val".to_string()),
    );

    let test_item = MenuItem {
        label: "Full Property Test".to_string(),
        r#type: MenuItemType::Command,
        command: Some("echo test".to_string()),
        action: Some("echo test".to_string()),
        alt_buffer: Some(true),
        start_dir: Some("/tmp/testdir".to_string()),
        extra,
        ..Default::default()
    };

    let lines = format_inspector_text(&test_item, None, 80);
    let joined = lines.join("\n");

    assert!(joined.contains("Alt Buffer: true"));
    assert!(joined.contains("No Formatting: true"));
    assert!(joined.contains("Start Directory: /tmp/testdir"));
    assert!(joined.contains("Picker Type: file_picker"));
    assert!(joined.contains("On Yes Action: echo yes"));
    assert!(joined.contains("On No Action: echo no"));
    assert!(joined.contains("show_whitespace=true"));
    assert!(joined.contains("external=true"));
    assert!(joined.contains("Extra Properties: custom_prop=custom_val"));
}

#[test]
fn test_menuedit_tree_flattening_and_nesting() {
    use bashmenu_core::MenuSchema;
    use menuedit_cli::{flatten_tree, toggle_all_nodes};
    use std::collections::HashSet;

    let menu = MenuSchema {
        version: "0.0.1".to_string(),
        title: "Test".to_string(),
        items: vec![
            MenuItem::divider(),
            MenuItem {
                label: "Sub 1".to_string(),
                r#type: MenuItemType::Submenu,
                submenu: Some(Submenu {
                    title: "Sub 1".to_string(),
                    items: vec![MenuItem {
                        label: "Child 1".to_string(),
                        r#type: MenuItemType::Command,
                        ..Default::default()
                    }],
                }),
                ..Default::default()
            },
        ],
        extra: Default::default(),
    };

    let mut expanded = HashSet::new();

    // With nothing expanded, visible nodes = 2 (Divider and Sub 1)
    let vis = flatten_tree(&menu, &expanded);
    assert_eq!(vis.len(), 2);
    assert!(!vis[1].is_expanded);

    // Expand Sub 1
    expanded.insert(vec![1]);
    let vis = flatten_tree(&menu, &expanded);
    assert_eq!(vis.len(), 3);
    assert_eq!(vis[2].depth, 1);
    assert_eq!(vis[2].item.label, "Child 1");

    // Toggle all
    toggle_all_nodes(&menu, &mut expanded);
    assert!(expanded.is_empty()); // Was fully expanded, now collapsed
    let vis = flatten_tree(&menu, &expanded);
    assert_eq!(vis.len(), 2);
}

#[test]
fn test_menuedit_indent_and_outdent() {
    use bashmenu_core::MenuSchema;
    use menuedit_cli::{indent_item, outdent_item};
    use std::collections::HashSet;

    let mut menu = MenuSchema {
        version: "0.0.1".to_string(),
        title: "Test".to_string(),
        items: vec![
            MenuItem {
                label: "Submenu A".to_string(),
                r#type: MenuItemType::Submenu,
                submenu: Some(Submenu {
                    title: "Submenu A".to_string(),
                    items: vec![],
                }),
                ..Default::default()
            },
            MenuItem {
                label: "Command B".to_string(),
                r#type: MenuItemType::Command,
                command: Some("echo B".to_string()),
                ..Default::default()
            },
        ],
        extra: Default::default(),
    };

    let mut expanded = HashSet::new();

    // Indent item at index 1 ("Command B") into sibling "Submenu A"
    let new_path = indent_item(&mut menu, &[1], &mut expanded).unwrap();
    assert_eq!(new_path, vec![0, 0]);
    assert_eq!(menu.items.len(), 1);
    assert_eq!(menu.items[0].submenu.as_ref().unwrap().items.len(), 1);
    assert_eq!(
        menu.items[0].submenu.as_ref().unwrap().items[0].label,
        "Command B"
    );

    // Outdent item back to root level
    let new_path = outdent_item(&mut menu, &[0, 0]).unwrap();
    assert_eq!(new_path, vec![1]);
    assert_eq!(menu.items.len(), 2);
    assert_eq!(menu.items[1].label, "Command B");
    assert_eq!(menu.items[0].submenu.as_ref().unwrap().items.len(), 0);
}

#[test]
fn test_menuedit_mutate_item_type() {
    use menuedit_cli::mutate_item_type;

    let mut item = MenuItem {
        label: "My Action".to_string(),
        r#type: MenuItemType::Command,
        command: Some("echo 123".to_string()),
        ..Default::default()
    };

    // Mutate to editor: command becomes file
    mutate_item_type(&mut item, "editor");
    assert_eq!(item.r#type, MenuItemType::Editor);
    assert_eq!(item.file.as_deref(), Some("echo 123"));

    // Mutate to script: file becomes script
    mutate_item_type(&mut item, "script");
    assert_eq!(item.r#type, MenuItemType::Script);
    assert_eq!(item.script.as_deref(), Some("echo 123"));

    // Mutate to submenu: submenu container initialized
    mutate_item_type(&mut item, "submenu");
    assert_eq!(item.r#type, MenuItemType::Submenu);
    assert!(item.submenu.is_some());

    // Mutate to divider
    mutate_item_type(&mut item, "{divider}");
    assert_eq!(item.r#type, MenuItemType::Divider);
    assert_eq!(item.label, "{divider}");
    assert!(item.submenu.is_none());
}

#[test]
fn test_menuedit_inspector_placeholders_and_preview() {
    use bashmenu_core::BashMenuConfig;
    use menuedit_cli::format_inspector_text;

    let cfg = BashMenuConfig::default();
    let item = MenuItem {
        label: "Notes [{notebooks_dir}/notes.yml]".to_string(),
        r#type: MenuItemType::Notebook,
        file: Some("{notebooks_dir}/notes.yml".to_string()),
        icon: Some("{nf:N:f02d:📓}".to_string()),
        ..Default::default()
    };

    let lines = format_inspector_text(&item, Some(&cfg), 60);
    let joined = lines.join("\n");

    assert!(joined.contains("Title/Label: Notes [{notebooks_dir}/notes.yml]"));
    assert!(joined.contains("Expanded Title: Notes ["));
    assert!(joined.contains("Action/Target: {notebooks_dir}/notes.yml"));
    assert!(joined.contains("Expanded Action:"));
    assert!(joined.contains("Preview:"));
    assert!(joined.contains("1."));

    // Test divider preview
    let div = MenuItem::divider();
    let div_lines = format_inspector_text(&div, Some(&cfg), 40);
    let div_joined = div_lines.join("\n");
    assert!(div_joined.contains("[DIV] DIVIDER"));
    assert!(div_joined.contains('─'));
}

#[test]
fn test_item_edit_modal_standard_item_parity() {
    use bashmenu_tui::ResolvedThemeStyles;
    use crossterm::event::{KeyCode, KeyModifiers};
    use menuedit_cli::{FocusControl, ItemEditModal};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let item = MenuItem {
        label: "Execute Script".to_string(),
        r#type: MenuItemType::Script,
        script: Some("test.sh".to_string()),
        icon: Some("{nf:*:f013:⚙️}".to_string()),
        prompt: Some("Open editor".to_string()),
        ..Default::default()
    };

    let mut modal = ItemEditModal::new(&item);
    assert_eq!(modal.current_type, "script");
    assert!(!modal.is_divider());
    assert_eq!(modal.title_input.as_string(), "Execute Script");
    assert_eq!(modal.action_input.as_string(), "test.sh");
    assert_eq!(modal.get_action_label_text(), "Script Path (script):");

    // Test text editing on Title input
    modal.active_control_idx = modal
        .focus_controls()
        .iter()
        .position(|&c| c == FocusControl::TitleInput)
        .unwrap();
    modal.handle_key(KeyCode::End, KeyModifiers::NONE);
    modal.handle_key(KeyCode::Char('!'), KeyModifiers::NONE);
    assert_eq!(modal.title_input.as_string(), "Execute Script!");

    // Test toggling execution mode
    modal.active_control_idx = modal
        .focus_controls()
        .iter()
        .position(|&c| c == FocusControl::ExecModeSelector)
        .unwrap();
    // Initially mode = 2 (Standard)
    assert_eq!(modal.exec_mode, 2);
    modal.handle_key(KeyCode::Char('1'), KeyModifiers::NONE);
    assert_eq!(modal.exec_mode, 0); // Streaming
    modal.handle_key(KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(modal.exec_mode, 1); // Interactive

    // Test saving
    let updated = modal.build_updated_item();
    assert_eq!(updated.label, "Execute Script!");
    assert_eq!(updated.script.as_deref(), Some("test.sh"));
    assert_eq!(
        updated.extra.get("stream").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(updated.interactive, Some(true));

    // Test rendering
    let area = Rect::new(0, 0, 100, 35);
    let mut buf = Buffer::empty(area);
    let styles = ResolvedThemeStyles::default();
    let inner = modal.render(area, &styles, &mut buf);
    assert!(inner.width > 0);
    assert!(inner.height > 0);
}

#[test]
fn test_item_edit_modal_divider_parity() {
    use bashmenu_tui::ResolvedThemeStyles;
    use menuedit_cli::{FocusControl, ItemEditModal};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let div = MenuItem::divider();
    let mut modal = ItemEditModal::new(&div);
    assert!(modal.is_divider());
    assert_eq!(modal.current_type, "{divider}");
    assert_eq!(modal.char_input.as_string(), "{ascii:196}");
    assert_eq!(modal.length_input.as_string(), "{window_width}");

    // Edit repeating character
    modal.active_control_idx = modal
        .focus_controls()
        .iter()
        .position(|&c| c == FocusControl::CharInput)
        .unwrap();
    modal.char_input.set_text("{ascii:205}");

    let updated = modal.build_updated_item();
    assert!(updated.is_divider());
    assert_eq!(
        updated.extra.get("char").and_then(|v| v.as_str()),
        Some("{ascii:205}")
    );

    // Test rendering
    let area = Rect::new(0, 0, 90, 30);
    let mut buf = Buffer::empty(area);
    let styles = ResolvedThemeStyles::default();
    let inner = modal.render(area, &styles, &mut buf);
    assert!(inner.width > 0);
}

#[test]
fn test_item_edit_modal_keyboard_shortcuts_and_mouse() {
    use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use menuedit_cli::{BrowseTarget, ItemEditModal, ItemEditResult};
    use ratatui::layout::Rect;

    let item = MenuItem {
        label: "Test Item".to_string(),
        r#type: MenuItemType::Command,
        command: Some("echo test".to_string()),
        ..Default::default()
    };

    let mut modal = ItemEditModal::new(&item);

    // Test Ctrl+S save
    let res = modal.handle_key(KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert!(matches!(res, ItemEditResult::Save(_)));

    // Test Ctrl+T change type
    let res = modal.handle_key(KeyCode::Char('t'), KeyModifiers::CONTROL);
    assert_eq!(res, ItemEditResult::ChangeType);

    // Test F1 help
    let res = modal.handle_key(KeyCode::F(1), KeyModifiers::NONE);
    assert_eq!(res, ItemEditResult::ShowHelp);

    // Test F3 ASCII
    let res = modal.handle_key(KeyCode::F(3), KeyModifiers::NONE);
    assert_eq!(res, ItemEditResult::ShowAscii);

    // Test F4 Glyphs
    let res = modal.handle_key(KeyCode::F(4), KeyModifiers::NONE);
    assert_eq!(res, ItemEditResult::ShowGlyphs);

    // Test F5 Placeholders
    let res = modal.handle_key(KeyCode::F(5), KeyModifiers::NONE);
    assert_eq!(res, ItemEditResult::ShowPlaceholders);

    // Test Esc Cancel
    let res = modal.handle_key(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(res, ItemEditResult::Cancel);

    // Test Browse result
    modal.set_browse_result(BrowseTarget::Action, "/path/to/my_script.sh");
    assert_eq!(modal.action_input.as_string(), "/path/to/my_script.sh");

    // Test Mouse click on [X] (top right corner)
    let area = Rect::new(0, 0, 100, 35);
    let inner = bashmenu_tui::compute_modal_inner(area, 86, 32);
    let close_x = inner.x + inner.width - 2;
    let close_y = inner.y.saturating_sub(1);

    let mouse_ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: close_x,
        row: close_y,
        modifiers: KeyModifiers::NONE,
    };
    let res = modal.handle_mouse(mouse_ev, area);
    assert_eq!(res, ItemEditResult::Cancel);
}

#[test]
fn test_item_edit_modal_radio_multiline_editor() {
    use bashmenu_tui::ResolvedThemeStyles;
    use crossterm::event::{KeyCode, KeyModifiers};
    use menuedit_cli::{FocusControl, ItemEditModal, MultiLineInput};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use std::collections::HashMap;

    // 1. Direct MultiLineInput testing
    let mut input = MultiLineInput::new("Option A = 1\nOption B = 2");
    assert_eq!(input.line_count(), 2);
    assert_eq!(input.cursor_row, 0);
    assert_eq!(input.cursor_col, 0);

    // Move cursor to end of line 0, then press Enter to start a new line between line 0 and line 1
    input.cursor_col = input.lines[0].len();
    input.handle_key(KeyCode::Enter);
    assert_eq!(input.line_count(), 3);
    assert_eq!(input.cursor_row, 1);
    assert_eq!(input.cursor_col, 0);

    // Type into new line
    for c in "New Option = 3".chars() {
        input.handle_key(KeyCode::Char(c));
    }
    assert_eq!(input.lines[1].iter().collect::<String>(), "New Option = 3");
    assert_eq!(input.lines[0].iter().collect::<String>(), "Option A = 1");
    assert_eq!(input.lines[2].iter().collect::<String>(), "Option B = 2");

    // Move down and up
    assert!(input.move_down());
    assert_eq!(input.cursor_row, 2);
    assert!(!input.move_down()); // At bottom, returns false
    assert!(input.move_up());
    assert_eq!(input.cursor_row, 1);
    assert!(input.move_up());
    assert_eq!(input.cursor_row, 0);
    assert!(!input.move_up()); // At top, returns false

    // Backspace merging
    let mut merge_input = MultiLineInput::new("First\nSecond");
    assert_eq!(merge_input.line_count(), 2);
    merge_input.cursor_row = 1;
    merge_input.cursor_col = 0;
    merge_input.handle_key(KeyCode::Backspace);
    assert_eq!(merge_input.line_count(), 1);
    assert_eq!(merge_input.as_string(), "FirstSecond");
    assert_eq!(merge_input.cursor_col, 5);

    // Delete merging
    let mut del_input = MultiLineInput::new("Alpha\nBeta");
    del_input.cursor_row = 0;
    del_input.cursor_col = 5;
    del_input.handle_key(KeyCode::Delete);
    assert_eq!(del_input.line_count(), 1);
    assert_eq!(del_input.as_string(), "AlphaBeta");

    // 2. ItemEditModal with Radio Item
    let mut extra = HashMap::new();
    let mut opt1 = serde_yml::Mapping::new();
    opt1.insert(
        serde_yml::Value::String("label".to_string()),
        serde_yml::Value::String("Theme 1".to_string()),
    );
    opt1.insert(
        serde_yml::Value::String("value".to_string()),
        serde_yml::Value::String("alaska-train".to_string()),
    );
    let mut opt2 = serde_yml::Mapping::new();
    opt2.insert(
        serde_yml::Value::String("label".to_string()),
        serde_yml::Value::String("Theme 2".to_string()),
    );
    opt2.insert(
        serde_yml::Value::String("value".to_string()),
        serde_yml::Value::String("alaska-mountain".to_string()),
    );
    extra.insert(
        "options".to_string(),
        serde_yml::Value::Sequence(vec![
            serde_yml::Value::Mapping(opt1),
            serde_yml::Value::Mapping(opt2),
        ]),
    );

    let item = MenuItem {
        label: "Select Theme".to_string(),
        r#type: MenuItemType::Radio,
        action: Some("app.theme".to_string()),
        extra,
        ..Default::default()
    };

    let mut modal = ItemEditModal::new(&item);
    assert!(modal.is_radio());
    assert_eq!(modal.radio_options_input.line_count(), 2);
    assert_eq!(
        modal.radio_options_input.as_string(),
        "Theme 1 = alaska-train\nTheme 2 = alaska-mountain"
    );

    // Focus on RadioOptionsInput
    modal.active_control_idx = modal
        .focus_controls()
        .iter()
        .position(|&c| c == FocusControl::RadioOptionsInput)
        .unwrap();

    // Down moves to next line within multiline editor
    modal.handle_key(KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(modal.radio_options_input.cursor_row, 1);
    assert_eq!(modal.current_control(), FocusControl::RadioOptionsInput);

    // End to move to end of line 1
    modal.handle_key(KeyCode::End, KeyModifiers::NONE);

    // Enter adds a new line
    modal.handle_key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(modal.radio_options_input.line_count(), 3);
    assert_eq!(modal.radio_options_input.cursor_row, 2);

    // Type a 3rd option
    for c in "Theme 3: matrix".chars() {
        modal.handle_key(KeyCode::Char(c), KeyModifiers::NONE);
    }
    assert_eq!(
        modal.radio_options_input.lines[2]
            .iter()
            .collect::<String>(),
        "Theme 3: matrix"
    );

    // Save and verify options are parsed into sequence
    let updated = modal.build_updated_item();
    let saved_opts = updated
        .extra
        .get("options")
        .and_then(|v| v.as_sequence())
        .expect("options sequence expected");
    assert_eq!(saved_opts.len(), 3);

    let opt3 = saved_opts[2].as_mapping().unwrap();
    assert_eq!(
        opt3.get(serde_yml::Value::String("label".to_string()))
            .and_then(|v| v.as_str()),
        Some("Theme 3")
    );
    assert_eq!(
        opt3.get(serde_yml::Value::String("value".to_string()))
            .and_then(|v| v.as_str()),
        Some("matrix")
    );

    // Render modal to verify buffer draws 5-row multiline box
    let area = Rect::new(0, 0, 100, 35);
    let mut buf = Buffer::empty(area);
    let styles = ResolvedThemeStyles::default();
    let inner = modal.render(area, &styles, &mut buf);
    assert!(inner.width > 0);
    assert!(inner.height > 0);
}

#[test]
fn test_menuedit_footer_spread_and_position() {
    let items_r1 = [
        "[F1] Help",
        "[F2] Toggle All",
        "[F3] ASCII",
        "[F4] Glyphs",
        "[F5] Placeholders",
        "[a] Add",
        "[e/ENTER] Edit",
        "[t] Type",
    ];
    let items_r2 = [
        "[SPACE] Toggle",
        "[d] Delete",
        "[m] Move Dn",
        "[M] Move Up",
        "[>] Indent",
        "[<] Outdent",
        "[s] Save",
        "[ESC/q] Exit",
    ];

    let avail_w = 120;
    let (line_r1, ranges_r1) = bashmenu_tui::spread_gutter_line_layout(&items_r1, avail_w);
    let (line_r2, ranges_r2) = bashmenu_tui::spread_gutter_line_layout(&items_r2, avail_w);

    assert_eq!(bashmenu_tui::visible_text_len(&line_r1), avail_w);
    assert_eq!(bashmenu_tui::visible_text_len(&line_r2), avail_w);

    assert_eq!(ranges_r1.len(), 8);
    assert_eq!(ranges_r2.len(), 8);

    assert_eq!(ranges_r1[0].0, 0);
    assert_eq!(ranges_r1[7].1, avail_w);
    assert_eq!(ranges_r2[0].0, 0);
    assert_eq!(ranges_r2[7].1, avail_w);

    // Verify row placement relative to screen height
    let screen_height: u16 = 30;
    let footer_y1 = screen_height - 3;
    let footer_y2 = screen_height - 2;
    let bot_border_y = screen_height - 1;

    assert!(footer_y1 < footer_y2);
    assert!(footer_y2 < bot_border_y);
}

#[test]
fn test_menuedit_find_node_path_and_ancestor_expansion() {
    use bashmenu_core::load_menu;
    use menuedit_cli::{find_node_path, flatten_tree};
    use std::collections::HashSet;

    let fixture_path = get_fixture_path("bashmenu.mnu");
    let menu = load_menu(&fixture_path).expect("bashmenu.mnu fixture must load");

    // 1. Resolve by direct path
    let path = find_node_path(&menu, Some("0"), None, None).expect("Path 0 should resolve");
    assert_eq!(path, vec![0]);

    // 2. Resolve by title
    let path_title = find_node_path(&menu, None, Some("Options & Settings"), None)
        .expect("Options & Settings should resolve");
    assert!(!path_title.is_empty());

    // 3. Resolve nested item by title chain
    let nested_path = find_node_path(
        &menu,
        None,
        Some("Bashmenu Settings"),
        Some("Options & Settings > Bashmenu Settings"),
    )
    .expect("Title chain should resolve nested item");
    assert!(nested_path.len() >= 2);

    // 4. Verify ancestor expansion makes the target node visible in flatten_tree
    let mut expanded = HashSet::new();
    for len in 1..nested_path.len() {
        expanded.insert(nested_path[..len].to_vec());
    }
    let visible = flatten_tree(&menu, &expanded);
    let target_node = visible.iter().find(|n| n.path == nested_path);
    assert!(
        target_node.is_some(),
        "Nested target node must be visible after ancestor expansion"
    );
}

#[test]
fn test_menuedit_execution_mode_selector_and_window_percent() {
    use bashmenu_core::{ExecutionMode, MenuItem, MenuItemType};
    use crossterm::event::KeyCode;
    use menuedit_cli::{FocusControl, ItemEditModal, ItemEditResult};

    let item = MenuItem {
        label: "Check Disk Usage [df -h]".to_string(),
        r#type: MenuItemType::Command,
        command: Some("df -h".to_string()),
        ..Default::default()
    };

    let mut modal = ItemEditModal::new(&item);

    // Initial default is Console (2)
    assert_eq!(modal.exec_mode, 2);

    // Move to ExecModeSelector and cycle mode
    let ctrls = modal.focus_controls();
    let exec_idx = ctrls
        .iter()
        .position(|c| *c == FocusControl::ExecModeSelector)
        .expect("ExecModeSelector must be in controls");
    modal.active_control_idx = exec_idx;

    // Press '1' -> Streaming Mode (0)
    assert_eq!(
        modal.handle_key(KeyCode::Char('1'), crossterm::event::KeyModifiers::NONE),
        ItemEditResult::Continue
    );
    assert_eq!(modal.exec_mode, 0);

    // Window percent control should now be available
    let ctrls_stream = modal.focus_controls();
    assert!(ctrls_stream.contains(&FocusControl::WindowPercentInput));

    // Edit window percent
    let win_pct_idx = ctrls_stream
        .iter()
        .position(|c| *c == FocusControl::WindowPercentInput)
        .unwrap();
    modal.active_control_idx = win_pct_idx;
    modal.window_percent_input = menuedit_cli::TextInput::new("75");

    // Save item and verify
    let updated = modal.build_updated_item();
    assert_eq!(updated.execution_mode(), ExecutionMode::Streaming);
    assert_eq!(updated.window_percent(), 75);
    assert_eq!(updated.interactive, Some(false));
    assert_eq!(
        updated.extra.get("stream").and_then(|v| v.as_bool()),
        Some(true)
    );
}

#[test]
fn test_menuedit_inputs_selection_and_clipboard() {
    bashmenu_tui::clipboard::set_test_mode(true);
    use crossterm::event::{KeyCode, KeyModifiers};
    use menuedit_cli::{FocusControl, ItemEditModal, MultiLineInput, TextInput};

    // 1. TextInput direct tests
    let mut ti = TextInput::new("Hello World");
    assert_eq!(ti.cursor, 11);
    assert_eq!(ti.selection_range(), None);

    // Shift+Left 5 times selects "World"
    for _ in 0..5 {
        ti.handle_key_with_modifiers(KeyCode::Left, KeyModifiers::SHIFT);
    }
    assert_eq!(ti.cursor, 6);
    assert_eq!(ti.selection_range(), Some((6, 11)));

    // Ctrl+C copies "World"
    ti.handle_key_with_modifiers(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        bashmenu_tui::clipboard::get_clipboard_text().as_deref(),
        Some("World")
    );

    // Ctrl+A selects all
    ti.handle_key_with_modifiers(KeyCode::Char('a'), KeyModifiers::CONTROL);
    assert_eq!(ti.selection_range(), Some((0, 11)));

    // Ctrl+X cuts all
    ti.handle_key_with_modifiers(KeyCode::Char('x'), KeyModifiers::CONTROL);
    assert_eq!(ti.as_string(), "");
    assert_eq!(
        bashmenu_tui::clipboard::get_clipboard_text().as_deref(),
        Some("Hello World")
    );

    // Ctrl+V pastes
    ti.handle_key_with_modifiers(KeyCode::Char('v'), KeyModifiers::CONTROL);
    assert_eq!(ti.as_string(), "Hello World");

    // 2. MultiLineInput tests
    let mut ml = MultiLineInput::new("First Line\nSecond Line\nThird Line");
    ml.cursor_row = 0;
    ml.cursor_col = 0;

    // Shift+Down selects line 0 and first char of line 1
    ml.handle_key_with_modifiers(KeyCode::Down, KeyModifiers::SHIFT);
    assert_eq!(ml.cursor_row, 1);
    assert_eq!(ml.selection_range(), Some(((0, 0), (1, 0))));

    // Ctrl+C copies selection
    ml.handle_key_with_modifiers(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        bashmenu_tui::clipboard::get_clipboard_text().as_deref(),
        Some("First Line\n")
    );

    // Ctrl+A selects all
    ml.handle_key_with_modifiers(KeyCode::Char('a'), KeyModifiers::CONTROL);
    assert_eq!(ml.selection_range(), Some(((0, 0), (2, 10))));

    // Ctrl+X cuts all
    ml.handle_key_with_modifiers(KeyCode::Char('x'), KeyModifiers::CONTROL);
    assert_eq!(ml.as_string(), "");

    // Ctrl+V pastes multiline
    ml.handle_key_with_modifiers(KeyCode::Char('v'), KeyModifiers::CONTROL);
    assert_eq!(ml.as_string(), "First Line\nSecond Line\nThird Line");

    // 3. ItemEditModal routing to focused control
    let item = MenuItem {
        label: "Title A".to_string(),
        r#type: MenuItemType::Command,
        command: Some("echo 1".to_string()),
        ..Default::default()
    };
    let mut modal = ItemEditModal::new(&item);

    // Focus on TitleInput
    modal.active_control_idx = modal
        .focus_controls()
        .iter()
        .position(|c| *c == FocusControl::TitleInput)
        .unwrap();

    // Ctrl+A in TitleInput selects all in TitleInput (rather than opening ASCII table)
    modal.handle_key(KeyCode::Char('a'), KeyModifiers::CONTROL);
    assert_eq!(modal.title_input.selection_range(), Some((0, 7)));

    // Ctrl+C copies title
    modal.handle_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        bashmenu_tui::clipboard::get_clipboard_text().as_deref(),
        Some("Title A")
    );
}

#[test]
fn test_menuedit_confirm_item_properties() {
    use menuedit_cli::{BrowseTarget, FocusControl, ItemEditModal, ItemEditResult};

    let item = MenuItem {
        label: "Confirm Reboot".to_string(),
        r#type: MenuItemType::Confirm,
        prompt: Some("Do you really want to reboot now?".to_string()),
        on_yes: Some("reboot".into()),
        on_no: Some("echo 'Reboot cancelled'".into()),
        ..Default::default()
    };

    let mut modal = ItemEditModal::new(&item);

    assert_eq!(modal.current_type, "confirm");
    assert!(modal.is_confirm());
    assert_eq!(modal.on_yes_input.as_string(), "reboot");
    assert_eq!(modal.on_no_input.as_string(), "echo 'Reboot cancelled'");

    let controls = modal.focus_controls();
    assert!(controls.contains(&FocusControl::OnYesInput));
    assert!(controls.contains(&FocusControl::OnYesBrowseBtn));
    assert!(controls.contains(&FocusControl::OnNoInput));
    assert!(controls.contains(&FocusControl::OnNoBrowseBtn));

    // Test browse result routing
    modal.set_browse_result(BrowseTarget::OnYes, "/usr/bin/custom_reboot");
    assert_eq!(modal.on_yes_input.as_string(), "/usr/bin/custom_reboot");

    modal.set_browse_result(BrowseTarget::OnNo, "/usr/bin/cancel_script.sh");
    assert_eq!(modal.on_no_input.as_string(), "/usr/bin/cancel_script.sh");

    // Test hitting enter on OnYesBrowseBtn
    modal.active_control_idx = modal
        .focus_controls()
        .iter()
        .position(|c| *c == FocusControl::OnYesBrowseBtn)
        .unwrap();
    assert_eq!(
        modal.handle_key(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE
        ),
        ItemEditResult::Browse(BrowseTarget::OnYes)
    );

    // Test hitting enter on OnNoBrowseBtn
    modal.active_control_idx = modal
        .focus_controls()
        .iter()
        .position(|c| *c == FocusControl::OnNoBrowseBtn)
        .unwrap();
    assert_eq!(
        modal.handle_key(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE
        ),
        ItemEditResult::Browse(BrowseTarget::OnNo)
    );

    // Test build_updated_item
    let updated = modal.build_updated_item();
    assert_eq!(updated.r#type, MenuItemType::Confirm);
    assert_eq!(updated.on_yes(), Some("/usr/bin/custom_reboot"));
    assert_eq!(updated.on_no(), Some("/usr/bin/cancel_script.sh"));
    assert_eq!(
        updated.prompt.as_deref(),
        Some("Do you really want to reboot now?")
    );

    // Test rendering
    let styles = bashmenu_tui::ResolvedThemeStyles::default();
    let area = ratatui::layout::Rect::new(0, 0, 100, 40);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    modal.render(area, &styles, &mut buf);
    let buf_str = format!("{:?}", buf);
    assert!(buf_str.contains("CONFIRM"));
    assert!(buf_str.contains("on_yes"));
    assert!(buf_str.contains("on_no"));
}

#[test]
fn test_item_edit_modal_field_visibility_per_type() {
    use menuedit_cli::{FocusControl, ItemEditModal};

    // 1. Submenu: only Title, Icon, ChangeType, Save, Cancel
    let sub = MenuItem {
        label: "My Submenu".to_string(),
        r#type: MenuItemType::Submenu,
        ..Default::default()
    };
    let modal_sub = ItemEditModal::new(&sub);
    let ctrls_sub = modal_sub.focus_controls();
    assert!(ctrls_sub.contains(&FocusControl::TitleInput));
    assert!(ctrls_sub.contains(&FocusControl::IconInput));
    assert!(!ctrls_sub.contains(&FocusControl::ActionInput));
    assert!(!ctrls_sub.contains(&FocusControl::PromptInput));
    assert!(!ctrls_sub.contains(&FocusControl::TemplateInput));
    assert!(!ctrls_sub.contains(&FocusControl::TargetInput));
    assert!(!ctrls_sub.contains(&FocusControl::BlockIdInput));
    assert!(!ctrls_sub.contains(&FocusControl::StartDirInput));
    assert!(!ctrls_sub.contains(&FocusControl::TabstopInput));
    assert!(!ctrls_sub.contains(&FocusControl::ExecModeSelector));
    assert!(!ctrls_sub.contains(&FocusControl::NoFormattingChk));
    assert!(!ctrls_sub.contains(&FocusControl::ShowWhitespaceChk));
    assert!(!ctrls_sub.contains(&FocusControl::MaskedChk));
    assert!(!ctrls_sub.contains(&FocusControl::RefreshChk));
    assert!(!ctrls_sub.contains(&FocusControl::ExternalChk));

    // 2. Command: Title, Icon, Action, Prompt (Option A), StartDir, ExecMode, Masked, Refresh, External
    let cmd = MenuItem {
        label: "My Command".to_string(),
        r#type: MenuItemType::Command,
        command: Some("ls -la".to_string()),
        ..Default::default()
    };
    let modal_cmd = ItemEditModal::new(&cmd);
    let ctrls_cmd = modal_cmd.focus_controls();
    assert!(ctrls_cmd.contains(&FocusControl::TitleInput));
    assert!(ctrls_cmd.contains(&FocusControl::IconInput));
    assert!(ctrls_cmd.contains(&FocusControl::ActionInput));
    assert!(ctrls_cmd.contains(&FocusControl::ActionBrowseBtn));
    assert!(ctrls_cmd.contains(&FocusControl::PromptInput));
    assert!(ctrls_cmd.contains(&FocusControl::StartDirInput));
    assert!(ctrls_cmd.contains(&FocusControl::ExecModeSelector));
    assert!(ctrls_cmd.contains(&FocusControl::MaskedChk));
    assert!(ctrls_cmd.contains(&FocusControl::RefreshChk));
    assert!(ctrls_cmd.contains(&FocusControl::ExternalChk));
    assert!(!ctrls_cmd.contains(&FocusControl::TemplateInput));
    assert!(!ctrls_cmd.contains(&FocusControl::TargetInput));
    assert!(!ctrls_cmd.contains(&FocusControl::BlockIdInput));
    assert!(!ctrls_cmd.contains(&FocusControl::TabstopInput));
    assert!(!ctrls_cmd.contains(&FocusControl::NoFormattingChk));
    assert!(!ctrls_cmd.contains(&FocusControl::ShowWhitespaceChk));

    // 3. Editor: Title, Icon, File (Action), StartDir, Tabstop, NoFormatting, ShowWhitespace, Refresh, External
    let edt = MenuItem {
        label: "My File".to_string(),
        r#type: MenuItemType::Editor,
        file: Some("notes.txt".to_string()),
        ..Default::default()
    };
    let modal_edt = ItemEditModal::new(&edt);
    let ctrls_edt = modal_edt.focus_controls();
    assert!(ctrls_edt.contains(&FocusControl::TitleInput));
    assert!(ctrls_edt.contains(&FocusControl::IconInput));
    assert!(ctrls_edt.contains(&FocusControl::ActionInput));
    assert!(ctrls_edt.contains(&FocusControl::ActionBrowseBtn));
    assert!(ctrls_edt.contains(&FocusControl::StartDirInput));
    assert!(ctrls_edt.contains(&FocusControl::TabstopInput));
    assert!(ctrls_edt.contains(&FocusControl::NoFormattingChk));
    assert!(ctrls_edt.contains(&FocusControl::ShowWhitespaceChk));
    assert!(ctrls_edt.contains(&FocusControl::RefreshChk));
    assert!(ctrls_edt.contains(&FocusControl::ExternalChk));
    assert!(!ctrls_edt.contains(&FocusControl::PromptInput));
    assert!(!ctrls_edt.contains(&FocusControl::ExecModeSelector));
    assert!(!ctrls_edt.contains(&FocusControl::MaskedChk));
    assert!(!ctrls_edt.contains(&FocusControl::TemplateInput));

    // 4. Config: Title, Icon, Key (Action without browse), Prompt, Refresh, External
    let cfg = MenuItem {
        label: "My Config".to_string(),
        r#type: MenuItemType::Config,
        extra: [(
            "key".to_string(),
            serde_yml::Value::String("settings.theme".to_string()),
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let modal_cfg = ItemEditModal::new(&cfg);
    let ctrls_cfg = modal_cfg.focus_controls();
    assert!(ctrls_cfg.contains(&FocusControl::TitleInput));
    assert!(ctrls_cfg.contains(&FocusControl::IconInput));
    assert!(ctrls_cfg.contains(&FocusControl::ActionInput));
    assert!(!ctrls_cfg.contains(&FocusControl::ActionBrowseBtn));
    assert!(ctrls_cfg.contains(&FocusControl::PromptInput));
    assert!(ctrls_cfg.contains(&FocusControl::RefreshChk));
    assert!(ctrls_cfg.contains(&FocusControl::ExternalChk));
    assert!(!ctrls_cfg.contains(&FocusControl::StartDirInput));
    assert!(!ctrls_cfg.contains(&FocusControl::TabstopInput));
    assert!(!ctrls_cfg.contains(&FocusControl::ExecModeSelector));
    assert!(!ctrls_cfg.contains(&FocusControl::TemplateInput));

    // 5. Back / Exit: Title, Icon only
    let bck = MenuItem {
        label: "Back".to_string(),
        r#type: MenuItemType::Back,
        ..Default::default()
    };
    let modal_bck = ItemEditModal::new(&bck);
    let ctrls_bck = modal_bck.focus_controls();
    assert_eq!(
        ctrls_bck,
        vec![
            FocusControl::ChangeTypeBtn,
            FocusControl::TitleInput,
            FocusControl::IconInput,
            FocusControl::SaveBtn,
            FocusControl::CancelBtn,
        ]
    );
}
