//! rtpv-menu — ratatui floating popup launcher.
//!
//! Bind to a key in your WM:
//!   bindsym $mod+p exec rtpv-menu
//!   bindsym $mod+o exec rtpv-menu otp
//!
//! Opens in the current terminal's alternate screen as a compact centered popup.
//! No rofi/wofi/dmenu needed.
//!
//! Keybindings:
//!   Type       filter entries
//!   ↑/↓ j/k   navigate
//!   Enter      copy password / confirm action
//!   u          copy username
//!   e          copy email
//!   o          copy OTP
//!   Tab        open action submenu
//!   Esc/q      quit

use std::io;
use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use rtpv_core::{config::Config, store::Store};

// ─────────────────────────────────────────────────────────────────────────────
// CLI
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Parser)]
#[command(name = "rtpv-menu", about = "rtpv popup launcher", version)]
struct Cli {
    #[command(subcommand)]
    mode: Option<MenuMode>,
}

#[derive(Subcommand)]
enum MenuMode {
    /// Show only OTP entries with live codes
    Otp,
    /// Quick add entry
    Add,
    /// Generate a password to clipboard
    Gen,
}

// ─────────────────────────────────────────────────────────────────────────────
// State
// ─────────────────────────────────────────────────────────────────────────────
#[derive(PartialEq, Eq, Clone)]
enum Screen { Picker, Action, OtpPicker }

struct App {
    cfg:         Config,
    entries:     Vec<String>,
    otp_entries: Vec<OtpLine>,
    filtered:    Vec<String>,
    list_state:  ListState,
    query:       String,
    screen:      Screen,
    status_line: String,
    actions:     Vec<&'static str>,
    action_state: ListState,
    matcher:     SkimMatcherV2,
}

struct OtpLine {
    path: String,
    code: String,
    secs: u64,
}

impl App {
    fn new(cfg: Config, entries: Vec<String>, otp_entries: Vec<OtpLine>, screen: Screen) -> Self {
        let filtered = entries.clone();
        let mut s = Self {
            cfg, entries, otp_entries, filtered,
            list_state:   ListState::default(),
            query:        String::new(),
            screen,
            status_line:  String::new(),
            actions: vec![
                "copy password",
                "copy username",
                "copy email",
                "copy otp",
                "─────────",
                "rotate",
                "back",
            ],
            action_state: ListState::default(),
            matcher: SkimMatcherV2::default(),
        };
        if !s.filtered.is_empty() { s.list_state.select(Some(0)); }
        s
    }

    fn refilter(&mut self) {
        if self.query.is_empty() {
            self.filtered = self.entries.clone();
        } else {
            let mut matches: Vec<(String, i64)> = self.entries.iter()
                .filter_map(|e| self.matcher.fuzzy_match(e, &self.query).map(|s| (e.clone(), s)))
                .collect();
            matches.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered = matches.into_iter().map(|(p, _)| p).collect();
        }
        let sel = if self.filtered.is_empty() { None } else { Some(0) };
        self.list_state.select(sel);
    }

    fn selected(&self) -> Option<&str> {
        self.list_state.selected()
            .and_then(|i| self.filtered.get(i))
            .map(String::as_str)
    }

    fn mv(&mut self, delta: i32) {
        let len = self.filtered.len();
        if len == 0 { return; }
        let cur = self.list_state.selected().unwrap_or(0) as i32;
        let next = (cur + delta).rem_euclid(len as i32) as usize;
        self.list_state.select(Some(next));
    }

    fn action_mv(&mut self, delta: i32) {
        let len = self.actions.len();
        let cur = self.action_state.selected().unwrap_or(0) as i32;
        let next = (cur + delta).rem_euclid(len as i32) as usize;
        self.action_state.select(Some(next));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Colors
// ─────────────────────────────────────────────────────────────────────────────
fn accent() -> Color { Color::Indexed(110) }
fn ok()     -> Color { Color::Indexed(151) }

// ─────────────────────────────────────────────────────────────────────────────
// Rendering — centered popup
// ─────────────────────────────────────────────────────────────────────────────
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw(f: &mut Frame, app: &mut App) {
    // Background overlay
    let area = centered_rect(60, 70, f.size());
    f.render_widget(Clear, area);

    match &app.screen {
        Screen::Picker | Screen::Action => draw_picker(f, app, area),
        Screen::OtpPicker               => draw_otp_picker(f, app, area),
    }

    // Action overlay
    if app.screen == Screen::Action {
        let action_area = centered_rect(40, 60, f.size());
        f.render_widget(Clear, action_area);
        draw_action_overlay(f, app, action_area);
    }
}

fn draw_picker(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    // Search
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent()))
        .title(" rtpv ");
    let search = Paragraph::new(format!(" {}_", app.query))
        .block(block)
        .style(Style::default().fg(Color::White));
    f.render_widget(search, chunks[0]);

