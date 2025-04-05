use ratatui::{
    Frame,
    layout::Constraint,
    widgets::{Block, Borders, Row, Table},
};
use crate::usecases::list_services::list_services;

pub fn draw_list_services(frame: &mut Frame) {
    let area = frame.area();

    let rows = if let Ok(services) = list_services() {
        services
            .into_iter()
            .map(|service| {
                Row::new(vec![
                    service.name,
                    service.description,
                    service.load_state,
                    service.active_state,
                    service.sub_state,
                ])
            })
            .collect()
    } else {
        vec![Row::new(vec!["Error loading services", "", "", "", ""])]
    };

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


