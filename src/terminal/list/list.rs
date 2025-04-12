use ratatui::{
    Frame,
    layout::Constraint,
    widgets::{Block, Borders, Row, Table, TableState},
};
use crate::usecases::services_manager::ServicesManager;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{Style, Color, Modifier};
use ratatui::layout::Rect;

use crate::domain::service::service::Service;

fn generate_rows(services: &Vec<Service>) -> Vec<Row<'static>> {
    services 
        .iter()
        .map(|service| {
            Row::new(vec![
                service.name.clone(),
                format!("{} ({})",service.active_state.clone(), service.sub_state.clone() ) ,
                service.file_state.clone(),
                service.load_state.clone(),
                service.description.clone(),
            ])
        })
        .collect()
}

pub struct TableServices {
    pub table_state: TableState,
    pub rows: Vec<Row<'static>>,
    pub services: Vec<Service>,
    old_filter_text: String,
    ignore_key_events: bool
}

impl TableServices {
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

        Self {
            table_state,
            rows,
            services,
            old_filter_text: String::new(),
            ignore_key_events: false,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect){
        let table = Table::new(
            self.rows.clone(),
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

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    pub fn toogle_ignore_key_events(&mut self, has_ignore_key_events: bool){
        self.ignore_key_events = has_ignore_key_events
    }

    pub fn get_selected_service(&self) -> Option<&Service>{
        if let Some(selected_index) = self.table_state.selected() {
            if let Some(service) = self.services.get(selected_index) {
                return Some(&service);
            }
        }
        return None
    }

    pub fn refresh(&mut self, filter_text: String) {
        let lower_filter = filter_text.to_lowercase();

        if let Ok(services) = ServicesManager::list_services() {
            let filtered_services: Vec<Service> = services
                .into_iter()
                .filter(|service| service.name.to_lowercase().contains(&lower_filter))
                .collect();

            self.rows = generate_rows(&filtered_services);
            self.services = filtered_services;
        } else {
            self.services = vec![];
            self.rows = vec![Row::new(vec!["Error loading services", "", "", "", ""])];
        }

        self.old_filter_text = filter_text;
    }

    pub fn on_key_event(&mut self, key: KeyEvent) {
        if self.ignore_key_events {
            return;
        }

        match (key.modifiers, key.code) {
            (_, KeyCode::Down) => {
                if let Some(selected_index) = self.table_state.selected() {
                    if selected_index == self.rows.len() - 1 {
                        self.table_state.select(Some(0));
                    } else {
                        self.table_state.select_next();
                    }
                }else {
                    self.table_state.select(Some(0));
                }
            }

            (_, KeyCode::Up) => {
                if let Some(selected_index) = self.table_state.selected() {
                    if selected_index == 0 {
                        self.table_state.select(Some(self.rows.len() - 1));
                    } else {
                        self.table_state.select_previous();
                    }
                }else {
                    self.table_state.select(Some(0));
                }
            },
            (_, KeyCode::Char('r')) => self.act_on_selected_service("restart"),
            (_, KeyCode::Char('s')) => self.act_on_selected_service("start"),
            (_, KeyCode::Char('e')) => self.act_on_selected_service("enable"),
            (_, KeyCode::Char('d')) => self.act_on_selected_service("disable"),
            (_, KeyCode::Char('u')) => self.act_on_selected_service("refresh_all"),
            (_, KeyCode::Char('x')) => self.act_on_selected_service("stop"),
            _ => {}
        }
    }

    fn act_on_selected_service(&mut self, action: &str) {
        if let Some(service) = self.get_selected_service() {
            match action {
                "start" => ServicesManager::start_service(&service.name),
                "stop"  => ServicesManager::stop_service(&service.name),
                "restart" => ServicesManager::restart_service(&service.name),
                "enable" => ServicesManager::enable_service(&service.name),
                "disable" => ServicesManager::disable_service(&service.name),
                _ => {}
            }
            self.refresh(self.old_filter_text.clone());
        }
    }
}


