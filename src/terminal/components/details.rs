use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::rc::Rc;

use crossterm::event::{KeyCode, KeyEvent};

use crate::domain::service::Service;
use crate::terminal::app::{Actions, AppEvent};
use crate::usecases::services_manager::ServicesManager;

pub struct ServiceDetails {
    service: Option<Arc<Mutex<Service>>>,
    sender: Sender<AppEvent>,
    scroll: u16,
    auto_refresh: Arc<Mutex<bool>>,
    usecase: Rc<ServicesManager>,
}

impl ServiceDetails {
    pub fn new(sender: Sender<AppEvent>,  usecase: Rc<ServicesManager>) -> Self {
        Self {
            service: None,
            sender,
            scroll: 0,
            auto_refresh: Arc::new(Mutex::new(false)),
            usecase
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if let Some(service_arc) = &self.service {
            if let Ok(service) = service_arc.lock() {
                if let Some(properties) = service.properties() {
                    let mut lines: Vec<Line> = vec![];

                    let exec_start = properties.formatted_exec_start();
                    lines.push(self.generate_line("ExecStart", &exec_start));

                    let exec_start_pre = properties.formatted_exec_start_pre();
                    lines.push(self.generate_line("ExecStartPre", &exec_start_pre));

                    let exec_start_post = properties.formatted_exec_start_post();
                    lines.push(self.generate_line("ExecStartPost", &exec_start_post));

                    let exec_stop = properties.formatted_exec_stop();
                    lines.push(self.generate_line("ExecStop", &exec_stop));

                    let exec_stop_post = properties.formatted_exec_stop_post();
                    lines.push(self.generate_line("ExecStopPost", &exec_stop_post));

                    lines.push(Line::from(""));

                    let exec_main_pid = properties.exec_main_pid().to_string();
                    lines.push(self.generate_line("ExecMainPID", &exec_main_pid));

                    let exec_main_start_timestamp =
                        properties.format_timestamp(properties.exec_main_start_timestamp());
                    lines.push(
                        self.generate_line("ExecMainStartTimestamp", &exec_main_start_timestamp),
                    );

                    let exec_main_exit_timestamp =
                        properties.format_timestamp(properties.exec_main_exit_timestamp());
                    lines.push(
                        self.generate_line("ExecMainExitTimestamp", &exec_main_exit_timestamp),
                    );

                    let exec_main_code = properties.exec_main_code().to_string();
                    lines.push(self.generate_line("ExecMainCode", &exec_main_code));

                    let exec_main_status = properties.exec_main_status().to_string();
                    lines.push(self.generate_line("ExecMainStatus", &exec_main_status));

                    lines.push(Line::from(""));

                    let main_pid = properties.main_pid().to_string();
                    lines.push(self.generate_line("MainPID", &main_pid));

                    let control_pid = properties.control_pid().to_string();
                    lines.push(self.generate_line("ControlPID", &control_pid));

                    lines.push(Line::from(""));

                    lines.push(self.generate_line("Restart", properties.restart()));

                    let restart_usec = format!("{}s", (properties.restart_usec() as f64 / 1000.0));
                    lines.push(self.generate_line("RestartUSec", &restart_usec));

                    lines.push(Line::from(""));
                    let status_text = properties.status_text().to_string();
                    lines.push(self.generate_line("StatusText", &status_text));

                    let result = properties.result().to_string();
                    lines.push(self.generate_line("Result", &result));

                    lines.push(Line::from(""));

                    let user = properties.user().to_string();
                    lines.push(self.generate_line("User", &user));

                    let group = properties.group().to_string();
                    lines.push(self.generate_line("Group", &group));

                    lines.push(Line::from(""));

                    fn format_bytes(bytes: u64) -> String {
                        if bytes >= 1_000_000_000_000 {
                            format!("{:.2} TB", bytes as f64 / 1_000_000_000_000.0)
                        } else if bytes >= 1_000_000_000 {
                            format!("{:.2} GB", bytes as f64 / 1_000_000_000.0)
                        } else if bytes >= 1_000_000 {
                            format!("{:.2} MB", bytes as f64 / 1_000_000.0)
                        } else if bytes >= 1_000 {
                            format!("{:.2} KB", bytes as f64 / 1_000.0)
                        } else {
                            format!("{} bytes", bytes)
                        }
                    }

                    fn format_units(value: u64) -> String {
                        if value >= 1_000_000_000_000 {
                            format!("{:.2} TB", value as f64 / 1_000_000_000_000.0)
                        } else if value >= 1_000_000_000 {
                            format!("{:.2} GB", value as f64 / 1_000_000_000.0)
                        } else if value >= 1_000_000 {
                            format!("{:.2} MB", value as f64 / 1_000_000.0)
                        } else if value >= 1_000 {
                            format!("{:.2} KB", value as f64 / 1_000.0)
                        } else {
                            value.to_string()
                        }
                    }

                    let limit_cpu = format_units(properties.limit_cpu());
                    lines.push(self.generate_line("CPU Limit", &limit_cpu));

                    let limit_nofile = format_units(properties.limit_nofile());
                    lines.push(self.generate_line("Open Files Limit", &limit_nofile));

                    let limit_nproc = properties.limit_nproc().to_string();
                    lines.push(self.generate_line("Process Limit", &limit_nproc));

                    let limit_memlock = format_bytes(properties.limit_memlock());
                    lines.push(self.generate_line("Memory Lock Limit", &limit_memlock));

                    let memory_limit = format_bytes(properties.memory_limit());
                    lines.push(self.generate_line("Memory Limit", &memory_limit));

                    let cpu_shares = format_units(properties.cpu_shares());
                    lines.push(self.generate_line("CPU Shares", &cpu_shares));

                    let mut scroll_state =
                        ScrollbarState::new(lines.len()).position(self.scroll as usize);
                    let paragraph = Paragraph::new(Text::from(lines))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(format!(" {} properties ", service.name()))
                                .title_alignment(Alignment::Center),
                        )
                        .scroll((self.scroll, 0));

                    frame.render_widget(paragraph, area);
                    frame.render_stateful_widget(
                        Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
                        area,
                        &mut scroll_state,
                    );
                }
            }
        }
    }

