use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, LeaveAlternateScreen, EnterAlternateScreen},
};
use std::{io::{self}, process::Command};
use ratatui::layout::{Alignment, Constraint, Margin, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Tabs, Padding};
use ratatui::DefaultTerminal;
use ratatui::Frame;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::cell::RefCell;
use std::rc::Rc;
use rayon::prelude::*;

use crate::infrastructure::systemd_service_adapter::ConnectionType;
use crate::domain::service::Service;
use crate::terminal::components::list::ActiveFilterState;
use crate::usecases::services_manager::ServicesManager;
use crate::Config;

use super::components::details::ServiceDetails;
use super::components::filter::{Filter, InputMode};
use super::components::list::{QueryUnitFile, TableServices, ServiceAction};
use super::components::log::ServiceLog;

#[derive(PartialEq)]
enum Status {
    List,
    Log,
    Details,
}

#[derive(Debug, PartialEq)]
enum AppEffect {
    Suspend,
    EditUnit(String),
    FetchLog(Service),
    FetchDetails(Service),
    RunServiceAction {
        service: Service,
        action: ServiceAction,
    },
    RefreshServices {
        filter_all: bool,
        filter_text: String,
        connection_request_id: Option<u64>,
    },
    ChangeConnection(ConnectionRequest),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ConnectionRequest {
    id: u64,
    target: ConnectionType,
}

pub enum Actions {
    RefreshLog,
    RefreshDetails,
    GoList,
    GoLog,
    GoDetails,
    #[allow(dead_code)]
    UpdateDetails,
    Filter(String),
    UpdateIgnoreListKeys(bool),
    EditCurrentService,
    ServiceAction(ServiceAction),
    ShowHelp,
    Redraw,
    UpdateTimestamp(String, Option<u64>),
    LogLoaded(Result<(String, String), String>),
    DetailsLoaded(Service, Result<String, String>),
    ServiceActionFinished(Result<Service, String>),
    ServicesLoaded(Result<Vec<Service>, String>, String, Option<u64>),
    ConnectionChanged(u64, ConnectionType, Result<(), String>),
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
    table_service: TableServices,
    filter: Filter,
    service_log: ServiceLog,
    details: ServiceDetails,
    usecases: Rc<RefCell<ServicesManager>>,
    event_rx: Receiver<AppEvent>,
    event_tx: Sender<AppEvent>,
    selected_tab_index: usize,
    show_help: bool,
    error_message: Option<String>,
    connection_tx: Sender<ConnectionRequest>,
    latest_connection_request_id: u64,
    active_connection: ConnectionType,
    connection_pending: bool,
}

impl App {
    pub fn new(
        event_tx: Sender<AppEvent>, 
        event_rx: Receiver<AppEvent>, 
        table_service: TableServices,
        filter: Filter,
        service_log: ServiceLog,
        details: ServiceDetails,
        usecases: Rc<RefCell<ServicesManager>>
    ) -> Self {
        let (connection_tx, connection_rx) = mpsc::channel::<ConnectionRequest>();
        let connection_manager = usecases.borrow().clone();
        let connection_event_tx = event_tx.clone();
        thread::spawn(move || {
            while let Ok(request) = connection_rx.recv() {
                let mut manager = connection_manager.clone();
                let result = manager
                    .change_repository_connection(request.target)
                    .map_err(|error| error.to_string());
                if connection_event_tx
                    .send(AppEvent::Action(Actions::ConnectionChanged(
                        request.id,
                        request.target,
                        result,
                    )))
                    .is_err()
                {
                    break;
                }
            }
        });

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
            show_help: false,
            error_message: None,
            connection_tx,
            latest_connection_request_id: 0,
            active_connection: ConnectionType::System,
            connection_pending: false,
        }
    }

