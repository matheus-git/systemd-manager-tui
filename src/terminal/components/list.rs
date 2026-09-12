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
use crate::Config;

use rayon::prelude::*;

const PADDING: Padding = Padding::new(1, 1, 1, 1);

pub const LOADING_PLACEHOLDER: &str = "Loading";

fn resolve_file<'a>(service: &'a Service, states: Option<&'a HashMap<String, String>>) -> &'a str {
    if service.state().file() != LOADING_PLACEHOLDER {
        return service.state().file()
    }
    states
        .and_then(|states| states.get(service.name()))
        .map(|service_state| service_state.as_str())
        .unwrap_or_else(|| service.state().file())
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

    let active_cell = if let Some((service_name, label)) = runtime_label && service_name == service.name() {
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

fn generate_rows(services: &[Service], states: Option<&HashMap<String, String>>, service_uptime: Option<(&str, &str)>) -> Vec<Row<'static>> {
    services
        .par_iter()
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

impl ServiceAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Enable => "enable",
            Self::Disable => "disable",
            Self::ToggleMask => "mask/unmask",
            Self::RefreshAll => "refresh",
            Self::ToggleFilter => "toggle filter",
        }
    }
}

pub enum QueryUnitFile {
    Finished(HashMap<String, String>),
    Error(String),
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

    pub fn init(&mut self, config: &Config) {
        self.services = self.usecase.borrow().list_services(self.filter_all, self.event_tx.clone())
            .unwrap_or_default();
        self.spawn_query_listener();
        self.spawn_timestamp_worker();
        self.refresh(&config.filter);
    }

