//! rtpv-tui — ratatui interactive picker.
//!
//! Launch with no args for the main picker.
//! Keybindings:
//!   j/k or ↑/↓   navigate
//!   /             focus search
//!   Enter         open action menu
//!   y             copy password
//!   u             copy username
//!   o             copy OTP code
//!   q / Esc       quit

use std::io;
use anyhow::Result;
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
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use rtpv_core::{config::Config, store::Store};

// ─────────────────────────────────────────────────────────────────────────────
// App state
// ─────────────────────────────────────────────────────────────────────────────
#[derive(PartialEq, Eq)]
enum Mode { List, Action, Confirm(String) }

struct App {
    cfg:        Config,
    all_entries: Vec<String>,
    filtered:   Vec<(String, i64)>,   // (path, score)
    list_state: ListState,
    query:      String,
    mode:       Mode,
    status:     String,
    actions:    Vec<&'static str>,
    action_state: ListState,
    matcher:    SkimMatcherV2,
}

impl App {
    fn new(cfg: Config, entries: Vec<String>) -> Self {
        let filtered = entries.iter().map(|e| (e.clone(), 0i64)).collect();
        let mut app = Self {
            cfg,
            all_entries: entries,
            filtered,
            list_state: ListState::default(),
            query: String::new(),
            mode: Mode::List,
            status: String::new(),
            actions: vec![
                "copy password",
                "copy username",
                "copy email",
                "copy otp",
                "show entry",
                "edit",
                "rotate",
                "delete",
                "─────────",
                "back",
            ],
            action_state: ListState::default(),
            matcher: SkimMatcherV2::default(),
        };
        if !app.filtered.is_empty() { app.list_state.select(Some(0)); }
        app
    }

    fn filter(&mut self) {
        if self.query.is_empty() {
            self.filtered = self.all_entries.iter().map(|e| (e.clone(), 0i64)).collect();
        } else {
            let mut matches: Vec<(String, i64)> = self.all_entries.iter()
                .filter_map(|e| self.matcher.fuzzy_match(e, &self.query)
                    .map(|s| (e.clone(), s)))
                .collect();
            matches.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered = matches;
        }
        let sel = if self.filtered.is_empty() { None } else { Some(0) };
        self.list_state.select(sel);
    }

    fn selected_path(&self) -> Option<&str> {
        self.list_state.selected()
            .and_then(|i| self.filtered.get(i))
            .map(|(p, _)| p.as_str())
    }

    fn move_up(&mut self) {
        let i = self.list_state.selected().unwrap_or(0);
        if i > 0 { self.list_state.select(Some(i - 1)); }
    }

    fn move_down(&mut self) {
        let len = self.filtered.len();
        if len == 0 { return; }
        let i = self.list_state.selected().unwrap_or(0);
        if i + 1 < len { self.list_state.select(Some(i + 1)); }
    }

    fn action_up(&mut self) {
        let i = self.action_state.selected().unwrap_or(0);
        if i > 0 { self.action_state.select(Some(i - 1)); }
    }

    fn action_down(&mut self) {
        let len = self.actions.len();
        let i = self.action_state.selected().unwrap_or(0);
        if i + 1 < len { self.action_state.select(Some(i + 1)); }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Theme colors
// ─────────────────────────────────────────────────────────────────────────────
fn accent() -> Color  { Color::Indexed(110) } // Catppuccin blue
fn ok()     -> Color  { Color::Indexed(151) } // green
fn warn()   -> Color  { Color::Indexed(222) } // yellow
fn surface()-> Color  { Color::Rgb(49, 50, 68) }

// ─────────────────────────────────────────────────────────────────────────────
// UI rendering
// ─────────────────────────────────────────────────────────────────────────────
fn draw(f: &mut Frame, app: &mut App) {
    let area = f.size();

    match app.mode {
        Mode::List => draw_picker(f, app, area),
        Mode::Action => draw_action_menu(f, app, area),
        Mode::Confirm(_) => draw_picker(f, app, area),
    }
}

fn draw_picker(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // search bar
            Constraint::Min(0),     // list
            Constraint::Length(1),  // status bar
        ])
        .split(area);