    pub fn init(&mut self, config: Config) {
        self.table_service.init(&config);
        self.event_tx.send(AppEvent::Action(Actions::Filter(config.filter))).unwrap();
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

                if event::poll(Duration::from_millis(100)).unwrap_or(false) 
                    && let Ok(Event::Key(key_event)) = event::read() 
                        && key_event.kind == KeyEventKind::Press
                            && event_tx.send(AppEvent::Key(key_event)).is_err()
                        {
                            break;
                        }
            }
        });
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;

        while self.running {
            match self.status {
                Status::Log => self.draw_log_status(&mut terminal)?,
                Status::List => self.draw_list_status(&mut terminal)?,
                Status::Details => self.draw_details_status(&mut terminal)?,
            }

            let use_timeout = self.status == Status::List
                && self.table_service.has_active_runtime();

            let event = if use_timeout {
                match self.event_rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(ev) => ev,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            } else {
                self.event_rx.recv()?
            };

            let effects = self.handle_event(event)?;
            self.execute_effects(effects, &mut terminal)?;
        }

        Ok(())
    }

    fn handle_event(&mut self, event: AppEvent) -> Result<Vec<AppEffect>> {
        let mut effects = Vec::new();

        match event {
            AppEvent::Key(key) => {
                if self.error_message.take().is_some() {
                    return Ok(effects);
                }

                if matches!(
                    key,
                    KeyEvent {
                        modifiers: KeyModifiers::CONTROL,
                        code: KeyCode::Char('c' | 'C'),
                        ..
                    }
                ) {
                    self.quit();
                    return Ok(effects);
                }

                if matches!(
                    key,
                    KeyEvent {
                        modifiers: KeyModifiers::CONTROL,
                        code: KeyCode::Char('z'),
                        ..
                    }
                ) {
                    effects.push(AppEffect::Suspend);
                    return Ok(effects);
                }

                match self.status {
                    Status::Log => {
                        if self.show_help {
                            self.show_help = false;
                        } else {
                            self.service_log.on_key_event(key);
                        }
                    }
                    Status::List => {
                        if self.show_help {
                            self.show_help = false;
                            // Ensure the table is active and can receive key events
                            self.table_service.set_ignore_key_events(false);
                            // If no item is selected and list is not empty, select first item
                            if self.table_service.table_state.selected().is_none() && !self.table_service.is_filtered_list_empty() {
                                self.table_service.set_selected_index(0);
                            }
                        } else {
                            if let Some(connection_type) = self.on_key_horizontal_event(
                                key,
                                self.filter.input_mode == InputMode::Editing,
                            ) {
                                self.connection_pending = true;
                                self.table_service.begin_connection_change();
                                effects.push(AppEffect::ChangeConnection(
                                    self.next_connection_request(connection_type),
                                ));
                            }
                            self.table_service.on_key_event(key);
                            self.filter.on_key_event(key);
                        }
                    }
                    Status::Details => {
                        if self.show_help {
                            self.show_help = false;
                        } else {
                            self.details.on_key_event(key);
                        }
                    }
                }
            }
                AppEvent::Action(Actions::ServiceAction(action)) => {
                    if self.connection_pending {
                        return Ok(effects);
                    }
                    match action {
                        ServiceAction::ToggleFilter => {
                            self.table_service.toggle_unit_listing();
                            effects.push(self.refresh_services_effect());
                        }
                        ServiceAction::RefreshAll => {
                            effects.push(self.refresh_services_effect());
                        }
                        _ => {
                            if let Some(service) = self.table_service.get_selected_service() {
                                effects.push(AppEffect::RunServiceAction { service, action });
                            } else {
                                self.table_service.set_ignore_key_events(false);
                            }
                        }
                    }
                }
                AppEvent::Action(Actions::UpdateIgnoreListKeys(bool)) => {
                    self.table_service.set_ignore_key_events(bool);
                }
                AppEvent::Action(Actions::Filter(input)) => {
                    self.table_service.set_selected_index(0);
                    self.table_service.refresh(&input);
                }
                AppEvent::Action(Actions::RefreshLog) => {
                    if self.status == Status::Log 
                        && let Some(service) = self.table_service.get_selected_service() {
                            effects.push(AppEffect::FetchLog(service));
                    }
                }
                AppEvent::Action(Actions::GoLog) => {
                    self.status = Status::Log;
                    self.event_tx.send(AppEvent::Action(Actions::RefreshLog))?;
                }
                AppEvent::Action(Actions::GoList) => self.status = Status::List,
                AppEvent::Action(Actions::UpdateTimestamp(name, ts)) => {
                    self.table_service.update_timestamp(name, ts);
                }
                AppEvent::Action(Actions::UpdateDetails | Actions::Redraw) => {}
                AppEvent::Action(Actions::LogLoaded(result)) => match result {
                    Ok((name, log)) => self.service_log.update(name, log),
                    Err(error) => self.error_message = Some(error),
                },
                AppEvent::Action(Actions::DetailsLoaded(service, result)) => match result {
                    Ok(unit_file) => self.details.update_unit_file(service, unit_file),
                    Err(error) => self.error_message = Some(error),
                },
                AppEvent::Action(Actions::ServiceActionFinished(result)) => {
                    self.table_service.apply_service_result(result);
                    self.table_service.invalidate_timestamp();
                }
                AppEvent::Action(Actions::ServicesLoaded(
                    result,
                    filter_text,
                    connection_request_id,
                )) => {
                    let belongs_to_current_connection = connection_request_id
                        .is_none_or(|id| id == self.latest_connection_request_id);
                    let expected_while_pending = !self.connection_pending
                        || connection_request_id == Some(self.latest_connection_request_id);

                    if belongs_to_current_connection && expected_while_pending {
                        self.connection_pending = false;
                        match result {
                            Ok(services) => {
                                self.table_service.apply_services(services, &filter_text)
                            }
                            Err(error) => {
                                self.table_service.set_ignore_key_events(false);
                                self.error_message = Some(error);
                            }
                        }
                    }
                }
                AppEvent::Action(Actions::ConnectionChanged(request_id, target, result)) => {
                    if request_id == self.latest_connection_request_id {
                        match result {
                            Ok(()) => {
                                self.active_connection = target;
                                effects.push(
                                    self.refresh_services_effect_for_connection(request_id),
                                );
                            }
                            Err(error) => {
                                self.selected_tab_index = match self.active_connection {
                                    ConnectionType::System => 0,
                                    ConnectionType::Session => 1,
                                };
                                self.error_message = Some(error);
                                effects.push(
                                    self.refresh_services_effect_for_connection(request_id),
                                );
                            }
                        }
                    }
                }
                AppEvent::Action(Actions::RefreshDetails) => {
                    if self.status == Status::Details
                        && let Some(service) = self.table_service.get_selected_service()
                    {
                        effects.push(AppEffect::FetchDetails(service));
                    }
                }
                AppEvent::Action(Actions::GoDetails) => {
                    if let Some(service) = self.table_service.get_selected_service() {
                        self.details.update(service.clone());
                        effects.push(AppEffect::FetchDetails(service));
                    }
                    self.status = Status::Details;
                }
                AppEvent::Action(Actions::EditCurrentService) => {
                    if let Some(service) = &self.table_service.get_selected_service() {
                        effects.push(AppEffect::EditUnit(service.name().to_string()));
                    }
                }
                AppEvent::Error(error_msg) => {
                    self.error_message = Some(error_msg);
                }
                AppEvent::Action(Actions::ShowHelp) => {
                    self.show_help = !self.show_help;
                },
        }

        Ok(effects)
    }

    fn execute_effects(
        &mut self,
        effects: Vec<AppEffect>,
        terminal: &mut DefaultTerminal,
    ) -> Result<()> {
        for effect in effects {
            match effect {
                AppEffect::Suspend => self.suspend_tui(terminal)?,
                AppEffect::EditUnit(unit_name) => {
                    self.edit_unit(terminal, &unit_name)?;
                    self.event_tx.send(AppEvent::Action(Actions::RefreshDetails))?;
                }
                AppEffect::FetchLog(service) => {
                    self.spawn_log_worker(service);
                }
                AppEffect::FetchDetails(service) => {
                    self.spawn_details_worker(service);
                }
                AppEffect::RunServiceAction { service, action } => {
                    self.spawn_service_action_worker(service, action);
                }
                AppEffect::RefreshServices {
                    filter_all,
                    filter_text,
                    connection_request_id,
                } => {
                    let query_sender = self.table_service.query_sender();
                    self.spawn_services_worker(
                        filter_all,
                        filter_text,
                        query_sender,
                        connection_request_id,
                    );
                }
                AppEffect::ChangeConnection(request) => {
                    self.connection_tx.send(request)?;
                }
            }
        }

        Ok(())
    }

    fn spawn_log_worker(&self, service: Service) {
        let manager = self.usecases.borrow().clone();
        let sender = self.event_tx.clone();
        thread::spawn(move || {
            let name = service.name().to_string();
            let result = manager
                .get_log(&service)
                .map(|log| (name, log))
                .map_err(|error| error.to_string());
            let _ = sender.send(AppEvent::Action(Actions::LogLoaded(result)));
        });
    }

    fn spawn_details_worker(&self, service: Service) {
        let manager = self.usecases.borrow().clone();
        let sender = self.event_tx.clone();
        thread::spawn(move || {
            let result = manager
                .systemctl_cat(&service)
                .map_err(|error| error.to_string());
            let _ = sender.send(AppEvent::Action(Actions::DetailsLoaded(service, result)));
        });
    }

    fn spawn_service_action_worker(&self, service: Service, action: ServiceAction) {
        let manager = self.usecases.borrow().clone();
        let sender = self.event_tx.clone();
        let file_state = self.table_service.file_state_for(&service);
        thread::spawn(move || {
            let result = match action {
                ServiceAction::Start => manager.start_service(&service),
                ServiceAction::Stop => manager.stop_service(&service),
                ServiceAction::Restart => manager.restart_service(&service),
                ServiceAction::Enable => manager.enable_service(&service),
                ServiceAction::Disable => manager.disable_service(&service),
                ServiceAction::ToggleMask
                    if matches!(file_state.as_str(), "masked" | "masked-runtime") =>
                {
                    manager.unmask_service(&service)
                }
                ServiceAction::ToggleMask => manager.mask_service(&service),
                ServiceAction::ToggleFilter | ServiceAction::RefreshAll => return,
            }
            .map_err(|error| error.to_string());

            let _ = sender.send(AppEvent::Action(Actions::ServiceActionFinished(result)));
        });
    }

    fn refresh_services_effect(&self) -> AppEffect {
        let (filter_all, filter_text) = self.table_service.refresh_parameters();
        AppEffect::RefreshServices {
            filter_all,
            filter_text,
            connection_request_id: None,
        }
    }

    fn refresh_services_effect_for_connection(&self, request_id: u64) -> AppEffect {
        let (filter_all, filter_text) = self.table_service.refresh_parameters();
        AppEffect::RefreshServices {
            filter_all,
            filter_text,
            connection_request_id: Some(request_id),
        }
    }

    fn spawn_services_worker(
        &self,
        filter_all: bool,
        filter_text: String,
        query_sender: Arc<Sender<QueryUnitFile>>,
        connection_request_id: Option<u64>,
    ) {
        let manager = self.usecases.borrow().clone();
        let sender = self.event_tx.clone();
        thread::spawn(move || {
            let result = manager
                .list_services(filter_all, query_sender)
                .map_err(|error| error.to_string());
            let _ = sender.send(AppEvent::Action(Actions::ServicesLoaded(
                result,
                filter_text,
                connection_request_id,
            )));
        });
    }

    fn next_connection_request(&mut self, target: ConnectionType) -> ConnectionRequest {
        self.latest_connection_request_id = self.latest_connection_request_id.wrapping_add(1);
        ConnectionRequest {
            id: self.latest_connection_request_id,
            target,
        }
    }

    #[allow(clippy::unused_self)]
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
            self.error_message = Some(format!("Failed to disable raw mode: {e}"));
            self.event_listener_enabled.store(true, Ordering::Relaxed);
            return Ok(());
        }

        let mut stdout = io::stdout();
        if let Err(e) = execute!(stdout, LeaveAlternateScreen) {
            self.resume_tui(terminal)?;
            self.error_message = Some(format!("Failed to leave alternate screen: {e}"));
            self.event_listener_enabled.store(true, Ordering::Relaxed);
            return Ok(());
        }

        if let Err(e) = terminal.show_cursor() {
            self.resume_tui(terminal)?;
            self.error_message = Some(format!("Failed to show cursor: {e}"));
            self.event_listener_enabled.store(true, Ordering::Relaxed);
            return Ok(());
        }

        let mut cmd = Command::new("systemctl");

        cmd
            .arg("edit")
            .arg("--full");

        if self.selected_tab_index==1{
            cmd
                .arg("--user");
        }

        let status = cmd
            .arg(unit_name)
            .status();

        let edit_error = match status {
            Ok(s) if s.success() => None,
            Ok(_) => Some("'systemctl edit' failed. Try running the program with sudo!".to_string()),
            Err(e) => Some(format!("Error executing systemctl: {e}")),
        };

        if let Err(e) = self.resume_tui(terminal) {
            self.error_message = Some(format!("Failed to return to TUI: {e}"));
            self.event_listener_enabled.store(true, Ordering::Relaxed);
            return Ok(());
        }

        self.error_message = edit_error;

        self.event_listener_enabled.store(true, Ordering::Relaxed);

        Ok(())
    }

    #[allow(clippy::unused_self)]
    fn draw_help_popup(&self, frame: &mut Frame, area: Rect) {
        let popup_width = std::cmp::min(80, area.width.saturating_sub(4));
        let popup_height = std::cmp::min(38, area.height.saturating_sub(4));

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
                "SYSTEMD MANAGER TUI - HELP",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled("Navigation:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            Line::from("↑/k - Move up    ↓/j - Move down"),
            Line::from("←/h - Previous tab    →/l - Next tab"),
            Line::from("PageUp/PageDown - Jump 10 items"),
            Line::from(""),
            Line::from(vec![Span::styled("Filter:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            Line::from("Ctrl+u - Delete until the start of the input "),
            Line::from("Ctrl+k - Delete until the end of the input "),
            Line::from("Alt+b or Ctrl+← - Go backwards a word "),
            Line::from("Alt+f or Ctrl+→ - Go fowards a word "),
            Line::from("Ctrl+e or End - Go to the end of the input "),
            Line::from("Ctrl+a or Home - Go to the end of input  "),
            Line::from("Ctrl+w or Ctrl+Backspace - Delete a word backwards "),
            Line::from(""),
            Line::from(vec![Span::styled("Service Control:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            Line::from("s - Start service    x - Stop service"),
            Line::from("r - Restart service"),
            Line::from("e - Enable service    d - Disable service"),
            Line::from("m - Mask/Unmask service"),
            Line::from(""),
            Line::from(vec![Span::styled("View & Filter list:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            Line::from("f - Toggle all/services filter"),
            Line::from("a - Cycle filter (all→active→inactive→failed)"),
            Line::from("u - Refresh service list"),
            Line::from(""),
            Line::from(vec![Span::styled("Information:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            Line::from("v - View service logs"),
            Line::from("c - View unit file details"),
            Line::from(""),
            Line::from(vec![Span::styled("Application:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            Line::from("Ctrl+z - Suspend"),
            Line::from("Ctrl+c - Quit"),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Press ? or any key to close",
                Style::default().fg(Color::Gray),
            )]),
        ];

        let help_block = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan))
                    .padding(Padding::new(1,1,0,0))
                    .title("Help"),
            )
            .alignment(Alignment::Left)
            .wrap(ratatui::widgets::Wrap { trim: true });

        frame.render_widget(help_block, popup_area.inner(Margin {
            vertical: 0,
            horizontal: 1
        }));
    }

    #[allow(clippy::unused_self)]
    fn draw_error_popup(&self, frame: &mut Frame, area: Rect, error_msg: &str) {
        let user_friendly_message = get_user_friendly_error(error_msg);
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
    }

    fn draw_details_status(
        &mut self,
        terminal: &mut DefaultTerminal,
    ) -> Result<()> {
        terminal.draw(|frame| {
            let area = frame.area();

            let [list_box, help_area_box] =
                Layout::vertical([Constraint::Min(0), Constraint::Max(7)]).areas(area);

            self.details.render(frame, list_box);
            self.draw_shortcuts(frame, help_area_box, &self.details.shortcuts());
            self.draw_overlays(frame, area);
        })?;

        Ok(())
    }

    fn draw_log_status(
        &mut self,
        terminal: &mut DefaultTerminal,
    ) -> Result<()> {
        terminal.draw(|frame| {
            let area = frame.area();

            let [list_box, help_area_box] =
                Layout::vertical([Constraint::Min(0), Constraint::Max(7)]).areas(area);

            self.service_log.render(frame, list_box);
            self.draw_shortcuts(frame, help_area_box, &self.service_log.shortcuts());
            self.draw_overlays(frame, area);
        })?;

        Ok(())
    }

    fn draw_list_status(
        &mut self,
        terminal: &mut DefaultTerminal,
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

            let filter_state = &self.table_service.get_active_filter_state();

            let system_tab = if self.selected_tab_index == 0 && *filter_state != ActiveFilterState::All {
                Line::from(vec![
                    Span::raw("System units"),
                    Span::styled(
                        format!(" (Filter: {})", filter_state.as_str()),
                        Style::default().fg(Color::Gray)
                    )
                ])
            } else {
                Line::from("System units")
            };

            let session_tab = if self.selected_tab_index == 1 && *filter_state != ActiveFilterState::All{
                Line::from(vec![
                    Span::raw("Session units"),
                    Span::styled(
                        format!(" (Filter: {})", filter_state.as_str()),
                        Style::default().fg(Color::Gray)
                    )
                ])
            } else {
                Line::from("Session units")
            };

            let tabs = Tabs::new(vec![system_tab, session_tab])
                .select(self.selected_tab_index)
                .highlight_style(Style::default().fg(Color::Yellow));

            frame.render_widget(tabs, tabs_box);

            let shortcuts = self.table_service.shortcuts();
            self.draw_shortcuts(frame, help_area_box, &shortcuts);
            let table_service = &mut self.table_service;
            self.filter.draw(frame, filter_box);
            table_service.render(frame, list_box);

            self.draw_overlays(frame, area);
        })?;

        Ok(())
    }

    #[allow(clippy::unused_self)]
    fn draw_shortcuts(&self, frame: &mut Frame, help_area: Rect, shortcuts: &[Line<'_>]) {
        let mut help_text: Vec<Line<'_>> = Vec::new();
        let shortcuts_lens = shortcuts.len();

        help_text.extend(shortcuts.to_owned());

        if shortcuts_lens > 0 {
            help_text.push(Line::raw(""));
            let shortcuts_width = shortcuts.to_owned()
                .par_iter()
                .map(|line|  line.spans.iter().map(ratatui::prelude::Span::width).sum())
                .max()
                .unwrap_or(0);
            let shortcuts_width = u16::try_from(shortcuts_width).expect("Failed to convert shortcuts_width to u16");
            if help_area.width > shortcuts_width {
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

    fn draw_overlays(&self, frame: &mut Frame, area: Rect) {
        if self.show_help {
            self.draw_help_popup(frame, area);
        }
        if let Some(error_message) = self.error_message.as_deref() {
            self.draw_error_popup(frame, area, error_message);
        }
    }

    fn suspend_tui(&mut self, 
        terminal: &mut DefaultTerminal,
    ) -> Result<()> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        unsafe {
            libc::raise(libc::SIGTSTP);
        }

        self.resume_tui(terminal)?;

        Ok(())
    }

    fn on_key_horizontal_event(
        &mut self,
        key: KeyEvent,
        is_filtering: bool,
    ) -> Option<ConnectionType> {
        let left_keys = [KeyCode::Left, KeyCode::Char('h')];
        let right_keys = [KeyCode::Right, KeyCode::Char('l')];
        match key {
            KeyEvent {
                code,
                ..
            } if left_keys.contains(&code) => {
                if !is_filtering && self. status == Status::List {
                    self.selected_tab_index = if self.selected_tab_index == 0 {
                        1
                    } else {
                        self.selected_tab_index - 1
                    };

                    self.table_service.invalidate_timestamp();
                    return Some(if self.selected_tab_index == 0 {
                        ConnectionType::System
                    } else {
                        ConnectionType::Session
                    });
                }
            }

            KeyEvent { code, .. } if right_keys.contains(&code) => {
                if !is_filtering && self.status == Status::List {
                    self.selected_tab_index = (self.selected_tab_index + 1) % 2;
                    self.table_service.invalidate_timestamp();
                    return Some(if self.selected_tab_index == 0 {
                        ConnectionType::System
                    } else {
                        ConnectionType::Session
                    });
                }
            }

            _ => {}
        }
        None
    }

    fn quit(&mut self) {
        self.running = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{service, FakeRepository};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::sync::mpsc;
    use std::time::Duration;

    fn test_app() -> App {
        test_app_with_repository(FakeRepository::default())
    }

    fn test_app_with_repository(repository: FakeRepository) -> App {
        let (event_tx, event_rx) = mpsc::channel();
        let manager = Rc::new(RefCell::new(ServicesManager::new(Box::new(
            repository,
        ))));
        let table = TableServices::new(event_tx.clone(), manager.clone());
        let filter = Filter::new(event_tx.clone(), String::new());
        let log = ServiceLog::new(event_tx.clone());
        let details = ServiceDetails::new(event_tx.clone());

        App::new(
            event_tx,
            event_rx,
            table,
            filter,
            log,
            details,
            manager,
        )
    }

    #[test]
    fn translates_known_dbus_errors() {
        let message = get_user_friendly_error(
            "org.freedesktop.DBus.Error.AccessDenied: rejected",
        );

        assert!(message.contains("Access denied"));
    }

    #[test]
    fn preserves_unknown_errors() {
        assert_eq!(get_user_friendly_error("custom failure"), "custom failure");
    }

    #[test]
    fn actions_navigate_between_all_screens() {
        let mut app = test_app();

        app.handle_event(AppEvent::Action(Actions::GoDetails)).unwrap();
        assert!(matches!(app.status, Status::Details));

        app.handle_event(AppEvent::Action(Actions::GoLog)).unwrap();
        assert!(matches!(app.status, Status::Log));

        app.handle_event(AppEvent::Action(Actions::GoList)).unwrap();
        assert!(matches!(app.status, Status::List));
    }

    #[test]
    fn error_event_opens_overlay_and_next_key_closes_it() {
        let mut app = test_app();

        app.handle_event(AppEvent::Error("failure".into())).unwrap();
        assert_eq!(app.error_message.as_deref(), Some("failure"));

        app.handle_event(AppEvent::Key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )))
        .unwrap();
        assert!(app.error_message.is_none());
    }

    #[test]
    fn error_overlay_is_rendered_by_test_backend() {
        let mut app = test_app();
        app.handle_event(AppEvent::Error(
            "org.freedesktop.DBus.Error.AccessDenied".into(),
        ))
        .unwrap();
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| app.draw_overlays(frame, frame.area()))
            .unwrap();

        let screen = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(screen.contains("ERROR"));
        assert!(screen.contains("Access denied"));
        assert!(screen.contains("Press any key to dismiss"));
    }

    #[test]
    fn help_overlay_can_be_opened_and_closed() {
        let mut app = test_app();

        app.handle_event(AppEvent::Action(Actions::ShowHelp)).unwrap();
        assert!(app.show_help);

        app.handle_event(AppEvent::Key(KeyEvent::new(
            KeyCode::Char('?'),
            KeyModifiers::NONE,
        )))
        .unwrap();
        assert!(!app.show_help);
    }

    #[test]
    fn ctrl_c_stops_the_application() {
        let mut app = test_app();

        let effects = app
            .handle_event(AppEvent::Key(KeyEvent::new(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL,
            )))
            .unwrap();

        assert!(!app.running);
        assert!(effects.is_empty());
        assert!(app.event_rx.try_recv().is_err());
    }

    #[test]
    fn ctrl_z_returns_suspend_effect_without_touching_terminal() {
        let mut app = test_app();

        let effects = app
            .handle_event(AppEvent::Key(KeyEvent::new(
                KeyCode::Char('z'),
                KeyModifiers::CONTROL,
            )))
            .unwrap();

        assert_eq!(effects, [AppEffect::Suspend]);
    }

    #[test]
    fn edit_action_returns_effect_for_selected_unit() {
        let mut app = test_app();
        app.table_service.services = vec![service("demo.service", "active", "enabled")];
        app.table_service.refresh("");

        let effects = app
            .handle_event(AppEvent::Action(Actions::EditCurrentService))
            .unwrap();

        assert_eq!(effects, [AppEffect::EditUnit("demo.service".into())]);
    }

    #[test]
    fn log_worker_returns_result_through_event_queue() {
        let fake = FakeRepository::with_content("journal output", "", 0);
        let mut app = test_app_with_repository(fake);
        let unit = service("demo.service", "active", "enabled");

        app.spawn_log_worker(unit);
        let event = app.event_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        app.handle_event(event).unwrap();

        let backend = TestBackend::new(50, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| app.service_log.render(frame, frame.area()))
            .unwrap();
        let screen = terminal.backend().to_string();
        assert!(screen.contains("demo.service log"));
        assert!(screen.contains("journal output"));
    }

    #[test]
    fn details_worker_returns_result_through_event_queue() {
        let fake = FakeRepository::with_content("", "[Service]\nExecStart=/bin/true", 0);
        let mut app = test_app_with_repository(fake);
        let unit = service("demo.service", "active", "enabled");

        app.spawn_details_worker(unit);
        let event = app.event_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        app.handle_event(event).unwrap();

        let backend = TestBackend::new(50, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| app.details.render(frame, frame.area()))
            .unwrap();
        assert!(terminal.backend().to_string().contains("ExecStart"));
    }

    #[test]
    fn service_worker_executes_operation_and_returns_updated_service() {
        let fake = FakeRepository::default();
        let observer = fake.clone();
        let mut app = test_app_with_repository(fake);
        let unit = service("demo.service", "inactive", "disabled");
        app.table_service.services = vec![unit.clone()];
        app.table_service.refresh("");

        app.spawn_service_action_worker(unit, ServiceAction::Start);
        let event = app.event_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        app.handle_event(event).unwrap();

        assert_eq!(observer.calls(), ["start:demo.service"]);
        assert_eq!(app.table_service.services[0].state().active(), "active");
    }

    #[test]
    fn refresh_action_produces_worker_effect_even_with_empty_list() {
        let mut app = test_app();

        let effects = app
            .handle_event(AppEvent::Action(Actions::ServiceAction(
                ServiceAction::RefreshAll,
            )))
            .unwrap();

        assert!(matches!(effects.as_slice(), [AppEffect::RefreshServices { .. }]));
    }

    #[test]
    fn changing_tab_produces_async_connection_effect() {
        let mut app = test_app();
        app.table_service.services = vec![service("system.service", "active", "enabled")];
        app.table_service.refresh("");

        let effects = app
            .handle_event(AppEvent::Key(KeyEvent::new(
                KeyCode::Right,
                KeyModifiers::NONE,
            )))
            .unwrap();

        assert_eq!(app.selected_tab_index, 1);
        assert!(app.connection_pending);
        assert!(app.table_service.services.is_empty());
        assert!(app.table_service.get_selected_service().is_none());
        assert!(app.table_service.ignore_key_events);
        assert_eq!(
            effects,
            [AppEffect::ChangeConnection(ConnectionRequest {
                id: 1,
                target: ConnectionType::Session,
            })]
        );
    }

    #[test]
    fn connection_worker_processes_rapid_tab_changes_in_request_order() {
        let fake = FakeRepository::default();
        let observer = fake.clone();
        let mut app = test_app_with_repository(fake);

        let first = app.next_connection_request(ConnectionType::Session);
        let second = app.next_connection_request(ConnectionType::System);
        app.connection_tx.send(first).unwrap();
        app.connection_tx.send(second).unwrap();

        let first_result = app.event_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let second_result = app.event_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(app.handle_event(first_result).unwrap().is_empty());
        assert!(matches!(
            app.handle_event(second_result).unwrap().as_slice(),
            [AppEffect::RefreshServices { .. }]
        ));
        assert_eq!(observer.calls(), ["connection:session", "connection:system"]);
    }

    #[test]
    fn stale_connection_result_cannot_trigger_a_refresh() {
        let mut app = test_app();
        app.latest_connection_request_id = 2;

        let stale_effects = app
            .handle_event(AppEvent::Action(Actions::ConnectionChanged(
                1,
                ConnectionType::Session,
                Ok(()),
            )))
            .unwrap();
        let current_effects = app
            .handle_event(AppEvent::Action(Actions::ConnectionChanged(
                2,
                ConnectionType::System,
                Ok(()),
            )))
            .unwrap();

        assert!(stale_effects.is_empty());
        assert!(matches!(
            current_effects.as_slice(),
            [AppEffect::RefreshServices { .. }]
        ));
    }

    #[test]
    fn service_actions_are_ignored_while_connection_is_pending() {
        let mut app = test_app();
        app.connection_pending = true;
        app.table_service.set_ignore_key_events(true);

        let effects = app
            .handle_event(AppEvent::Action(Actions::ServiceAction(
                ServiceAction::Start,
            )))
            .unwrap();

        assert!(effects.is_empty());
        assert!(app.connection_pending);
        assert!(app.table_service.ignore_key_events);
    }

    #[test]
    fn pending_connection_accepts_only_its_own_service_list() {
        let mut app = test_app();
        app.latest_connection_request_id = 2;
        app.connection_pending = true;
        app.table_service.begin_connection_change();

        app.handle_event(AppEvent::Action(Actions::ServicesLoaded(
            Ok(vec![service("old.service", "active", "enabled")]),
            String::new(),
            Some(1),
        )))
        .unwrap();
        app.handle_event(AppEvent::Action(Actions::ServicesLoaded(
            Ok(vec![service("unrelated.service", "active", "enabled")]),
            String::new(),
            None,
        )))
        .unwrap();

        assert!(app.connection_pending);
        assert!(app.table_service.services.is_empty());
        assert!(app.table_service.ignore_key_events);

        app.handle_event(AppEvent::Action(Actions::ServicesLoaded(
            Ok(vec![service("current.service", "active", "enabled")]),
            String::new(),
            Some(2),
        )))
        .unwrap();

        assert!(!app.connection_pending);
        assert_eq!(app.table_service.services[0].name(), "current.service");
        assert!(!app.table_service.ignore_key_events);
    }

    #[test]
    fn failed_connection_restores_active_tab_and_refreshes_before_unlocking() {
        let mut app = test_app();
        app.selected_tab_index = 1;
        app.latest_connection_request_id = 1;
        app.connection_pending = true;
        app.table_service.begin_connection_change();

        let effects = app
            .handle_event(AppEvent::Action(Actions::ConnectionChanged(
                1,
                ConnectionType::Session,
                Err("connection failed".into()),
            )))
            .unwrap();

        assert_eq!(app.selected_tab_index, 0);
        assert!(app.connection_pending);
        assert!(app.table_service.ignore_key_events);
        assert_eq!(app.error_message.as_deref(), Some("connection failed"));
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::RefreshServices {
                connection_request_id: Some(1),
                ..
            }]
        ));
    }
}
