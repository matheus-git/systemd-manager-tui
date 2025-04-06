use ratatui::{
    Frame,
    layout::Constraint,
    widgets::{Block, Borders, Row, Table, TableState},
};
use color_eyre::Result;
use crate::usecases::list_services::list_services;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::style::{Style, Color, Modifier};

pub struct TableServices {
    pub table_state: TableState,
    pub rows: Vec<Row<'static>>,
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

        Self { table_state, rows }
    }

    pub fn render(&mut self, frame: &mut Frame){
        let area = frame.area();

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
            .highlight_style(
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
            _ => {}
        }
    }
}


