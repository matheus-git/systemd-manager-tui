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
use std::sync::mpsc::Sender;
use std::rc::Rc;
use std::cell::RefCell;

use crate::domain::service::Service;
use crate::terminal::app::{Actions, AppEvent};

const PADDING: Padding = Padding::new(1, 1, 1, 1);

fn generate_rows(services: &[Service]) -> Vec<Row<'static>> {
    services
        .iter()
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
            let active = if !sub.is_empty() {
                format!("{} ({})", service.state().active(), sub)
            } else {
                service.state().active().to_string()
            };

            Row::new(vec![
                Cell::from(service.name().to_string()).style(highlight_style),
                Cell::from(active).style(state_style),
                Cell::from(service.state().file().to_string()).style(normal_style),
                Cell::from(service.state().load().to_string()).style(normal_style),
                Cell::from(service.description().to_string()).style(normal_style),
            ])
        })
        .collect()
}

pub enum ServiceAction {
    Start,
    Stop,
    Restart,
    Enable,
    Disable,
    RefreshAll,
    ToggleFilter
}
 
pub struct TableServices {
    table: Table<'static>,
    pub table_state: TableState,
    pub rows: Vec<Row<'static>>,
    pub services: Vec<Service>,
    filtered_services: Vec<Service>,
    old_filter_text: String,
    pub ignore_key_events: bool,
    sender: Sender<AppEvent>,
    usecase: Rc<RefCell<ServicesManager>>,
    filter_all: bool,
}

impl TableServices {
    pub fn new(sender: Sender<AppEvent>,  usecase: Rc<RefCell<ServicesManager>>) -> Self {
        let filter_all = true;
        let (services, rows) = match usecase.borrow().list_services(filter_all) {
            Ok(svcs) => {
                let rows = generate_rows(&svcs);
                (svcs, rows)
            }
            Err(_) => {
                let error_row = Row::new(vec!["Error loading services", "", "", "", ""]);
                (vec![], vec![error_row])
            }
        };

        let mut table_state = TableState::default();
        table_state.select(Some(0));
        let table = Table::new(
            rows.clone(),
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
        Self {
            table,
            table_state,
            rows,
            filtered_services: services.clone(),
            services,
            sender,
            old_filter_text: String::new(),
            ignore_key_events: false,
            usecase,
            filter_all,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_stateful_widget(&self.table, area, &mut self.table_state);
    }

    pub fn set_usecase(&mut self, usecase: Rc<RefCell<ServicesManager>>) {
        self.usecase = usecase;
        self.table = self.table.clone().block( 
            Block::default()
                .borders(Borders::NONE)
                .padding(PADDING),
        );
        self.rows.clear();
        self.table_state.select(Some(0));
        self.services.clear();
        self.filtered_services.clear();
        self.fetch_and_refresh(self.old_filter_text.clone());
    }

    pub fn set_ignore_key_events(&mut self, has_ignore_key_events: bool) {
        if has_ignore_key_events {
            self.table = self.table.clone().row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );
        } else {
            self.table = self.table.clone().row_highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );
        }
        self.ignore_key_events = has_ignore_key_events
    }

    pub fn get_selected_service(&self) -> Option<&Service> {
        if let Some(selected_index) = self.table_state.selected() {
            if let Some(service) = self.filtered_services.get(selected_index) {
                return Some(service);
            }
        }
        None
    }

    pub fn set_selected_index(&mut self, index: usize) {
        self.table_state.select(Some(index));
    }

    pub fn refresh(&mut self, filter_text: String) {
        self.old_filter_text = filter_text.clone();
        self.filtered_services = self.filter(filter_text, self.services.clone());
        self.rows = generate_rows(&self.filtered_services.clone());
        self.table = self.table.clone().rows(self.rows.clone());
    }

    fn fetch_services(&mut self) {
        if let Ok(services) = self.usecase.borrow().list_services(self.filter_all) {
            self.services = services
        } else {
            self.services = vec![]
        }
    }

