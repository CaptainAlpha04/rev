use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};
use rev_core::types::Variable;

pub fn render_variables<'a>(variables: &'a [Variable], block: Block<'a>) -> Paragraph<'a> {
    let mut lines = Vec::new();

    if variables.is_empty() {
        lines.push(Line::from(Span::styled(
            " No variables in scope",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        )));
    } else {
        for var in variables {
            let mut line_spans = Vec::new();

            // Changed marker
            if var.is_changed {
                line_spans.push(Span::styled("● ", Style::default().fg(Color::Yellow)));
            } else {
                line_spans.push(Span::raw("  "));
            }

            // Variable Name
            line_spans.push(Span::styled(
                format!("{}: ", var.name),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));

            // Variable Type
            line_spans.push(Span::styled(
                format!("({}) ", var.type_name),
                Style::default().fg(Color::DarkGray),
            ));

            // Variable Value
            let val_str = match &var.value {
                serde_json::Value::String(s) => format!("\"{}\"", s),
                other => other.to_string(),
            };
            line_spans.push(Span::styled(val_str, Style::default().fg(Color::Green)));

            lines.push(Line::from(line_spans));
        }
    }

    Paragraph::new(lines).block(block).wrap(Wrap { trim: true })
}
