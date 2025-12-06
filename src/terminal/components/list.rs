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
use std::collections::HashMap;

use crate::domain::service::Service;
use crate::terminal::app::{Actions, AppEvent};
use rayon::prelude::*;

const PADDING: Padding = Padding::new(1, 1, 1, 1);

fn generate_rows(services: &[Service], states: &HashMap<String, String>) -> Vec<Row<'static>> {
    services
        .par_iter()
        .map(|service| {
            let highlight_style = Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD);
            let normal_style = Style::default().fg(Color::Gray);

            let state_style = match service.state().active() {
                "active" => Style::default().fg(Color::Green),
                "activating" => Style::default().fg(Color::Yellow),
                _ => Style::default().fg(Color::Red),
            };

            let sub = service.state().sub();
            let active = if sub.is_empty() {
                service.state().active().to_string()
            } else {
                format!("{} ({})", service.state().active(), sub)
            };

            let file = if let Some(state) = states.get(service.name()) && service.state().file() == "..." {
                state
            } else {
                service.state().file()  
            };

            Row::new(vec![
                Cell::from(service.name().to_string()).style(highlight_style),
                Cell::from(active).style(state_style),
                Cell::from(file.to_string()).style(normal_style),
                Cell::from(service.state().load().to_string()).style(normal_style),
                Cell::from(service.description().to_string()).style(normal_style),
            ])
        })
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
                .fg(Color::White)
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
    event_tx: Arc<Sender<QueryUnitFile>>
}

impl TableServices {
    pub fn new(sender: Sender<AppEvent>,  usecase: Rc<RefCell<ServicesManager>>) -> Self {
        let (event_tx, event_rx) = mpsc::channel::<QueryUnitFile>();
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
            event_tx: Arc::new(event_tx)
        }
    }

    pub fn init(&mut self) {
        let services = if let Ok(svcs) = self.usecase.borrow().list_services(self.filter_all, self.event_tx.clone()) {
                svcs
         } else {
                vec![]
        };

        self.filtered_services.clone_from(&services);
        self.services = services;
        self.spawn_query_listener();
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
        let states: HashMap<String, String> = if let Ok(states) = self.states.try_lock() {
            states.clone()
        } else {
            HashMap::new()
        };
        let rows = generate_rows(&self.filtered_services, &states);
        let table = generate_table(&rows, self.ignore_key_events); 
        frame.render_stateful_widget(&table, area, &mut self.table_state);
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
                "Navigate: ↑/↓ | Switch tab: ←/→ | Start: s | Stop: x | Restart: r | Enable: e | Disable: d | Help: ? | List all units: f | Filter: a | Mask/Unmask: m | Refresh: u | Log: v | Unit File: c"
            ));
        }

        help_text
    }
}
