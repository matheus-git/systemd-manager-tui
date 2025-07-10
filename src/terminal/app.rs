use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, LeaveAlternateScreen, EnterAlternateScreen},
};
use std::{io::{self}, process::Command};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Tabs};
use ratatui::DefaultTerminal;
use ratatui::Frame;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::cell::RefCell;
use std::rc::Rc;

use crate::infrastructure::systemd_service_adapter::ConnectionType;
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
    ResetList,
    GoLog,
    GoDetails,
    Updatelog((String, String)),
    #[allow(dead_code)]
    UpdateDetails,
    Filter(String),
    UpdateIgnoreListKeys(bool),
    EditCurrentService
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

pub struct App {
    running: bool,
    status: Status,
    event_listener_enabled: Arc<AtomicBool>,
    table_service: Rc<RefCell<TableServices>>,
    filter: Rc<RefCell<Filter>>,
    service_log: Rc<RefCell<ServiceLog>>,
    details: Rc<RefCell<ServiceDetails>>,
    usecases: Rc<RefCell<ServicesManager>>,
    event_rx: Receiver<AppEvent>,
    event_tx: Sender<AppEvent>,
    selected_tab_index: usize,
}

impl App {
    pub fn new(
        event_tx: Sender<AppEvent>, 
        event_rx: Receiver<AppEvent>, 
        table_service: Rc<RefCell<TableServices>>,
        filter: Rc<RefCell<Filter>>,
        service_log: Rc<RefCell<ServiceLog>>,
        details: Rc<RefCell<ServiceDetails>>,
        usecases: Rc<RefCell<ServicesManager>>
    ) -> Self {
        Self {
            running: true,
            status: Status::List,
            event_listener_enabled: Arc::new(AtomicBool::new(true)),
            table_service,
            filter,
            service_log,
            details,
            usecases,
            event_rx,
            event_tx,
            selected_tab_index: 0,
        }
    }

