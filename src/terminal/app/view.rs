use color_eyre::Result;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Tabs};
use ratatui::{DefaultTerminal, Frame};
use rayon::prelude::*;

use super::{App, get_user_friendly_error};
use crate::terminal::components::list::ActiveFilterState;

impl App {
    fn draw_help_popup(&self, frame: &mut Frame, area: Rect) {
        let popup_width = std::cmp::min(80, area.width.saturating_sub(4));
        let popup_height = std::cmp::min(38, area.height.saturating_sub(4));
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(
            area.x + popup_x,
            area.y + popup_y,
            popup_width,
            popup_height,
        );

        frame.render_widget(Clear, popup_area);

        let heading = |text| {
            Line::from(vec![Span::styled(
                text,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )])
        };
        let text = vec![
            Line::from(vec![Span::styled(
                "SYSTEMD MANAGER TUI - HELP",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::default(),
            heading("Navigation:"),
            Line::from("↑/k - Move up    ↓/j - Move down"),
            Line::from("←/h - Previous tab    →/l - Next tab"),
            Line::from("PageUp/PageDown - Jump 10 items"),
            Line::default(),
            heading("Filter:"),
            Line::from("Ctrl+u - Delete until the start of the input "),
            Line::from("Ctrl+k - Delete until the end of the input "),
            Line::from("Alt+b or Ctrl+← - Go backwards a word "),
            Line::from("Alt+f or Ctrl+→ - Go fowards a word "),
            Line::from("Ctrl+e or End - Go to the end of the input "),
            Line::from("Ctrl+a or Home - Go to the end of input  "),
            Line::from("Ctrl+w or Ctrl+Backspace - Delete a word backwards "),
            Line::default(),
            heading("Service Control:"),
            Line::from("s - Start service    x - Stop service"),
            Line::from("r - Restart service"),
            Line::from("e - Enable service    d - Disable service"),
            Line::from("m - Mask/Unmask service"),
            Line::default(),
            heading("View & Filter list:"),
            Line::from("f - Toggle all/services filter"),
            Line::from("a - Cycle filter (all→active→inactive→failed)"),
            Line::from("u - Refresh service list"),
            Line::default(),
            heading("Information:"),
            Line::from("v - View service logs"),
            Line::from("c - View unit file details"),
            Line::default(),
            heading("Application:"),
            Line::from("Ctrl+z - Suspend"),
            Line::from("Ctrl+c - Quit"),
            Line::default(),
            Line::from(vec![Span::styled(
                "Press ? or any key to close",
                Style::default().fg(Color::Gray),
            )]),
        ];

        let help_block = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan))
                    .padding(Padding::new(1, 1, 0, 0))
                    .title("Help"),
            )
            .alignment(Alignment::Left)
            .wrap(ratatui::widgets::Wrap { trim: true });

        frame.render_widget(
            help_block,
            popup_area.inner(Margin {
                vertical: 0,
                horizontal: 1,
            }),
        );
    }

    fn draw_error_popup(&self, frame: &mut Frame, area: Rect, error_msg: &str) {
        let user_friendly_message = get_user_friendly_error(error_msg);
        let popup_width = std::cmp::min(70, area.width.saturating_sub(4));
        let popup_height = std::cmp::min(10, area.height.saturating_sub(4));
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(
            area.x + popup_x,
            area.y + popup_y,
            popup_width,
            popup_height,
        );

        frame.render_widget(Clear, popup_area);
        let text = vec![
            Line::from(vec![Span::styled(
                "ERROR",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )]),
            Line::default(),
            Line::from(user_friendly_message),
            Line::default(),
            Line::from(vec![Span::styled(
                "Press any key to dismiss",
                Style::default().fg(Color::Gray),
            )]),
        ];
        let error_block = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red))
                    .title("Error"),
            )
            .alignment(Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: true });

        frame.render_widget(error_block, popup_area);
    }

    pub(super) fn draw_details_status(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        terminal.draw(|frame| {
            let area = frame.area();
            let [list_box, help_area_box] =
                Layout::vertical([Constraint::Min(0), Constraint::Max(7)]).areas(area);
            self.details.render(frame, list_box);
            self.draw_shortcuts(frame, help_area_box, &self.details.shortcuts());
            self.draw_overlays(frame, area);
        })?;
        Ok(())
    }

    pub(super) fn draw_log_status(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        terminal.draw(|frame| {
            let area = frame.area();
            let [list_box, help_area_box] =
                Layout::vertical([Constraint::Min(0), Constraint::Max(7)]).areas(area);
            self.service_log.render(frame, list_box);
            self.draw_shortcuts(frame, help_area_box, &self.service_log.shortcuts());
            self.draw_overlays(frame, area);
        })?;
        Ok(())
    }

    pub(super) fn draw_list_status(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        terminal.draw(|frame| {
            let area = frame.area();
            let [filter_box, tabs_box, list_box, help_area_box] = Layout::vertical([
                Constraint::Length(4),
                Constraint::Length(1),
                Constraint::Min(10),
                Constraint::Max(7),
            ])
            .areas(area);
            let filter_state = &self.table_service.get_active_filter_state();
            let system_tab = self.tab_title("System units", 0, filter_state);
            let session_tab = self.tab_title("Session units", 1, filter_state);
            let tabs = Tabs::new(vec![system_tab, session_tab])
                .select(self.selected_tab_index)
                .highlight_style(Style::default().fg(Color::Yellow));

            frame.render_widget(tabs, tabs_box);
            self.draw_shortcuts(frame, help_area_box, &self.table_service.shortcuts());
            self.filter.draw(frame, filter_box);
            self.table_service.render(frame, list_box);
            self.draw_overlays(frame, area);
        })?;
        Ok(())
    }

    fn tab_title<'a>(
        &self,
        title: &'a str,
        tab_index: usize,
        filter_state: &ActiveFilterState,
    ) -> Line<'a> {
        if self.selected_tab_index == tab_index && *filter_state != ActiveFilterState::All {
            Line::from(vec![
                Span::raw(title),
                Span::styled(
                    format!(" (Filter: {})", filter_state.as_str()),
                    Style::default().fg(Color::Gray),
                ),
            ])
        } else {
            Line::from(title)
        }
    }

    fn draw_shortcuts(&self, frame: &mut Frame, help_area: Rect, shortcuts: &[Line<'_>]) {
        let mut help_text = shortcuts.to_owned();
        if !shortcuts.is_empty() {
            help_text.push(Line::default());
            let shortcuts_width = shortcuts
                .par_iter()
                .map(|line| line.spans.iter().map(Span::width).sum())
                .max()
                .unwrap_or(0);
            let shortcuts_width = u16::try_from(shortcuts_width).unwrap_or(u16::MAX);
            if help_area.width > shortcuts_width {
                help_text.push(Line::default());
            }
        }
        help_text.push(Line::from(vec![
            Span::styled(
                "Exit",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Ctrl + c"),
        ]));
        let help_block = Paragraph::new(help_text)
            .block(Block::default().title("Shortcuts").borders(Borders::ALL))
            .wrap(ratatui::widgets::Wrap { trim: true });
        frame.render_widget(help_block, help_area);
    }

    pub(super) fn draw_overlays(&self, frame: &mut Frame, area: Rect) {
        if self.show_help {
            self.draw_help_popup(frame, area);
        }
        if let Some(error_message) = self.error_message.as_deref() {
            self.draw_error_popup(frame, area, error_message);
        }
    }
}
