use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Debug, Clone, Copy)]
pub struct TuiLayout {
    pub call_stack: Rect,
    pub last_event: Rect,
    pub variable_inspector: Rect,
    pub timeline_scrubber: Rect,
    pub footer: Rect,
}

impl TuiLayout {
    pub fn new(area: Rect) -> Self {
        // Vertical layout: Main Section (top), Timeline (middle/bottom), Footer (bottom)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),    // Main panels
                Constraint::Length(4), // Timeline scrubber
                Constraint::Length(1), // Footer status/keybindings
            ])
            .split(area);

        let main_area = main_chunks[0];
        let timeline_scrubber = main_chunks[1];
        let footer = main_chunks[2];

        // Horizontal split for main area: Call Stack, Last Event, Variable Inspector
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25), // Call Stack (left)
                Constraint::Percentage(40), // Last Event (middle)
                Constraint::Percentage(35), // Variable Inspector (right)
            ])
            .split(main_area);

        TuiLayout {
            call_stack: horizontal_chunks[0],
            last_event: horizontal_chunks[1],
            variable_inspector: horizontal_chunks[2],
            timeline_scrubber,
            footer,
        }
    }
}
