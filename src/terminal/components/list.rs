use ratatui::{
    Frame,
    layout::Constraint,
    widgets::{Cell, Block, Borders, Row, Table, TableState},
};
use crate::usecases::services_manager::ServicesManager;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{Style, Color, Modifier};
use ratatui::text::{Line, Span};
use ratatui::layout::Rect;
use core::panic;
use std::error::Error;
use std::sync::mpsc::Sender;

use crate::terminal::app::{Actions, AppEvent};
use crate::domain::service::Service;

fn generate_rows(services: &[Service]) -> Vec<Row<'static>> {
    services
        .iter()
        .map(|service| {
            let highlight_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
            let normal_style = Style::default().fg(Color::Gray);

            let state_style = match service.state().active() {
                "active" => Style::default().fg(Color::Green),    
                "activating" => Style::default().fg(Color::Yellow), 
                _ => Style::default().fg(Color::Red),     
            };

            Row::new(vec![
                Cell::from(service.formatted_name().to_string()).style(highlight_style),
                Cell::from(format!("{} ({})", service.state().active(), service.state().sub()))
                    .style(state_style),
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
}

pub struct TableServices<'a> {
    table: Table<'a>,
    pub table_state: TableState,
    pub rows: Vec<Row<'static>>,
    pub services: Vec<Service>,
    filtered_services: Vec<Service>,
    old_filter_text: String,
    pub ignore_key_events: bool,
    sender: Sender<AppEvent>
}

impl TableServices<'_> {
    pub fn new(sender: Sender<AppEvent>) -> Self {
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
                Constraint::Percentage(15),
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
            filtered_services: services.clone(),
            services,
            sender,
            old_filter_text: String::new(),
            ignore_key_events: false,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect){
        frame.render_stateful_widget(&self.table, area, &mut self.table_state);
    }

    pub fn set_ignore_key_events(&mut self, has_ignore_key_events: bool){
        if has_ignore_key_events {
             self.table = self.table.clone().row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );
        }else {
            self.table = self.table.clone().row_highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );
        }
        self.ignore_key_events = has_ignore_key_events
    }

    pub fn get_selected_service(&self) -> Option<&Service>{
        if let Some(selected_index) = self.table_state.selected() {
            if let Some(service) = self.filtered_services.get(selected_index) {
                return Some(service);
            }
        }
        None
    }

    pub fn refresh(&mut self, filter_text: String) {
        self.old_filter_text = filter_text.clone();
        self.filtered_services = self.filter(filter_text, self.services.clone());
        self.rows = generate_rows(&self.filtered_services);

        if self.table_state.selected().is_none() && !self.rows.is_empty() {
            self.table_state.select(Some(0));
        }
        self.table = self.table.clone().rows(self.rows.clone());
    }

    fn fetch_services(&mut self) {
        if let Ok(services) = ServicesManager::list_services() {
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
        services.into_iter().filter(|service| service.formatted_name().to_lowercase().contains(&lower_filter)).collect()
    }

    pub fn on_key_event(&mut self, key: KeyEvent) {
        if self.ignore_key_events {
            return;
        }

        match key.code {
            KeyCode::Down => self.select_next(),
            KeyCode::Up => self.select_previous(),
            KeyCode::PageDown => self.select_page_down(),
            KeyCode::PageUp => self.select_page_up(),
            KeyCode::Char('r') => self.act_on_selected_service(ServiceAction::Restart),
            KeyCode::Char('s') => self.act_on_selected_service(ServiceAction::Start),
            KeyCode::Char('e') => self.act_on_selected_service(ServiceAction::Enable),
            KeyCode::Char('d') => self.act_on_selected_service(ServiceAction::Disable),
            KeyCode::Char('u') => self.act_on_selected_service(ServiceAction::RefreshAll),
            KeyCode::Char('x') => self.act_on_selected_service(ServiceAction::Stop),
            KeyCode::Char('v') => self.sender.send(AppEvent::Action(Actions::GoLog)).unwrap(),
            KeyCode::Char('p') => self.sender.send(AppEvent::Action(Actions::GoDetails)).unwrap(),
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

    fn act_on_selected_service(&mut self, action: ServiceAction) {
        if let Some(service) = self.get_selected_service() {
            match action {
                ServiceAction::Start => self.handle_result(ServicesManager::start_service(service)),
                ServiceAction::Stop => self.handle_result(ServicesManager::stop_service(service)),
                ServiceAction::Restart => self.handle_result(ServicesManager::restart_service(service)),
                ServiceAction::Enable => self.handle_result(ServicesManager::enable_service(service)),
                ServiceAction::Disable => self.handle_result(ServicesManager::disable_service(service)),
                ServiceAction::RefreshAll => self.fetch_services(),
            }
        }
        self.fetch_and_refresh(self.old_filter_text.clone());
    }

    fn handle_result(&mut self, result: Result<(), Box<dyn Error>>) {
        match result {
            Ok(_) => {},
            Err(e) => {panic!("{e}")}
        }
    }

    pub fn shortcuts(&mut self) -> Vec<Line<'_>> {
        let mut help_text: Vec<Line<'_>> = Vec::new(); 
        if !self.ignore_key_events {
            help_text.push(Line::from(Span::styled(
                "Actions on the selected service",
                Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD),
            )));

            help_text.push(Line::from(
                "Navigate: ↑/↓ | Start: s | Stop: x | Restart: r | Enable: e | Disable: d | Refresh all: u | View logs: v | Properties: p"
            ));
        }

        help_text 
    }
}


