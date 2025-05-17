use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::DefaultTerminal;
use ratatui::Frame;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;

use std::cell::RefCell;
use std::rc::Rc;

use crate::usecases::services_manager::ServicesManager;

use super::components::details::ServiceDetails;
use super::components::filter::Filter;
use super::components::list::TableServices;
use super::components::log::ServiceLog;

#[derive(PartialEq)]
enum Status {
    List,
    Log,
    Details,
}

pub enum Actions {
    RefreshLog,
    RefreshDetails,
    GoList,
    GoLog,
    GoDetails,
    Updatelog((String, String)),
    UpdateDetails,
    Filter(String),
    UpdateIgnoreListKeys(bool),
}

pub enum AppEvent {
    Key(KeyEvent),
    Action(Actions),
    Error(String),
}

fn get_user_friendly_error(error: &str) -> &str {
    if error.contains("org.freedesktop.DBus.Error.InteractiveAuthorizationRequired") {
        "You do not have the permission to do that. Try running the program with sudo."
    } else if error.contains("org.freedesktop.DBus.Error.ServiceUnknown") {
        "The requested service is not available or not running."
    } else if error.contains("org.freedesktop.DBus.Error.NoReply") {
        "The service did not respond in time. It might be busy or not functioning properly."
    } else if error.contains("org.freedesktop.DBus.Error.AccessDenied") {
        "Access denied. You don't have sufficient permissions for this operation."
    } else if error.contains("org.freedesktop.systemd1.NoSuchUnit") {
        "The requested service unit doesn't exist."
    } else {
        error
    }
}
fn spawn_key_event_listener(event_tx: Sender<AppEvent>) {
    thread::spawn(move || {
        loop {
            if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(Event::Key(key_event)) = event::read() {
                    if key_event.kind == KeyEventKind::Press
                        && event_tx.send(AppEvent::Key(key_event)).is_err()
                    {
                        break;
                    }
                }
            }
        }
    });
}
pub struct App {
    running: bool,
    status: Status,
    table_service: Rc<RefCell<TableServices>>,
    filter: Rc<RefCell<Filter>>,
    service_log: Rc<RefCell<ServiceLog>>,
    details: Rc<RefCell<ServiceDetails>>,
    usecases: Rc<ServicesManager>,
    event_rx: Receiver<AppEvent>,
    event_tx: Sender<AppEvent>,
}

impl App {
    pub fn new(
        event_tx: Sender<AppEvent>, 
        event_rx: Receiver<AppEvent>, 
        table_service: Rc<RefCell<TableServices>>,
        filter: Rc<RefCell<Filter>>,
        service_log: Rc<RefCell<ServiceLog>>,
        details: Rc<RefCell<ServiceDetails>>,
        usecases: Rc<ServicesManager>
    ) -> Self {
        Self {
            running: true,
            status: Status::List,
            table_service,
            filter,
            service_log,
            details,
            usecases,
            event_rx,
            event_tx,
        }
    }

    pub fn init(&mut self) {
        spawn_key_event_listener(self.event_tx.clone());
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;

        let binding_table_service = self.table_service.clone();
        let mut table_service = binding_table_service.borrow_mut();
        let binding_filter = self.filter.clone();
        let mut filter = binding_filter.borrow_mut();
        let binding_log = self.service_log.clone();
        let mut log = binding_log.borrow_mut();
        let bindind_details = self.details.clone();
        let mut details = bindind_details.borrow_mut();

        while self.running {
            match self.status {
                Status::Log => self.draw_log_status(&mut terminal, &mut log)?,
                Status::List => self.draw_list_status(&mut terminal, &mut filter, &mut table_service)?,
                Status::Details => self.draw_details_status(&mut terminal, &mut details)?,
            }

            match self.event_rx.recv()? {
                AppEvent::Key(key) => match self.status {
                    Status::Log => {
                        self.on_key_event(key);
                        log.on_key_event(key)
                    }
                    Status::List => {
                        self.on_key_event(key);
                        table_service.on_key_event(key);
                        filter.on_key_event(key);
                    }
                    Status::Details => {
                        self.on_key_event(key);
                        details.on_key_event(key);
                    }
                },
                AppEvent::Action(Actions::UpdateIgnoreListKeys(bool)) => {
                    table_service.set_ignore_key_events(bool);
                }
                AppEvent::Action(Actions::Filter(input)) => {
                    table_service.set_selected_index(0);
                    table_service.refresh(input);
                }
                AppEvent::Action(Actions::Updatelog(data)) => {
                    log.update(data.0, data.1);
                }
                AppEvent::Action(Actions::RefreshLog) => {
                    if self.status == Status::Log {
                        if let Some(service) =
                            table_service.get_selected_service()
                        {
                            log
                                .fetch_log_and_dispatch(service.clone());
                        }
                    }
                }
                AppEvent::Action(Actions::GoLog) => {
                    self.status = Status::Log;
                    self.event_tx.send(AppEvent::Action(Actions::RefreshLog))?;
                    log.start_auto_refresh();
                }
                AppEvent::Action(Actions::GoList) => self.status = Status::List,
                AppEvent::Action(Actions::UpdateDetails) => {}
                AppEvent::Action(Actions::RefreshDetails) => {
                    if self.status == Status::Details {
                        details.fetch_log_and_dispatch();
                    }
                }
                AppEvent::Action(Actions::GoDetails) => {
                    if let Some(service) = table_service.get_selected_service() {
                        details.update(service.clone());
                    }
                    self.event_tx
                        .send(AppEvent::Action(Actions::RefreshDetails))?;
                    self.status = Status::Details;
                    details.start_auto_refresh();
                }
                AppEvent::Error(error_msg) => {
                    self.error_popup(&mut terminal, error_msg)?;    
                }
            }
        }

        Ok(())
    }