    // List
    let items: Vec<ListItem> = app.filtered.iter().map(|p| {
        let short = p.split('/').last().unwrap_or(p);
        let dir   = if p.contains('/') { format!("{}/", p.rsplit_once('/').unwrap().0) } else { String::new() };
        ListItem::new(Line::from(vec![
            Span::styled(format!("  {}", dir), Style::default().fg(Color::DarkGray)),
            Span::styled(short.to_string(),    Style::default().fg(Color::White)),
        ]))
    }).collect();

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(format!(" {} / {} ", app.filtered.len(), app.entries.len())))
        .highlight_style(Style::default().fg(accent()).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, chunks[1], &mut app.list_state);

    // Status
    let status = Paragraph::new(format!(" {}", app.status_line))
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[2]);
}

fn draw_otp_picker(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent()))
        .title(" rtpv  OTP ");
    let search = Paragraph::new(format!(" {}_", app.query)).block(block);
    f.render_widget(search, chunks[0]);

    let items: Vec<ListItem> = app.otp_entries.iter().map(|e| {
        ListItem::new(Line::from(vec![
            Span::styled(format!("  {:30} ", e.path), Style::default().fg(Color::White)),
            Span::styled(e.code.clone(), Style::default().fg(ok()).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {}s", e.secs), Style::default().fg(Color::DarkGray)),
        ]))
    }).collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .highlight_style(Style::default().fg(accent()).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, chunks[1], &mut app.list_state);

    let status = Paragraph::new(format!(" {}", app.status_line))
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[2]);
}

fn draw_action_overlay(f: &mut Frame, app: &mut App, area: Rect) {
    let path = app.selected().unwrap_or("").to_string();
    let title = format!(" {} ", path.split('/').last().unwrap_or(&path));

    let items: Vec<ListItem> = app.actions.iter().map(|a| {
        if a.starts_with('─') {
            ListItem::new(Span::styled(*a, Style::default().fg(Color::DarkGray)))
        } else {
            ListItem::new(format!("  {}", a))
        }
    }).collect();

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent()))
            .title(title))
        .highlight_style(Style::default().fg(Color::Black).bg(accent()).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

    let mut state = app.action_state.clone();
    f.render_stateful_widget(list, area, &mut state);
    app.action_state = state;
}

// ─────────────────────────────────────────────────────────────────────────────
// Events
// ─────────────────────────────────────────────────────────────────────────────
fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) -> Result<bool> {
    use KeyCode::*;
    match app.screen.clone() {
        Screen::Picker => match (key.code, key.modifiers) {
            (Char('q'), _) | (Esc, _) | (Char('c'), KeyModifiers::CONTROL) => return Ok(true),
            (Down, _) | (Char('j'), _) => app.mv(1),
            (Up,   _) | (Char('k'), _) => app.mv(-1),
            (Tab, _) | (Enter, _) => {
                if app.selected().is_some() {
                    if key.code == Tab {
                        app.screen = Screen::Action;
                        app.action_state.select(Some(0));
                    } else {
                        // Enter = copy password immediately
                        if let Some(path) = app.selected() {
                            let path = path.to_string();
                            copy_password(app, &path);
                        }
                    }
                }
            }
            (Char('u'), _) => { if let Some(p) = app.selected() { let p = p.to_string(); copy_field(app, &p, "username"); } }
            (Char('e'), _) => { if let Some(p) = app.selected() { let p = p.to_string(); copy_field(app, &p, "email"); } }
            (Char('o'), _) => { if let Some(p) = app.selected() { let p = p.to_string(); copy_otp(app, &p); } }
            (Char(c), _) => { app.query.push(c); app.refilter(); }
            (Backspace, _) => { app.query.pop(); app.refilter(); }
            _ => {}
        },
        Screen::Action => match key.code {
            Esc | Char('q') => { app.screen = Screen::Picker; }
            Down | Char('j') => app.action_mv(1),
            Up   | Char('k') => app.action_mv(-1),
            Enter => {
                if let Some(path) = app.selected() {
                    let path = path.to_string();
                    let idx = app.action_state.selected().unwrap_or(0);
                    let action = app.actions[idx];
                    dispatch_action(app, &path, action);
                }
            }
            _ => {}
        },
        Screen::OtpPicker => match (key.code, key.modifiers) {
            (Char('q'), _) | (Esc, _) | (Char('c'), KeyModifiers::CONTROL) => return Ok(true),
            (Down, _) | (Char('j'), _) => {
                let len = app.otp_entries.len();
                let cur = app.list_state.selected().unwrap_or(0);
                app.list_state.select(Some((cur + 1).min(len.saturating_sub(1))));
            }
            (Up, _) | (Char('k'), _) => {
                let cur = app.list_state.selected().unwrap_or(0);
                app.list_state.select(Some(cur.saturating_sub(1)));
            }
            (Enter, _) => {
                if let Some(idx) = app.list_state.selected() {
                    if let Some(e) = app.otp_entries.get(idx) {
                        let code = e.code.clone();
                        let secs = e.secs;
                        match rtpv_core::clipboard::copy_with_clear(&code, secs) {
                            Ok(_)  => { app.status_line = format!("✔ OTP {} copied ({}s)", code, secs); return Ok(true); }
                            Err(e) => app.status_line = format!("✖ {}", e),
                        }
                    }
                }
            }
            _ => {}
        },
    }
    Ok(false)
}

