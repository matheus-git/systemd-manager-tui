use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use ratatui::style::{Modifier, Style, Color};
use ratatui::widgets::{Paragraph, Block, Borders};
use ratatui::layout::{Constraint, Layout};
use std::rc::Rc;
use std::cell::RefCell;
use super::list::list::TableServices;
use super::filter::filter::Filter;
use ratatui::text::{Line, Span};

#[derive(Debug, Default)]
pub struct App { 
    running: bool,
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

                let chunks = Layout::vertical([
                    Constraint::Length(4),    
                    Constraint::Min(10),     
                    Constraint::Length(6),  
                ])
                    .split(area);

                let filter_box = chunks[0];
                let list = chunks[1];
                let help_area = chunks[2];

                filter.draw(frame, filter_box);
                table_service.borrow_mut().render(frame, list);

                let help_text = vec![
                    Line::from(vec![
                        Span::styled("Actions on the selected service", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from("↑/↓: Navigate | Start: s | Restart: r | Enable: e | Disable: d | Stop: x | Refresh all: u"),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Exit", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                        Span::raw(": Ctrl + c"),
                    ]),
                ];

                let help_block = Paragraph::new(help_text)
                    .block(Block::default().title("Shortcuts").borders(Borders::ALL))
                    .wrap(ratatui::widgets::Wrap { trim: true });

                frame.render_widget(help_block, help_area);
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
            (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit(),
            _ => {}
        }
    }

    fn quit(&mut self) {
        self.running = false;
    }
}
