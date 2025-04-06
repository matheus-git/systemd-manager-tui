use ratatui::{
    Frame,
    layout::Constraint,
    widgets::{Block, Borders, Row, Table, TableState},
};
use crate::usecases::list_services::list_services;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{Style, Color, Modifier};
use ratatui::layout::Rect;
use std::time::Duration;
use std::thread;

use crate::domain::service::service::Service;
use crate::{domain::service::service_repository::ServiceRepository, infrastructure::systemd_service_adapter::SystemdServiceAdapter};

pub struct TableServices {
    pub table_state: TableState,
    pub rows: Vec<Row<'static>>,
    pub services: Vec<Service>,
    old_filter_text: String,
    ignore_key_events: bool
}

impl TableServices {
    pub fn new() -> Self {
        let mut services = vec![];
        let rows = if let Ok(list) = list_services() {
            services = list;
            services
                .iter()
                .map(|service| {
                    Row::new(vec![
                        service.name.clone(),
                        service.active_state.clone(),
                        service.file_state.clone(),
                        service.load_state.clone(),
                        service.description.clone(),
                    ])
                })
                .collect()
        } else {
            vec![Row::new(vec!["Error loading services", "", "", "", ""])]
        };

        let mut table_state = TableState::default();
        table_state.select(Some(0));

        Self {
            table_state,
            rows,
            services,
            old_filter_text: "".to_string(),
            ignore_key_events: false
        }
    }

    pub fn toogle_ignore_key_events(&mut self, has_ignore_key_events: bool){
        self.ignore_key_events = has_ignore_key_events
    }

    pub fn refresh(&mut self, filter_text: String) {
        if self.ignore_key_events {
            return;
        }

        let lower_filter = filter_text.to_lowercase();

        if let Ok(services) = list_services() {
            let filtered_services: Vec<Service> = services
                .into_iter()
                .filter(|service| service.name.to_lowercase().contains(&lower_filter))
                .collect();

            let rows = filtered_services
                .iter()
                .map(|service| {
                    Row::new(vec![
                        service.name.clone(),
                        service.active_state.clone(),
                        service.file_state.clone(),
                        service.load_state.clone(),
                        service.description.clone(),
                    ])
                })
                .collect();

            self.services = filtered_services;
            self.rows = rows;
        } else {
            self.services = vec![];
            self.rows = vec![Row::new(vec!["Error loading services", "", "", "", ""])];
        }

        self.old_filter_text = filter_text;
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect){
        let table = Table::new(
            self.rows.clone(),
            [
                Constraint::Percentage(20),
                Constraint::Length(10),
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

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    pub fn on_key_event(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (_,KeyCode::Down) => self.table_state.select_next(),
            (_,KeyCode::Up) => self.table_state.select_previous(),
            (_, KeyCode::Char('r')) => self.restart_service(),
            (_, KeyCode::Char('s')) => self.start_service(),
            (_, KeyCode::Char('e')) => self.enable_service(),
            (_, KeyCode::Char('d')) => self.disable_service(),
            (_, KeyCode::Char('u')) => self.refresh(self.old_filter_text.clone()),
            (_, KeyCode::Char('x')) => self.stop_service(),
            _ => {}
        }
    }
    fn enable_service(&mut self) {
        match self.table_state.selected() {
            Some(selected) => {
                if let Some(service) = self.services.get(selected) {
                    SystemdServiceAdapter.enable_service(service.name.as_str()).expect("REASON");
                    thread::sleep(Duration::from_millis(200));
                    self.refresh(self.old_filter_text.clone());
                }
            }
            None => {},
        }
    }
    fn disable_service(&mut self) {
        match self.table_state.selected() {
            Some(selected) => {
                if let Some(service) = self.services.get(selected) {
                    SystemdServiceAdapter.disable_service(service.name.as_str()).expect("REASON");
                    thread::sleep(Duration::from_millis(200));
                    self.refresh(self.old_filter_text.clone());
                }
            }
            None => {},
        }
    }
    fn stop_service(&mut self) {
        match self.table_state.selected() {
            Some(selected) => {
                if let Some(service) = self.services.get(selected) {
                    SystemdServiceAdapter.stop_service(service.name.as_str()).expect("REASON");
                    thread::sleep(Duration::from_millis(200));
                    self.refresh(self.old_filter_text.clone());
                }
            }
            None => {},
        }
    }
    fn start_service(&mut self) {
        match self.table_state.selected() {
            Some(selected) => {
                if let Some(service) = self.services.get(selected) {
                    SystemdServiceAdapter.start_service(service.name.as_str()).expect("REASON");
                    thread::sleep(Duration::from_millis(200));
                    self.refresh(self.old_filter_text.clone());
                }
            }
            None => {},
        }
    }
    fn restart_service(&mut self) {
        match self.table_state.selected() {
            Some(selected) => {
                if let Some(service) = self.services.get(selected) {
                    SystemdServiceAdapter.restart_service(service.name.as_str()).expect("REASON");
                    thread::sleep(Duration::from_millis(200));
                    self.refresh(self.old_filter_text.clone());
                }
            }
            None => {},
        }
    }   
}


