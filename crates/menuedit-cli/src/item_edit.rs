//! Comprehensive modal editor for menu item properties and flags.

use bashmenu_core::{MenuItem, MenuItemType};
use bashmenu_tui::{render_modal_frame, ResolvedThemeStyles};
use crossterm::event::{KeyCode, KeyModifiers, MouseEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Field indices in the property edit modal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditField {
    Title = 0,
    Icon = 1,
    Action = 2,
    Prompt = 3,
    StartDir = 4,
    Tabstop = 5,
    AltBuffer = 6,
    NoFormatting = 7,
    Quiet = 8,
    Interactive = 9,
    ShowWhitespace = 10,
    External = 11,
}

const TOTAL_FIELDS: usize = 12;

/// Modal dialog for editing all schema properties of a menu item
#[derive(Debug, Clone)]
pub struct ItemEditModal {
    pub item: MenuItem,
    pub title_val: String,
    pub icon_val: String,
    pub action_val: String,
    pub prompt_val: String,
    pub start_dir_val: String,
    pub tabstop_val: String,
    pub alt_buffer: bool,
    pub no_formatting: bool,
    pub quiet: bool,
    pub interactive: bool,
    pub show_whitespace: bool,
    pub external: bool,
    pub active_field: usize,
    pub cursor_pos: usize,
}

impl ItemEditModal {
    pub fn new(item: &MenuItem) -> Self {
        let title_val = item.label.clone();
        let icon_val = item.icon.clone().unwrap_or_default();
        let action_val = item
            .action
            .clone()
            .or_else(|| item.command.clone())
            .or_else(|| item.script.clone())
            .or_else(|| item.file.clone())
            .unwrap_or_default();
        let prompt_val = item.prompt.clone().unwrap_or_default();
        let start_dir_val = item.start_dir.clone().unwrap_or_default();
        let tabstop_val = item
            .tabstop
            .map(|t| t.to_string())
            .unwrap_or_else(|| "4".to_string());
        let alt_buffer = item.alt_buffer.unwrap_or(true);
        let no_formatting = item
            .extra
            .get("no_formatting")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let quiet = item.quiet.unwrap_or(false);
        let interactive = item.interactive.unwrap_or(true);
        let show_whitespace = item
            .extra
            .get("show_whitespace")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let external = item
            .extra
            .get("external")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let cur_len = title_val.len();

        Self {
            item: item.clone(),
            title_val,
            icon_val,
            action_val,
            prompt_val,
            start_dir_val,
            tabstop_val,
            alt_buffer,
            no_formatting,
            quiet,
            interactive,
            show_whitespace,
            external,
            active_field: 0,
            cursor_pos: cur_len,
        }
    }

    /// Handles keyboard events. Returns `Some(Some(item))` on save, `Some(None)` on cancel, `None` to continue.
    pub fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> (Option<Option<MenuItem>>, bool) {
        // (Result, request_change_type)
        if modifiers.contains(KeyModifiers::CONTROL) {
            match code {
                KeyCode::Char('s') => return (Some(Some(self.build_updated_item())), false),
                KeyCode::Char('t') => return (None, true),
                _ => {}
            }
        }

        match code {
            KeyCode::F(2) => (Some(Some(self.build_updated_item())), false),
            KeyCode::Esc => (Some(None), false),
            KeyCode::Tab | KeyCode::Down => {
                self.active_field = (self.active_field + 1) % TOTAL_FIELDS;
                self.cursor_pos = self.current_field_len();
                (None, false)
            }
            KeyCode::BackTab | KeyCode::Up => {
                if self.active_field == 0 {
                    self.active_field = TOTAL_FIELDS - 1;
                } else {
                    self.active_field -= 1;
                }
                self.cursor_pos = self.current_field_len();
                (None, false)
            }
            KeyCode::Char(' ') if self.is_boolean_field() => {
                self.toggle_current_boolean();
                (None, false)
            }
            KeyCode::Enter => {
                if self.active_field == TOTAL_FIELDS - 1 {
                    (Some(Some(self.build_updated_item())), false)
                } else {
                    self.active_field = (self.active_field + 1) % TOTAL_FIELDS;
                    self.cursor_pos = self.current_field_len();
                    (None, false)
                }
            }
            KeyCode::Backspace => {
                self.handle_backspace();
                (None, false)
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                (None, false)
            }
            KeyCode::Right => {
                if self.cursor_pos < self.current_field_len() {
                    self.cursor_pos += 1;
                }
                (None, false)
            }
            KeyCode::Char(c) if !self.is_boolean_field() => {
                self.handle_insert_char(c);
                (None, false)
            }
            _ => (None, false),
        }
    }

    /// Handles mouse events
    pub fn handle_mouse(&mut self, _mouse: MouseEvent, _area: Rect) {
        // Clicks could switch field
    }

    fn is_boolean_field(&self) -> bool {
        self.active_field >= 6
    }

    fn toggle_current_boolean(&mut self) {
        match self.active_field {
            6 => self.alt_buffer = !self.alt_buffer,
            7 => self.no_formatting = !self.no_formatting,
            8 => self.quiet = !self.quiet,
            9 => self.interactive = !self.interactive,
            10 => self.show_whitespace = !self.show_whitespace,
            11 => self.external = !self.external,
            _ => {}
        }
    }

    fn current_field_len(&self) -> usize {
        match self.active_field {
            0 => self.title_val.len(),
            1 => self.icon_val.len(),
            2 => self.action_val.len(),
            3 => self.prompt_val.len(),
            4 => self.start_dir_val.len(),
            5 => self.tabstop_val.len(),
            _ => 0,
        }
    }

    fn handle_backspace(&mut self) {
        let text = match self.active_field {
            0 => &mut self.title_val,
            1 => &mut self.icon_val,
            2 => &mut self.action_val,
            3 => &mut self.prompt_val,
            4 => &mut self.start_dir_val,
            5 => &mut self.tabstop_val,
            _ => return,
        };
        if self.cursor_pos > 0 && self.cursor_pos <= text.len() {
            text.remove(self.cursor_pos - 1);
            self.cursor_pos -= 1;
        }
    }

    fn handle_insert_char(&mut self, c: char) {
        let text = match self.active_field {
            0 => &mut self.title_val,
            1 => &mut self.icon_val,
            2 => &mut self.action_val,
            3 => &mut self.prompt_val,
            4 => &mut self.start_dir_val,
            5 => &mut self.tabstop_val,
            _ => return,
        };
        if self.cursor_pos <= text.len() {
            text.insert(self.cursor_pos, c);
            self.cursor_pos += 1;
        }
    }

    /// Produces the updated `MenuItem`
    pub fn build_updated_item(&self) -> MenuItem {
        let mut item = self.item.clone();
        item.label = self.title_val.clone();
        item.icon = if self.icon_val.is_empty() {
            None
        } else {
            Some(self.icon_val.clone())
        };

        let trimmed_act = self.action_val.trim();
        match &item.r#type {
            MenuItemType::Command => {
                item.command = if trimmed_act.is_empty() {
                    None
                } else {
                    Some(trimmed_act.to_string())
                };
                item.action = item.command.clone();
            }
            MenuItemType::Script => {
                item.script = if trimmed_act.is_empty() {
                    None
                } else {
                    Some(trimmed_act.to_string())
                };
                item.action = item.script.clone();
            }
            MenuItemType::Editor | MenuItemType::Notebook => {
                item.file = if trimmed_act.is_empty() {
                    None
                } else {
                    Some(trimmed_act.to_string())
                };
                item.action = item.file.clone();
            }
            _ => {
                item.action = if trimmed_act.is_empty() {
                    None
                } else {
                    Some(trimmed_act.to_string())
                };
            }
        }

        item.prompt = if self.prompt_val.is_empty() {
            None
        } else {
            Some(self.prompt_val.clone())
        };
        item.start_dir = if self.start_dir_val.is_empty() {
            None
        } else {
            Some(self.start_dir_val.clone())
        };
        item.tabstop = self.tabstop_val.trim().parse().ok();
        item.alt_buffer = Some(self.alt_buffer);
        item.quiet = Some(self.quiet);
        item.interactive = Some(self.interactive);

        item.extra.insert(
            "no_formatting".to_string(),
            serde_yml::Value::Bool(self.no_formatting),
        );
        item.extra.insert(
            "show_whitespace".to_string(),
            serde_yml::Value::Bool(self.show_whitespace),
        );
        item.extra.insert(
            "external".to_string(),
            serde_yml::Value::Bool(self.external),
        );

        item
    }

    /// Renders the property editor modal
    pub fn render(&self, area: Rect, styles: &ResolvedThemeStyles, buf: &mut Buffer) -> Rect {
        let dialog_w = 74.min(area.width.saturating_sub(4));
        let dialog_h = 24.min(area.height.saturating_sub(4));

        let title_header = format!(" [ Edit Item: {} ] ", self.title_val);
        let inner = render_modal_frame(area, dialog_w, dialog_h, &title_header, styles, buf);

        let alt_str = if self.alt_buffer {
            "[X] alt_buffer=true"
        } else {
            "[ ] alt_buffer=false"
        };
        let nf_str = if self.no_formatting {
            "[X] no_formatting=true"
        } else {
            "[ ] no_formatting=false"
        };
        let q_str = if self.quiet {
            "[X] quiet=true"
        } else {
            "[ ] quiet=false"
        };
        let int_str = if self.interactive {
            "[X] interactive=true"
        } else {
            "[ ] interactive=false"
        };
        let sw_str = if self.show_whitespace {
            "[X] show_whitespace=true"
        } else {
            "[ ] show_whitespace=false"
        };
        let ext_str = if self.external {
            "[X] external=true"
        } else {
            "[ ] external=false"
        };

        let fields: [(&str, &str, bool); 12] = [
            ("Title / Label", self.title_val.as_str(), false),
            ("Icon / Glyph", self.icon_val.as_str(), false),
            ("Action / Target", self.action_val.as_str(), false),
            ("Prompt / Message", self.prompt_val.as_str(), false),
            ("Start Directory", self.start_dir_val.as_str(), false),
            ("Tabstop", self.tabstop_val.as_str(), false),
            ("Alt Screen Buffer", alt_str, true),
            ("No Formatting", nf_str, true),
            ("Quiet Execution", q_str, true),
            ("Interactive Terminal", int_str, true),
            ("Show Whitespace", sw_str, true),
            ("External Execution", ext_str, true),
        ];

        for (idx, (label, val_str, is_bool)) in fields.iter().enumerate() {
            let y = inner.y + 1 + idx as u16;
            if y >= inner.y + inner.height - 3 {
                break;
            }
            let is_active = idx == self.active_field;

            let lbl_style = if is_active {
                styles.highlight.add_modifier(Modifier::BOLD)
            } else {
                styles.text
            };

            let row_str = if *is_bool {
                format!(" {:<20} {}", label, val_str)
            } else {
                format!(" {:<20} : {}", label, val_str)
            };

            let max_w = inner.width.saturating_sub(2) as usize;
            let display_text: String = row_str.chars().take(max_w).collect();

            buf.set_string(
                inner.x + 1,
                y,
                &display_text,
                if is_active {
                    styles.highlight
                } else {
                    lbl_style
                },
            );

            // Cursor for text fields
            if is_active && !*is_bool {
                let cursor_x = inner.x + 24 + self.cursor_pos as u16;
                if cursor_x < inner.x + inner.width - 2 {
                    buf[(cursor_x, y)].set_style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::White)
                            .add_modifier(Modifier::REVERSED),
                    );
                }
            }
        }

        // Footer help text
        let footer_y = inner.y + inner.height - 2;
        let footer_text =
            " [Ctrl+S/F2] Save | [Ctrl+T] Type | [TAB] Next | [SPACE] Toggle | [ESC] Cancel ";
        let footer_x = inner.x + (inner.width.saturating_sub(footer_text.len() as u16) / 2);
        buf.set_string(
            footer_x,
            footer_y,
            footer_text,
            Style::default().fg(Color::DarkGray),
        );

        inner
    }
}