    fn generate_line<'a>(&self, key: &'a str, value: &'a str) -> Line<'a> {
        Line::from(vec![
            Span::styled(key, Style::new().bold()),
            Span::raw("="),
            Span::styled(value, Style::new()),
        ])
    }

    fn set_auto_refresh(&mut self, value: bool) {
        if let Ok(mut auto) = self.auto_refresh.lock() {
            *auto = value;
        }
    }

    pub fn on_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Right => {
                self.reset();
                self.sender.send(AppEvent::Action(Actions::GoLog)).unwrap();
            }
            KeyCode::Left => {
                self.reset();
                self.sender.send(AppEvent::Action(Actions::GoLog)).unwrap();
            }
            KeyCode::Up => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                self.scroll += 1;
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.scroll += 10;
            }

            KeyCode::Char('q') => {
                self.reset();
                self.exit();
            }
            _ => {}
        }
    }

    pub fn shortcuts(&mut self) -> Vec<Line<'_>> {
        let help_text = vec![
            Line::from(vec![Span::styled(
                "Actions",
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("Switch tabs: ←/→ | Go back: q"),
        ];

        help_text
    }

    pub fn start_auto_refresh(&mut self) {
        self.set_auto_refresh(true);
        self.auto_refresh_thread();
    }

    pub fn reset(&mut self) {
        self.set_auto_refresh(false);
        self.service = None;
        self.scroll = 0;
    }

    fn exit(&self) {
        self.sender.send(AppEvent::Action(Actions::GoList)).unwrap();
    }

    pub fn auto_refresh_thread(&mut self) {
        let auto_refresh = Arc::clone(&self.auto_refresh);
        let sender = self.sender.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(1000));
                if let Ok(is_active) = auto_refresh.lock() {
                    if *is_active {
                        sender
                            .send(AppEvent::Action(Actions::RefreshDetails))
                            .unwrap();
                    } else {
                        break;
                    }
                }
            }
        });
    }

    pub fn fetch_log_and_dispatch(&self) {
        if let Some(service_arc) = &self.service {
            let event_tx = self.sender.clone();
            let mut service = service_arc.lock().unwrap();
            if self.usecase.update_properties(&mut service).is_ok() {
                event_tx
                    .send(AppEvent::Action(Actions::UpdateDetails))
                    .expect("Failed to send UpdateDetails event");
            }
        }
    }
    pub fn update(&mut self, service: Service) {
        self.service = Some(Arc::new(Mutex::new(service)));
    }
}
