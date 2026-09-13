use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

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
mod tests;

impl ServiceDetails {
    pub fn new(sender: Sender<AppEvent>, usecase: Rc<RefCell<ServicesManager>>) -> Self {
        Self {
            service: None,
            sender,
            unit_file: String::new(),
            scroll: 0,
            usecase,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if let Some(service_arc) = &self.service {
            let service = match service_arc.lock() {
                Ok(service) => service,
                Err(error) => {
                    let _ = self.sender.send(AppEvent::Error(format!(
                        "Could not render service details: {error}"
                    )));
                    return;
                }
            };
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
                let _ = self.sender.send(AppEvent::Action(Actions::GoLog));
            }
            code if left_keys.contains(&code) => {
                self.reset();
                let _ = self.sender.send(AppEvent::Action(Actions::GoLog));
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
                let _ = self
                    .sender
                    .send(AppEvent::Action(Actions::EditCurrentService));
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
        let _ = self.sender.send(AppEvent::Action(Actions::GoList));
    }

    pub fn fetch_unit_file(&mut self) {
        let maybe_service = self.service.clone();

        if let Some(service_arc) = maybe_service {
            let service = match service_arc.lock() {
                Ok(service) => service,
                Err(error) => {
                    let _ = self.sender.send(AppEvent::Error(format!(
                        "Could not read the selected service: {error}"
                    )));
                    return;
                }
            };

            let result = self.usecase.borrow().systemctl_cat(&service);

            match result {
                Ok(content) => {
                    self.unit_file = content;
                }
                Err(e) => {
                    let _ = self.sender.send(AppEvent::Error(e.to_string()));
                }
            }
        }
    }

    pub fn update(&mut self, service: Service) {
        self.service = Some(Arc::new(Mutex::new(service)));
    }
}
