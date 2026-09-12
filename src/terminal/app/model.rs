use std::collections::HashMap;

use crossterm::event::KeyEvent;

use crate::domain::service::Service;
use crate::infrastructure::systemd_service_adapter::ConnectionType;
use crate::terminal::components::list::ServiceAction;

#[derive(PartialEq)]
pub(super) enum Status {
    List,
    Log,
    Details,
}

#[derive(Debug, PartialEq)]
pub(super) enum AppEffect {
    Suspend,
    EditUnit(String),
    FetchLog {
        request_id: u64,
        service: Service,
    },
    FetchDetails {
        request_id: u64,
        service: Service,
    },
    RunServiceAction {
        request_id: u64,
        connection_request_id: u64,
        service: Service,
        action: ServiceAction,
    },
    RefreshServices {
        request_id: u64,
        filter_all: bool,
        filter_text: String,
        connection_request_id: Option<u64>,
    },
    ChangeConnection(ConnectionRequest),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ConnectionRequest {
    pub(super) id: u64,
    pub(super) target: ConnectionType,
}

pub enum Actions {
    RefreshLog,
    RefreshDetails,
    GoList,
    GoLog,
    GoDetails,
    #[allow(dead_code)]
    UpdateDetails,
    Filter(String),
    UpdateIgnoreListKeys(bool),
    EditCurrentService,
    ServiceAction(ServiceAction),
    ShowHelp,
    UpdateTimestamp(String, Option<u64>),
    LogLoaded(u64, Result<(String, String), String>),
    DetailsLoaded(u64, Service, Result<String, String>),
    ServiceActionFinished(u64, u64, Result<Service, String>),
    ServicesLoaded(u64, Result<Vec<Service>, String>, String, Option<u64>),
    UnitFileStatesLoaded(u64, Option<u64>, Result<HashMap<String, String>, String>),
    ConnectionChanged(u64, ConnectionType, Result<(), String>),
}

pub enum AppEvent {
    Key(KeyEvent),
    Action(Actions),
    Error(String),
}