    fn error_popup(&self, terminal: &mut DefaultTerminal, error_msg: String) -> Result<()> {
        let user_friendly_message = get_user_friendly_error(&error_msg);

        terminal.draw(|frame| {
            let area = frame.area();

            let popup_width = std::cmp::min(70, area.width.saturating_sub(4));
            let popup_height = std::cmp::min(12, area.height.saturating_sub(4));

            let popup_x = (area.width.saturating_sub(popup_width)) / 2;
            let popup_y = (area.height.saturating_sub(popup_height)) / 2;

            let popup_area = Rect::new(
                area.x + popup_x,
                area.y + popup_y,
                popup_width,
                popup_height,
            );

            frame.render_widget(Clear, popup_area);

            let text = vec![
                Line::from(vec![Span::styled(
                    "ERROR",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from(user_friendly_message),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Press any key to dismiss",
                    Style::default().fg(Color::Gray),
                )]),
            ];

            let error_block = Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red))
                        .title("Error"),
                )
                .alignment(Alignment::Center)
                .wrap(ratatui::widgets::Wrap { trim: true });

            frame.render_widget(error_block, popup_area);
        })?;

        if let Ok(Event::Key(_)) = event::read() {
            // Continue after key press
        };
        Ok(())
    }

    fn draw_details_status(
        &mut self,
        terminal: &mut DefaultTerminal,
        service_details: &mut ServiceDetails,
    ) -> Result<()> {
        terminal.draw(|frame| {
            let area = frame.area();

            let [list_box, help_area_box] =
                Layout::vertical([Constraint::Min(0), Constraint::Max(7)]).areas(area);

            service_details.render(frame, list_box);
            self.draw_shortcuts(frame, help_area_box, service_details.shortcuts());
        })?;

        Ok(())
    }

    fn draw_log_status(
        &mut self,
        terminal: &mut DefaultTerminal,
        service_log: &mut ServiceLog,
    ) -> Result<()> {
        terminal.draw(|frame| {
            let area = frame.area();

            let [list_box, help_area_box] =
                Layout::vertical([Constraint::Min(0), Constraint::Max(7)]).areas(area);

            service_log.render(frame, list_box);
            self.draw_shortcuts(frame, help_area_box, service_log.shortcuts());
        })?;

        Ok(())
    }

    fn draw_list_status(
        &mut self,
        terminal: &mut DefaultTerminal,
        filter: &mut Filter,
        table: &mut TableServices,
    ) -> Result<()> {
        terminal.draw(|frame| {
            let area = frame.area();

            let [filter_box, list_box, help_area_box] = Layout::vertical([
                Constraint::Length(4),
                Constraint::Min(10),
                Constraint::Max(7),
            ])
            .areas(area);

            filter.draw(frame, filter_box);
            table.render(frame, list_box);
            self.draw_shortcuts(frame, help_area_box, table.shortcuts());
        })?;

        Ok(())
    }

    fn draw_shortcuts(&mut self, frame: &mut Frame, help_area: Rect, shortcuts: Vec<Line<'_>>) {
        let mut help_text: Vec<Line<'_>> = Vec::new();
        let shortcuts_lens = shortcuts.len();

        help_text.extend(shortcuts);

        if shortcuts_lens > 0 {
            help_text.push(Line::raw(""));
            if help_area.width > 125 {
                help_text.push(Line::raw(""));
            }
        }

        help_text.push(Line::from(vec![
            Span::styled(
                "Exit",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Ctrl + c"),
        ]));

        let help_block = Paragraph::new(help_text)
            .block(Block::default().title("Shortcuts").borders(Borders::ALL))
            .wrap(ratatui::widgets::Wrap { trim: true });

        frame.render_widget(help_block, help_area);
    }

    fn on_key_event(&mut self, key: KeyEvent) {
        if let (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) =
            (key.modifiers, key.code)
        {
            self.quit()
        }
    }

    fn quit(&mut self) {
        self.running = false;
    }
}
