use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crate::Config;
use crate::infrastructure::systemd_service_adapter::ConnectionType;
use crate::usecases::services_manager::ServicesManager;

use super::components::details::ServiceDetails;
use super::components::filter::{Filter, InputMode};
use super::components::list::{ServiceAction, TableServices};
use super::components::log::ServiceLog;

mod effects;
mod model;
mod terminal_control;
mod view;
pub use model::{Actions, AppEvent};
use model::{AppEffect, ConnectionRequest, Status};

fn get_user_friendly_error(error: &str) -> &str {
    if error.contains("org.freedesktop.DBus.Error.InteractiveAuthorizationRequired") {
        "You do not have the permission to do that. Try running the program with sudo."
    } else if error.contains("org.freedesktop.DBus.Error.ServiceUnknown") {
        "The requested service is not available or not running."
    } else if error.contains("org.freedesktop.DBus.Error.NoReply") {
        "The service did not respond in time. It might be busy or not functioning properly."
    } else if error.contains("org.freedesktop.DBus.Error.AccessDenied") {
        "Access denied. You don't have sufficient permissions for this operation."
    } else if error.contains("org.freedesktop.systemd1.NoSuchUnit") {
        "The requested service unit doesn't exist."
    } else {
        error
    }
}

pub struct App {
    running: bool,
    status: Status,
    event_listener_enabled: Arc<AtomicBool>,
    table_service: TableServices,
    filter: Filter,
    service_log: ServiceLog,
    details: ServiceDetails,
    usecases: Rc<RefCell<ServicesManager>>,
    event_rx: Receiver<AppEvent>,
    event_tx: Sender<AppEvent>,
    selected_tab_index: usize,
    show_help: bool,
    error_message: Option<String>,
    connection_tx: Sender<ConnectionRequest>,
    latest_connection_request_id: u64,
    active_connection: ConnectionType,
    connection_pending: bool,
    latest_services_request_id: u64,
    latest_log_request_id: u64,
    latest_details_request_id: u64,
    latest_service_action_request_id: u64,
    pending_service_action_request_id: Option<u64>,
}

impl App {
    pub fn new(
        event_tx: Sender<AppEvent>,
        event_rx: Receiver<AppEvent>,
        table_service: TableServices,
        filter: Filter,
        service_log: ServiceLog,
        details: ServiceDetails,
        usecases: Rc<RefCell<ServicesManager>>,
    ) -> Self {
        let (connection_tx, connection_rx) = mpsc::channel::<ConnectionRequest>();
        let connection_manager = usecases.borrow().clone();
        let connection_event_tx = event_tx.clone();
        thread::spawn(move || {
            while let Ok(request) = connection_rx.recv() {
                let mut manager = connection_manager.clone();
                let result = manager
                    .change_repository_connection(request.target)
                    .map_err(|error| error.to_string());
                if connection_event_tx
                    .send(AppEvent::Action(Actions::ConnectionChanged(
                        request.id,
                        request.target,
                        result,
                    )))
                    .is_err()
                {
                    break;
                }
            }
        });

        Self {
            running: true,
            status: Status::List,
            event_listener_enabled: Arc::new(AtomicBool::new(true)),
            table_service,
            filter,
            service_log,
            details,
            usecases,
            event_rx,
            event_tx,
            selected_tab_index: 0,
            show_help: false,
            error_message: None,
            connection_tx,
            latest_connection_request_id: 0,
            active_connection: ConnectionType::System,
            connection_pending: false,
            latest_services_request_id: 0,
            latest_log_request_id: 0,
            latest_details_request_id: 0,
            latest_service_action_request_id: 0,
            pending_service_action_request_id: None,
        }
    }

    pub fn init(&mut self, config: Config) {
        self.table_service.init(&config);
        self.event_tx
            .send(AppEvent::Action(Actions::Filter(config.filter)))
            .unwrap();
        self.spawn_key_event_listener();
    }

