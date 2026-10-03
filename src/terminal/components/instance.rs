use crate::domain::service::Service;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap},
};

pub enum PromptResult {
    Editing,
    Cancelled,
    Submitted(Service),
}

pub struct InstancePrompt {
    template: Service,
    action: &'static str,
    input: String,
    cursor: usize,
    error: Option<String>,
}

impl InstancePrompt {
    pub fn new(template: Service, action: &'static str) -> Self {
        Self {
            template,
            action,
            input: String::new(),
            cursor: 0,
            error: None,
        }
    }

    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .nth(self.cursor)
            .map_or(self.input.len(), |(index, _)| index)
    }

    pub fn on_key(&mut self, key: KeyEvent) -> PromptResult {
        if key.kind == KeyEventKind::Release {
            return PromptResult::Editing;
        }
        match key.code {
            KeyCode::Esc => return PromptResult::Cancelled,
            KeyCode::Enter => match self.template.instantiate(&self.input) {
                Ok(service) => return PromptResult::Submitted(service),
                Err(error) => self.error = Some(error),
            },
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.input.chars().count()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.input.chars().count(),
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.input.remove(self.byte_index());
                self.error = None;
            }
            KeyCode::Delete if self.cursor < self.input.chars().count() => {
                self.input.remove(self.byte_index());
                self.error = None;
            }
            KeyCode::Char(c)
                if !c.is_control()
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && self.input.len() < 255 =>
            {
                self.input.insert(self.byte_index(), c);
                self.cursor += 1;
                self.error = None;
            }
            _ => {}
        }
        PromptResult::Editing
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let width = area.width.saturating_sub(4).min(80);
        let target = self.template.instantiate(&self.input).map_or_else(
            |_| "Enter a valid instance name".into(),
            |service| service.name().to_string(),
        );
        let content_width = usize::from(width.saturating_sub(4).max(1));
        let lines = |text: &str| textwrap::wrap(text, content_width).len() as u16;
        let template_lines = lines(&format!("Template: {}", self.template.name()));
        let target_lines = lines(&format!("Target: {target}"));
        let error_lines = self.error.as_deref().map_or(0, lines);
        let height = area
            .height
            .saturating_sub(4)
            .min(2 + template_lines + 3 + target_lines + error_lines + 2);
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        frame.render_widget(Clear, popup);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::new(1, 1, 0, 0))
            .title(format!("{} instance", self.action))
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        let [template, input, preview, error, _spacing, help] = Layout::vertical([
            Constraint::Length(template_lines),
            Constraint::Length(3),
            Constraint::Length(target_lines),
            Constraint::Length(error_lines),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    "Template: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(self.template.name()),
            ]))
            .wrap(Wrap { trim: false }),
            template,
        );
        let input_block = Block::default()
            .borders(Borders::ALL)
            .title("Instance name");
        let input_inner = input_block.inner(input);
        let cursor_width = Line::from(&self.input[..self.byte_index()]).width() as u16;
        let scroll = cursor_width.saturating_sub(input_inner.width.saturating_sub(1));
        frame.render_widget(
            Paragraph::new(self.input.as_str())
                .style(Style::default().fg(Color::Yellow))
                .block(input_block)
                .scroll((0, scroll)),
            input,
        );
        if input_inner.width > 0 && input_inner.height > 0 {
            frame.set_cursor_position((input_inner.x + cursor_width - scroll, input_inner.y));
        }
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    "Target: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(target),
            ]))
            .wrap(Wrap { trim: false }),
            preview,
        );
        frame.render_widget(
            Paragraph::new(self.error.as_deref().unwrap_or(""))
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: false }),
            error,
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": confirm | "),
                Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(": cancel"),
            ])),
            help,
        );
    }
}

#[cfg(test)]
mod tests;
