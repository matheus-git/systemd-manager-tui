use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::rc::Rc;
use std::cell::RefCell;

use crossterm::event::{KeyCode, KeyEvent};

use crate::domain::service::Service;
use crate::terminal::app::{Actions, AppEvent};
use crate::usecases::services_manager::ServicesManager;

pub struct ServiceDetails {
    service: Option<Arc<Mutex<Service>>>,
    unit_file: String,
    sender: Sender<AppEvent>,
    scroll: u16,
    usecase: Rc<RefCell<ServicesManager>>,
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::test_support::{service, FakeRepository};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::sync::mpsc;

    fn details_with(repository: FakeRepository) -> (ServiceDetails, mpsc::Receiver<AppEvent>) {
        let (sender, receiver) = mpsc::channel();
        let manager = ServicesManager::new(Box::new(repository));
        (ServiceDetails::new(sender, Rc::new(RefCell::new(manager))), receiver)
    }

    fn rendered_text(details: &mut ServiceDetails, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| details.render(frame, frame.area())).unwrap();
        terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect()
    }

    #[test]
    fn fetches_and_renders_a_unit_file() {
        let fake = FakeRepository::with_content("", "[Unit]\nDescription=Demo\n# comment", 0);
        let observer = fake.clone();
        let (mut details, _receiver) = details_with(fake);
        details.update(service("demo.service", "active", "enabled"));

        details.fetch_unit_file();
        let screen = rendered_text(&mut details, 60, 8);

        assert!(screen.contains("demo.service file"));
        assert!(screen.contains("[Unit]"));
        assert!(screen.contains("Description=Demo"));
        assert!(screen.contains("# comment"));
        assert_eq!(observer.calls(), ["cat:demo.service"]);
    }

    #[test]
    fn reports_repository_error_as_app_event() {
        let fake = FakeRepository::default();
        fake.fail("cat");
        let (mut details, receiver) = details_with(fake);
        details.update(service("broken.service", "active", "enabled"));

        details.fetch_unit_file();

        assert!(matches!(receiver.recv().unwrap(), AppEvent::Error(message) if message == "cat failed"));
    }

    #[test]
    fn navigation_updates_scroll_and_emits_actions() {
        let (mut details, receiver) = details_with(FakeRepository::default());
        details.on_key_event(KeyEvent::new(KeyCode::PageDown, crossterm::event::KeyModifiers::NONE));
        assert_eq!(details.scroll, 10);

        details.on_key_event(KeyEvent::new(KeyCode::Char('e'), crossterm::event::KeyModifiers::NONE));
        assert!(matches!(receiver.recv().unwrap(), AppEvent::Action(Actions::EditCurrentService)));
    }

    #[test]
    fn unit_file_rendering_applies_systemd_syntax_colors() {
        let fake = FakeRepository::with_content(
            "",
            "[Service]\nDescription=Demo\n# comment\n; another comment",
            0,
        );
        let (mut details, _receiver) = details_with(fake);
        details.update(service("demo.service", "active", "enabled"));
        details.fetch_unit_file();
        let backend = TestBackend::new(60, 8);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| details.render(frame, frame.area()))
            .unwrap();

        let cells = terminal.backend().buffer().content();
        assert!(cells
            .iter()
            .any(|cell| cell.symbol() == "[" && cell.fg == Color::LightBlue));
        assert!(cells
            .iter()
            .any(|cell| cell.symbol() == "D" && cell.fg == Color::Yellow));
        assert!(cells
            .iter()
            .any(|cell| cell.symbol() == "#" && cell.fg == Color::Green));
        assert!(cells
            .iter()
            .any(|cell| cell.symbol() == ";" && cell.fg == Color::Green));
    }

    #[test]
    fn leaving_details_clears_content_and_returns_to_list() {
        let fake = FakeRepository::with_content("", "[Service]\nExecStart=/bin/true", 0);
        let (mut details, receiver) = details_with(fake);
        details.update(service("demo.service", "active", "enabled"));
        details.fetch_unit_file();
        details.scroll = 12;

        details.on_key_event(KeyEvent::new(
            KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));

        assert!(details.service.is_none());
        assert!(details.unit_file.is_empty());
        assert_eq!(details.scroll, 0);
        assert!(matches!(
            receiver.recv().unwrap(),
            AppEvent::Action(Actions::GoList)
        ));
    }

    #[test]
    fn switching_from_details_clears_content_and_opens_log() {
        let fake = FakeRepository::with_content("", "old content", 0);
        let (mut details, receiver) = details_with(fake);
        details.update(service("demo.service", "active", "enabled"));
        details.fetch_unit_file();

        details.on_key_event(KeyEvent::new(
            KeyCode::Right,
            crossterm::event::KeyModifiers::NONE,
        ));

        assert!(details.service.is_none());
        assert!(details.unit_file.is_empty());
        assert!(matches!(
            receiver.recv().unwrap(),
            AppEvent::Action(Actions::GoLog)
        ));
    }
}

impl ServiceDetails {
    pub fn new(sender: Sender<AppEvent>,  usecase: Rc<RefCell<ServicesManager>>) -> Self {
        Self {
            service: None,
            sender,
            unit_file: String::new(),
            scroll: 0,
            usecase
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

    pub fn fetch_unit_file(&mut self) {
        let maybe_service = self.service.clone();

        if let Some(service_arc) = maybe_service {
            let service = service_arc.lock().unwrap();

            let result = self.usecase.borrow().systemctl_cat(&service);

            match result {
                Ok(content) => {
                    self.unit_file = content;
                }
                Err(e) => {
                    self.sender.send(AppEvent::Error(e.to_string())).unwrap();
                }
            }
        }
    }

    pub fn update(&mut self, service: Service) {
        self.service = Some(Arc::new(Mutex::new(service)));
    }
}
