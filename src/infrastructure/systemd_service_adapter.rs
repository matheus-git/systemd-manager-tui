use crate::domain::service::Service;
use crate::domain::service_repository::ServiceRepository;
use crate::domain::service_state::ServiceState;
use crate::terminal::components::list::LOADING_PLACEHOLDER;
use async_io::Timer;
use futures_lite::{StreamExt, future};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fmt;
use std::io::{self};
use std::process::Command;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};
use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::message::Type;
use zbus::proxy::MethodFlags;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::{Error, MatchRule, MessageStream};

const LATE_COMPLETION_WATCH_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
}

impl fmt::Display for ServiceAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let action = match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
        };
        formatter.write_str(action)
    }
}

#[derive(Debug)]
pub struct OperationTimeout {
    pub service: String,
    pub action: ServiceAction,
    pub timeout: Duration,
}

impl fmt::Display for OperationTimeout {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Timed out after {:?} waiting to {} '{}'; the operation may still be in progress in systemd",
            self.timeout, self.action, self.service
        )
    }
}

impl std::error::Error for OperationTimeout {}

#[derive(Debug)]
pub struct SystemdJobFailed {
    pub service: String,
    pub action: ServiceAction,
    pub result: String,
}

impl fmt::Display for SystemdJobFailed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "systemd could not {} '{}': job finished with result '{}'",
            self.action, self.service, self.result
        )
    }
}

impl std::error::Error for SystemdJobFailed {}

#[derive(Debug)]
pub struct CompletedOperation {
    pub connection: ConnectionType,
    pub service: String,
    pub action: ServiceAction,
    pub result: String,
}

enum JobWaitOutcome {
    Finished(String),
    TimedOut(Box<MessageStream>),
}

fn wait_for_job_result(
    timeout: Duration,
    job_path: &OwnedObjectPath,
    mut events: MessageStream,
) -> Result<JobWaitOutcome, Box<dyn std::error::Error>> {
    let started_at = Instant::now();
    loop {
        let remaining = timeout.saturating_sub(started_at.elapsed());
        let message = future::block_on(future::race(async { events.next().await }, async {
            Timer::after(remaining).await;
            None
        }));
        let Some(message) = message else {
            return Ok(JobWaitOutcome::TimedOut(Box::new(events)));
        };
        let message = message?;
        let (_id, removed_path, _unit, result): (u32, OwnedObjectPath, String, String) =
            message.body().deserialize()?;
        if removed_path != *job_path {
            continue;
        }
        return Ok(JobWaitOutcome::Finished(result));
    }
}

type SystemdUnit = (
    String,
    String,
    String,
    String,
    String,
    String,
    OwnedObjectPath,
    u32,
    String,
    OwnedObjectPath,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionType {
    Session,
    System,
}

impl fmt::Display for ConnectionType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Session => "user",
            Self::System => "system",
        })
    }
}

pub struct SystemdServiceAdapter {
    connection: Connection,
    active_connection: ConnectionType,
    operation_timeout: Duration,
    completion_tx: Option<Sender<CompletedOperation>>,
}

impl SystemdServiceAdapter {
    pub fn new(
        connection_type: ConnectionType,
        operation_timeout: Duration,
    ) -> Result<Self, Error> {
        Self::new_with_completion_sender(connection_type, operation_timeout, None)
    }

    pub fn new_with_completion_sender(
        connection_type: ConnectionType,
        operation_timeout: Duration,
        completion_tx: Option<Sender<CompletedOperation>>,
    ) -> Result<Self, Error> {
        let connection = match connection_type {
            ConnectionType::Session => Connection::session()?,
            ConnectionType::System => Connection::system()?,
        };
        Self::subscribe_to_jobs(&connection)?;

        Ok(Self {
            connection,
            active_connection: connection_type,
            operation_timeout,
            completion_tx,
        })
    }

    pub fn set_completion_sender(&mut self, completion_tx: Sender<CompletedOperation>) {
        self.completion_tx = Some(completion_tx);
    }

