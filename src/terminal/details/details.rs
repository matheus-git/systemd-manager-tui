use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crossterm::event::{KeyCode, KeyEvent};
use crate::domain::service::service::Service;

pub struct ServiceDetails {
    service: Option<Service>,
    log_lines: Option<String>,  
    scroll: u16,
}

impl ServiceDetails {
    pub fn new() -> Self {
        Self {
            service: None,
            log_lines: None,
            scroll: 0,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let [info_box, log_box] = Layout::vertical([
            Constraint::Length(5),
            Constraint::Min(0),
        ])
        .areas(area);

        if let Some(service) = &self.service {
            let info_text = Text::from(vec![
                Line::from(vec![
                    Span::styled("Nome: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(&service.name),
                ]),
                Line::from(vec![
                    Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(&service.active_state),
                ]),
            ]);

            let info = Paragraph::new(info_text)
                .block(Block::default().title("Informações").borders(Borders::ALL));
            frame.render_widget(info, info_box);
        } else {
            let info_text = Text::from("Nenhum serviço selecionado.");
            let info = Paragraph::new(info_text)
                .block(Block::default().title("Informações").borders(Borders::ALL));
            frame.render_widget(info, info_box);
        }

        let visible_height = log_box.height.saturating_sub(2); 

        if let Some(log_lines) = &self.log_lines {
            let log_paragraph = Paragraph::new(log_lines.clone())  
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
            }
            _ => {}
        }
    }

    
    pub fn set_service(&mut self, service: Service) {
        self.service = Some(service);
    }

    pub fn set_log_lines(&mut self, log_lines: String) {
        self.log_lines = Some(log_lines);
    }
}

