use crossterm::event::KeyEvent;
use ratatui::{backend::CrosstermBackend, Frame, Terminal};
use rev_core::error::RevError;
use rev_core::types::ProgramState;
use rev_replay::ReplayEngine;
use std::io::Stdout;

pub struct LayoutStub;

pub enum AppMode {
    Normal,
    Search(String),
    EventExpanded,
    ExportConfirm,
}

pub enum AppAction {
    Quit,
    None,
}

pub struct TuiApp {
    _replay: ReplayEngine,
    _current_step: u64,
    _total_steps: u64,
    _current_state: Option<ProgramState>,
    _layout: LayoutStub,
    _mode: AppMode,
}

impl TuiApp {
    pub fn new(replay: ReplayEngine) -> Self {
        TuiApp {
            _replay: replay,
            _current_step: 0,
            _total_steps: 0,
            _current_state: None,
            _layout: LayoutStub,
            _mode: AppMode::Normal,
        }
    }

    /// Main loop: render → handle input → replay → re-render
    pub fn run(
        &mut self,
        _terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), RevError> {
        todo!("TuiApp::run is not implemented yet")
    }

    /// Called on any step change
    pub fn seek_to(&mut self, _step: u64) {
        todo!("TuiApp::seek_to is not implemented yet")
    }

    /// Render the application frame
    pub fn render(&self, _frame: &mut Frame) {
        todo!("TuiApp::render is not implemented yet")
    }

    /// Handle key input
    pub fn handle_key(&mut self, _key: KeyEvent) -> AppAction {
        todo!("TuiApp::handle_key is not implemented yet")
    }
}
