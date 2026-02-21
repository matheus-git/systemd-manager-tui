use crate::usecases::services_manager::ServicesManager;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::{
    layout::Constraint,
    widgets::{Block, Borders, Cell, Row, Table, TableState, Padding},
    Frame,
};
use std::error::Error;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::mpsc;
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::collections::HashMap;

use crate::domain::service::Service;
use crate::terminal::app::{Actions, AppEvent};
use rayon::prelude::*;

const PADDING: Padding = Padding::new(1, 1, 1, 1);

pub const LOADING_PLACEHOLDER: &str = "Loading";

fn resolve_file<'a>(service: &'a Service, states: &'a HashMap<String, String>) -> &'a str {
    if service.state().file() == LOADING_PLACEHOLDER {
        if let Some(state) = states.get(service.name()) {
            state.as_str()
        } else {
            service.state().file()
        }
    } else {
        service.state().file()
    }
}

fn build_service_row(
    service: &Service,
    states: &HashMap<String, String>,
    runtime_label: Option<&str>,
) -> Row<'static> {
    let file = resolve_file(service, states);

    let highlight_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let normal_style = Style::default().fg(Color::Gray);

    let active_cell = if let Some(label) = runtime_label {
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
    }else {
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

fn generate_rows(services: &[Service], states: &HashMap<String, String>) -> Vec<Row<'static>> {
    services
        .par_iter()
        .map(|service| build_service_row(service, states, None))
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
    .block( 
        Block::default()
            .borders(Borders::NONE)
            .padding(PADDING),
    )
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

pub enum QueryUnitFile {
    Finished(HashMap<String, String>)
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
    timestamp_request_tx: Sender<String>,
    timestamp_request_rx: Option<Receiver<String>>,
}

impl TableServices {
    pub fn new(sender: Sender<AppEvent>,  usecase: Rc<RefCell<ServicesManager>>) -> Self {
        let (event_tx, event_rx) = mpsc::channel::<QueryUnitFile>();
        let (timestamp_request_tx, timestamp_request_rx) = mpsc::channel::<String>();
        let filter_all = false;

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
        }
    }

    pub fn init(&mut self) {
        let services = self.usecase.borrow().list_services(self.filter_all, self.event_tx.clone()).unwrap_or_default();
        self.filtered_services.clone_from(&services);
        self.services = services;
        self.spawn_query_listener();
        self.spawn_timestamp_worker();
    }

    fn spawn_query_listener(&self) {
        let event_rx = self.event_rx.clone();
        let sender = self.sender.clone();
        let states = self.states.clone();

        thread::spawn(move || {
            loop {
                let rx = event_rx.lock().unwrap();
                if let Ok(msg) = rx.recv() {
                    match msg {
                        QueryUnitFile::Finished(s) => {
                            *states.lock().unwrap() = s;
                            sender.send(AppEvent::Action(Actions::Redraw)).expect("Error");
                        }
                    }
                }
            }
        });
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.refresh_selected_timestamp();
        let runtime_label = self.format_runtime();

        let states: HashMap<String, String> = if let Ok(states) = self.states.try_lock() {
            states.clone()
        } else {
            HashMap::new()
        };
        let mut rows = generate_rows(&self.filtered_services, &states);

        if let (Some(label), Some(idx)) = (runtime_label.as_deref(), self.table_state.selected())
            && let Some(service) = self.filtered_services.get(idx) 
                && service.state().active() == "active" {
                    rows[idx] = build_service_row(service, &states, Some(label));
        }

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
        let rx = self.timestamp_request_rx.take().expect("timestamp receiver already taken");
        let repo = self.usecase.borrow().repository_handle();
        let sender = self.sender.clone();

        thread::spawn(move || {
            while let Ok(n) = rx.recv() {
                let mut name = n;
                // Drain stale requests, keep only the latest
                while let Ok(n) = rx.try_recv() {
                    name = n;
                }
                let ts = repo.lock().unwrap()
                    .get_active_enter_timestamp(&name)
                    .ok()
                    .filter(|&t| t > 0);
                let _ = sender.send(AppEvent::Action(Actions::UpdateTimestamp(name, ts)));
            }
        });
    }

    fn refresh_selected_timestamp(&mut self) {
        let selected = self.get_selected_service();
        let current_name = selected.as_ref().map(|s| s.name().to_string());

        let selection_changed = current_name != self.selected_service_name;
        let stale = self.last_timestamp_fetch
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
            let _ = self.timestamp_request_tx.send(s.name().to_string());
        }
    }

    pub fn update_timestamp(&mut self, name: String, ts: Option<u64>) {
        if self.selected_service_name.as_deref() == Some(name.as_str()) {
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
        if let Some(selected_index) = self.table_state.selected() 
            && let Some(service) = self.filtered_services.get(selected_index) {
                return Some(service.clone());
            
        }
        None
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
            && selected >= self.filtered_services.len() {
                if self.filtered_services.is_empty() {
                    self.table_state.select(None);
                } else {
                    self.table_state.select(Some(0));
                }
            
        }
    }

    fn fetch_services(&mut self) {
        if let Ok(services) = self.usecase.borrow().list_services(self.filter_all, self.event_tx.clone()) {
            self.services = services;
        } else {
            self.services = vec![];
        }
    }

    fn fetch_and_refresh(&mut self, filter_text: &str) {
        self.fetch_services();
        self.refresh(filter_text);
    }

    fn filter(&self, filter_text: &str, services: &[Service]) -> Vec<Service> {
        let lower_filter = filter_text.to_lowercase();

        services
            .iter()
            .filter(|service| {
                let name_matches =
                    service.name().to_lowercase().contains(&lower_filter);

                let active_matches = match self.active_filter_state {
                    ActiveFilterState::All => true,
                    ActiveFilterState::Active => service.state().active() == "active",
                    ActiveFilterState::Inactive => {
                        service.state().active() != "active"
                            && service.state().active() != "failed"
                    }
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
                self.sender.send(AppEvent::Action(Actions::ServiceAction(ServiceAction::Restart))).unwrap();
                return;
            }
            KeyCode::Char('s') => {
                self.sender.send(AppEvent::Action(Actions::ServiceAction(ServiceAction::Start))).unwrap();
                return;
            }
            KeyCode::Char('x') => {
                self.sender.send(AppEvent::Action(Actions::ServiceAction(ServiceAction::Stop))).unwrap();
                return;
            }
            KeyCode::Char('e') => {
                self.sender.send(AppEvent::Action(Actions::ServiceAction(ServiceAction::Enable))).unwrap();
                return;
            }
            KeyCode::Char('d') => {
                self.sender.send(AppEvent::Action(Actions::ServiceAction(ServiceAction::Disable))).unwrap();
                return;
            }
            KeyCode::Char('u') => {
                self.sender.send(AppEvent::Action(Actions::ServiceAction(ServiceAction::RefreshAll))).unwrap();
                return;
            }
            KeyCode::Char('f') => {
                self.sender.send(AppEvent::Action(Actions::ServiceAction(ServiceAction::ToggleFilter))).unwrap();
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
                self.sender.send(AppEvent::Action(Actions::ServiceAction(ServiceAction::ToggleMask))).unwrap();
                return;
            }
            KeyCode::Char('?') => {
                self.sender.send(AppEvent::Action(Actions::ShowHelp)).unwrap();
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
                self.sender.send(AppEvent::Action(Actions::GoDetails)).unwrap();
            }
            KeyCode::Char('v') => {
                self.sender.send(AppEvent::Action(Actions::GoLog)).unwrap();
            }
            _ => {}
        }
    }

    fn select_page_down(&mut self) {
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
        let jump = 10;
        if let Some(selected_index) = self.table_state.selected() {
            let selected_index = isize::try_from(selected_index).expect("Failed to convert selected index to isize");
            let new_index = selected_index - jump as isize;
            let wrapped_index = if new_index < 0 {
                let len = isize::try_from(self.filtered_services.len()).expect("Failed to convert table length to isize");
                usize::try_from(len + new_index % len).expect("Failed to convert calculated circular index to usize")
            } else {
                usize::try_from(new_index).expect("Failed to convert new_index to usize")
            };
            self.table_state.select(Some(wrapped_index));
        } else {
            self.table_state.select(Some(0));
        }
    }

    fn select_next(&mut self) {
        if let Some(selected_index) = self.table_state.selected() {
            let next_index = if !self.filtered_services.is_empty() && selected_index == self.filtered_services.len() - 1 {
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
                    match service.state().file() {
                        "masked" | "masked-runtime" => self.handle_service_result(usecase.unmask_service(&service)),
                        _ => self.handle_service_result(usecase.mask_service(&service)),
                    }
                    self.fetch_services();
                    self.fetch_and_refresh(&self.old_filter_text.clone());
                },
                ServiceAction::Start => self.handle_service_result(usecase.start_service(&service)),
                ServiceAction::Stop => self.handle_service_result(usecase.stop_service(&service)),
                ServiceAction::Restart => self.handle_service_result(usecase.restart_service(&service)),
                ServiceAction::Enable => self.handle_service_result(usecase.enable_service(&service)),
                ServiceAction::Disable => self.handle_service_result(usecase.disable_service(&service)),
                ServiceAction::ToggleFilter => {
                    self.table_state.select(Some(0));
                    self.filter_all = !self.filter_all;
                    self.fetch_and_refresh(&self.old_filter_text.clone());
                },
                ServiceAction::RefreshAll => {
                    self.fetch_and_refresh(&self.old_filter_text.clone());
                },
            }
        }
        self.set_ignore_key_events(false);
    }

    fn handle_service_result(&mut self, result: Result<Service, Box<dyn Error>>) {
        match result {
            Ok(service) => {
                let services = &mut self.services;

                if let Some(pos) = services.iter().position(|s| s.name() == service.name()) {
                    services[pos] = service;
                } else {
                    services.push(service);
                }

                self.refresh(&self.old_filter_text.clone());
            }
            Err(e) => {
                self.sender.send(AppEvent::Error(e.to_string())).unwrap();
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