    pub fn init(&mut self) {
        self.spawn_key_event_listener();
    }

fn spawn_key_event_listener(&self) {
    let event_tx = self.event_tx.clone();
    let event_listener_enabled = self.event_listener_enabled.clone();

    thread::spawn(move || {
        loop {
            if !event_listener_enabled.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(50));
                continue;
            }

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
                }
                AppEvent::Action(Actions::GoList) => self.status = Status::List,
                AppEvent::Action(Actions::ResetList) => {
                    table_service.set_usecase(self.usecases.clone());
                },
                AppEvent::Action(Actions::UpdateDetails) => {}
                AppEvent::Action(Actions::RefreshDetails) => {
                    if self.status == Status::Details {
                        details.fetch_unit_file();
                    }
                }
                AppEvent::Action(Actions::GoDetails) => {
                    if let Some(service) = table_service.get_selected_service() {
                        details.update(service.clone());
                    }
                    self.event_tx
                        .send(AppEvent::Action(Actions::RefreshDetails))?;
                    self.status = Status::Details;
                }
                AppEvent::Action(Actions::EditCurrentService) => {
                    if let Some(service) = table_service.get_selected_service() {
                        self.edit_unit(&mut terminal, service.name())?;    
                        self.event_tx.send(AppEvent::Action(Actions::RefreshDetails))?;
                    }
                }
                AppEvent::Error(error_msg) => {
                    self.error_popup(&mut terminal, error_msg)?;    
                }
            }
        }

        Ok(())
    }

    fn resume_tui(&self, terminal: &mut DefaultTerminal) -> Result<()> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        terminal.draw(|f| {
            let area = f.area();
            f.render_widget(Clear, area);
        })?;
        Ok(())
    }


    fn edit_unit(&mut self, terminal: &mut DefaultTerminal, unit_name: &str) -> Result<()> {
        self.event_listener_enabled.store(false, Ordering::Relaxed);

        if let Err(e) = disable_raw_mode() {
                self.resume_tui(terminal)?;
            self.error_popup(terminal, format!("Failed to disable raw mode: {}", e))?;
            return Ok(());
        }

        let mut stdout = io::stdout();
        if let Err(e) = execute!(stdout, LeaveAlternateScreen) {
                self.resume_tui(terminal)?;
            self.error_popup(terminal, format!("Failed to leave alternate screen: {}", e))?;
            return Ok(());
        }

        if let Err(e) = terminal.show_cursor() {
                self.resume_tui(terminal)?;
            self.error_popup(terminal, format!("Failed to show cursor: {}", e))?;
            return Ok(());
        }

        let status = Command::new("systemctl")
            .arg("edit")
            .arg("--full")
            .arg(unit_name)
            .status();

        match status {
            Ok(s) if s.success() => {},
            Ok(s) => {
                self.resume_tui(terminal)?;
                self.error_popup(terminal, format!("systemctl edit failed with status: {}", s))?;
            },
            Err(e) => {
                self.resume_tui(terminal)?;
                self.error_popup(terminal, format!("Error executing systemctl: {}", e))?;
            }
        }

        if let Err(e) = self.resume_tui(terminal) {
            self.error_popup(terminal, format!("Failed to return to TUI: {}", e))?;
            return Ok(());
        }

        self.event_listener_enabled.store(true, Ordering::Relaxed);

        Ok(())
    }

    fn error_popup(&self, terminal: &mut DefaultTerminal, error_msg: String) -> Result<()> {
        let user_friendly_message = get_user_friendly_error(&error_msg);

        terminal.draw(|frame| {
            let area = frame.area();

            let popup_width = std::cmp::min(70, area.width.saturating_sub(4));
            let popup_height = std::cmp::min(10, area.height.saturating_sub(4));

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

            let [filter_box, tabs_box, list_box, help_area_box] = Layout::vertical([
                Constraint::Length(4),
                Constraint::Length(1),
                Constraint::Min(10),
                Constraint::Max(7),
            ])
            .areas(area);

            let tabs = Tabs::new(vec!["System units","Session units"])
                .select(self.selected_tab_index)
                .highlight_style(Style::default().fg(Color::Yellow));

            frame.render_widget(tabs, tabs_box);
            filter.draw(frame, filter_box);
            table.render(frame, list_box);
            self.draw_shortcuts(frame, help_area_box, table.shortcuts());
        })?;

        Ok(())
    }

    fn draw_shortcuts(&mut self, frame: &mut Frame, help_area: Rect, shortcuts: Vec<Line<'_>>) {
        let mut help_text: Vec<Line<'_>> = Vec::new();
        let shortcuts_lens = shortcuts.len();

        help_text.extend(shortcuts.clone());

        if shortcuts_lens > 0 {
            help_text.push(Line::raw(""));
            let shortcuts_width = shortcuts.clone()
                .iter()
                .map(|line|  line.spans.iter().map(|span| span.width()).sum())
                .max()
                .unwrap_or(0);
            if help_area.width > shortcuts_width as u16 {
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
        let left_keys = [KeyCode::Left, KeyCode::Char('h')];
        let right_keys = [KeyCode::Right, KeyCode::Char('l')];

        match key {
            KeyEvent {
                modifiers: KeyModifiers::CONTROL,
                code: KeyCode::Char('c') | KeyCode::Char('C'),
                ..
            } => {
                self.quit();
            }

            KeyEvent {
                code,
                ..
            } if left_keys.contains(&code) => {
                if matches!(self.status, Status::List) {
                    self.selected_tab_index = if self.selected_tab_index == 0 {
                        1
                    } else {
                        self.selected_tab_index - 1
                    };

                    self.update_connection_and_reset();
                }
            }

            KeyEvent { code, .. } if right_keys.contains(&code) => {
                if matches!(self.status, Status::List) {
                    self.selected_tab_index = (self.selected_tab_index + 1) % 2;
                    self.update_connection_and_reset();
                }
            }

            _ => {}
        }
    }

    fn update_connection_and_reset(&mut self) {
        let conn_type = match self.selected_tab_index {
            0 => ConnectionType::System,
            _ => ConnectionType::Session,
        };

        if let Err(_err) = self.usecases
            .borrow_mut()
            .change_repository_connection(conn_type)
        {
            self.event_tx.send(AppEvent::Error("Failed to change connection type with D-Bus, try run without sudo".to_string())).expect("Failed to change connection type");
            self.selected_tab_index = 0;
            return
        }

        self.event_tx
            .send(AppEvent::Action(Actions::ResetList))
            .expect("Failed to send ResetList event");
    }

    fn quit(&mut self) {
        self.running = false;
    }
}
