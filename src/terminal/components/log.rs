use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, List, ListItem},
    Frame,
};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::rc::Rc;
use std::cell::RefCell;
use textwrap::wrap;
use rayon::prelude::*;

use crate::domain::service::Service;
use crate::terminal::app::{Actions, AppEvent};
use crate::usecases::services_manager::ServicesManager;

fn render_loading(frame: &mut Frame, area: Rect) {
    let block = Block::default().borders(Borders::ALL);

    frame.render_widget(block.clone(), area);

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Length(1),
            Constraint::Percentage(45),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
        ])
        .split(vertical[1]);

    let loading = Paragraph::new("Loading...").alignment(Alignment::Center);

    frame.render_widget(loading, horizontal[1]);
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::test_support::{service, FakeRepository};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::sync::mpsc;

    fn log_with(repository: FakeRepository) -> (ServiceLog, mpsc::Receiver<AppEvent>) {
        let (sender, receiver) = mpsc::channel();
        let manager = ServicesManager::new(Box::new(repository));
        (ServiceLog::new(sender, Rc::new(RefCell::new(manager))), receiver)
    }

    fn rendered_text(log: &mut ServiceLog, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| log.render(frame, frame.area())).unwrap();
        terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect()
    }

    #[test]
    fn empty_log_renders_loading_state() {
        let (mut log, _receiver) = log_with(FakeRepository::default());

        assert!(rendered_text(&mut log, 40, 8).contains("Loading..."));
    }

    #[test]
    fn renders_service_name_and_latest_log_lines() {
        let (mut log, _receiver) = log_with(FakeRepository::default());
        log.update("demo.service".into(), "first\nsecond\nthird".into());

        let screen = rendered_text(&mut log, 40, 5);

        assert!(screen.contains("demo.service log"));
        assert!(screen.contains("first"));
        assert!(screen.contains("third"));
    }

    #[test]
    fn fetch_dispatches_log_update_event() {
        let fake = FakeRepository::with_content("journal output", "", 0);
        let observer = fake.clone();
        let (mut log, receiver) = log_with(fake);
        let unit = service("demo.service", "active", "enabled");

        log.fetch_log_and_dispatch(&unit);

        assert!(matches!(
            receiver.recv().unwrap(),
            AppEvent::Action(Actions::Updatelog((name, content)))
                if name == "demo.service" && content == "journal output"
        ));
        assert_eq!(observer.calls(), ["log:demo.service"]);
    }

    #[test]
    fn auto_refresh_changes_border_and_shortcut_label() {
        let (mut log, _receiver) = log_with(FakeRepository::default());

        log.set_auto_refresh(true);

        assert!(matches!(log.border_color, BorderColor::Orange));
        let shortcuts = log.shortcuts();
        assert!(shortcuts.iter().any(|line| line.to_string().contains("Disable auto-refresh")));
        log.set_auto_refresh(false);
    }

    #[test]
    fn auto_refresh_state_is_visible_in_rendered_border_color() {
        let (mut log, _receiver) = log_with(FakeRepository::default());
        log.update("demo.service".into(), "journal output".into());
        log.set_auto_refresh(true);
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| log.render(frame, frame.area())).unwrap();

        assert_eq!(
            terminal.backend().buffer().content()[0].fg,
            Color::Rgb(255, 165, 0)
        );
        log.set_auto_refresh(false);
    }

    #[test]
    fn scrolling_changes_the_visible_log_window() {
        let (mut log, _receiver) = log_with(FakeRepository::default());
        log.update(
            "demo.service".into(),
            "line1\nline2\nline3\nline4\nline5\nline6".into(),
        );

        let latest = rendered_text(&mut log, 40, 4);
        assert!(latest.contains("line5"));
        assert!(latest.contains("line6"));
        assert!(!latest.contains("line4"));

        log.on_key_event(KeyEvent::new(
            KeyCode::Up,
            crossterm::event::KeyModifiers::NONE,
        ));
        let older = rendered_text(&mut log, 40, 4);
        assert!(older.contains("line4"));
        assert!(older.contains("line5"));
        assert!(!older.contains("line6"));
    }

    #[test]
    fn leaving_log_resets_state_and_returns_to_list() {
        let (mut log, receiver) = log_with(FakeRepository::default());
        log.update("demo.service".into(), "journal output".into());
        log.set_auto_refresh(true);
        log.scroll = 10;

        log.on_key_event(KeyEvent::new(
            KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));

        assert!(log.log.is_empty());
        assert_eq!(log.scroll, 0);
        assert!(!*log.auto_refresh.lock().unwrap());
        assert!(matches!(
            receiver.recv().unwrap(),
            AppEvent::Action(Actions::GoList)
        ));
    }
}

enum BorderColor {
    White,
    Orange,
}

impl BorderColor {
    fn to_color(&self) -> Color {
        match self {
            BorderColor::White => Color::White,
            BorderColor::Orange => Color::Rgb(255, 165, 0),
        }
    }
}

