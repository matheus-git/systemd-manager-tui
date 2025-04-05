use ratatui::Frame;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};

pub fn list_services(frame: &mut Frame){
    let title = Line::from("Ratatui Simple Template")
        .bold()
        .blue()
        .centered();
    let text = "Hello, Ratatui!\n\n\
        Created using https://github.com/ratatui/templates\n\
        Press `Esc`, `Ctrl-C` or `q` to stop running.";
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::bordered().title(title))
            .centered(),
        frame.area(),
    )
}
