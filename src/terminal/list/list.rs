use ratatui::Frame;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};

use crate::usecases::list_services::list_services;

pub fn draw_list_services(frame: &mut Frame){
    let title = Line::from("Ratatui Simple Template")
        .bold()
        .blue()
        .centered();
    let text = "Hello, Ratatui!\n\n\
        Created using https://github.com/ratatui/templates\n\
        Press `Esc`, `Ctrl-C` or `q` to stop running.";
    frame.render_widget(
        Paragraph::new(list_services()[0])
            .block(Block::bordered().title(title))
            .centered(),
        frame.area(),
    )
}