    fn manager_proxy(&self) -> Result<Proxy<'static>, Box<dyn std::error::Error>> {
        let proxy = Proxy::new(
            &self.connection,
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
        )?;
        Ok(proxy)
    }

    fn subscribe_to_jobs(connection: &Connection) -> Result<(), Error> {
        let proxy = Proxy::new(
            connection,
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
        )?;
        proxy.call::<_, _, ()>("Subscribe", &())
    }

    fn run_unit_job(
        &self,
        name: &str,
        action: ServiceAction,
        method: &str,
    ) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender("org.freedesktop.systemd1")?
            .interface("org.freedesktop.systemd1.Manager")?
            .member("JobRemoved")?
            .arg(2, name)?
            .build();
        let events = MessageIterator::for_match_rule(rule, &self.connection, Some(8))?;
        let reply: Option<OwnedObjectPath> = proxy.call_with_flags(
            method,
            MethodFlags::AllowInteractiveAuth.into(),
            &(name, "replace"),
        )?;
        let job_path = reply.ok_or_else(|| format!("No job returned from {method}"))?;

        match wait_for_job_result(self.operation_timeout, &job_path, events.into_inner())? {
            JobWaitOutcome::Finished(result) if result == "done" => {}
            JobWaitOutcome::Finished(result) => {
                return Err(Box::new(SystemdJobFailed {
                    service: name.to_string(),
                    action,
                    result,
                }));
            }
            JobWaitOutcome::TimedOut(mut events) => {
                if let Some(sender) = self.completion_tx.clone() {
                    let connection = self.active_connection;
                    let service = name.to_string();
                    let job_path = job_path.clone();
                    thread::spawn(move || {
                        let started_at = Instant::now();
                        loop {
                            let remaining =
                                LATE_COMPLETION_WATCH_TIMEOUT.saturating_sub(started_at.elapsed());
                            let message = future::block_on(future::race(
                                async { events.next().await },
                                async {
                                    Timer::after(remaining).await;
                                    None
                                },
                            ));
                            let Some(Ok(message)) = message else {
                                break;
                            };
                            let Ok((_id, removed_path, _unit, result)) = message
                                .body()
                                .deserialize::<(u32, OwnedObjectPath, String, String)>()
                            else {
                                continue;
                            };
                            if removed_path == job_path {
                                let _ = sender.send(CompletedOperation {
                                    connection,
                                    service,
                                    action,
                                    result,
                                });
                                break;
                            }
                        }
                    });
                }
                return Err(Box::new(OperationTimeout {
                    service: name.to_string(),
                    action,
                    timeout: self.operation_timeout,
                }));
            }
        }

        match self.get_unit(name) {
            Ok(service) => Ok(service),
            Err(error) if action == ServiceAction::Stop && is_no_such_unit(error.as_ref()) => {
                Ok(Service::new(
                    name.to_string(),
                    String::new(),
                    ServiceState::new(
                        "not-found".to_string(),
                        "inactive".to_string(),
                        "dead".to_string(),
                        "unknown".to_string(),
                    ),
                ))
            }
            Err(error) => Err(error),
        }
    }
}