fn copy_password(app: &mut App, path: &str) {
    if let Ok(store) = Store::new(&app.cfg) {
        if let Ok(entry) = store.read(path) {
            match rtpv_core::clipboard::copy_with_clear(&entry.password, app.cfg.clip_timeout) {
                Ok(_)  => { app.status_line = format!("✔ copied ({}s)", app.cfg.clip_timeout); }
                Err(e) => { app.status_line = format!("✖ {}", e); }
            }
        }
    }
}

fn copy_field(app: &mut App, path: &str, field: &str) {
    if let Ok(store) = Store::new(&app.cfg) {
        if let Ok(entry) = store.read(path) {
            let val = match field {
                "username" => entry.username().map(String::from),
                "email"    => entry.email().map(String::from),
                f          => entry.get_field(f).map(String::from),
            };
            match val {
                Some(v) => match rtpv_core::clipboard::copy_with_clear(&v, app.cfg.clip_timeout) {
                    Ok(_)  => app.status_line = format!("✔ {} copied", field),
                    Err(e) => app.status_line = format!("✖ {}", e),
                },
                None => app.status_line = format!("⚠ no {} in {}", field, path),
            }
        }
    }
}

fn copy_otp(app: &mut App, path: &str) {
    if let Ok(store) = Store::new(&app.cfg) {
        if let Ok(entry) = store.read(path) {
            if let Some(uri) = entry.otp_uri() {
                if let Ok(state) = rtpv_core::otp::code_for_uri(uri) {
                    match rtpv_core::clipboard::copy_with_clear(&state.code, 30) {
                        Ok(_)  => app.status_line = format!("✔ OTP {} ({}s)", state.code, state.remaining),
                        Err(e) => app.status_line = format!("✖ {}", e),
                    }
                    return;
                }
            }
        }
    }
    app.status_line = "⚠ no OTP configured".to_string();
}

fn dispatch_action(app: &mut App, path: &str, action: &str) {
    match action {
        "copy password" => { copy_password(app, path); app.screen = Screen::Picker; }
        "copy username" => { copy_field(app, path, "username"); app.screen = Screen::Picker; }
        "copy email"    => { copy_field(app, path, "email"); app.screen = Screen::Picker; }
        "copy otp"      => { copy_otp(app, path); app.screen = Screen::Picker; }
        "rotate" => {
            let cfg = app.cfg.clone();
            match rtpv_core::cmd::rotate::run(&cfg, path, None) {
                Ok(r) => {
                    let _ = rtpv_core::clipboard::copy_with_clear(&r.new_password, cfg.clip_timeout);
                    app.status_line = "✔ rotated — new password copied".to_string();
                }
                Err(e) => app.status_line = format!("✖ {}", e),
            }
            app.screen = Screen::Picker;
        }
        "back" | "─────────" => app.screen = Screen::Picker,
        _ => app.screen = Screen::Picker,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────
fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = Config::load()?;

    let (entries, otp_entries, start_screen) = match cli.mode {
        Some(MenuMode::Otp) => {
            let otp = rtpv_core::cmd::otp::list_all(&cfg)?;
            let lines: Vec<OtpLine> = otp.into_iter().map(|e| OtpLine {
                path: e.path, code: e.state.code, secs: e.state.remaining,
            }).collect();
            (vec![], lines, Screen::OtpPicker)
        }
        Some(MenuMode::Gen) => {
            let pw = rtpv_core::gen::random_password(cfg.gen_length, &cfg.gen_chars)?;
            rtpv_core::clipboard::copy_with_clear(&pw, cfg.clip_timeout)?;
            println!("Generated password copied to clipboard");
            return Ok(());
        }
        _ => {
            let store = Store::new(&cfg)?;
            (store.list()?, vec![], Screen::Picker)
        }
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend  = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let mut app = App::new(cfg, entries, otp_entries, start_screen);

    loop {
        term.draw(|f| draw(f, &mut app))?;
        if let Event::Key(key) = event::read()? {
            if handle_key(&mut app, key)? { break; }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    term.show_cursor()?;

    // Print status to stdout so WM notification scripts can pick it up
    if !app.status_line.is_empty() {
        eprintln!("{}", app.status_line);
    }
    Ok(())
}
