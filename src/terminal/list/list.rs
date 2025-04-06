use ratatui::{
    Frame,
    layout::Constraint,
    widgets::{Block, Borders, Row, Table, TableState},
};
use color_eyre::Result;
use crate::usecases::list_services::list_services;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::style::{Style, Color, Modifier};
use ratatui::layout::Rect;

pub struct TableServices {
    pub table_state: TableState,
    pub rows: Vec<Row<'static>>,
    old_filter_text: String
}

impl TableServices {
    pub fn new() -> Self {
        let rows = if let Ok(services) = list_services() {
            services
                .into_iter()
                .map(|service| {
                    Row::new(vec![
                        service.name,
                        service.active_state,
                        service.file_state,
                        service.load_state,
                        service.description,
                    ])
                })
                .collect()
        } else {
            vec![Row::new(vec!["Error loading services", "", "", "", ""])]
        };

        let mut table_state = TableState::default();
        table_state.select(Some(0));

        Self { table_state, rows, old_filter_text: "".to_string() }
    }

    pub fn refresh(&mut self, filter_text: String) {
        if self.old_filter_text == filter_text {
            return;
        }

        let lower_filter = filter_text.to_lowercase();

        let rows = if let Ok(services) = list_services() {
            services
                .into_iter()
                .filter(|service| service.name.to_lowercase().contains(&lower_filter))
                .map(|service| {
                    Row::new(vec![
                        service.name,
                        service.active_state,
                        service.file_state,
                        service.load_state,
                        service.description,
                    ])
                })
                .collect()
        } else {
            vec![Row::new(vec!["Error loading services", "", "", "", ""])]
        };
        self.table_state.select(Some(0));

        self.rows = rows;
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
            //(_, KeyCode::Char('r')) => self.refresh(self.old_filter_text.clone()),
            _ => {}
        }
    }
}


