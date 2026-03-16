// tui/actions.rs — action menu per entry type
// State (list + selection) lives in TuiState — no thread_local.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use super::theme::TuiTheme;
use super::{PostAction, TuiState};
use crate::clipboard;
use crate::gen;
use crate::otp;
use crate::vault::EntryKind;

fn build_actions(kind: &EntryKind, has_otp: bool, has_url: bool) -> Vec<&'static str> {
    let mut acts: Vec<&'static str> = match kind {
        EntryKind::Login => vec!["copy password", "copy username", "copy email", "copy token"],
        EntryKind::Note => vec!["copy note", "show note"],
        EntryKind::Ssh | EntryKind::Gpg => vec!["copy public key", "copy private key", "show info"],
        EntryKind::Card => vec!["copy card number", "copy CVV", "show card info"],
        EntryKind::Env => vec!["copy token", "copy password"],
        EntryKind::Wifi => vec!["copy password", "show SSID"],
        EntryKind::Database => vec!["copy password", "copy username"],
        EntryKind::License => vec!["copy license key", "copy password"],
    };
    if has_otp {
        acts.push("──────────────");
        acts.push("otp → copy");
        acts.push("otp → show");
    }
    if has_url {
        acts.push("open URL");
    }
    acts.push("──────────────");
    acts.push("rotate password");
    acts.push("set field");
    acts.push("edit in $EDITOR");
    acts.push("rename");
    acts.push("clone");
    acts.push("──────────────");
    acts.push("check strength");
    acts.push("check HIBP");
    acts.push("──────────────");
    acts.push("toggle starred");
    acts.push("delete");
    acts.push("── back ───────");
    acts
}

pub fn init_for_entry(path: &str, state: &mut TuiState) {
    let (kind, has_otp, has_url) = match state.vault.read(path) {
        Ok(e) => (e.meta.kind, e.fields.has_otp(), e.fields.url.is_some()),
        Err(_) => (EntryKind::Login, false, false),
    };
    state.action_list = build_actions(&kind, has_otp, has_url);
    state.action_sel = 0;
}

pub fn draw(f: &mut Frame, path: &str, state: &TuiState) {
    let theme = TuiTheme::from(&state.cfg.theme);
    let area = f.size();
    let popup_w = area.width.min(44);
    let popup_h = (state.action_list.len() as u16 + 2).min(area.height.saturating_sub(4));
    let popup_x = area.width.saturating_sub(popup_w + 2);
    let popup_area = Rect::new(popup_x, 4, popup_w, popup_h);

    f.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = state
        .action_list
        .iter()
        .map(|a| {
            if a.starts_with('\u{2500}') {
                ListItem::new(Line::from(Span::styled(*a, theme.dim_style())))
            } else {
                ListItem::new(format!("  {}", a))
            }
        })
        .collect();

    let mut ls = ListState::default();
    ls.select(Some(state.action_sel));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.accent_style())
                .title(format!("  {}  ", path.split('/').last().unwrap_or(path))),
        )
        .highlight_style(
            Style::default()
                .fg(ratatui::style::Color::Black)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, popup_area, &mut ls);

    let status_area = Rect::new(0, area.height.saturating_sub(1), area.width, 1);
    f.render_widget(
        ratatui::widgets::Paragraph::new(format!(" {}", state.status)).style(theme.dim_style()),
        status_area,
    );
}

pub enum ActionResult {
    Back,
    Quit,
    PostAction(PostAction),
}

pub fn handle_key(path: &str, state: &mut TuiState, key: KeyEvent) -> Result<Option<ActionResult>> {
    use KeyCode::*;
    match (key.code, key.modifiers) {
        (Esc, _) | (Char('q'), _) => return Ok(Some(ActionResult::Back)),
        (Char('c'), KeyModifiers::CONTROL) => return Ok(Some(ActionResult::Quit)),
        (Down, _) | (Char('j'), _) => {
            let len = state.action_list.len();
            if len > 0 && state.action_sel + 1 < len {
                state.action_sel += 1;
            }
        }
        (Up, _) | (Char('k'), _) => {
            if state.action_sel > 0 {
                state.action_sel -= 1;
            }
        }
        (Enter, _) => {
            let action = state
                .action_list
                .get(state.action_sel)
                .copied()
                .unwrap_or("");
            if !action.starts_with('\u{2500}') && !action.is_empty() {
                if let Some(r) = execute_action(path, action, state)? {
                    return Ok(Some(r));
                }
            }
        }
        _ => {}
    }
    Ok(None)
}