    // Search bar
    let search = Paragraph::new(format!(" {}", app.query))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent()))
            .title(" rtpv  —  / to search, Enter to open, q to quit "))
        .style(Style::default().fg(Color::White));
    f.render_widget(search, chunks[0]);

    // Entry list
    let items: Vec<ListItem> = app.filtered.iter().map(|(path, _)| {
        ListItem::new(format!("  {}", path))
    }).collect();

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(format!(" {} entries ", app.filtered.len())))
        .highlight_style(Style::default()
            .fg(accent())
            .add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, chunks[1], &mut app.list_state);

    // Status bar
    let status = Paragraph::new(app.status.clone())
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[2]);
}

fn draw_action_menu(f: &mut Frame, app: &mut App, area: Rect) {
    // Main list behind (dimmed)
    draw_picker(f, app, area);

    // Overlay popup
    let path = app.selected_path().unwrap_or("").to_string();
    let popup_width  = 36u16;
    let popup_height = (app.actions.len() + 4) as u16;
    let popup_x = area.width.saturating_sub(popup_width + 2);
    let popup_y = 4u16;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height.min(area.height - 4));

    // Clear the area first
    f.render_widget(ratatui::widgets::Clear, popup_area);

    let title = format!(" {} ", path.split('/').last().unwrap_or(&path));
    let items: Vec<ListItem> = app.actions.iter().map(|a| {
        if a.starts_with('─') {
            ListItem::new(Line::from(vec![
                Span::styled(*a, Style::default().fg(Color::DarkGray))
            ]))
        } else {
            ListItem::new(format!("  {}", a))
        }
    }).collect();

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent()))
            .title(title))
        .highlight_style(Style::default()
            .fg(Color::Black)
            .bg(accent())
            .add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

    let mut state = app.action_state.clone();
    f.render_stateful_widget(list, popup_area, &mut state);
    app.action_state = state;
}

// ─────────────────────────────────────────────────────────────────────────────
// Event handling
// ─────────────────────────────────────────────────────────────────────────────
fn handle_event(app: &mut App) -> Result<bool> {
    if let Event::Key(key) = event::read()? {
        match &app.mode {
            Mode::List => {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => return Ok(true),
                    (KeyCode::Char('c'), KeyModifiers::CONTROL)  => return Ok(true),

                    (KeyCode::Down, _) | (KeyCode::Char('j'), _) => app.move_down(),
                    (KeyCode::Up,   _) | (KeyCode::Char('k'), _) => app.move_up(),

                    (KeyCode::Enter, _) => {
                        if app.selected_path().is_some() {
                            app.mode = Mode::Action;
                            app.action_state.select(Some(0));
                        }
                    }

                    // Quick copy password
                    (KeyCode::Char('y'), _) => {
                        if let Some(path) = app.selected_path() {
                            let path = path.to_string();
                            quick_copy_password(app, &path);
                        }
                    }
                    // Quick copy username
                    (KeyCode::Char('u'), _) => {
                        if let Some(path) = app.selected_path() {
                            let path = path.to_string();
                            quick_copy_field(app, &path, "username");
                        }
                    }
                    // Quick copy OTP
                    (KeyCode::Char('o'), _) => {
                        if let Some(path) = app.selected_path() {
                            let path = path.to_string();
                            quick_copy_otp(app, &path);
                        }
                    }

                    // Search
                    (KeyCode::Char(c), _) => {
                        app.query.push(c);
                        app.filter();
                    }
                    (KeyCode::Backspace, _) => {
                        app.query.pop();
                        app.filter();
                    }
                    _ => {}
                }
            }

            Mode::Action => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        app.mode = Mode::List;
                    }
                    KeyCode::Down | KeyCode::Char('j') => app.action_down(),
                    KeyCode::Up   | KeyCode::Char('k') => app.action_up(),
                    KeyCode::Enter => {
                        if let Some(path) = app.selected_path() {
                            let path = path.to_string();
                            let action_idx = app.action_state.selected().unwrap_or(0);
                            let action = app.actions[action_idx];
                            execute_action(app, &path, action)?;
                        }
                    }
                    _ => {}
                }
            }

            Mode::Confirm(_) => {
                app.mode = Mode::List;
            }
        }
    }
    Ok(false)
}