pub struct ServiceLog {
    border_color: BorderColor,
    service_name: String,
    scroll: u16,
    sender: Sender<AppEvent>,
    auto_refresh: Arc<Mutex<bool>>,
    usecase: Rc<RefCell<ServicesManager>>,
    log: String,
}

impl ServiceLog {
    pub fn new(sender: Sender<AppEvent>,  usecase: Rc<RefCell<ServicesManager>>) -> Self {
        Self {
            border_color: BorderColor::White,
            service_name: String::new(),
            scroll: 0,
            sender,
            auto_refresh: Arc::new(Mutex::new(false)),
            usecase,
            log: String::new(),
        }
    }


    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if self.log.is_empty()  {
            render_loading(frame, area);
            return;
        }

        let width = area.width.saturating_sub(2) as usize;

        let log_lines: Vec<ListItem> = self
            .log
            .lines()
            .flat_map(|line| {
                wrap(line,width)
                    .into_par_iter()
                    .map(|wrapped| ListItem::new(Span::raw(wrapped.into_owned())))
                    .collect::<Vec<_>>()
            })
            .collect();

        let total_lines = log_lines.len();
        let height = area.height.saturating_sub(2) as usize;

        let start = total_lines
            .saturating_sub(height + self.scroll as usize);
        let end = (start + height).min(total_lines);

        let log_lines: Vec<ListItem> = log_lines[start..end].to_vec();

        let log_list = 
            List::new(log_lines)
                .block(
                    Block::default()
                        .title(format!(" {} log ", self.service_name))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(self.border_color.to_color()))
                        .title_alignment(Alignment::Center),
                );

        frame.render_widget(log_list, area);
    }

    fn toogle_auto_refresh(&mut self) {
        let new_value = {
            if let Ok(auto) = self.auto_refresh.lock() {
                !*auto
            } else {
                return;
            }
        };

        self.set_auto_refresh(new_value);
    }

    fn set_auto_refresh(&mut self, value: bool) {
        self.border_color = if value {
            BorderColor::Orange
        } else {
            BorderColor::White
        };

        if let Ok(mut auto) = self.auto_refresh.lock() {
            *auto = value;
        }
    }

    pub fn on_key_event(&mut self, key: KeyEvent) {
        let right_keys = [KeyCode::Right, KeyCode::Char('l')];
        let left_keys = [KeyCode::Left, KeyCode::Char('h')];
        let up_keys = [KeyCode::Up, KeyCode::Char('k')];
        let down_keys = [KeyCode::Down, KeyCode::Char('j')];

        match key.code {
            code if right_keys.contains(&code) => {
                self.reset();
                self.sender.send(AppEvent::Action(Actions::GoDetails)).unwrap();
            }
            code if left_keys.contains(&code) => {
                self.reset();
                self.sender.send(AppEvent::Action(Actions::GoDetails)).unwrap();
            }
            code if up_keys.contains(&code) => {
                self.scroll = self.scroll.saturating_add(1);
            }
            code if down_keys.contains(&code) => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_add(10);
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_sub(10);
            }
            KeyCode::Char('a') => {
                self.toogle_auto_refresh();
                self.auto_refresh_thread();
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.reset();
                self.exit();
            }
            _ => {}
        }
    }

    pub fn shortcuts(&self) -> Vec<Line<'_>> {
        let is_refreshing = self.auto_refresh.lock().map(|r| *r).unwrap_or(false);
        let mut auto_refresh_label = "Enable auto-refresh";
        if is_refreshing {
            auto_refresh_label = "Disable auto-refresh";
        }

        let help_text = vec![
            Line::from(vec![Span::styled(
                "Actions",
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(format!(
                "Scroll: ↑/↓ | Switch tabs: ←/→ | {auto_refresh_label}: a | Go back: q/Esc",
            )),
        ];

        help_text
    }

    pub fn reset(&mut self) {
        self.set_auto_refresh(false);
        self.scroll = 0;
        self.log = String::new();
    }

    fn exit(&mut self) {
        self.sender.send(AppEvent::Action(Actions::GoList)).unwrap();
    }

    pub fn auto_refresh_thread(&mut self) {
        let auto_refresh = Arc::clone(&self.auto_refresh);
        let sender = self.sender.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(1000));
                if let Ok(is_active) = auto_refresh.lock() {
                    if *is_active {
                        sender.send(AppEvent::Action(Actions::RefreshLog)).unwrap();
                    } else {
                        break;
                    }
                }
            }
        });
    }

    pub fn fetch_log_and_dispatch(&mut self, service: &Service) {
        let event_tx = self.sender.clone();
        if let Ok(log) = self.usecase.borrow().get_log(service) {
            event_tx
                .send(AppEvent::Action(Actions::Updatelog((
                    service.name().to_string(),
                    log,
                ))))
                .expect("Failed to send Updatelog event");
        }
    }

    pub fn update(&mut self, service_name: String, log: String) {
        self.service_name = service_name;
        self.log = log;
    }

}
