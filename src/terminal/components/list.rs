use ratatui::{
    Frame,
    layout::Constraint,
    widgets::{Block, Borders, Row, Table, TableState},
};
use crate::usecases::services_manager::ServicesManager;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{Style, Color, Modifier};
use ratatui::layout::Rect;
use std::error::Error;

use crate::domain::service::Service;

fn generate_rows(services: &[Service]) -> Vec<Row<'static>> {
    services 
        .iter()
        .map(|service| {
            Row::new(vec![
                service.name().to_string(),
                format!("{} ({})",service.state().active(), service.state().sub() ),
                service.state().file().to_string(),
                service.state().load().to_string(),
                service.description().to_string(),
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
}

pub struct TableServices<'a> {
    table: Table<'a>,
    pub table_state: TableState,
    pub rows: Vec<Row<'static>>,
    pub services: Vec<Service>,
    old_filter_text: String,
    ignore_key_events: bool
}

impl TableServices<'_> {
    pub fn new() -> Self {
        let (services, rows) = match ServicesManager::list_services() {
            Ok(svcs) => {
                let rows = generate_rows(&svcs);
                (svcs, rows)
            },
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
                Constraint::Percentage(30),
            ],
        )
            .header(
                Row::new(["Name", "Active", "State", "Load", "Description"])
                    .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            )
            .block(Block::default().title("Systemd Services").borders(Borders::ALL))
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
            services,
            old_filter_text: String::new(),
            ignore_key_events: false,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect){
        frame.render_stateful_widget(self.table.clone(), area, &mut self.table_state);
    }

    pub fn toogle_ignore_key_events(&mut self, has_ignore_key_events: bool){
        self.ignore_key_events = has_ignore_key_events
    }

    pub fn get_selected_service(&self) -> Option<&Service>{
        if let Some(selected_index) = self.table_state.selected() {
            if let Some(service) = self.services.get(selected_index) {
                return Some(service);
            }
        }
        None
    }

    pub fn refresh(&mut self, filter_text: String) {
        self.old_filter_text = filter_text.clone();
        self.services = self.filter_services(&filter_text);
        self.rows = generate_rows(&self.services);
        self.table = self.table.clone().rows(self.rows.clone());
    }

    fn filter_services(&self, filter_text: &str) -> Vec<Service> {
        let lower_filter = filter_text.to_lowercase();
        if let Ok(services) = ServicesManager::list_services() {
            services.into_iter().filter(|service| service.name().to_lowercase().contains(&lower_filter)).collect()
        } else {
            vec![]
        }
    }

    pub fn on_key_event(&mut self, key: KeyEvent) {
        if self.ignore_key_events {
            return;
        }

        match key.code {
            KeyCode::Down => self.select_next(),
            KeyCode::Up => self.select_previous(),
            KeyCode::Char('r') => self.act_on_selected_service(ServiceAction::Restart),
            KeyCode::Char('s') => self.act_on_selected_service(ServiceAction::Start),
            KeyCode::Char('e') => self.act_on_selected_service(ServiceAction::Enable),
            KeyCode::Char('d') => self.act_on_selected_service(ServiceAction::Disable),
            KeyCode::Char('u') => self.act_on_selected_service(ServiceAction::RefreshAll),
            KeyCode::Char('x') => self.act_on_selected_service(ServiceAction::Stop),
            _ => {}
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

    fn act_on_selected_service(&mut self, action: ServiceAction) {
        if let Some(service) = self.get_selected_service() {
            match action {
                ServiceAction::Start => self.handle_result(ServicesManager::start_service(service)),
                ServiceAction::Stop => self.handle_result(ServicesManager::stop_service(service)),
                ServiceAction::Restart => self.handle_result(ServicesManager::restart_service(service)),
                ServiceAction::Enable => self.handle_result(ServicesManager::enable_service(service)),
                ServiceAction::Disable => self.handle_result(ServicesManager::disable_service(service)),
                ServiceAction::RefreshAll => self.refresh(self.old_filter_text.clone()),
            }
        }
        self.refresh(self.old_filter_text.clone());
    }

    fn handle_result(&mut self, result: Result<(), Box<dyn Error>>) {
        match result {
            Ok(_) => {},
            Err(e) => {panic!("{e}")}
        }
    }
}


