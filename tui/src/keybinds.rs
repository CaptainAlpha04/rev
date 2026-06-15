use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Quit,
    StepForward,
    StepBackward,
    JumpForward,
    JumpBackward,
    ToggleSearch,
    ToggleDetails,
    ExportState,
    Cancel,
    Confirm,
    Char(char),
    Backspace,
    None,
}

pub fn map_key(key: KeyEvent, in_search_mode: bool) -> KeyAction {
    if in_search_mode {
        match key.code {
            KeyCode::Esc => KeyAction::Cancel,
            KeyCode::Enter => KeyAction::Confirm,
            KeyCode::Backspace => KeyAction::Backspace,
            KeyCode::Char(c) => KeyAction::Char(c),
            _ => KeyAction::None,
        }
    } else {
        match key.code {
            KeyCode::Char('q') => KeyAction::Quit,
            KeyCode::Esc => KeyAction::Quit,
            KeyCode::Left | KeyCode::Char('h') => KeyAction::StepBackward,
            KeyCode::Right | KeyCode::Char('l') => KeyAction::StepForward,
            KeyCode::Up | KeyCode::Char('k') => KeyAction::JumpBackward,
            KeyCode::Down | KeyCode::Char('j') => KeyAction::JumpForward,
            KeyCode::Char('/') => KeyAction::ToggleSearch,
            KeyCode::Char('d') => KeyAction::ToggleDetails,
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => KeyAction::ExportState,
            _ => KeyAction::None,
        }
    }
}
