use crate::keybinds::{map_key, KeyAction};
use crate::layout::TuiLayout;
use crate::timeline::TimelineScrubber;
use crate::inspector::render_variables;

use crossterm::event::{self, Event};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Rect, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame, Terminal,
};
use rev_core::error::RevError;
use rev_core::types::ProgramState;
use rev_recorder::TraceReader;
use rev_replay::ReplayEngine;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

pub enum WorkerRequest {
    Seek(u64),
}

pub enum WorkerResponse {
    SeekResult {
        step: u64,
        result: Result<ProgramState, RevError>,
    },
}

pub struct ReplayWorker {
    tx: Sender<WorkerRequest>,
    rx: Receiver<WorkerResponse>,
}

impl ReplayWorker {
    pub fn spawn(mut engine: ReplayEngine) -> Self {
        let (req_tx, req_rx) = channel::<WorkerRequest>();
        let (resp_tx, resp_rx) = channel::<WorkerResponse>();

        thread::spawn(move || {
            while let Ok(req) = req_rx.recv() {
                match req {
                    WorkerRequest::Seek(step) => {
                        let res = engine.state_at(step);
                        let _ = resp_tx.send(WorkerResponse::SeekResult {
                            step,
                            result: res,
                        });
                    }
                }
            }
        });

        Self {
            tx: req_tx,
            rx: resp_rx,
        }
    }

    pub fn request_seek(&self, step: u64) {
        let _ = self.tx.send(WorkerRequest::Seek(step));
    }

    pub fn poll_response(&self) -> Option<WorkerResponse> {
        self.rx.try_recv().ok()
    }
}

pub enum AppMode {
    Normal,
    Search(String),
    EventExpanded,
    ExportConfirm,
    Error,
}

struct PendingSeek {
    step: u64,
    requested_at: Instant,
}

pub struct TuiApp {
    worker: ReplayWorker,
    trace_path: PathBuf,
    current_step: u64,
    total_steps: u64,
    current_state: Option<ProgramState>,
    mode: AppMode,
    pending_seek: Option<PendingSeek>,
    error_message: Option<String>,
    status_message: Option<String>,
    should_quit: bool,
}

impl TuiApp {
    pub fn new(replay: ReplayEngine, trace_path: PathBuf) -> Self {
        let total_steps = replay.step_count();
        let worker = ReplayWorker::spawn(replay);

        let mut mode = AppMode::Normal;
        let mut error_message = None;

        if total_steps == 0 {
            error_message = Some("No events found in trace file. Replay cannot proceed.".to_string());
            mode = AppMode::Error;
        }

        TuiApp {
            worker,
            trace_path,
            current_step: 0,
            total_steps,
            current_state: None,
            mode,
            pending_seek: None,
            error_message,
            status_message: None,
            should_quit: false,
        }
    }

    pub fn seek_to(&mut self, step: u64) {
        if self.total_steps == 0 {
            return;
        }
        let target_step = step.min(self.total_steps.saturating_sub(1));
        self.pending_seek = Some(PendingSeek {
            step: target_step,
            requested_at: Instant::now(),
        });
        self.worker.request_seek(target_step);
    }

