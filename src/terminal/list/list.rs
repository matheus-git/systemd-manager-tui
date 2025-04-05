use ratatui::{
    Frame,
    layout::Constraint,
    widgets::{Block, Borders, Row, Table, TableState},
};

use crate::usecases::list_services::list_services;

pub fn draw_list_services(frame: &mut Frame) {
    let area = frame.area();

    let mut rows = Vec::new();

    if let Ok(services) = list_services() {
        rows = services
            .into_iter()
            .map(|cols| Row::new(cols))
            .collect();
    }

    let table = Table::new(
            rows,
            [
                Constraint::Percentage(20),
                Constraint::Percentage(30),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(10),
            ],
        )
        .header(Row::new(["Name", "Description", "Load", "Active", "Sub"]))
        .block(Block::default().title("Systemd Services").borders(Borders::ALL));

    frame.render_widget(table, area);
}

