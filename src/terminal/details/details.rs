use std::sync::mpsc::Sender;

use ratatui::{
    layout::{Constraint, Layout, Rect, Alignment},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use crossterm::event::{KeyCode, KeyEvent};

use crate::domain::service::service::Service;
use crate::terminal::terminal::Actions;

pub struct ServiceDetails {
    service: Option<Service>,
    log_lines: Option<String>,  
    scroll: u16,
    sender: Option<Sender<Actions>>
}

impl ServiceDetails {
    pub fn new() -> Self {
        Self {
            service: None,
            log_lines: None,
            scroll: 0,
            sender: None
        }
    }

    pub fn init_refresh_thread(&self) {
    }

    pub fn set_sender(&mut self, sender: Sender<Actions>){
        self.sender = Some(sender);
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let [log_box] = Layout::vertical([
            Constraint::Min(0),
        ])
            .areas(area);

        if let Some(service) = &self.service {
            if let Some(log_lines) = &self.log_lines {
                let log_paragraph = Paragraph::new(log_lines.clone())  
                    .scroll((self.scroll,0)) 
                    .wrap(Wrap { trim: false})
                    .block(Block::default()
                        .title(format!(" {} logs (newest at the top) ", &service.name))
                        .borders(Borders::ALL)
                        .title_alignment(Alignment::Center));

                frame.render_widget(log_paragraph, log_box);
            } else {
                let log_text = Text::from("No logs available.");
                let log_paragraph = Paragraph::new(log_text)
                    .block(Block::default().title("Logs").borders(Borders::ALL));

                frame.render_widget(log_paragraph, log_box);
            }
        }
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
                    let _ = sender.send(Actions::RefreshLog);
                }
            },
            KeyCode::Char('q') => {
                if let Some(sender) = &self.sender {
                    let _ = sender.send(Actions::GoList);
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

    pub fn set_service(&mut self, service: Service) {
        self.service = Some(service);
    }

    pub fn set_log_lines(&mut self, log_lines: String) {
        let reversed = log_lines
            .lines()
            .rev() 
            .collect::<Vec<_>>()
            .join("\n");

        self.log_lines = Some(reversed);
        self.scroll = 0;
    }
}