    pub fn poll_worker(&mut self) {
        while let Some(res) = self.worker.poll_response() {
            match res {
                WorkerResponse::SeekResult { step, result } => {
                    if let Some(pending) = &self.pending_seek {
                        if pending.step == step {
                            self.pending_seek = None;
                            match result {
                                Ok(state) => {
                                    self.current_state = Some(state);
                                    self.current_step = step;
                                }
                                Err(e) => {
                                    self.error_message = Some(format!("{}", e));
                                    self.mode = AppMode::Error;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), RevError> {
        // Kickoff initial seek to step 0
        self.seek_to(0);

        let mut last_status_update: Option<Instant> = None;

        while !self.should_quit {
            self.poll_worker();

            if self.status_message.is_some() {
                if let Some(inst) = last_status_update {
                    if inst.elapsed() > Duration::from_secs(2) {
                        self.status_message = None;
                        last_status_update = None;
                    }
                } else {
                    last_status_update = Some(Instant::now());
                }
            }

            terminal.draw(|f| self.render(f))?;

            if event::poll(Duration::from_millis(20))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == event::KeyEventKind::Press {
                        let is_search = matches!(self.mode, AppMode::Search(_));
                        let action = map_key(key, is_search);
                        self.handle_key(action);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn handle_key(&mut self, action: KeyAction) {
        match &mut self.mode {
            AppMode::Search(ref mut query) => {
                match action {
                    KeyAction::Char(c) => {
                        query.push(c);
                    }
                    KeyAction::Backspace => {
                        query.pop();
                    }
                    KeyAction::Confirm => {
                        let q = query.clone();
                        self.mode = AppMode::Normal;
                        self.search_for(&q);
                    }
                    KeyAction::Cancel => {
                        self.mode = AppMode::Normal;
                    }
                    _ => {}
                }
            }
            AppMode::ExportConfirm => {
                match action {
                    KeyAction::Char('y') | KeyAction::Char('Y') => {
                        self.export_state();
                        self.mode = AppMode::Normal;
                    }
                    KeyAction::Char('n') | KeyAction::Char('N') | KeyAction::Cancel => {
                        self.mode = AppMode::Normal;
                    }
                    _ => {}
                }
            }
            AppMode::EventExpanded => {
                match action {
                    KeyAction::ToggleDetails | KeyAction::Quit | KeyAction::Cancel => {
                        self.mode = AppMode::Normal;
                    }
                    _ => {}
                }
            }
            AppMode::Error => {
                match action {
                    KeyAction::Quit | KeyAction::Cancel => {
                        self.should_quit = true;
                    }
                    _ => {}
                }
            }
            AppMode::Normal => {
                match action {
                    KeyAction::Quit => {
                        self.should_quit = true;
                    }
                    KeyAction::StepForward => {
                        if self.current_step + 1 < self.total_steps {
                            self.seek_to(self.current_step + 1);
                        }
                    }
                    KeyAction::StepBackward => {
                        if self.current_step > 0 {
                            self.seek_to(self.current_step - 1);
                        }
                    }
                    KeyAction::JumpForward => {
                        let target = (self.current_step + 10).min(self.total_steps.saturating_sub(1));
                        self.seek_to(target);
                    }
                    KeyAction::JumpBackward => {
                        let target = self.current_step.saturating_sub(10);
                        self.seek_to(target);
                    }
                    KeyAction::ToggleSearch => {
                        self.mode = AppMode::Search(String::new());
                    }
                    KeyAction::ToggleDetails => {
                        self.mode = AppMode::EventExpanded;
                    }
                    KeyAction::ExportState => {
                        self.mode = AppMode::ExportConfirm;
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn search_for(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        let query_lower = query.to_lowercase();
        if let Ok(mut r) = TraceReader::new(&self.trace_path) {
            if let Ok(events) = r.read_all_events() {
                let events: Vec<rev_core::types::SyscallEvent> = events;
                let start_idx = (self.current_step + 1) as usize;
                let mut found_step = None;
                for event in events.iter().skip(start_idx) {
                    if format!("{:?}", event.syscall).to_lowercase().contains(&query_lower) {
                        found_step = Some(event.id);
                        break;
                    }
                }
                if found_step.is_none() {
                    for event in events.iter().take(start_idx.min(events.len())) {
                        if format!("{:?}", event.syscall).to_lowercase().contains(&query_lower) {
                            found_step = Some(event.id);
                            break;
                        }
                    }
                }
                if let Some(step) = found_step {
                    self.seek_to(step);
                } else {
                    self.status_message = Some(format!("No match found for '{}'", query));
                }
            }
        }
    }

    pub fn export_state(&mut self) {
        if let Some(state) = &self.current_state {
            let filename = format!("state_step_{}.json", state.step);
            let export_path = self.trace_path.parent().unwrap_or(Path::new(".")).join(filename);
            if let Ok(json) = serde_json::to_string_pretty(state) {
                if std::fs::write(&export_path, json).is_ok() {
                    self.status_message = Some(format!("Exported to {:?}", export_path.file_name().unwrap_or_default()));
                } else {
                    self.status_message = Some("Failed to write state file".to_string());
                }
            }
        } else {
            self.status_message = Some("No state available to export".to_string());
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        let size = frame.area();
        let layout = TuiLayout::new(size);

        // 1. Call Stack Panel
        let stack_block = Block::default()
            .borders(Borders::ALL)
            .title(" Call Stack ");
        
        let stack_text = if let Some(state) = &self.current_state {
            if state.call_stack.is_empty() {
                vec![Line::from(Span::styled("No stack frames", Style::default().fg(Color::DarkGray)))]
            } else {
                state.call_stack
                    .iter()
                    .map(|f| {
                        Line::from(vec![
                            Span::styled(format!("{} ", f.function), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                            Span::styled(format!("({}:{})", f.file, f.line), Style::default().fg(Color::DarkGray)),
                        ])
                    })
                    .collect()
            }
        } else {
            vec![Line::from(Span::styled("Loading...", Style::default().fg(Color::DarkGray)))]
        };
        let stack_paragraph = Paragraph::new(stack_text).block(stack_block).wrap(Wrap { trim: true });
        frame.render_widget(stack_paragraph, layout.call_stack);

        // 2. Variable Inspector Panel
        let var_block = Block::default()
            .borders(Borders::ALL)
            .title(" Variables ");
        let var_paragraph = if let Some(state) = &self.current_state {
            render_variables(&state.variables, var_block)
        } else {
            Paragraph::new(vec![Line::from(Span::styled("Loading...", Style::default().fg(Color::DarkGray)))]).block(var_block)
        };
        frame.render_widget(var_paragraph, layout.variable_inspector);

        // 3. Event Details Panel
        let event_block = Block::default()
            .borders(Borders::ALL)
            .title(" Event Details ");
        let event_text = if let Some(state) = &self.current_state {
            let event = &state.last_event;
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Event ID: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(event.id.to_string(), Style::default().fg(Color::Yellow)),
                ]),
                Line::from(vec![
                    Span::styled("Timestamp: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{} ns", event.timestamp_ns), Style::default().fg(Color::Magenta)),
                ]),
                Line::from(vec![
                    Span::styled("Syscall: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{:?}", event.syscall), Style::default().fg(Color::Cyan)),
                ]),
            ];
            if let Some(fd) = event.fd {
                lines.push(Line::from(vec![
                    Span::styled("FD: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(fd.to_string(), Style::default().fg(Color::Green)),
                ]));
            }
            lines.push(Line::from(vec![
                Span::styled("Return Bytes: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(format_bytes(&event.return_bytes), Style::default().fg(Color::Green)),
            ]));
            lines
        } else {
            vec![Line::from(Span::styled("Loading...", Style::default().fg(Color::DarkGray)))]
        };
        let event_paragraph = Paragraph::new(event_text).block(event_block).wrap(Wrap { trim: true });
        frame.render_widget(event_paragraph, layout.last_event);

        // 4. Timeline Scrubber
        let scrubber = TimelineScrubber::new(self.current_step, self.total_steps);
        frame.render_widget(scrubber, layout.timeline_scrubber);

        // 5. Footer / Status Bar
        let footer_text = match &self.mode {
            AppMode::Search(query) => {
                format!("SEARCH: {}█ | [Enter] Confirm | [Esc] Cancel", query)
            }
            AppMode::ExportConfirm => {
                "EXPORT STATE? [y] Export to JSON | [n/Esc] Cancel".to_string()
            }
            AppMode::EventExpanded => {
                "EVENT DETAILS EXPANDED | [d/Esc] Close details".to_string()
            }
            AppMode::Error => {
                "ERROR STATE | [Esc/q] Quit".to_string()
            }
            AppMode::Normal => {
                if let Some(status) = &self.status_message {
                    status.clone()
                } else {
                    format!(
                        "[q] Quit | [h/l] Step | [j/k] Skip 10 | [/] Search | [d] Details | [Ctrl-E] Export (Step {}/{})",
                        self.current_step,
                        self.total_steps.saturating_sub(1)
                    )
                }
            }
        };
        let footer_paragraph = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::Black).bg(Color::Cyan));
        frame.render_widget(footer_paragraph, layout.footer);

        // 6. Overlays (Modals)
        if let AppMode::EventExpanded = self.mode {
            if let Some(state) = &self.current_state {
                let popup_area = centered_rect(60, 60, size);
                frame.render_widget(Clear, popup_area);

                let detail_block = Block::default()
                    .borders(Borders::ALL)
                    .title(" Expanded Event Details ");
                
                let event = &state.last_event;
                let mut detailed_lines = vec![
                    Line::from(vec![Span::styled("Detailed Syscall Interception Report", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
                    Line::from(""),
                    Line::from(vec![Span::raw("Event Number:  "), Span::styled(event.id.to_string(), Style::default().fg(Color::Cyan))]),
                    Line::from(vec![Span::raw("Time Offset:   "), Span::styled(format!("{} ns", event.timestamp_ns), Style::default().fg(Color::Cyan))]),
                    Line::from(vec![Span::raw("Syscall Type:  "), Span::styled(format!("{:?}", event.syscall), Style::default().fg(Color::Cyan))]),
                ];
                if let Some(fd) = event.fd {
                    detailed_lines.push(Line::from(vec![Span::raw("File Desc:     "), Span::styled(fd.to_string(), Style::default().fg(Color::Cyan))]));
                }
                detailed_lines.push(Line::from(""));
                detailed_lines.push(Line::from(vec![Span::styled("Return Payload Bytes:", Style::default().add_modifier(Modifier::BOLD))]));
                
                if event.return_bytes.is_empty() {
                    detailed_lines.push(Line::from(Span::styled("  <no payload bytes returned>", Style::default().fg(Color::DarkGray))));
                } else {
                    for chunk in event.return_bytes.chunks(16) {
                        let hex_str: String = chunk.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
                        let ascii_str: String = chunk.iter().map(|&b| {
                            if (b.is_ascii_graphic() || b == b' ') && b != b'\n' && b != b'\r' {
                                b as char
                            } else {
                                '.'
                            }
                        }).collect();
                        detailed_lines.push(Line::from(format!("  {:48} | {}", hex_str, ascii_str)));
                    }
                }
                
                let detail_paragraph = Paragraph::new(detailed_lines).block(detail_block).wrap(Wrap { trim: true });
                frame.render_widget(detail_paragraph, popup_area);
            }
        }

        if let Some(pending) = &self.pending_seek {
            if pending.requested_at.elapsed() > Duration::from_millis(100) {
                let popup_area = centered_rect(40, 20, size);
                frame.render_widget(Clear, popup_area);

                let loading_block = Block::default()
                    .borders(Borders::ALL)
                    .title(" Loading ");
                let loading_text = vec![
                    Line::from(""),
                    Line::from(Span::styled("      Reconstructing Memory...", Style::default().add_modifier(Modifier::BOLD))),
                    Line::from(Span::styled(format!("       Seeking to Step {}...", pending.step), Style::default().fg(Color::Yellow))),
                ];
                let loading_paragraph = Paragraph::new(loading_text).block(loading_block);
                frame.render_widget(loading_paragraph, popup_area);
            }
        }

        if let Some(err_msg) = &self.error_message {
            let popup_area = centered_rect(60, 35, size);
            frame.render_widget(Clear, popup_area);

            let error_block = Block::default()
                .borders(Borders::ALL)
                .title(" Error ")
                .style(Style::default().fg(Color::Red));
            
            let is_unsupported = err_msg.contains("Unsupported") || err_msg.contains("supported on Linux");
            let title = if is_unsupported {
                "  Replay Unsupported on this Platform"
            } else {
                "  Error Replaying Trace"
            };

            let error_text = vec![
                Line::from(""),
                Line::from(Span::styled(title, Style::default().add_modifier(Modifier::BOLD).fg(Color::Red))),
                Line::from(""),
                Line::from(Span::styled(format!("  {}", err_msg), Style::default().fg(Color::White))),
                Line::from(""),
                Line::from(Span::styled("  Press ESC or Q to exit", Style::default().fg(Color::DarkGray))),
            ];
            let error_paragraph = Paragraph::new(error_text).block(error_block).wrap(Wrap { trim: true });
            frame.render_widget(error_paragraph, popup_area);
        }
    }
}

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

fn format_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "<empty>".to_string();
    }
    let hex_part: String = bytes.iter().take(8).map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
    let ascii_part: String = bytes.iter().take(8).map(|&b| {
        if (b.is_ascii_graphic() || b == b' ') && b != b'\n' && b != b'\r' {
            b as char
        } else {
            '.'
        }
    }).collect();
    if bytes.len() > 8 {
        format!("{}... | {}...", hex_part, ascii_part)
    } else {
        format!("{} | {}", hex_part, ascii_part)
    }
}
