use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};
use ratatui::style::{Style, Color};
use ratatui::widgets::{Paragraph, Block, Borders, Wrap};
use ratatui::text::Text;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use super::list::list::TableServices;
use super::filter::filter::Filter;

#[derive(Debug, Default)]
pub enum Status {
    #[default]
    Log,
    List,
    PopUp
}

#[derive(Debug, Default)]
pub struct App { 
    running: bool,
    status: Status
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;
        let mut table_service = TableServices::new();
        let mut filter = Filter::new();

        while self.running {
            terminal.draw(|frame| {
                let area = frame.area();
                let chunks = Layout::vertical(
                    [Constraint::Length(4), Constraint::Fill(1)]
                );
                let [filter_box, list] = chunks.areas(area);

                filter.draw(frame, filter_box);
                table_service.render(frame, list);
            })?;

            self.handle_crossterm_events(|key| {
                table_service.on_key_event(key);
                filter.on_key_event(key);
            })?;
        }

        Ok(())
    }

    fn handle_crossterm_events<F>(&mut self, mut external_handler: F) -> Result<()>
where
        F: FnMut(KeyEvent),
    {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                self.on_key_event(key);
                external_handler(key);
            },
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
            _ => {}
        }
        Ok(())
    }

    fn on_key_event(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (_, KeyCode::Char('q'))
            | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit(),
            _ => {}
        }
    }

    fn quit(&mut self) {
        self.running = false;
    }
}