    fn spawn_key_event_listener(&self) {
        let event_tx = self.event_tx.clone();
        let event_listener_enabled = self.event_listener_enabled.clone();

        thread::spawn(move || {
            loop {
                if !event_listener_enabled.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(50));
                    continue;
                }

                if event::poll(Duration::from_millis(100)).unwrap_or(false)
                    && let Ok(Event::Key(key_event)) = event::read()
                    && key_event.kind == KeyEventKind::Press
                    && event_tx.send(AppEvent::Key(key_event)).is_err()
                {
                    break;
                }
            }
        });
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;

        while self.running {
            match self.status {
                Status::Log => self.draw_log_status(&mut terminal)?,
                Status::List => self.draw_list_status(&mut terminal)?,
                Status::Details => self.draw_details_status(&mut terminal)?,
            }

            let use_timeout =
                self.status == Status::List && self.table_service.has_active_runtime();

            let event = if use_timeout {
                match self.event_rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(ev) => ev,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            } else {
                self.event_rx.recv()?
            };

            let effects = self.handle_event(event)?;
            self.execute_effects(effects, &mut terminal)?;
        }

        Ok(())
    }

    fn handle_event(&mut self, event: AppEvent) -> Result<Vec<AppEffect>> {
        let mut effects = Vec::new();

        match event {
            AppEvent::Key(key) => {
                if self.error_message.take().is_some() {
                    return Ok(effects);
                }

                if matches!(
                    key,
                    KeyEvent {
                        modifiers: KeyModifiers::CONTROL,
                        code: KeyCode::Char('c' | 'C'),
                        ..
                    }
                ) {
                    self.quit();
                    return Ok(effects);
                }

                if matches!(
                    key,
                    KeyEvent {
                        modifiers: KeyModifiers::CONTROL,
                        code: KeyCode::Char('z'),
                        ..
                    }
                ) {
                    effects.push(AppEffect::Suspend);
                    return Ok(effects);
                }

                match self.status {
                    Status::Log => {
                        if self.show_help {
                            self.show_help = false;
                        } else {
                            self.service_log.on_key_event(key);
                        }
                    }
                    Status::List => {
                        if self.show_help {
                            self.show_help = false;
                            // Ensure the table is active and can receive key events
                            self.table_service.set_ignore_key_events(false);
                            // If no item is selected and list is not empty, select first item
                            if self.table_service.table_state.selected().is_none()
                                && !self.table_service.is_filtered_list_empty()
                            {
                                self.table_service.set_selected_index(0);
                            }
                        } else {
                            let connection_type = (!self.has_pending_service_action())
                                .then(|| {
                                    self.on_key_horizontal_event(
                                        key,
                                        self.filter.input_mode == InputMode::Editing,
                                    )
                                })
                                .flatten();
                            if let Some(connection_type) = connection_type {
                                self.connection_pending = true;
                                self.table_service.begin_connection_change();
                                effects.push(AppEffect::ChangeConnection(
                                    self.next_connection_request(connection_type),
                                ));
                            }
                            self.table_service.on_key_event(key);
                            self.filter.on_key_event(key);
                        }
                    }
                    Status::Details => {
                        if self.show_help {
                            self.show_help = false;
                        } else {
                            self.details.on_key_event(key);
                        }
                    }
                }
            }
            AppEvent::Action(Actions::ServiceAction(action)) => {
                if self.connection_pending {
                    return Ok(effects);
                }
                match action {
                    ServiceAction::ToggleFilter => {
                        self.table_service.toggle_unit_listing();
                        effects.push(self.refresh_services_effect());
                    }
                    ServiceAction::RefreshAll => {
                        effects.push(self.refresh_services_effect());
                    }
                    _ => {
                        if let Some(service) = self.table_service.get_selected_service() {
                            let request_id = self.next_service_action_request_id();
                            self.pending_service_action_request_id = Some(request_id);
                            self.table_service.set_ignore_key_events(true);
                            effects.push(AppEffect::RunServiceAction {
                                request_id,
                                connection_request_id: self.latest_connection_request_id,
                                service,
                                action,
                            });
                        } else {
                            self.table_service.set_ignore_key_events(false);
                        }
                    }
                }
            }
            AppEvent::Action(Actions::UpdateIgnoreListKeys(bool)) => {
                self.table_service.set_ignore_key_events(bool);
            }
            AppEvent::Action(Actions::Filter(input)) => {
                self.table_service.set_selected_index(0);
                self.table_service.refresh(&input);
            }
            AppEvent::Action(Actions::RefreshLog) => {
                if self.status == Status::Log
                    && let Some(service) = self.table_service.get_selected_service()
                {
                    effects.push(self.fetch_log_effect(service));
                }
            }
            AppEvent::Action(Actions::GoLog) => {
                self.status = Status::Log;
                self.event_tx.send(AppEvent::Action(Actions::RefreshLog))?;
            }
            AppEvent::Action(Actions::GoList) => self.status = Status::List,
            AppEvent::Action(Actions::UpdateTimestamp(name, ts)) => {
                self.table_service.update_timestamp(name, ts);
            }
            AppEvent::Action(Actions::UpdateDetails) => {}
            AppEvent::Action(Actions::LogLoaded(request_id, result)) => {
                if request_id == self.latest_log_request_id && self.status == Status::Log {
                    match result {
                        Ok((name, log))
                            if self
                                .table_service
                                .get_selected_service()
                                .is_some_and(|service| service.name() == name) =>
                        {
                            self.service_log.update(name, log);
                        }
                        Ok(_) => {}
                        Err(error) => self.error_message = Some(error),
                    }
                }
            }
            AppEvent::Action(Actions::DetailsLoaded(request_id, service, result)) => {
                let is_current_service = self
                    .table_service
                    .get_selected_service()
                    .is_some_and(|selected| selected.name() == service.name());
                if request_id == self.latest_details_request_id
                    && self.status == Status::Details
                    && is_current_service
                {
                    match result {
                        Ok(unit_file) => self.details.update_unit_file(service, unit_file),
                        Err(error) => self.error_message = Some(error),
                    }
                }
            }
            AppEvent::Action(Actions::ServiceActionFinished(
                request_id,
                connection_request_id,
                result,
            )) => {
                let belongs_to_pending_action =
                    self.pending_service_action_request_id == Some(request_id);
                if belongs_to_pending_action {
                    self.pending_service_action_request_id = None;
                }

                if belongs_to_pending_action
                    && connection_request_id == self.latest_connection_request_id
                    && !self.connection_pending
                {
                    self.table_service.apply_service_result(result);
                    self.table_service.invalidate_timestamp();
                }
            }
            AppEvent::Action(Actions::ServicesLoaded(
                request_id,
                result,
                filter_text,
                connection_request_id,
            )) => {
                let belongs_to_latest_refresh = request_id == self.latest_services_request_id;
                let belongs_to_current_connection =
                    connection_request_id.is_none_or(|id| id == self.latest_connection_request_id);
                let expected_while_pending = !self.connection_pending
                    || connection_request_id == Some(self.latest_connection_request_id);

                if belongs_to_latest_refresh
                    && belongs_to_current_connection
                    && expected_while_pending
                {
                    self.connection_pending = false;
                    match result {
                        Ok(services) => self.table_service.apply_services(services, &filter_text),
                        Err(error) => {
                            self.table_service.set_ignore_key_events(false);
                            self.error_message = Some(error);
                        }
                    }
                }
            }
            AppEvent::Action(Actions::UnitFileStatesLoaded(
                request_id,
                connection_request_id,
                result,
            )) => {
                let belongs_to_latest_refresh = request_id == self.latest_services_request_id;
                let belongs_to_current_connection =
                    connection_request_id.is_none_or(|id| id == self.latest_connection_request_id);
                let expected_while_pending = !self.connection_pending
                    || connection_request_id == Some(self.latest_connection_request_id);

                if belongs_to_latest_refresh
                    && belongs_to_current_connection
                    && expected_while_pending
                {
                    match result {
                        Ok(states) => self.table_service.apply_unit_file_states(states),
                        Err(error) => self.error_message = Some(error),
                    }
                }
            }
            AppEvent::Action(Actions::ConnectionChanged(request_id, target, result)) => {
                match result {
                    Ok(()) => {
                        self.active_connection = target;
                        if request_id == self.latest_connection_request_id {
                            effects.push(self.refresh_services_effect_for_connection(request_id));
                        }
                    }
                    Err(error) if request_id == self.latest_connection_request_id => {
                        self.selected_tab_index = match self.active_connection {
                            ConnectionType::System => 0,
                            ConnectionType::Session => 1,
                        };
                        self.error_message = Some(error);
                        effects.push(self.refresh_services_effect_for_connection(request_id));
                    }
                    Err(_) => {}
                }
            }
            AppEvent::Action(Actions::RefreshDetails) => {
                if self.status == Status::Details
                    && let Some(service) = self.table_service.get_selected_service()
                {
                    effects.push(self.fetch_details_effect(service));
                }
            }
            AppEvent::Action(Actions::GoDetails) => {
                if let Some(service) = self.table_service.get_selected_service() {
                    self.details.update(service.clone());
                    effects.push(self.fetch_details_effect(service));
                }
                self.status = Status::Details;
            }
            AppEvent::Action(Actions::EditCurrentService) => {
                if let Some(service) = &self.table_service.get_selected_service() {
                    effects.push(AppEffect::EditUnit(service.name().to_string()));
                }
            }
            AppEvent::Error(error_msg) => {
                self.error_message = Some(error_msg);
            }
            AppEvent::Action(Actions::ShowHelp) => {
                self.show_help = !self.show_help;
            }
        }

        Ok(effects)
    }

    fn on_key_horizontal_event(
        &mut self,
        key: KeyEvent,
        is_filtering: bool,
    ) -> Option<ConnectionType> {
        let left_keys = [KeyCode::Left, KeyCode::Char('h')];
        let right_keys = [KeyCode::Right, KeyCode::Char('l')];
        match key {
            KeyEvent { code, .. } if left_keys.contains(&code) => {
                if !is_filtering && self.status == Status::List {
                    self.selected_tab_index = if self.selected_tab_index == 0 {
                        1
                    } else {
                        self.selected_tab_index - 1
                    };

                    self.table_service.invalidate_timestamp();
                    return Some(if self.selected_tab_index == 0 {
                        ConnectionType::System
                    } else {
                        ConnectionType::Session
                    });
                }
            }

            KeyEvent { code, .. } if right_keys.contains(&code) => {
                if !is_filtering && self.status == Status::List {
                    self.selected_tab_index = (self.selected_tab_index + 1) % 2;
                    self.table_service.invalidate_timestamp();
                    return Some(if self.selected_tab_index == 0 {
                        ConnectionType::System
                    } else {
                        ConnectionType::Session
                    });
                }
            }

            _ => {}
        }
        None
    }

    fn quit(&mut self) {
        self.running = false;
    }
}

#[cfg(test)]
mod tests;
