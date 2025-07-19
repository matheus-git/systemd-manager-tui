use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;
use zbus::Error;
use std::time::Duration;
use std::process::Command;
use std::io::{self};
use std::thread;
use crate::domain::service::Service;
use crate::domain::service_repository::ServiceRepository;
use crate::domain::service_state::ServiceState;

const SLEEP_DURATION: u64 = 300;

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

    fn list_service_files(&self, filter: bool) -> Result<Vec<Service>, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;

        let units: Vec<(String, String)> = proxy.call("ListUnitFiles", &())?;

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
                    state,
                )| {
                    let service_state = ServiceState::new(String::new(), "inactive".to_string(), String::new(), state );
                    let short_name = name.rsplit('/').next().unwrap_or(&name);
                    Service::new(short_name.to_string(), String::new(), service_state)
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

    fn get_unit(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;

        let units: Vec<SystemdUnit> = proxy.call("ListUnitsByNames", &(vec![name]))?;

        let state: String = proxy
                        .call("GetUnitFileState", &name)
                        .unwrap_or_else(|_| "unknown".into());
        
        if let Some(unit) = units.first() {
            let service_state = ServiceState::new(unit.2.clone(), unit.3.clone(), unit.4.clone(), state );
            let service = Service::new(unit.0.clone(), unit.1.clone(), service_state);
            Ok(service)
        }else {
            Err(format!("Unit '{}' not found", name).into())
        }
    }

    fn start_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let _job: OwnedObjectPath = proxy.call("StartUnit", &(name, "replace"))?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        let mut service = self.get_unit(name)?;
        while service.state().active().ends_with("ing") {
            service = self.get_unit(name)?;
        }
        Ok(service)
    }

    fn stop_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let _job: OwnedObjectPath = proxy.call("StopUnit", &(name.to_string(), "replace"))?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        let mut service = self.get_unit(name)?;
        while service.state().active().ends_with("ing") {
            service = self.get_unit(name)?;
        }
        Ok(service)
    }

    fn restart_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let _job: OwnedObjectPath = proxy.call("RestartUnit", &(name, "replace"))?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        let mut service = self.get_unit(name)?;
        while service.state().active().ends_with("ing") {
            service = self.get_unit(name)?;
        }
        Ok(service)
    }

    fn enable_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let (_carries_install_info, _changes): (bool, Vec<(String, String, String)>) =
        proxy.call("EnableUnitFiles", &(vec![name], false, true))?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        self.get_unit(name)
    }

    fn disable_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let _changes: Vec<(String, String, String)> =
        proxy.call("DisableUnitFiles", &(vec![name], false))?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        self.get_unit(name)
    }

    fn reload_daemon(&self) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        proxy.call::<&str, (), ()>("Reload", &())?;
        Ok(())
    }
}
