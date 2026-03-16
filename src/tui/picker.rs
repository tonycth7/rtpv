// tui/picker.rs — fuzzy entry picker with preview pane

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use super::theme::TuiTheme;
use super::{PostAction, TuiState};
use crate::clipboard;
use crate::otp;
use crate::vault::Vault;

pub struct PickerState {
    pub all_entries: Vec<String>,
    pub filtered: Vec<(String, i64)>,
    pub list_state: ListState,
    pub query: String,
    pub matcher: SkimMatcherV2,
}

impl PickerState {
    pub fn new(vault: &Vault, _cfg: &crate::config::Config) -> Result<Self> {
        let entries = vault.list()?;
        let filtered = entries.iter().map(|e| (e.clone(), 0i64)).collect();
        let mut s = Self {
            all_entries: entries,
            filtered,
            list_state: ListState::default(),
            query: String::new(),
            matcher: SkimMatcherV2::default(),
        };
        if !s.all_entries.is_empty() {
            s.list_state.select(Some(0));
        }
        Ok(s)
    }

    pub fn selected(&self) -> Option<&str> {
        self.list_state
            .selected()
            .and_then(|i| self.filtered.get(i))
            .map(|(p, _)| p.as_str())
    }

    pub fn refilter(&mut self) {
        if self.query.is_empty() {
            self.filtered = self.all_entries.iter().map(|e| (e.clone(), 0)).collect();
        } else {
            let mut matches: Vec<(String, i64)> = self
                .all_entries
                .iter()
                .filter_map(|e| {
                    self.matcher
                        .fuzzy_match(e, &self.query)
                        .map(|s| (e.clone(), s))
                })
                .collect();
            matches.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered = matches;
        }
        let sel = if self.filtered.is_empty() {
            None
        } else {
            Some(0)
        };
        self.list_state.select(sel);
    }

    pub fn move_up(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        if i > 0 {
            self.list_state.select(Some(i - 1));
        }
    }

    pub fn move_down(&mut self) {
        let len = self.filtered.len();
        if len == 0 {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        if i + 1 < len {
            self.list_state.select(Some(i + 1));
        }
    }
}

pub enum PickerAction {
    Quit,
    OpenActions(String),
    PostAction(PostAction),
}

pub fn draw(f: &mut Frame, picker: &mut PickerState, state: &TuiState) {
    let theme = TuiTheme::from(&state.cfg.theme);
    let area = f.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search bar
            Constraint::Min(0),    // list + preview side by side
            Constraint::Length(1), // status bar
        ])
        .split(area);

    // ── Search bar ───────────────────────────────────────────────────────────
    let search_text = format!(" {} {}_", "❯", picker.query);
    let search = Paragraph::new(search_text)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(theme.accent_style())
            .title("  rtpv  ─  ↑↓ navigate  ·  Tab/Enter action menu  ·  y copy  ·  o OTP  ·  ? help  ·  q quit "))
        .style(Style::default().fg(ratatui::style::Color::White));
    f.render_widget(search, chunks[0]);

    // ── Split list + preview ─────────────────────────────────────────────────
    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);

    // ── Entry list ───────────────────────────────────────────────────────────
    let items: Vec<ListItem> = picker
        .filtered
        .iter()
        .map(|(path, _)| {
            let parts: Vec<&str> = path.rsplitn(2, '/').collect();
            let (name, dir) = if parts.len() == 2 {
                (parts[0], format!("{}/", parts[1]))
            } else {
                (parts[0], String::new())
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("  {}", dir), theme.dim_style()),
                Span::styled(
                    name.to_string(),
                    Style::default().fg(ratatui::style::Color::White),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ratatui::style::Color::DarkGray))
                .title(format!(
                    "  {} / {}  ",
                    picker.filtered.len(),
                    picker.all_entries.len()
                )),
        )
        .highlight_style(theme.selected_style().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, mid[0], &mut picker.list_state);

    // ── Preview pane ─────────────────────────────────────────────────────────
    let preview_text = if let Some(path) = picker.selected() {
        match state.vault.read(path) {
            Ok(entry) => {
                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("  type  ", theme.dim_style()),
                        Span::styled(entry.meta.kind.display(), theme.accent_style()),
                    ]),
                    Line::from(""),
                ];

                macro_rules! preview_field {
                    ($label:expr, $val:expr) => {
                        if let Some(v) = $val {
                            lines.push(Line::from(vec![
                                Span::styled(format!("  {:<10}", $label), theme.dim_style()),
                                Span::raw(v.to_string()),
                            ]));
                        }
                    };
                }
                macro_rules! preview_secret {
                    ($label:expr, $val:expr) => {
                        if $val.is_some() {
                            lines.push(Line::from(vec![
                                Span::styled(format!("  {:<10}", $label), theme.dim_style()),
                                Span::styled("●●●●●●●●", theme.dim_style()),
                            ]));
                        }
                    };
                }

                preview_secret!("password", entry.fields.password.as_ref());
                preview_field!("username", entry.fields.username.as_deref());
                preview_field!("email", entry.fields.email.as_deref());
                preview_field!("url", entry.fields.url.as_deref());
                preview_field!("host", entry.fields.host.as_deref());
                preview_field!("ssid", entry.fields.ssid.as_deref());
                preview_field!("product", entry.fields.product.as_deref());

                if entry.fields.has_otp() {
                    lines.push(Line::from(vec![
                        Span::styled("  otp       ", theme.dim_style()),
                        Span::styled("● configured", theme.ok_style()),
                    ]));
                }

                if let Some(notes) = entry.fields.notes.as_deref() {
                    lines.push(Line::from(""));
                    for note_line in notes.lines().take(4) {
                        lines.push(Line::from(vec![
                            Span::styled("  ", theme.dim_style()),
                            Span::raw(note_line.to_string()),
                        ]));
                    }
                }

                if entry.meta.starred {
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![Span::styled(
                        "  ★ starred",
                        theme.warn_style(),
                    )]));
                }

                if !entry.meta.tags.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("  tags      ", theme.dim_style()),
                        Span::raw(entry.meta.tags.join(", ")),
                    ]));
                }

                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled("  modified  ", theme.dim_style()),
                    Span::styled(
                        entry.meta.modified.format("%Y-%m-%d %H:%M").to_string(),
                        theme.dim_style(),
                    ),
                ]));

                lines
            }
            Err(e) => vec![
                Line::from(""),
                Line::from(vec![Span::styled(format!("  ✖ {}", e), theme.err_style())]),
            ],
        }
    } else {
        vec![Line::from(vec![Span::styled(
            "  No entries found",
            theme.dim_style(),
        )])]
    };

    let preview = Paragraph::new(preview_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ratatui::style::Color::DarkGray))
                .title(format!("  {} ", picker.selected().unwrap_or(""))),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(preview, mid[1]);

    // ── Status bar ───────────────────────────────────────────────────────────
    let status = Paragraph::new(format!(" {}", state.status)).style(theme.dim_style());
    f.render_widget(status, chunks[2]);
}

