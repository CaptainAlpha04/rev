use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};

pub struct TimelineScrubber {
    current_step: u64,
    total_steps: u64,
}

impl TimelineScrubber {
    pub fn new(current_step: u64, total_steps: u64) -> Self {
        Self {
            current_step,
            total_steps,
        }
    }

    /// Calculate the column for a given step index within the available width.
    pub fn calculate_col(step: u64, total_steps: u64, available_width: usize) -> usize {
        if available_width == 0 {
            return 0;
        }
        if total_steps <= 1 {
            return 0;
        }
        if total_steps <= available_width as u64 {
            return step as usize;
        }
        // Sampled scaling
        let ratio = step as f64 / (total_steps - 1) as f64;
        let col = (ratio * (available_width - 1) as f64).round() as usize;
        col.min(available_width - 1)
    }
}

impl Widget for TimelineScrubber {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Timeline Scrubber ");
        let inner_area = block.inner(area);
        block.render(area, buf);

        if inner_area.width == 0 || inner_area.height == 0 {
            return;
        }

        let width = inner_area.width as usize;
        let total_steps = self.total_steps;
        let current_step = self.current_step;

        // Draw background line for the scrubber
        for x in 0..inner_area.width {
            buf[(inner_area.x + x, inner_area.y)]
                .set_char('·')
                .set_style(Style::default().fg(Color::DarkGray));
        }

        // Draw ticks or marks
        if total_steps > 0 {
            if total_steps <= width as u64 {
                // Draw every step
                for step in 0..total_steps {
                    let x = inner_area.x + step as u16;
                    let style = if step == current_step {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray)
                    };
                    let char_to_draw = if step == current_step { '●' } else { '•' };
                    buf[(x, inner_area.y)].set_char(char_to_draw).set_style(style);
                }
            } else {
                // Sampled rendering
                // Draw dots representing steps
                for col in 0..width {
                    let x = inner_area.x + col as u16;
                    buf[(x, inner_area.y)]
                        .set_char('•')
                        .set_style(Style::default().fg(Color::DarkGray));
                }

                // Highlight current step column
                let cur_col = Self::calculate_col(current_step, total_steps, width);
                let x = inner_area.x + cur_col as u16;
                buf[(x, inner_area.y)]
                    .set_char('●')
                    .set_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
            }

            // Draw caret and step text below the line if there's height
            if inner_area.height >= 2 {
                let cur_col = Self::calculate_col(current_step, total_steps, width);
                let caret_x = inner_area.x + cur_col as u16;
                buf[(caret_x, inner_area.y + 1)]
                    .set_char('▲')
                    .set_style(Style::default().fg(Color::Yellow));

                // If height is 3 or more, print the step numbers underneath the scrubber
                if inner_area.height >= 3 {
                    let label = format!("Step {} / {}", current_step, total_steps.saturating_sub(1));
                    let label_len = label.len() as u16;
                    let label_x = if caret_x + label_len / 2 >= inner_area.x + inner_area.width {
                        (inner_area.x + inner_area.width).saturating_sub(label_len)
                    } else if caret_x >= inner_area.x + label_len / 2 {
                        caret_x - label_len / 2
                    } else {
                        inner_area.x
                    };
                    buf.set_string(label_x, inner_area.y + 2, &label, Style::default().fg(Color::Cyan));
                }
            }
        } else {
            buf.set_string(inner_area.x, inner_area.y, "No events recorded", Style::default().fg(Color::Red));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_col() {
        assert_eq!(TimelineScrubber::calculate_col(0, 10, 5), 0);
        assert_eq!(TimelineScrubber::calculate_col(9, 10, 5), 4);
        assert_eq!(TimelineScrubber::calculate_col(4, 10, 5), 2); // 4 / 9 * 4 = 1.77 -> rounded to 2
        assert_eq!(TimelineScrubber::calculate_col(5, 10, 5), 2); // 5 / 9 * 4 = 2.22 -> rounded to 2
        assert_eq!(TimelineScrubber::calculate_col(0, 0, 5), 0);
        assert_eq!(TimelineScrubber::calculate_col(0, 1, 5), 0);
        assert_eq!(TimelineScrubber::calculate_col(0, 10, 0), 0);
    }
}