    fn spawn_query_listener(&self) {
        let event_rx = self.event_rx.clone();
        let sender = self.sender.clone();
        let states = self.states.clone();

        thread::spawn(move || {
            loop {
                let message = match event_rx.lock() {
                    Ok(receiver) => receiver.recv(),
                    Err(error) => {
                        let _ = sender.send(AppEvent::Error(error.to_string()));
                        break;
                    }
                };
                match message {
                    Ok(QueryUnitFile::Finished(new_states)) => {
                        match states.lock() {
                            Ok(mut states) => *states = new_states,
                            Err(error) => {
                                let _ = sender.send(AppEvent::Error(error.to_string()));
                                break;
                            }
                        }
                        if sender.send(AppEvent::Action(Actions::Redraw)).is_err() {
                            break;
                        }
                    }
                    Ok(QueryUnitFile::Error(error)) => {
                        if sender.send(AppEvent::Error(error)).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.refresh_selected_timestamp();
        let runtime_label = self.format_runtime();

        let service_uptime: Option<(&str, &str)> = runtime_label.as_deref()
            .and_then(|label| {
                let service = self.table_state.selected()
                    .and_then(|idx| self.filtered_services.get(idx))
                    .filter(|s| s.state().active() == "active");
                service.map(|service| (service.name(), label))
            });

        let rows = self.states.try_lock()
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
                let ts = match repo.lock() {
                    Ok(repo) => repo
                        .get_active_enter_timestamp(&name)
                        .ok()
                        .filter(|&t| t > 0),
                    Err(error) => {
                        let _ = sender.send(AppEvent::Error(error.to_string()));
                        break;
                    }
                };
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

    pub fn set_ignore_key_events(&mut self, has_ignore_key_events: bool) {
        self.ignore_key_events = has_ignore_key_events;
    }

    pub fn toggle_unit_listing(&mut self) {
        self.table_state.select(Some(0));
        self.filter_all = !self.filter_all;
    }

    pub fn refresh_parameters(&self) -> (bool, String) {
        (self.filter_all, self.old_filter_text.clone())
    }

    pub fn query_sender(&self) -> Arc<Sender<QueryUnitFile>> {
        self.event_tx.clone()
    }

    pub fn apply_services(&mut self, services: Vec<Service>, filter_text: &str) {
        self.services = services;
        self.refresh(filter_text);
        self.set_ignore_key_events(false);
    }

    pub fn get_selected_service(&self) -> Option<Service> {
        self.table_state.selected()
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
            && selected >= self.filtered_services.len() {
                if self.filtered_services.is_empty() {
                    self.table_state.select(None);
                } else {
                    self.table_state.select(Some(0));
                }
            
        }
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
        if self.filtered_services.is_empty() {
            self.table_state.select(None);
            return;
        }

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

    pub fn apply_service_result(&mut self, result: Result<Service, String>) {
        self.handle_service_result(result.map_err(Into::into));
        self.set_ignore_key_events(false);
    }

    pub fn file_state_for(&self, service: &Service) -> String {
        self.states
            .lock()
            .ok()
            .and_then(|states| states.get(service.name()).cloned())
            .unwrap_or_else(|| service.state().file().to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::service_repository::ServiceRepository;
    use crate::domain::service_state::ServiceState;
    use crate::infrastructure::systemd_service_adapter::ConnectionType;
    use std::collections::HashMap;
    use std::sync::mpsc;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    struct EmptyRepository;

    impl ServiceRepository for EmptyRepository {
        fn list_services(&self, _: bool) -> Result<Vec<Service>, Box<dyn Error>> { Ok(vec![]) }
        fn unit_files_state(&self, _: Vec<Service>) -> Result<HashMap<String, String>, Box<dyn Error>> { Ok(HashMap::new()) }
        fn list_service_files(&self) -> Result<Vec<Service>, Box<dyn Error>> { Ok(vec![]) }
        fn get_unit(&self, _: &str) -> Result<Service, Box<dyn Error>> { Err("not found".into()) }
        fn get_service_log(&self, _: &str) -> Result<String, Box<dyn Error>> { Ok(String::new()) }
        fn start_service(&self, _: &str) -> Result<Service, Box<dyn Error>> { Err("unsupported".into()) }
        fn stop_service(&self, _: &str) -> Result<Service, Box<dyn Error>> { Err("unsupported".into()) }
        fn restart_service(&self, _: &str) -> Result<Service, Box<dyn Error>> { Err("unsupported".into()) }
        fn enable_service(&self, _: &str) -> Result<Service, Box<dyn Error>> { Err("unsupported".into()) }
        fn disable_service(&self, _: &str) -> Result<Service, Box<dyn Error>> { Err("unsupported".into()) }
        fn mask_service(&self, _: &str) -> Result<Service, Box<dyn Error>> { Err("unsupported".into()) }
        fn unmask_service(&self, _: &str) -> Result<Service, Box<dyn Error>> { Err("unsupported".into()) }
        fn reload_daemon(&self) -> Result<(), Box<dyn Error>> { Ok(()) }
        fn change_connection(&mut self, _: ConnectionType) -> Result<(), zbus::Error> { Ok(()) }
        fn systemctl_cat(&self, _: &str) -> Result<String, Box<dyn Error>> { Ok(String::new()) }
        fn get_active_enter_timestamp(&self, _: &str) -> Result<u64, Box<dyn Error>> { Ok(0) }
    }

    fn table() -> TableServices {
        let (sender, _receiver) = mpsc::channel();
        let manager = ServicesManager::new(Box::new(EmptyRepository));
        TableServices::new(sender, Rc::new(RefCell::new(manager)))
    }

    fn service(name: &str, active: &str) -> Service {
        Service::new(
            name.to_string(),
            String::new(),
            ServiceState::new("loaded".into(), active.into(), "running".into(), "enabled".into()),
        )
    }

    #[test]
    fn navigation_on_empty_list_clears_selection() {
        let mut table = table();

        table.select_next();
        table.select_previous();
        table.select_page_down();
        table.select_page_up();

        assert_eq!(table.table_state.selected(), None);
    }

    #[test]
    fn navigation_wraps_in_both_directions() {
        let mut table = table();
        table.filtered_services = vec![service("a.service", "active"), service("b.service", "inactive")];
        table.table_state.select(Some(1));

        table.select_next();
        assert_eq!(table.table_state.selected(), Some(0));

        table.select_previous();
        assert_eq!(table.table_state.selected(), Some(1));
    }

    #[test]
    fn filter_combines_name_and_active_state() {
        let mut table = table();
        table.active_filter_state = ActiveFilterState::Active;
        let services = vec![
            service("alpha.service", "active"),
            service("beta.service", "active"),
            service("alpha.timer", "inactive"),
        ];

        let filtered = table.filter("alpha", &services);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name(), "alpha.service");
    }

    #[test]
    fn file_state_resolution_prefers_loaded_state_map() {
        let loading = Service::new(
            "demo.service".into(),
            String::new(),
            ServiceState::new("loaded".into(), "active".into(), "running".into(), LOADING_PLACEHOLDER.into()),
        );
        let states = HashMap::from([("demo.service".to_string(), "enabled".to_string())]);

        assert_eq!(resolve_file(&loading, Some(&states)), "enabled");
    }

    #[test]
    fn renders_service_table_to_test_backend() {
        let mut table = table();
        table.filtered_services = vec![service("demo.service", "active")];
        table.table_state.select(Some(0));
        let backend = TestBackend::new(90, 6);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| table.render(frame, frame.area())).unwrap();

        let rendered = terminal.backend().buffer().content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("Name"));
        assert!(rendered.contains("demo.service"));
        assert!(rendered.contains("active (running)"));
        assert!(rendered.contains("enabled"));
    }
}