fn execute_action(path: &str, action: &str, state: &mut TuiState) -> Result<Option<ActionResult>> {
    let timeout = state.cfg.clip_timeout;

    macro_rules! copy_field {
    ($field:expr, $label:expr) => {{
        match state.vault.read(path) {
            Ok(entry) => match entry.fields.get($field) {
                Some(v) => {
                    let v = v.to_string();
                    match clipboard::copy_with_clear(&v, timeout) {
                        Ok(_)  => state.status = format!("✔  {} copied  ({}s)", $label, timeout),
                        Err(e) => state.status = format!("✖  clipboard: {}", e),
                    }
                }
                None => state.status = format!("⚠  no {} in {}", $label, path),
            },
            Err(e) => state.status = format!("✖  {}", e),
        }
        return Ok(None);
    }};
}

    match action {
        "copy password" => copy_field!("password", "password"),
        "copy username" => copy_field!("username", "username"),
        "copy email" => copy_field!("email", "email"),
        "copy token" => copy_field!("token", "token"),
        "copy note" => copy_field!("notes", "note"),
        "copy public key" => copy_field!("pub_key", "public key"),
        "copy private key" => copy_field!("pvt_key", "private key"),
        "copy card number" => copy_field!("number", "card number"),
        "copy CVV" => copy_field!("cvv", "CVV"),
        "copy license key" => copy_field!("key", "license key"),

        "otp → copy" => match state.vault.read(path) {
            Ok(e) => match e.fields.otp.as_deref() {
                Some(uri) => match otp::current_state(uri) {
                    Ok(s) => {
                        let _ = clipboard::copy_with_clear(&s.code, s.remaining);
                        state.status = format!("✔  OTP {}  ({}s)", s.code, s.remaining);
                    }
                    Err(e) => state.status = format!("✖  OTP: {}", e),
                },
                None => state.status = "⚠  no OTP configured".to_string(),
            },
            Err(e) => state.status = format!("✖  {}", e),
        },

        "otp → show" => match state.vault.read(path) {
            Ok(e) => match e.fields.otp.as_deref() {
                Some(uri) => match otp::current_state(uri) {
                    Ok(s) => {
                        state.status = format!(
                            "OTP: {}  —  {}s remaining  —  {}",
                            s.code,
                            s.remaining,
                            s.issuer.as_deref().unwrap_or("")
                        )
                    }
                    Err(e) => state.status = format!("✖  OTP: {}", e),
                },
                None => state.status = "⚠  no OTP".to_string(),
            },
            Err(e) => state.status = format!("✖  {}", e),
        },

        "rotate password" => {
            let gen_len = state.cfg.gen_length;
            let gen_chars = state.cfg.gen_chars.clone();
            match state.vault.read(path) {
                Ok(mut entry) => match gen::random(gen_len, &gen_chars) {
                    Ok(pw) => {
                        entry.fields.password = Some(pw.clone());
                        entry.touch();
                        match state.vault.write(path, &entry) {
                            Ok(_) => {
                                let _ = clipboard::copy_with_clear(&pw, timeout);
                                state.status = format!("✔  rotated  ({}s)", timeout);
                            }
                            Err(e) => state.status = format!("✖  {}", e),
                        }
                    }
                    Err(e) => state.status = format!("✖  gen: {}", e),
                },
                Err(e) => state.status = format!("✖  {}", e),
            }
        }

        "check strength" => match state.vault.read(path) {
            Ok(e) => match e.fields.password.as_deref() {
                Some(pw) => {
                    let s = gen::check_strength(pw);
                    state.status = format!(
                        "strength: {}/4 ({})  {:.0}bits  crack: {}",
                        s.score, s.label, s.bits, s.crack_time
                    );
                }
                None => state.status = "⚠  no password".to_string(),
            },
            Err(e) => state.status = format!("✖  {}", e),
        },

        "check HIBP" => match state.vault.read(path) {
            Ok(e) => match e.fields.password.as_deref() {
                Some(pw) => {
                    let pw_owned = pw.to_string();
                    let t = state.cfg.hibp_timeout;
                    match crate::audit::hibp_check(&pw_owned, t) {
                        Ok(0) => state.status = "✔  not found in HIBP".to_string(),
                        Ok(n) => state.status = format!("⚠  BREACHED — {} times", n),
                        Err(e) => state.status = format!("✖  HIBP: {}", e),
                    }
                }
                None => state.status = "⚠  no password".to_string(),
            },
            Err(e) => state.status = format!("✖  {}", e),
        },

        "toggle starred" => match state.vault.read(path) {
            Ok(mut e) => {
                e.meta.starred = !e.meta.starred;
                let v = e.meta.starred;
                match state.vault.write(path, &e) {
                    Ok(_) => state.status = format!("✔  starred: {}", v),
                    Err(e) => state.status = format!("✖  {}", e),
                }
            }
            Err(e) => state.status = format!("✖  {}", e),
        },

        "edit in $EDITOR" => {
            return Ok(Some(ActionResult::PostAction(PostAction::Edit(
                path.to_string(),
            ))));
        }

        "── back ───────" => {
            return Ok(Some(ActionResult::Back));
        }

        _ => {
            state.status = format!("hint: rtpv {} {}", action.replace(' ', "-"), path);
        }
    }
    Ok(None)
}
