// tui/mod.rs — TUI entry point

pub mod actions;
pub mod picker;
pub mod theme;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use crate::config::Config;
use crate::vault::Vault;

pub struct TuiState {
    pub vault: Vault,
    pub cfg: Config,
    pub screen: Screen,
    pub status: String,
    /// Action list for current entry (populated when entering Actions screen)
    pub action_list: Vec<&'static str>,
    pub action_sel: usize,
}

#[derive(PartialEq, Eq, Clone)]
pub enum Screen {
    Picker,
    Actions { path: String },
}

/// Launch the full-screen TUI. Returns the optional CLI action to perform
/// after the TUI exits (e.g. launch $EDITOR).
pub fn run(vault: Vault, cfg: Config) -> Result<Option<PostAction>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let state = TuiState {
        vault,
        cfg,
        screen: Screen::Picker,
        status: String::new(),
        action_list: Vec::new(),
        action_sel: 0,
    };

    let result = event_loop(&mut term, state);

    disable_raw_mode()?;
    execute!(
        term.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    term.show_cursor()?;

    result
}

fn event_loop(
    term: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut state: TuiState,
) -> Result<Option<PostAction>> {
    let mut picker_state = picker::PickerState::new(&state.vault, &state.cfg)?;

    loop {
        // Draw
        term.draw(|f| match &state.screen {
            Screen::Picker => picker::draw(f, &mut picker_state, &state),
            Screen::Actions { path } => {
                let path = path.clone();
                actions::draw(f, &path, &state);
            }
        })?;

        // Event
        if let Event::Key(key) = event::read()? {
            match &state.screen.clone() {
                Screen::Picker => {
                    if let Some(action) = picker::handle_key(&mut picker_state, &mut state, key)? {
                        match action {
                            picker::PickerAction::Quit => return Ok(None),
                            picker::PickerAction::OpenActions(path) => {
                                actions::init_for_entry(&path, &mut state);
                                state.screen = Screen::Actions { path };
                            }
                            picker::PickerAction::PostAction(a) => return Ok(Some(a)),
                        }
                    }
                }
                Screen::Actions { path } => {
                    let path = path.clone();
                    if let Some(action) = actions::handle_key(&path, &mut state, key)? {
                        match action {
                            actions::ActionResult::Back => {
                                state.screen = Screen::Picker;
                            }
                            actions::ActionResult::PostAction(a) => return Ok(Some(a)),
                            actions::ActionResult::Quit => return Ok(None),
                        }
                    }
                }
            }
        }
    }
}

/// Action that requires leaving the TUI (launching editor, etc.)
#[derive(Debug)]
pub enum PostAction {
    Edit(String),
}
