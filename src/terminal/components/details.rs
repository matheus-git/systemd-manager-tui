use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use crossterm::event::{KeyCode, KeyEvent};

use crate::domain::service::Service;
use crate::terminal::app::{Actions, AppEvent};

pub struct ServiceDetails {
    service: Option<Arc<Mutex<Service>>>,
    unit_file: String,
    sender: Sender<AppEvent>,
    scroll: u16,
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::test_support::service;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::sync::mpsc;

    fn details() -> (ServiceDetails, mpsc::Receiver<AppEvent>) {
        let (sender, receiver) = mpsc::channel();
        (ServiceDetails::new(sender), receiver)
    }

    fn rendered_text(details: &mut ServiceDetails, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| details.render(frame, frame.area())).unwrap();
        terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect()
    }

    #[test]
    fn renders_a_loaded_unit_file() {
        let (mut details, _receiver) = details();
        details.update(service("demo.service", "active", "enabled"));
        details.unit_file = "[Unit]\nDescription=Demo\n# comment".into();
        let screen = rendered_text(&mut details, 60, 8);

        assert!(screen.contains("demo.service file"));
        assert!(screen.contains("[Unit]"));
        assert!(screen.contains("Description=Demo"));
        assert!(screen.contains("# comment"));
    }

    #[test]
    fn navigation_updates_scroll_and_emits_actions() {
        let (mut details, receiver) = details();
        details.on_key_event(KeyEvent::new(KeyCode::PageDown, crossterm::event::KeyModifiers::NONE));
        assert_eq!(details.scroll, 10);

        details.on_key_event(KeyEvent::new(KeyCode::Char('e'), crossterm::event::KeyModifiers::NONE));
        assert!(matches!(receiver.recv().unwrap(), AppEvent::Action(Actions::EditCurrentService)));
    }

    #[test]
    fn selecting_another_service_clears_previous_content_and_scroll() {
        let (mut details, _receiver) = details();
        details.update_unit_file(
            service("old.service", "active", "enabled"),
            "[Service]\nExecStart=/old".into(),
        );
        details.scroll = 10;

        details.update(service("current.service", "active", "enabled"));

        assert!(details.unit_file.is_empty());
        assert_eq!(details.scroll, 0);
        let screen = rendered_text(&mut details, 60, 8);
        assert!(screen.contains("current.service file"));
        assert!(!screen.contains("ExecStart=/old"));
    }
}

impl ServiceDetails {
    pub fn new(sender: Sender<AppEvent>) -> Self {
        Self {
            service: None,
            sender,
            unit_file: String::new(),
            scroll: 0,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if let Some(service_arc) = &self.service {
            let service = service_arc.lock().unwrap();
            let paragraph = self.generate_styled_unit_file_paragraph();
            let paragraph = paragraph
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(" {} file ", service.name()))
                        .title_alignment(Alignment::Center),
                )
                .scroll((self.scroll, 0))
                .wrap(Wrap { trim: true });

            frame.render_widget(paragraph, area);
        }
    }

    fn generate_styled_unit_file_paragraph(&self) -> Paragraph<'_> {
        let mut lines: Vec<Line<'_>> = vec![];
        for line in self.unit_file.lines() {
            let line = line.trim();

            if line.is_empty() {
                lines.push(Line::raw(""));
            } else if line.starts_with('#') || line.starts_with(';') {
                lines.push(Line::styled(line, Style::default().fg(Color::Green)));
            } else if line.starts_with('[') && line.ends_with(']') {
                lines.push(Line::styled(line, Style::default().fg(Color::LightBlue)));
            } else if let Some((key, value)) = line.split_once('=') {
                lines.push(Line::from(vec![
                    Span::styled(format!("{key}="), Style::default().fg(Color::Yellow)),
                    Span::raw(value),
                ]));
            } else {
                lines.push(Line::styled(line, Style::default()));
            }
        }
        Paragraph::new(lines)
    }

    pub fn on_key_event(&mut self, key: KeyEvent) {
        let right_keys = [KeyCode::Right, KeyCode::Char('l')];
        let left_keys = [KeyCode::Left, KeyCode::Char('h')];
        let up_keys = [KeyCode::Up, KeyCode::Char('k')];
        let down_keys = [KeyCode::Down, KeyCode::Char('j')];

        match key.code {
            code if right_keys.contains(&code) => {
                self.reset();
                self.sender.send(AppEvent::Action(Actions::GoLog)).unwrap();
            }
            code if left_keys.contains(&code) => {
                self.reset();
                self.sender.send(AppEvent::Action(Actions::GoLog)).unwrap();
            }
            code if up_keys.contains(&code) => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            code if down_keys.contains(&code) => {
                self.scroll += 1;
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.scroll += 10;
            }
            KeyCode::Char('e') => {
                self.sender.send(AppEvent::Action(Actions::EditCurrentService)).unwrap();
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.exit();
            }
            _ => {}
        }
    }
    
    #[allow(clippy::unused_self)]
    pub fn shortcuts(&self) -> Vec<Line<'_>> {
        let help_text = vec![
            Line::from(vec![Span::styled(
                "Actions",
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("Switch tabs: ←/→ | Edit: e | Go back: q/Esc"),
        ];

        help_text
    }

    pub fn reset(&mut self) {
        self.service = None;
        self.scroll = 0;
        self.unit_file = String::new();
    }

    fn exit(&mut self) {
        self.reset();
        self.sender.send(AppEvent::Action(Actions::GoList)).unwrap();
    }

    pub fn update(&mut self, service: Service) {
        self.service = Some(Arc::new(Mutex::new(service)));
        self.unit_file.clear();
        self.scroll = 0;
    }

    pub fn update_unit_file(&mut self, service: Service, unit_file: String) {
        self.update(service);
        self.unit_file = unit_file;
    }
}
