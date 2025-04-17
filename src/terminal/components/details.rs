use std::sync::mpsc::Sender;

use ratatui::{
    layout::{Rect, Alignment},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use crossterm::event::{KeyCode, KeyEvent};

use crate::terminal::app::{Actions, AppEvent};

pub struct ServiceDetails<'a> {
    log_paragraph: Option<Paragraph<'a>>,
    service_name: String,
    scroll: u16,
    sender: Option<Sender<AppEvent>>
}

impl ServiceDetails<'_> {
    pub fn new() -> Self {
        Self {
            log_paragraph: None,
            service_name: String::new(),
            scroll: 0,
            sender: None
        }
    }

    pub fn init_refresh_thread(&self) {
    }

    pub fn set_sender(&mut self, sender: Sender<AppEvent>){
        self.sender = Some(sender);
    }

    pub fn set_service_name(&mut self, service_name: String){
        self.service_name = service_name;
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {

        let paragraph = self.log_paragraph.clone().unwrap_or_else(|| {
            Paragraph::new(Text::from("No logs available."))
                .block(Block::default().title("Logs").borders(Borders::ALL))
        }).scroll((self.scroll,0));

        frame.render_widget(paragraph, area);
    }

    pub fn on_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                if self.scroll > 0 {
                    self.scroll -= 1;
                }
            }
            KeyCode::Down => {
                self.scroll += 1;
            },
            KeyCode::Char('v') => {
                if let Some(sender) = &self.sender {
                    let _ = sender.send(AppEvent::Action(Actions::RefreshLog));
                }
            },
            KeyCode::Char('q') => {
                if let Some(sender) = &self.sender {
                    let _ = sender.send(AppEvent::Action(Actions::GoList));
                }
            },
            _ => {}
        }
    }

    pub fn draw_shortcuts(&mut self, frame: &mut Frame, help_area: Rect){
        let help_text = vec![
            Line::from(vec![
                Span::styled("Actions", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("Scroll: ↑/↓ | Refresh: v | Go back: q"),
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
    }

    pub fn set_log_lines(&mut self, log_lines: String) {
        let reversed = log_lines
            .lines()
            .rev() 
            .collect::<Vec<_>>()
            .join("\n");

        self.log_paragraph = Some(Paragraph::new(reversed.clone())  
            .scroll((0,0)) 
            .wrap(Wrap { trim: false})
            .block(Block::default()
                .title(format!(" {} logs (newest at the top) ", self.service_name))
                .borders(Borders::ALL)
                .title_alignment(Alignment::Center)));
        self.scroll = 0;
    }
}