fn quick_copy_password(app: &mut App, path: &str) {
    match rtpv_core::store::Store::new(&app.cfg) {
        Ok(store) => match store.read(path) {
            Ok(entry) => {
                match rtpv_core::clipboard::copy_with_clear(&entry.password, app.cfg.clip_timeout) {
                    Ok(_)  => app.status = format!("  ✔  Password copied for {} (clears in {}s)", path, app.cfg.clip_timeout),
                    Err(e) => app.status = format!("  ✖  Clipboard error: {}", e),
                }
            }
            Err(e) => app.status = format!("  ✖  {}", e),
        }
        Err(e) => app.status = format!("  ✖  {}", e),
    }
}

fn quick_copy_field(app: &mut App, path: &str, field: &str) {
    match rtpv_core::store::Store::new(&app.cfg) {
        Ok(store) => match store.read(path) {
            Ok(entry) => {
                let val = match field {
                    "username" => entry.username().map(String::from),
                    "email"    => entry.email().map(String::from),
                    other      => entry.get_field(other).map(String::from),
                };
                match val {
                    Some(v) => match rtpv_core::clipboard::copy_with_clear(&v, app.cfg.clip_timeout) {
                        Ok(_)  => app.status = format!("  ✔  {} copied", field),
                        Err(e) => app.status = format!("  ✖  {}", e),
                    },
                    None => app.status = format!("  ⚠  No {} in {}", field, path),
                }
            }
            Err(e) => app.status = format!("  ✖  {}", e),
        }
        Err(e) => app.status = format!("  ✖  {}", e),
    }
}

fn quick_copy_otp(app: &mut App, path: &str) {
    match rtpv_core::store::Store::new(&app.cfg) {
        Ok(store) => match store.read(path) {
            Ok(entry) => match entry.otp_uri() {
                Some(uri) => match rtpv_core::otp::code_for_uri(uri) {
                    Ok(state) => match rtpv_core::clipboard::copy_with_clear(&state.code, 30) {
                        Ok(_)  => app.status = format!("  ✔  OTP {} copied ({}s remaining)", state.code, state.remaining),
                        Err(e) => app.status = format!("  ✖  {}", e),
                    },
                    Err(e) => app.status = format!("  ✖  OTP error: {}", e),
                },
                None => app.status = format!("  ⚠  No OTP configured for {}", path),
            },
            Err(e) => app.status = format!("  ✖  {}", e),
        },
        Err(e) => app.status = format!("  ✖  {}", e),
    }
}

fn execute_action(app: &mut App, path: &str, action: &str) -> Result<()> {
    let cfg = app.cfg.clone();
    match action {
        "copy password" => quick_copy_password(app, path),
        "copy username" => quick_copy_field(app, path, "username"),
        "copy email"    => quick_copy_field(app, path, "email"),
        "copy otp"      => quick_copy_otp(app, path),
        "rotate" => {
            match rtpv_core::cmd::rotate::run(&cfg, path, None) {
                Ok(r) => {
                    let _ = rtpv_core::clipboard::copy_with_clear(&r.new_password, cfg.clip_timeout);
                    app.status = format!("  ✔  Rotated {} — new password copied", path);
                }
                Err(e) => app.status = format!("  ✖  {}", e),
            }
            app.mode = Mode::List;
        }
        "back" | "─────────" => app.mode = Mode::List,
        // edit and show require leaving the TUI — handled at call site
        _ => app.mode = Mode::List,
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────
fn main() -> Result<()> {
    let cfg = Config::load()?;
    let store = Store::new(&cfg)?;
    let entries = store.list()?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend  = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let mut app = App::new(cfg, entries);

    // Event loop
    let result = run_loop(&mut term, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    term.show_cursor()?;

    // result is always Ok(None) currently — edit/show launch external editor
    let _ = result;
    Ok(())
}

fn run_loop(
    term: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app:  &mut App,
) -> Result<Option<String>> {
    loop {
        term.draw(|f| draw(f, app))?;
        if handle_event(app)? {
            break;
        }
    }
    Ok(None)
}