    fn fetch_and_refresh(&mut self, filter_text: String) {
        self.fetch_services();
        self.refresh(filter_text);
    }

    fn filter(&self, filter_text: String, services: Vec<Service>) -> Vec<Service> {
        let lower_filter = filter_text.to_lowercase();
        services
            .into_iter()
            .filter(|service| {
                let name = service.name();
                name.to_lowercase().contains(&lower_filter)
            })
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
            KeyCode::Char('v') => {
                self.sender.send(AppEvent::Action(Actions::GoLog)).unwrap();
                return;
            }
            KeyCode::Char('f') => {
                self.sender.send(AppEvent::Action(Actions::ServiceAction(ServiceAction::ToggleFilter))).unwrap();
                return;
            }
            KeyCode::Char('c') => {
                self.sender.send(AppEvent::Action(Actions::GoDetails)).unwrap();
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
            _ => {}
        }
    }

    fn select_page_down(&mut self) {
        let jump = 10;
        if let Some(selected_index) = self.table_state.selected() {
            let new_index = selected_index + jump;
            let wrapped_index = if new_index >= self.rows.len() {
                (new_index) % self.rows.len()
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
            let new_index = selected_index as isize - jump as isize;
            let wrapped_index = if new_index < 0 {
                (self.rows.len() as isize + new_index % self.rows.len() as isize) as usize
            } else {
                new_index as usize
            };
            self.table_state.select(Some(wrapped_index));
        } else {
            self.table_state.select(Some(0));
        }
    }

    fn select_next(&mut self) {
        if let Some(selected_index) = self.table_state.selected() {
            let next_index = if selected_index == self.rows.len() - 1 {
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
                self.rows.len() - 1
            } else {
                selected_index - 1
            };
            self.table_state.select(Some(prev_index));
        } else {
            self.table_state.select(Some(0));
        }
    }

    pub fn act_on_selected_service(&mut self, action: ServiceAction) {
        if let Some(service) = self.get_selected_service() {
            let binding_usecase = self.usecase.clone();
            let usecase = binding_usecase.borrow();
            match action {
                ServiceAction::Start => self.handle_service_result(usecase.start_service(service)),
                ServiceAction::Stop => self.handle_service_result(usecase.stop_service(service)),
                ServiceAction::Restart => self.handle_service_result(usecase.restart_service(service)),
                ServiceAction::Enable => self.handle_service_result(usecase.enable_service(service)),
                ServiceAction::Disable => self.handle_service_result(usecase.disable_service(service)),
                ServiceAction::ToggleFilter => {
                    self.table_state.select(Some(0));
                    self.filter_all = !self.filter_all;
                    self.fetch_and_refresh(self.old_filter_text.clone());
                },
                ServiceAction::RefreshAll => {
                    self.fetch_services();
                    self.fetch_and_refresh(self.old_filter_text.clone());
                },
            }
        }
        self.set_ignore_key_events(false);
    }

    fn handle_service_result(&mut self, result: Result<Service, Box<dyn Error>>) {
        match result {
            Ok(service) => {
                if let Some(pos) = self.services.iter().position(|s| s.name() == service.name()) {
                    self.services[pos] = service;
                } else {
                    self.services.push(service);
                }
                self.refresh(self.old_filter_text.clone());
            }
            Err(e) => {
                self.sender.send(AppEvent::Error(e.to_string())).unwrap();
            }
        }
    }

    pub fn shortcuts(&mut self) -> Vec<Line<'_>> {
        let mut help_text: Vec<Line<'_>> = Vec::new();
        if !self.ignore_key_events {
            help_text.push(Line::from(Span::styled(
                "Actions on the selected service",
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            )));

            help_text.push(Line::from(
                "Navigate: ↑/↓ | Switch tab: ←/→ | Toggle Filter: f | Start: s | Stop: x | Restart: r | Enable: e | Disable: d | Refresh all: u | Log: v | Unit File: c"
            ));
        }

        help_text
    }
}
