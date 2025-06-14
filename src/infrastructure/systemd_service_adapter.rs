use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;
use zbus::Error;
use std::process::Command;
use std::io::{self};
use crate::domain::service::Service;
use crate::domain::service_repository::ServiceRepository;
use crate::domain::service_state::ServiceState;

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

pub enum ConnectionType {
    Session,
    System
}

pub struct SystemdServiceAdapter {
    connection: Connection
}

impl SystemdServiceAdapter {
    pub fn new(connection_type: ConnectionType) -> Result<Self, Error> {
        let connection = 
            match connection_type {
                ConnectionType::Session => Connection::session()?,
                ConnectionType::System => Connection::system()?
            };

        Ok(Self {connection})
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
}

impl ServiceRepository for SystemdServiceAdapter {
    fn change_connection(&mut self, connection_type: ConnectionType) -> Result<(), Error> {
        self.connection = match connection_type {
            ConnectionType::Session => Connection::session()?,
            ConnectionType::System => Connection::system()?
        };
        Ok(())
    }

    fn list_services(&self, filter: bool) -> Result<Vec<Service>, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;

        let units: Vec<SystemdUnit> = proxy.call("ListUnits", &())?;

        let services = units
            .into_iter()
            .filter_map(|unit_tuple| {
                let name = &unit_tuple.0;
                if !filter || name.ends_with(".service") {
                    Some(unit_tuple)
                } else {
                    None
                }
            })
            .map(
                |(
                    name,
                    description,
                    load_state,
                    active_state,
                    sub_state,
                    _followed,
                    _object_path,
                    _job_id,
                    _job_type,
                    _job_object,
                )| {
                    let state: String = proxy
                        .call("GetUnitFileState", &name)
                        .unwrap_or_else(|_| "unknown".into());

                    let service_state = ServiceState::new(load_state, active_state, sub_state, state);

                    Service::new(name, description, service_state)
                },
            )
            .collect::<Vec<_>>();

        Ok(services)
    }

    fn get_service_log(&self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let output = std::process::Command::new("journalctl")
            .arg("-e")
            .arg(format!("--unit={}", name))
            .arg("--no-pager")
            .output()?;

        let log = if output.status.success() {
            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            String::from_utf8_lossy(&output.stderr).to_string()
        };

        Ok(log)
    }
    
    fn systemctl_cat(&self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let output = Command::new("systemctl")
            .arg("cat")
            .arg("--no-pager")
            .arg("--")
            .arg(name)
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
            Err(Box::new(io::Error::other(err_msg)))
        }
    }

    fn start_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let _job: OwnedObjectPath = proxy.call("StartUnit", &(name, "replace"))?;
        Ok(())
    }

    fn stop_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let _job: OwnedObjectPath = proxy.call("StopUnit", &(name, "replace"))?;
        Ok(())
    }

    fn restart_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let _job: OwnedObjectPath = proxy.call("RestartUnit", &(name, "replace"))?;
        Ok(())
    }

    fn enable_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let (_carries_install_info, _changes): (bool, Vec<(String, String, String)>) =
            proxy.call("EnableUnitFiles", &(vec![name], false, true))?;
        Ok(())
    }

    fn disable_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let _changes: Vec<(String, String, String)> =
            proxy.call("DisableUnitFiles", &(vec![name], false))?;
        Ok(())
    }

    fn reload_daemon(&self) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        proxy.call::<&str, (), ()>("Reload", &())?;
        Ok(())
    }
}
