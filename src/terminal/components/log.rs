use crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use rayon::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use textwrap::wrap;

use crate::keybindings::{Action, Context, KeyBindings};
use crate::terminal::app::{Actions, AppEvent, ServiceRequestContext};
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
mod tests;

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
    auto_refresh_worker: Option<LogRefreshWorker>,
    usecase: Rc<RefCell<ServicesManager>>,
    log: String,
    keybindings: Arc<KeyBindings>,
}

struct LogRefreshWorker {
    stop_tx: Sender<()>,
    handle: JoinHandle<()>,
}

impl ServiceLog {
    pub fn new(
        sender: Sender<AppEvent>,
        usecase: Rc<RefCell<ServicesManager>>,
        keybindings: Arc<KeyBindings>,
    ) -> Self {
        Self {
            border_color: BorderColor::White,
            service_name: String::new(),
            scroll: 0,
            sender,
            auto_refresh: Arc::new(Mutex::new(false)),
            auto_refresh_worker: None,
            usecase,
            log: String::new(),
            keybindings,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if self.log.is_empty() {
            render_loading(frame, area);
            return;
        }

        let width = area.width.saturating_sub(2) as usize;

        let log_lines: Vec<ListItem> = self
            .log
            .lines()
            .flat_map(|line| {
                wrap(line, width)
                    .into_par_iter()
                    .map(|wrapped| ListItem::new(Span::raw(wrapped.into_owned())))
                    .collect::<Vec<_>>()
            })
            .collect();

        let total_lines = log_lines.len();
        let height = area.height.saturating_sub(2) as usize;

        let start = total_lines.saturating_sub(height + self.scroll as usize);
        let end = (start + height).min(total_lines);

        let log_lines: Vec<ListItem> = log_lines[start..end].to_vec();

        let log_list = List::new(log_lines).block(
            Block::default()
                .title(format!(" {} log ", self.service_name))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.border_color.to_color()))
                .title_alignment(Alignment::Center),
        );

        frame.render_widget(log_list, area);
    }

    fn toggle_auto_refresh(&mut self) {
        if self.auto_refresh_worker.is_some() {
            self.stop_auto_refresh();
        } else {
            self.start_auto_refresh();
        }
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
        match self.keybindings.action(Context::Log, key) {
            Some(Action::NavLeft | Action::NavRight) => {
                self.reset();
                let _ = self.sender.send(AppEvent::Action(Actions::GoDetails));
            }
            Some(Action::MoveUp) => {
                self.scroll = self.scroll.saturating_add(1);
            }
            Some(Action::MoveDown) => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            Some(Action::PageUp) => {
                self.scroll = self.scroll.saturating_add(10);
            }
            Some(Action::PageDown) => {
                self.scroll = self.scroll.saturating_sub(10);
            }
            Some(Action::ToggleAutoRefresh) => {
                self.toggle_auto_refresh();
            }
            Some(Action::Back) => {
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

        let keys = &self.keybindings;
        let help_text = vec![
            Line::from(vec![Span::styled(
                "Actions",
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(format!(
                "Scroll: {}/{} | Switch tabs: {}/{} | {auto_refresh_label}: {} | {}: {}",
                keys.primary_label(Context::Log, Action::MoveUp),
                keys.primary_label(Context::Log, Action::MoveDown),
                keys.primary_label(Context::Log, Action::NavLeft),
                keys.primary_label(Context::Log, Action::NavRight),
                keys.label(Context::Log, Action::ToggleAutoRefresh),
                Action::Back.title(),
                keys.label(Context::Log, Action::Back),
            )),
        ];

        help_text
    }

    pub fn reset(&mut self) {
        self.stop_auto_refresh();
        self.scroll = 0;
        self.log = String::new();
    }

    fn exit(&mut self) {
        let _ = self.sender.send(AppEvent::Action(Actions::GoList));
    }

    fn start_auto_refresh(&mut self) {
        if self.auto_refresh_worker.is_some() {
            return;
        }

        self.set_auto_refresh(true);
        let sender = self.sender.clone();
        let (stop_tx, stop_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            loop {
                match stop_rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {
                        if sender.send(AppEvent::Action(Actions::RefreshLog)).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        self.auto_refresh_worker = Some(LogRefreshWorker { stop_tx, handle });
    }

    fn stop_auto_refresh(&mut self) {
        self.set_auto_refresh(false);
        if let Some(worker) = self.auto_refresh_worker.take() {
            let _ = worker.stop_tx.send(());
            let _ = worker.handle.join();
        }
    }

    pub fn fetch_log_and_dispatch(&mut self, context: ServiceRequestContext) {
        let event_tx = self.sender.clone();
        match self.usecase.borrow().get_log(&context.service_name) {
            Ok(log) => {
                let _ = event_tx.send(AppEvent::Action(Actions::UpdateLog(context, log)));
            }
            Err(error) => {
                let _ = event_tx.send(AppEvent::Error(format!(
                    "Could not load logs for '{}': {error}",
                    context.service_name
                )));
            }
        }
    }

    pub fn update(&mut self, service_name: String, log: String) {
        self.service_name = service_name;
        self.log = log;
    }
}

impl Drop for ServiceLog {
    fn drop(&mut self) {
        self.stop_auto_refresh();
    }
}