fn is_no_such_unit(error: &(dyn std::error::Error + 'static)) -> bool {
    matches!(
        error.downcast_ref::<Error>(),
        Some(Error::MethodError(name, _, _))
            if name.as_str() == "org.freedesktop.systemd1.NoSuchUnit"
    )
}

impl ServiceRepository for SystemdServiceAdapter {
    fn change_connection(
        &mut self,
        connection_type: ConnectionType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let connection = match connection_type {
            ConnectionType::Session => Connection::session()?,
            ConnectionType::System => Connection::system()?,
        };
        Self::subscribe_to_jobs(&connection)?;
        self.connection = connection;
        self.active_connection = connection_type;
        Ok(())
    }

    fn unit_files_state(
        &self,
        services: Vec<Service>,
    ) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;

        let states_vec: Vec<(String, String)> = services
            .par_iter()
            .map(|service| {
                let name = service.name().to_string();
                let state = proxy
                    .call("GetUnitFileState", &name)
                    .unwrap_or_else(|_| "unknown".to_string());
                (name, state)
            })
            .collect();

        let states: HashMap<String, String> = states_vec.into_iter().collect();

        Ok(states)
    }

    fn list_services(&self, filter: bool) -> Result<Vec<Service>, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;

        let units: Vec<SystemdUnit> = proxy.call("ListUnits", &())?;

        let services: Vec<Service> = if filter {
            units
                .into_par_iter()
                .map(
                    |(name, description, load_state, active_state, sub_state, ..)| {
                        let service_state = ServiceState::new(
                            load_state,
                            active_state,
                            sub_state,
                            LOADING_PLACEHOLDER.to_string(),
                        );

                        Service::new(name, description, service_state)
                    },
                )
                .collect::<Vec<_>>()
        } else {
            units
                .into_par_iter()
                .filter(|(name, ..)| name.ends_with(".service"))
                .map(
                    |(name, description, load_state, active_state, sub_state, ..)| {
                        let service_state = ServiceState::new(
                            load_state,
                            active_state,
                            sub_state,
                            LOADING_PLACEHOLDER.to_string(),
                        );

                        Service::new(name, description, service_state)
                    },
                )
                .collect::<Vec<_>>()
        };

        Ok(services)
    }

    fn list_service_files(&self) -> Result<Vec<Service>, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;

        let units: Vec<(String, String)> = proxy.call("ListUnitFiles", &())?;

        let services = units
            .into_par_iter()
            .map(|(name, state)| {
                let service_state =
                    ServiceState::new(String::new(), "inactive".to_string(), String::new(), state);
                let short_name = name.rsplit('/').next().unwrap_or(&name);
                Service::new(short_name.to_string(), String::new(), service_state)
            })
            .collect::<Vec<_>>();

        Ok(services)
    }

    fn get_service_log(&self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let mut cmd = std::process::Command::new("journalctl");

        cmd.arg("-e")
            .arg(format!("--unit={name}"))
            .arg("--no-pager");

        if matches!(self.active_connection, ConnectionType::Session) {
            cmd.arg("--user");
        }

        let output = cmd.output()?;

        let log = if output.status.success() {
            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            String::from_utf8_lossy(&output.stderr).to_string()
        };

        Ok(log)
    }

    fn systemctl_cat(&self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let mut cmd = Command::new("systemctl");

        cmd.arg("cat").arg("--no-pager");

        if matches!(self.active_connection, ConnectionType::Session) {
            cmd.arg("--user");
        }

        let output = cmd.arg("--").arg(name).output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
            Err(Box::new(io::Error::other(err_msg)))
        }
    }

    fn get_unit(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;

        let units: Vec<SystemdUnit> = proxy.call("ListUnitsByNames", &(vec![name]))?;

        let state: String = proxy
            .call("GetUnitFileState", &name)
            .unwrap_or_else(|_| "unknown".into());

        if let Some(unit) = units.first() {
            let service_state =
                ServiceState::new(unit.2.clone(), unit.3.clone(), unit.4.clone(), state);
            let service = Service::new(unit.0.clone(), unit.1.clone(), service_state);
            Ok(service)
        } else {
            Err(format!("Unit '{name}' not found").into())
        }
    }

    fn start_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        self.run_unit_job(name, ServiceAction::Start, "StartUnit")
    }

    fn stop_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        self.run_unit_job(name, ServiceAction::Stop, "StopUnit")
    }

    fn restart_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        self.run_unit_job(name, ServiceAction::Restart, "RestartUnit")
    }

    fn enable_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        #[allow(clippy::type_complexity)]
        let reply: Option<(bool, Vec<(String, String, String)>)> = proxy.call_with_flags(
            "EnableUnitFiles",
            MethodFlags::AllowInteractiveAuth.into(),
            &(vec![name], false, false),
        )?;
        reply.ok_or("No reply from EnableUnitFiles")?;
        self.get_unit(name)
    }

    fn disable_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let reply: Option<Vec<(String, String, String)>> = proxy.call_with_flags(
            "DisableUnitFiles",
            MethodFlags::AllowInteractiveAuth.into(),
            &(vec![name], false),
        )?;
        reply.ok_or("No reply from DisableUnitFiles")?;
        self.get_unit(name)
    }

    fn mask_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let reply: Option<Vec<(String, String, String)>> = proxy.call_with_flags(
            "MaskUnitFiles",
            MethodFlags::AllowInteractiveAuth.into(),
            &(vec![name], false, true),
        )?;
        reply.ok_or("No reply from MaskUnitFiles")?;
        self.get_unit(name)
    }

    fn unmask_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let reply: Option<Vec<(String, String, String)>> = proxy.call_with_flags(
            "UnmaskUnitFiles",
            MethodFlags::AllowInteractiveAuth.into(),
            &(vec![name], false),
        )?;
        reply.ok_or("No reply from UnmaskUnitFiles")?;
        self.get_unit(name)
    }

    fn reload_daemon(&self) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        proxy.call_with_flags::<&str, (), ()>(
            "Reload",
            MethodFlags::AllowInteractiveAuth.into(),
            &(),
        )?;
        Ok(())
    }

    fn get_active_enter_timestamp(&self, name: &str) -> Result<u64, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let unit_path: OwnedObjectPath = proxy.call("LoadUnit", &name)?;
        let unit_proxy = Proxy::new(
            &self.connection,
            "org.freedesktop.systemd1",
            unit_path.as_ref(),
            "org.freedesktop.DBus.Properties",
        )?;
        let variant: OwnedValue = unit_proxy.call(
            "Get",
            &("org.freedesktop.systemd1.Unit", "ActiveEnterTimestamp"),
        )?;
        let timestamp: u64 = variant.try_into()?;
        Ok(timestamp)
    }
}

#[cfg(test)]
mod tests;
