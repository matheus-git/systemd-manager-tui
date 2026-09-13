use crate::usecases::services_manager::ServicesManager;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::{
    Frame,
    layout::Constraint,
    widgets::{Block, Borders, Cell, Padding, Row, Table, TableState},
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::Config;
use crate::domain::service::Service;
use crate::infrastructure::systemd_service_adapter::ConnectionType;
use crate::terminal::app::{Actions, AppEvent, ServiceRequestContext};

const PADDING: Padding = Padding::new(1, 1, 1, 1);
const WORKER_SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub const LOADING_PLACEHOLDER: &str = "Loading";

fn resolve_file<'a>(service: &'a Service, states: Option<&'a HashMap<String, String>>) -> &'a str {
    if service.state().file() != LOADING_PLACEHOLDER {
        return service.state().file();
    }
    states
        .and_then(|states| states.get(service.name()))
        .map(|service_state| service_state.as_str())
        .unwrap_or(LOADING_PLACEHOLDER)
}

fn build_service_row(
    service: &Service,
    states: Option<&HashMap<String, String>>,
    runtime_label: Option<(&str, &str)>,
) -> Row<'static> {
    let file = resolve_file(service, states);

    let highlight_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let normal_style = Style::default().fg(Color::Gray);

    let active_cell = if let Some((service_name, label)) = runtime_label
        && service_name == service.name()
    {
        Cell::from(label.to_string()).style(Style::default().fg(Color::Green))
    } else {
        let state_style = match service.state().active() {
            "active" => Style::default().fg(Color::Green),
            "activating" => Style::default().fg(Color::Yellow),
            "inactive" => Style::default().fg(Color::DarkGray),
            _ => Style::default().fg(Color::Red),
        };
        let sub = service.state().sub();
        let active = if sub.is_empty() {
            service.state().active().to_string()
        } else {
            format!("{} ({})", service.state().active(), sub)
        };
        Cell::from(active).style(state_style)
    };

    let file_style = if file == LOADING_PLACEHOLDER {
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::ITALIC | Modifier::DIM)
    } else {
        normal_style
    };

    Row::new(vec![
        Cell::from(service.name().to_string()).style(highlight_style),
        active_cell,
        Cell::from(file.to_string()).style(file_style),
        Cell::from(service.state().load().to_string()).style(normal_style),
        Cell::from(service.description().to_string()).style(normal_style),
    ])
}

fn generate_rows(
    services: &[Service],
    states: Option<&HashMap<String, String>>,
    service_uptime: Option<(&str, &str)>,
) -> Vec<Row<'static>> {
    services
        .iter()
        .map(|service| build_service_row(service, states, service_uptime))
        .collect()
}

