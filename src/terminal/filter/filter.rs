use color_eyre::Result;
use ratatui::{
    crossterm::event::{self, KeyEvent, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, Paragraph},
    DefaultTerminal, Frame,
};
use std::rc::Rc;
use std::cell::RefCell;

use super::super::list::list::TableServices;

pub struct Filter {
    pub input: String,
    character_index: usize,
    input_mode: InputMode,
    messages: Vec<String>,
    table_service: Option<Rc<RefCell<TableServices>>>,
}

enum InputMode {
    Normal,
    Editing,
}

impl Filter {
    pub const fn new() -> Self {
        Self {
            input: String::new(),
            input_mode: InputMode::Normal,
            messages: Vec::new(),
            character_index: 0,
            table_service: None
        }
    }

        pub fn set_table_service(&mut self, ts: Rc<RefCell<TableServices>>) {
        self.table_service = Some(ts);
    }
    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {

            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            let after_char_to_delete = self.input.chars().skip(current_index);

            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    fn reset_cursor(&mut self) {
        self.character_index = 0;
    }

    fn submit_message(&mut self) {
        if let Some(ref ts) = self.table_service {
            // aqui você pode chamar funções do TableServices, por exemplo:
            let mut ts_mut = ts.borrow_mut();
            ts_mut.refresh(self.input.clone());
        }
    }

    pub fn on_key_event(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (_, KeyCode::Enter) => self.submit_message(),
            (_, KeyCode::Char(to_insert)) => self.enter_char(to_insert),
            (_, KeyCode::Backspace) => self.delete_char(),
            (_, KeyCode::Left) => self.move_cursor_left(),
            (_, KeyCode::Right) => self.move_cursor_right(),
            _ => {}
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let vertical = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
        ]);
        let [help_area, input_area] = vertical.areas(area);

        let (msg, style) =  (
            vec![
                "Press ".into(),
                "Esc".bold(),
                " to stop editing, ".into(),
                "Enter".bold(),
                " to record the message".into(),
            ],
            Style::default(),
        )
        ;
        let text = Text::from(Line::from(msg)).patch_style(style);
        let help_message = Paragraph::new(text);
        frame.render_widget(help_message, help_area);

        let input = Paragraph::new(self.input.as_str())
            .style(match self.input_mode {
                InputMode::Normal => Style::default(),
                InputMode::Editing => Style::default(),
            })
            .block(Block::bordered().title("Input"));
        frame.render_widget(input, input_area);
    }
}