pub fn handle_key(
    picker: &mut PickerState,
    state: &mut TuiState,
    key: KeyEvent,
) -> Result<Option<PickerAction>> {
    match (key.code, key.modifiers) {
        // Quit
        (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => return Ok(Some(PickerAction::Quit)),
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Ok(Some(PickerAction::Quit)),

        // Navigation
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => picker.move_down(),
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => picker.move_up(),

        // Open action menu
        (KeyCode::Enter, _) | (KeyCode::Tab, _) => {
            if let Some(path) = picker.selected() {
                return Ok(Some(PickerAction::OpenActions(path.to_string())));
            }
        }

        // Quick copy password
        (KeyCode::Char('y'), _) => {
            if let Some(path) = picker.selected() {
                let path = path.to_string();
                match state.vault.read(&path) {
                    Ok(entry) => match entry.fields.password.as_deref() {
                        Some(pw) => match clipboard::copy_with_clear(pw, state.cfg.clip_timeout) {
                            Ok(_) => {
                                state.status = format!(
                                    "✔ password copied for {} ({}s)",
                                    path, state.cfg.clip_timeout
                                )
                            }
                            Err(e) => state.status = format!("✖ clipboard: {}", e),
                        },
                        None => state.status = format!("⚠ no password in {}", path),
                    },
                    Err(e) => state.status = format!("✖ {}", e),
                }
            }
        }

        // Quick copy OTP
        (KeyCode::Char('o'), _) => {
            if let Some(path) = picker.selected() {
                let path = path.to_string();
                match state.vault.read(&path) {
                    Ok(entry) => match entry.fields.otp.as_deref() {
                        Some(uri) => match otp::current_state(uri) {
                            Ok(s) => {
                                let _ = clipboard::copy_with_clear(&s.code, s.remaining);
                                state.status =
                                    format!("✔ OTP {} copied ({}s remaining)", s.code, s.remaining);
                            }
                            Err(e) => state.status = format!("✖ OTP: {}", e),
                        },
                        None => state.status = format!("⚠ no OTP configured for {}", path),
                    },
                    Err(e) => state.status = format!("✖ {}", e),
                }
            }
        }

        // Delete key — go to action menu with delete pre-selected
        (KeyCode::Char('d'), _) => {
            if let Some(path) = picker.selected() {
                return Ok(Some(PickerAction::OpenActions(path.to_string())));
            }
        }

        // Search input
        (KeyCode::Char(c), _) => {
            picker.query.push(c);
            picker.refilter();
        }
        (KeyCode::Backspace, _) => {
            picker.query.pop();
            picker.refilter();
        }

        _ => {}
    }
    Ok(None)
}
