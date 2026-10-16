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

use crossterm::event::KeyEvent;

use crate::domain::service::Service;
use crate::keybindings::{Action, Context, KeyBindings};
use crate::terminal::app::{Actions, AppEvent};
use crate::usecases::services_manager::ServicesManager;

pub struct ServiceDetails {
    service: Option<Arc<Mutex<Service>>>,
    unit_file: String,
    sender: Sender<AppEvent>,
    scroll: u16,
    usecase: Rc<RefCell<ServicesManager>>,
    keybindings: Arc<KeyBindings>,
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests;

impl ServiceDetails {
    pub fn new(
        sender: Sender<AppEvent>,
        usecase: Rc<RefCell<ServicesManager>>,
        keybindings: Arc<KeyBindings>,
    ) -> Self {
        Self {
            service: None,
            sender,
            unit_file: String::new(),
            scroll: 0,
            usecase,
            keybindings,
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
        match self.keybindings.action(Context::UnitFile, key) {
            Some(Action::NavLeft | Action::NavRight) => {
                self.reset();
                let _ = self.sender.send(AppEvent::Action(Actions::GoLog));
            }
            Some(Action::MoveUp) => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            Some(Action::MoveDown) => {
                self.scroll += 1;
            }
            Some(Action::PageUp) => {
                self.scroll = self.scroll.saturating_sub(10);
            }
            Some(Action::PageDown) => {
                self.scroll += 10;
            }
            Some(Action::EditUnit) => {
                let _ = self
                    .sender
                    .send(AppEvent::Action(Actions::EditCurrentService));
            }
            Some(Action::Back) => {
                self.exit();
            }
            _ => {}
        }
    }

    pub fn shortcuts(&self) -> Vec<Line<'_>> {
        let keys = &self.keybindings;
        let help_text = vec![
            Line::from(vec![Span::styled(
                "Actions",
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(format!(
                "Switch tabs: {}/{} | {}: {} | {}: {}",
                keys.primary_label(Context::UnitFile, Action::NavLeft),
                keys.primary_label(Context::UnitFile, Action::NavRight),
                Action::EditUnit.title(),
                keys.label(Context::UnitFile, Action::EditUnit),
                Action::Back.title(),
                keys.label(Context::UnitFile, Action::Back),
            )),
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
