use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread;

use color_eyre::Result;
use ratatui::DefaultTerminal;

use super::{Actions, App, AppEffect, AppEvent, ConnectionRequest};
use crate::domain::service::Service;
use crate::infrastructure::systemd_service_adapter::ConnectionType;
use crate::terminal::components::list::{QueryUnitFile, ServiceAction};

impl App {
    pub(super) fn execute_effects(
        &mut self,
        effects: Vec<AppEffect>,
        terminal: &mut DefaultTerminal,
    ) -> Result<()> {
        for effect in effects {
            match effect {
                AppEffect::Suspend => self.suspend_tui(terminal)?,
                AppEffect::EditUnit(unit_name) => {
                    self.edit_unit(terminal, &unit_name)?;
                    self.event_tx
                        .send(AppEvent::Action(Actions::RefreshDetails))?;
                }
                AppEffect::FetchLog {
                    request_id,
                    service,
                } => self.spawn_log_worker(request_id, service),
                AppEffect::FetchDetails {
                    request_id,
                    service,
                } => self.spawn_details_worker(request_id, service),
                AppEffect::RunServiceAction {
                    request_id,
                    connection_request_id,
                    service,
                    action,
                } => self.spawn_service_action_worker(
                    request_id,
                    connection_request_id,
                    service,
                    action,
                ),
                AppEffect::RefreshServices {
                    request_id,
                    filter_all,
                    filter_text,
                    connection_request_id,
                } => self.spawn_services_worker(
                    filter_all,
                    filter_text,
                    self.table_service.query_sender(),
                    request_id,
                    connection_request_id,
                ),
                AppEffect::ChangeConnection(request) => self.connection_tx.send(request)?,
            }
        }
        Ok(())
    }

    pub(super) fn spawn_log_worker(&self, request_id: u64, service: Service) {
        let manager = self.usecases.borrow().clone();
        let sender = self.event_tx.clone();
        thread::spawn(move || {
            let name = service.name().to_string();
            let result = manager
                .get_log(&service)
                .map(|log| (name, log))
                .map_err(|error| error.to_string());
            let _ = sender.send(AppEvent::Action(Actions::LogLoaded(request_id, result)));
        });
    }

    pub(super) fn spawn_details_worker(&self, request_id: u64, service: Service) {
        let manager = self.usecases.borrow().clone();
        let sender = self.event_tx.clone();
        thread::spawn(move || {
            let result = manager
                .systemctl_cat(&service)
                .map_err(|error| error.to_string());
            let _ = sender.send(AppEvent::Action(Actions::DetailsLoaded(
                request_id, service, result,
            )));
        });
    }

    pub(super) fn spawn_service_action_worker(
        &self,
        request_id: u64,
        connection_request_id: u64,
        service: Service,
        action: ServiceAction,
    ) {
        let manager = self.usecases.borrow().clone();
        let sender = self.event_tx.clone();
        let file_state = self.table_service.file_state_for(&service);
        thread::spawn(move || {
            let result = match action {
                ServiceAction::Start => manager.start_service(&service),
                ServiceAction::Stop => manager.stop_service(&service),
                ServiceAction::Restart => manager.restart_service(&service),
                ServiceAction::Enable => manager.enable_service(&service),
                ServiceAction::Disable => manager.disable_service(&service),
                ServiceAction::ToggleMask
                    if matches!(file_state.as_str(), "masked" | "masked-runtime") =>
                {
                    manager.unmask_service(&service)
                }
                ServiceAction::ToggleMask => manager.mask_service(&service),
                ServiceAction::ToggleFilter | ServiceAction::RefreshAll => return,
            }
            .map_err(|error| error.to_string());

            let _ = sender.send(AppEvent::Action(Actions::ServiceActionFinished(
                request_id,
                connection_request_id,
                result,
            )));
        });
    }

    pub(super) fn refresh_services_effect(&mut self) -> AppEffect {
        let request_id = self.next_services_request_id();
        let (filter_all, filter_text) = self.table_service.refresh_parameters();
        AppEffect::RefreshServices {
            request_id,
            filter_all,
            filter_text,
            connection_request_id: None,
        }
    }

    pub(super) fn refresh_services_effect_for_connection(
        &mut self,
        connection_request_id: u64,
    ) -> AppEffect {
        let request_id = self.next_services_request_id();
        let (filter_all, filter_text) = self.table_service.refresh_parameters();
        AppEffect::RefreshServices {
            request_id,
            filter_all,
            filter_text,
            connection_request_id: Some(connection_request_id),
        }
    }

    fn spawn_services_worker(
        &self,
        filter_all: bool,
        filter_text: String,
        query_sender: Arc<Sender<QueryUnitFile>>,
        request_id: u64,
        connection_request_id: Option<u64>,
    ) {
        let manager = self.usecases.borrow().clone();
        let sender = self.event_tx.clone();
        thread::spawn(move || {
            let result = manager
                .list_services(filter_all, query_sender, request_id, connection_request_id)
                .map_err(|error| error.to_string());
            let _ = sender.send(AppEvent::Action(Actions::ServicesLoaded(
                request_id,
                result,
                filter_text,
                connection_request_id,
            )));
        });
    }

    pub(super) fn next_connection_request(&mut self, target: ConnectionType) -> ConnectionRequest {
        self.latest_connection_request_id = self.latest_connection_request_id.wrapping_add(1);
        ConnectionRequest {
            id: self.latest_connection_request_id,
            target,
        }
    }

    fn next_services_request_id(&mut self) -> u64 {
        self.latest_services_request_id = self.latest_services_request_id.wrapping_add(1);
        self.latest_services_request_id
    }

    pub(super) fn next_service_action_request_id(&mut self) -> u64 {
        self.latest_service_action_request_id =
            self.latest_service_action_request_id.wrapping_add(1);
        self.latest_service_action_request_id
    }

    pub(super) fn has_pending_service_action(&self) -> bool {
        self.pending_service_action_request_id.is_some()
    }

    pub(super) fn fetch_log_effect(&mut self, service: Service) -> AppEffect {
        self.latest_log_request_id = self.latest_log_request_id.wrapping_add(1);
        AppEffect::FetchLog {
            request_id: self.latest_log_request_id,
            service,
        }
    }

    pub(super) fn fetch_details_effect(&mut self, service: Service) -> AppEffect {
        self.latest_details_request_id = self.latest_details_request_id.wrapping_add(1);
        AppEffect::FetchDetails {
            request_id: self.latest_details_request_id,
            service,
        }
    }
}
