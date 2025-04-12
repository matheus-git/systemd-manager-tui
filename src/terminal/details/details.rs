use std::sync::mpsc::Sender;

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use crossterm::event::{KeyCode, KeyEvent};

use crate::domain::service::service::Service;

pub struct ServiceDetails {
    service: Option<Service>,
    log_lines: Option<String>,  
    scroll: u16,
    sender: Option<Sender<String>>
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

    pub fn set_sender(&mut self, sender: Sender<String>){
        self.sender = Some(sender);
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let [log_box] = Layout::vertical([
            Constraint::Min(0),
        ])
        .areas(area);

        if let Some(log_lines) = &self.log_lines {
            let log_paragraph = Paragraph::new(log_lines.clone())  
                .scroll((self.scroll,0)) 
                .wrap(Wrap { trim: false})
                .block(Block::default().title("Logs").borders(Borders::ALL));

            frame.render_widget(log_paragraph, log_box);
        } else {
            let log_text = Text::from("Nenhum log disponível.");
            let log_paragraph = Paragraph::new(log_text)
                .block(Block::default().title("Logs").borders(Borders::ALL));

            frame.render_widget(log_paragraph, log_box);
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
            KeyCode::Char('q') => {
                if let Some(sender) = &self.sender {
                    sender.send("cliquei".to_string());
                }
            },
            _ => {}
        }
    }

    
    pub fn set_service(&mut self, service: Service) {
        self.service = Some(service);
    }

    pub fn set_log_lines(&mut self, log_lines: String) {
    let reversed = log_lines
        .lines()
        .rev() // inverte a ordem das linhas
        .collect::<Vec<_>>()
        .join("\n");

    self.log_lines = Some(reversed);
    self.scroll = 0; // começa do topo agora
}}