fn generate_table<'a>(rows: &'a [Row<'a>], ignore_key_events: bool) -> Table<'a> {
    let mut table = Table::new(
        rows.to_owned(),
        [
            Constraint::Percentage(20),
            Constraint::Length(20),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Min(0),
        ],
    )
    .header(
        Row::new(["Name", "Active", "State", "Load", "Description"]).style(
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(Block::default().borders(Borders::NONE).padding(PADDING))
    .row_highlight_style(
        Style::default()
            .bg(Color::Blue)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol(">> ");

    if ignore_key_events {
        table = table.row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
    }

    table
}

#[derive(Clone, Copy, PartialEq)]
pub enum ActiveFilterState {
    All,
    Active,
    Inactive,
    Failed,
}

impl ActiveFilterState {
    pub fn next(self) -> Self {
        match self {
            ActiveFilterState::All => ActiveFilterState::Active,
            ActiveFilterState::Active => ActiveFilterState::Inactive,
            ActiveFilterState::Inactive => ActiveFilterState::Failed,
            ActiveFilterState::Failed => ActiveFilterState::All,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ActiveFilterState::All => "all",
            ActiveFilterState::Active => "active",
            ActiveFilterState::Inactive => "inactive",
            ActiveFilterState::Failed => "failed",
        }
    }
}

pub enum ServiceAction {
    Start,
    Stop,
    Restart,
    Enable,
    Disable,
    RefreshAll,
    ToggleFilter,
    ToggleMask,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListRequestContext {
    pub connection: ConnectionType,
    pub generation: u64,
}

pub enum QueryUnitFile {
    Finished(ListRequestContext, HashMap<String, String>),
    Error(ListRequestContext, String),
}

pub struct TableServices {
    pub table_state: TableState,
    pub services: Vec<Service>,
    filtered_services: Vec<Service>,
    states: Arc<Mutex<HashMap<String, String>>>,
    old_filter_text: String,
    pub ignore_key_events: bool,
    sender: Sender<AppEvent>,
    usecase: Rc<RefCell<ServicesManager>>,
    filter_all: bool,
    active_filter_state: ActiveFilterState,
    event_rx: Arc<Mutex<Receiver<QueryUnitFile>>>,
    event_tx: Arc<Sender<QueryUnitFile>>,
    active_enter_timestamp: Option<u64>,
    selected_service_name: Option<String>,
    last_timestamp_fetch: Option<Instant>,
    timestamp_request_tx: Sender<ServiceRequestContext>,
    timestamp_request_rx: Option<Receiver<ServiceRequestContext>>,
    active_connection: ConnectionType,
    list_generation: u64,
    current_list_context: Arc<Mutex<ListRequestContext>>,
    workers_shutdown: Arc<AtomicBool>,
    query_listener_handle: Option<JoinHandle<()>>,
    timestamp_worker_handle: Option<JoinHandle<()>>,
}

impl TableServices {
    fn dispatch(&self, action: Actions) {
        let _ = self.sender.send(AppEvent::Action(action));
    }

    pub fn new(sender: Sender<AppEvent>, usecase: Rc<RefCell<ServicesManager>>) -> Self {
        let (event_tx, event_rx) = mpsc::channel::<QueryUnitFile>();
        let (timestamp_request_tx, timestamp_request_rx) = mpsc::channel::<ServiceRequestContext>();
        let filter_all = false;
        let initial_context = ListRequestContext {
            connection: ConnectionType::System,
            generation: 0,
        };

        let mut table_state = TableState::default();
        table_state.select(Some(0));

        Self {
            table_state,
            filtered_services: Vec::new(),
            services: Vec::new(),
            states: Arc::new(Mutex::new(HashMap::new())),
            sender,
            old_filter_text: String::new(),
            ignore_key_events: false,
            usecase,
            filter_all,
            active_filter_state: ActiveFilterState::All,
            event_rx: Arc::new(Mutex::new(event_rx)),
            event_tx: Arc::new(event_tx),
            active_enter_timestamp: None,
            selected_service_name: None,
            last_timestamp_fetch: None,
            timestamp_request_tx,
            timestamp_request_rx: Some(timestamp_request_rx),
            active_connection: ConnectionType::System,
            list_generation: 0,
            current_list_context: Arc::new(Mutex::new(initial_context)),
            workers_shutdown: Arc::new(AtomicBool::new(false)),
            query_listener_handle: None,
            timestamp_worker_handle: None,
        }
    }

    pub fn init(&mut self, config: &Config) {
        let _ = self.fetch_services();
        self.spawn_query_listener();
        self.spawn_timestamp_worker();
        self.refresh(&config.filter);
    }

    fn spawn_query_listener(&mut self) {
        if self.query_listener_handle.is_some() {
            return;
        }
        let event_rx = self.event_rx.clone();
        let sender = self.sender.clone();
        let states = self.states.clone();
        let current_context = self.current_list_context.clone();
        let shutdown = self.workers_shutdown.clone();

        self.query_listener_handle = Some(thread::spawn(move || {
            loop {
                if shutdown.load(Ordering::Acquire) {
                    break;
                }
                let message = match event_rx.lock() {
                    Ok(receiver) => receiver.recv_timeout(WORKER_SHUTDOWN_POLL_INTERVAL),
                    Err(error) => {
                        let _ = sender.send(AppEvent::Error(format!(
                            "Unit-file result channel failed: {error}"
                        )));
                        break;
                    }
                };

                match message {
                    Ok(QueryUnitFile::Finished(context, new_states)) => {
                        let current = match current_context.lock() {
                            Ok(current) => current,
                            Err(error) => {
                                let _ = sender.send(AppEvent::Error(format!(
                                    "List request context failed: {error}"
                                )));
                                break;
                            }
                        };
                        if *current != context {
                            continue;
                        }
                        match states.lock() {
                            Ok(mut states) => *states = new_states,
                            Err(error) => {
                                let _ = sender.send(AppEvent::Error(format!(
                                    "Unit-file state storage failed: {error}"
                                )));
                                break;
                            }
                        }
                        if sender.send(AppEvent::Action(Actions::Redraw)).is_err() {
                            break;
                        }
                    }
                    Ok(QueryUnitFile::Error(context, error)) => {
                        let is_current = current_context
                            .lock()
                            .is_ok_and(|current| *current == context);
                        if !is_current {
                            continue;
                        }
                        if sender.send(AppEvent::Error(error)).is_err() {
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        }));
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.refresh_selected_timestamp();
        let runtime_label = self.format_runtime();

        let service_uptime: Option<(&str, &str)> = runtime_label.as_deref().and_then(|label| {
            let service = self
                .table_state
                .selected()
                .and_then(|idx| self.filtered_services.get(idx))
                .filter(|s| s.state().active() == "active");
            service.map(|service| (service.name(), label))
        });

        let rows = self
            .states
            .try_lock()
            .ok()
            .map(|states| generate_rows(&self.filtered_services, Some(&states), service_uptime))
            .unwrap_or_else(|| generate_rows(&self.filtered_services, None, service_uptime));

        let table = generate_table(&rows, self.ignore_key_events);
        frame.render_stateful_widget(&table, area, &mut self.table_state);
    }

    pub fn has_active_runtime(&self) -> bool {
        self.active_enter_timestamp.is_some()
    }

    pub fn invalidate_timestamp(&mut self) {
        self.selected_service_name = None;
        self.active_enter_timestamp = None;
        self.last_timestamp_fetch = None;
    }

    fn spawn_timestamp_worker(&mut self) {
        if self.timestamp_worker_handle.is_some() {
            return;
        }
        let Some(rx) = self.timestamp_request_rx.take() else {
            let _ = self.sender.send(AppEvent::Error(
                "Timestamp worker was already started".to_string(),
            ));
            return;
        };
        let repo = self.usecase.borrow().repository_handle();
        let sender = self.sender.clone();
        let shutdown = self.workers_shutdown.clone();

        self.timestamp_worker_handle = Some(thread::spawn(move || {
            loop {
                if shutdown.load(Ordering::Acquire) {
                    break;
                }
                match rx.recv_timeout(WORKER_SHUTDOWN_POLL_INTERVAL) {
                    Ok(request) => {
                        let mut context = request;
                        // Drain stale requests, keep only the latest
                        while let Ok(request) = rx.try_recv() {
                            context = request;
                        }
                        let ts = match repo.lock() {
                            Ok(repo) => repo
                                .get_active_enter_timestamp(&context.service_name)
                                .ok()
                                .filter(|&t| t > 0),
                            Err(error) => {
                                let _ = sender.send(AppEvent::Error(format!(
                                    "Timestamp repository lock failed: {error}"
                                )));
                                break;
                            }
                        };
                        let _ =
                            sender.send(AppEvent::Action(Actions::UpdateTimestamp(context, ts)));
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        }));
    }

    fn refresh_selected_timestamp(&mut self) {
        let selected = self.get_selected_service();
        let current_name = selected.as_ref().map(|s| s.name().to_string());

        let selection_changed = current_name != self.selected_service_name;
        let stale = self
            .last_timestamp_fetch
            .map(|t| t.elapsed() >= Duration::from_secs(30))
            .unwrap_or(true);

        if !selection_changed && !stale {
            return;
        }

        if selection_changed {
            self.active_enter_timestamp = None;
        }

        self.selected_service_name = current_name;
        self.last_timestamp_fetch = Some(Instant::now());

        if let Some(s) = selected.as_ref().filter(|s| s.state().active() == "active") {
            let _ = self.timestamp_request_tx.send(ServiceRequestContext {
                connection: self.active_connection,
                service_name: s.name().to_string(),
            });
        }
    }

    pub fn update_timestamp(&mut self, context: ServiceRequestContext, ts: Option<u64>) {
        if context.connection == self.active_connection
            && self.selected_service_name.as_deref() == Some(context.service_name.as_str())
        {
            self.active_enter_timestamp = ts;
            self.last_timestamp_fetch = Some(Instant::now());
        }
    }

    fn format_runtime(&self) -> Option<String> {
        let ts = self.active_enter_timestamp?;
        let now_micros = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        if now_micros <= ts {
            return Some("0s".to_string());
        }
        let secs = (now_micros - ts) / 1_000_000;
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        let mins = (secs % 3600) / 60;
        let s = secs % 60;
        let prefix = "Uptime:";
        Some(if days > 0 {
            format!("{prefix} {days}d {hours}h")
        } else if hours > 0 {
            format!("{prefix} {hours}h {mins}m")
        } else if mins > 0 {
            format!("{prefix} {mins}m {s}s")
        } else {
            format!("{prefix} {s}s")
        })
    }

    pub fn set_usecase(&mut self, usecase: Rc<RefCell<ServicesManager>>) {
        self.usecase = usecase;
        self.table_state.select(Some(0));
        self.services.clear();
        self.filtered_services.clear();
        self.fetch_and_refresh(&self.old_filter_text.clone());
    }

    pub fn set_ignore_key_events(&mut self, has_ignore_key_events: bool) {
        self.ignore_key_events = has_ignore_key_events;
    }

    pub fn get_selected_service(&self) -> Option<Service> {
        self.table_state
            .selected()
            .and_then(|idx| self.filtered_services.get(idx).cloned())
    }

    pub fn set_selected_index(&mut self, index: usize) {
        self.table_state.select(Some(index));
    }

    pub fn refresh(&mut self, filter_text: &str) {
        self.old_filter_text.clear();
        self.old_filter_text.push_str(filter_text);
        self.filtered_services = self.filter(filter_text, &self.services);

        // If no item is selected and the list is not empty, select the first item
        if self.table_state.selected().is_none() && !self.filtered_services.is_empty() {
            self.table_state.select(Some(0));
        }
        // If the selected index is out of bounds, reset to first item or None
        else if let Some(selected) = self.table_state.selected()
            && selected >= self.filtered_services.len()
        {
            if self.filtered_services.is_empty() {
                self.table_state.select(None);
            } else {
                self.table_state.select(Some(0));
            }
        }
    }

    fn fetch_services(&mut self) -> bool {
        self.list_generation = self.list_generation.wrapping_add(1);
        let context = ListRequestContext {
            connection: self.active_connection,
            generation: self.list_generation,
        };
        if let Ok(mut current) = self.current_list_context.lock() {
            *current = context;
        }
        let result =
            self.usecase
                .borrow()
                .list_services(self.filter_all, context, self.event_tx.clone());
        match result {
            Ok(services) => {
                self.services = services;
                true
            }
            Err(error) => {
                let _ = self.sender.send(AppEvent::Error(format!(
                    "Could not refresh services from the {} connection: {error}. The current list was kept.",
                    self.active_connection
                )));
                false
            }
        }
    }

    pub fn set_active_connection(&mut self, connection: ConnectionType) {
        self.active_connection = connection;
        self.list_generation = self.list_generation.wrapping_add(1);
        if let Ok(mut current) = self.current_list_context.lock() {
            *current = ListRequestContext {
                connection,
                generation: self.list_generation,
            };
        }
        if let Ok(mut states) = self.states.lock() {
            states.clear();
        }
        self.invalidate_timestamp();
    }

    fn fetch_and_refresh(&mut self, filter_text: &str) {
        if self.fetch_services() {
            self.refresh(filter_text);
        }
    }

    pub fn refresh_current(&mut self) {
        self.fetch_and_refresh(&self.old_filter_text.clone());
    }

    fn filter(&self, filter_text: &str, services: &[Service]) -> Vec<Service> {
        let lower_filter = filter_text.to_lowercase();

        services
            .iter()
            .filter(|service| {
                let name_matches = service.name().to_lowercase().contains(&lower_filter);

                let active_matches = match self.active_filter_state {
                    ActiveFilterState::All => true,
                    ActiveFilterState::Active => service.state().active() == "active",
                    ActiveFilterState::Inactive => service.state().active() == "inactive",
                    ActiveFilterState::Failed => service.state().active() == "failed",
                };

                name_matches && active_matches
            })
            .cloned()
            .collect()
    }

    pub fn on_key_event(&mut self, key: KeyEvent) {
        if self.ignore_key_events {
            return;
        }

        self.set_ignore_key_events(true);

        match key.code {
            KeyCode::Char('r') => {
                self.dispatch(Actions::ServiceAction(ServiceAction::Restart));
                return;
            }
            KeyCode::Char('s') => {
                self.dispatch(Actions::ServiceAction(ServiceAction::Start));
                return;
            }
            KeyCode::Char('x') => {
                self.dispatch(Actions::ServiceAction(ServiceAction::Stop));
                return;
            }
            KeyCode::Char('e') => {
                self.dispatch(Actions::ServiceAction(ServiceAction::Enable));
                return;
            }
            KeyCode::Char('d') => {
                self.dispatch(Actions::ServiceAction(ServiceAction::Disable));
                return;
            }
            KeyCode::Char('u') => {
                self.dispatch(Actions::ServiceAction(ServiceAction::RefreshAll));
                return;
            }
            KeyCode::Char('f') => {
                self.dispatch(Actions::ServiceAction(ServiceAction::ToggleFilter));
                return;
            }
            KeyCode::Char('a') => {
                self.active_filter_state = self.active_filter_state.next();
                self.refresh(&self.old_filter_text.clone());
                // Select the first element only if the list is not empty
                if self.filtered_services.is_empty() {
                    self.table_state.select(None);
                } else {
                    self.table_state.select(Some(0));
                }
                self.set_ignore_key_events(false);
                return;
            }
            KeyCode::Char('m') => {
                self.dispatch(Actions::ServiceAction(ServiceAction::ToggleMask));
                return;
            }
            KeyCode::Char('?') => {
                self.dispatch(Actions::ShowHelp);
                return;
            }
            _ => {}
        }

        self.set_ignore_key_events(false);

        let up_keys = [KeyCode::Up, KeyCode::Char('k')];
        let down_keys = [KeyCode::Down, KeyCode::Char('j')];

        match key.code {
            code if down_keys.contains(&code) => self.select_next(),
            code if up_keys.contains(&code) => self.select_previous(),
            KeyCode::PageDown => self.select_page_down(),
            KeyCode::PageUp => self.select_page_up(),
            KeyCode::Char('c') => {
                self.dispatch(Actions::GoDetails);
            }
            KeyCode::Char('v') => {
                self.dispatch(Actions::GoLog);
            }
            _ => {}
        }
    }

    fn select_page_down(&mut self) {
        if self.filtered_services.is_empty() {
            self.table_state.select(None);
            return;
        }

        let jump = 10;
        if let Some(selected_index) = self.table_state.selected() {
            let new_index = selected_index + jump;
            let wrapped_index = if new_index >= self.filtered_services.len() {
                (new_index) % self.filtered_services.len()
            } else {
                new_index
            };
            self.table_state.select(Some(wrapped_index));
        } else {
            self.table_state.select(Some(0));
        }
    }

    fn select_page_up(&mut self) {
        if self.filtered_services.is_empty() {
            self.table_state.select(None);
            return;
        }

        let jump = 10;
        if let Some(selected_index) = self.table_state.selected() {
            let len = self.filtered_services.len();
            let wrapped_index = (selected_index + len - jump % len) % len;
            self.table_state.select(Some(wrapped_index));
        } else {
            self.table_state.select(Some(0));
        }
    }

    fn select_next(&mut self) {
        if self.filtered_services.is_empty() {
            self.table_state.select(None);
            return;
        }

        if let Some(selected_index) = self.table_state.selected() {
            let next_index = if !self.filtered_services.is_empty()
                && selected_index == self.filtered_services.len() - 1
            {
                0
            } else {
                selected_index + 1
            };
            self.table_state.select(Some(next_index));
        } else {
            self.table_state.select(Some(0));
        }
    }

    fn select_previous(&mut self) {
        if self.filtered_services.is_empty() {
            self.table_state.select(None);
            return;
        }

        if let Some(selected_index) = self.table_state.selected() {
            let prev_index = if selected_index == 0 {
                self.filtered_services.len() - 1
            } else {
                selected_index - 1
            };
            self.table_state.select(Some(prev_index));
        } else {
            self.table_state.select(Some(0));
        }
    }

    pub fn act_on_selected_service(&mut self, action: &ServiceAction) {
        if let Some(service) = self.get_selected_service() {
            let binding_usecase = self.usecase.clone();
            let usecase = binding_usecase.borrow();
            match action {
                ServiceAction::ToggleMask => {
                    let state_opt = match self.states.lock() {
                        Ok(guard) => guard.get(service.name()).cloned(),
                        Err(e) => {
                            let _ = self.sender.send(AppEvent::Error(format!(
                                "Could not read the service state: {e}"
                            )));
                            return;
                        }
                    };

                    if let Some(state) = state_opt {
                        match state.as_str() {
                            "masked" | "masked-runtime" => {
                                self.handle_service_result(usecase.unmask_service(&service));
                            }
                            _ => {
                                self.handle_service_result(usecase.mask_service(&service));
                            }
                        }

                        let _ = self.fetch_services();
                        self.fetch_and_refresh(&self.old_filter_text.clone());
                    }
                }
                ServiceAction::Start => self.handle_service_result(usecase.start_service(&service)),
                ServiceAction::Stop => self.handle_service_result(usecase.stop_service(&service)),
                ServiceAction::Restart => {
                    self.handle_service_result(usecase.restart_service(&service))
                }
                ServiceAction::Enable => {
                    self.handle_service_result(usecase.enable_service(&service))
                }
                ServiceAction::Disable => {
                    self.handle_service_result(usecase.disable_service(&service))
                }
                ServiceAction::ToggleFilter => {
                    self.table_state.select(Some(0));
                    self.filter_all = !self.filter_all;
                    self.fetch_and_refresh(&self.old_filter_text.clone());
                }
                ServiceAction::RefreshAll => {
                    self.fetch_and_refresh(&self.old_filter_text.clone());
                }
            }
        }
        self.set_ignore_key_events(false);
    }

    fn handle_service_result(&mut self, result: Result<Service, Box<dyn Error>>) {
        match result {
            Ok(service) => {
                let services = &mut self.services;

                match services.iter().position(|s| s.name() == service.name()) {
                    Some(pos) => services[pos] = service,
                    None => services.push(service),
                };

                self.refresh(&self.old_filter_text.clone());
            }
            Err(e) => {
                // A timeout only bounds our wait; systemd may still complete the job.
                // Re-read the unit list before returning control so the synchronous UI
                // reflects the freshest state available.
                self.fetch_and_refresh(&self.old_filter_text.clone());
                let _ = self.sender.send(AppEvent::Error(e.to_string()));
            }
        }
    }

    pub fn is_filtered_list_empty(&self) -> bool {
        self.filtered_services.is_empty()
    }

    pub fn get_active_filter_state(&self) -> ActiveFilterState {
        self.active_filter_state
    }

    pub fn shortcuts(&self) -> Vec<Line<'_>> {
        let mut help_text: Vec<Line<'_>> = Vec::new();
        if !self.ignore_key_events {
            help_text.push(Line::from(Span::styled(
                "Actions on the selected service",
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            )));

            help_text.push(Line::from(
                "Navigate: ↑/↓ | Switch tab: ←/→ | Start: s | Stop: x | Restart: r | Enable: e | Disable: d | List all units: f | Filter: a | Mask/Unmask: m | Refresh: u | Log: v | Unit File: c | Help: ?"
            ));
        }

        help_text
    }
}

impl Drop for TableServices {
    fn drop(&mut self) {
        self.workers_shutdown.store(true, Ordering::Release);
        if let Some(handle) = self.query_listener_handle.take() {
            let _ = handle.join();
        }
        // Timestamp lookup is read-only and may be inside a blocking D-Bus call.
        // Dropping the handle detaches it so an optional refresh cannot hold up
        // Ctrl+C; the worker owns every resource it still needs.
        let _ = self.timestamp_worker_handle.take();
    }
}

#[cfg(test)]
mod tests;
