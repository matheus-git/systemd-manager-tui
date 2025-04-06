use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};


use super::list::list::TableServices;

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
        while self.running {
            terminal.draw(|frame| table_service.render(frame)) ?;
            self.handle_crossterm_events(|key| {
                table_service.on_key_event(key);
            })?;
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
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
            (_, KeyCode::Esc | KeyCode::Char('q'))
            | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit(),
            _ => {}
        }
    }

    fn quit(&mut self) {
        self.running = false;
    }
}
