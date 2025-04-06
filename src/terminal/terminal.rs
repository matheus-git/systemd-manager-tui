use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};
use ratatui::style::{Style, Color};
use ratatui::widgets::{Paragraph, Block, Borders, Wrap};
use ratatui::text::Text;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use std::rc::Rc;
use std::cell::RefCell;
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
        let table_service = Rc::new(RefCell::new(TableServices::new()));
        let mut filter = Filter::new();
        filter.set_table_service(Rc::clone(&table_service));

        while self.running {
            terminal.draw(|frame| {
                let area = frame.area();
                let chunks = Layout::vertical(
                    [Constraint::Max(4), Constraint::Fill(1)]
                );
                let [filter_box, list] = chunks.areas(area);

                filter.draw(frame, filter_box);
                table_service.borrow_mut().render(frame, list);
            })?;

            self.handle_crossterm_events(|key| {
                table_service.borrow_mut().on_key_event(key);
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
            _ => {}
        }
        Ok(())
    }

    fn on_key_event(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (_, KeyCode::Esc)
            | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit(),
            _ => {}
        }
    }

    fn quit(&mut self) {
        self.running = false;
    }
}
