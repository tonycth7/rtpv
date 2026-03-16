// tui/theme.rs — ratatui style helpers

use crate::config::Theme;
use ratatui::style::{Color, Modifier, Style};

pub struct TuiTheme {
    pub accent: Color,
    pub ok: Color,
    pub warn: Color,
    pub err: Color,
    pub dim: Color,
    pub surface: Color,
}

impl TuiTheme {
    pub fn from(theme: &Theme) -> Self {
        Self {
            accent: theme.accent(),
            ok: theme.ok_color(),
            warn: theme.warn_color(),
            err: theme.err_color(),
            dim: Color::DarkGray,
            surface: Color::Rgb(40, 40, 55),
        }
    }

    pub fn accent_style(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn selected_style(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn dim_style(&self) -> Style {
        Style::default().fg(self.dim)
    }

    pub fn ok_style(&self) -> Style {
        Style::default().fg(self.ok)
    }

    pub fn warn_style(&self) -> Style {
        Style::default().fg(self.warn)
    }

    pub fn err_style(&self) -> Style {
        Style::default().fg(self.err)
    }
}
