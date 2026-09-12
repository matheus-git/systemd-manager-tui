use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::Error;
use zbus::proxy::MethodFlags;
use std::time::Duration;
use std::time::Instant;
use std::process::Command;
use std::io::{self};
use std::thread;
use crate::domain::service::Service;
use crate::domain::service_repository::ServiceRepository;
use crate::domain::service_state::ServiceState;
use rayon::prelude::*;
use std::collections::HashMap;
use crate::terminal::components::list::LOADING_PLACEHOLDER;

const SLEEP_DURATION: u64 = 100;

#[derive(Clone, Copy, Debug)]
pub struct PollingConfig {
    timeout: Duration,
    interval: Duration,
}

impl PollingConfig {
    pub const fn new(timeout: Duration, interval: Duration) -> Self {
        Self { timeout, interval }
    }
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self::new(Duration::from_secs(30), Duration::from_millis(SLEEP_DURATION))
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

pub enum ConnectionType {
    Session,
    System
}

pub struct SystemdServiceAdapter {
    connection: Connection,
    connection_type: ConnectionType,
    polling: PollingConfig,
}

impl SystemdServiceAdapter {
    pub fn with_polling_config(
        connection_type: ConnectionType,
        polling: PollingConfig,
    ) -> Result<Self, Error> {
        let connection = match connection_type {
            ConnectionType::Session => Connection::session()?,
            ConnectionType::System => Connection::system()?
        };

        Ok(Self {
            connection, 
            connection_type,
            polling,
        })
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
        self.connection_type = connection_type;
        Ok(())
    }

    fn unit_files_state(
        &self,
        services: Vec<Service>
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
                    |(
                        name,
                        description,
                        load_state,
                        active_state,
                        sub_state,
                        .. 
                    )| {
                        let service_state =
                            ServiceState::new(load_state, active_state, sub_state, LOADING_PLACEHOLDER.to_string());

                        Service::new(name, description, service_state)
                    },
                )
                .collect::<Vec<_>>()
        }else {
            units
                .into_par_iter()
                .filter(|(name, ..)| name.ends_with(".service"))
                .map(
                    |(
                        name,
                        description,
                        load_state,
                        active_state,
                        sub_state,
                        .. 
                    )| {
                        let service_state =
                            ServiceState::new(load_state, active_state, sub_state, LOADING_PLACEHOLDER.to_string());

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
                let service_state = ServiceState::new(
                    String::new(),
                    "inactive".to_string(),
                    String::new(),
                    state,
                );
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

        if matches!(self.connection_type, ConnectionType::Session){
            cmd.arg("--user");
        }

        let output = cmd
            .output()?;

        let log = if output.status.success() {
            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            String::from_utf8_lossy(&output.stderr).to_string()
        };

        Ok(log)
    }
    
    fn systemctl_cat(&self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let mut cmd = Command::new("systemctl");

        cmd
            .arg("cat")
            .arg("--no-pager");

        if matches!(self.connection_type, ConnectionType::Session){
            cmd
                .arg("--user");
        }

        let output = cmd
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
            Err(format!("Unit '{name}' not found").into())
        }
    }

    fn start_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let reply: Option<OwnedObjectPath> = proxy.call_with_flags(
            "StartUnit", 
            MethodFlags::AllowInteractiveAuth.into(),  
            &(name, "replace")
        )?;
        reply.ok_or("No reply from StartUnit")?;
        wait_for_stable_service(self.polling, || self.get_unit(name), thread::sleep)
    }

    fn stop_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let reply: Option<OwnedObjectPath> = proxy.call_with_flags(
            "StopUnit", 
            MethodFlags::AllowInteractiveAuth.into(), 
            &(name.to_string(), "replace")
        )?;
        reply.ok_or("No reply from StopUnit")?;
        wait_for_stable_service(self.polling, || self.get_unit(name), thread::sleep)
    }

    fn restart_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let reply: Option<OwnedObjectPath> = proxy.call_with_flags(
            "RestartUnit", 
            MethodFlags::AllowInteractiveAuth.into(), 
            &(name, "replace")
        )?;
        reply.ok_or("No reply from Start")?;
        wait_for_stable_service(self.polling, || self.get_unit(name), thread::sleep)
    }

    fn enable_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        #[allow(clippy::type_complexity)]
        let reply: Option<(bool, Vec<(String, String, String)>)> =
            proxy.call_with_flags(
                "EnableUnitFiles",
                MethodFlags::AllowInteractiveAuth.into(),
                &(vec![name], false, false),
            )?;
        reply.ok_or("No reply from EnableUnitFiles")?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        self.get_unit(name)
    }

    fn disable_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let reply: Option<Vec<(String, String, String)>> =
            proxy.call_with_flags(
                "DisableUnitFiles",
                MethodFlags::AllowInteractiveAuth.into(),
                &(vec![name], false),
            )?;
        reply.ok_or("No reply from DisableUnitFiles")?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        self.get_unit(name)
    }

    fn mask_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let reply: Option<Vec<(String, String, String)>> =
            proxy.call_with_flags(
                "MaskUnitFiles", 
                MethodFlags::AllowInteractiveAuth.into(), 
                &(vec![name], false, true)
            )?;
        reply.ok_or("No reply from MaskUnitFiles")?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        self.get_unit(name)
    }

    fn unmask_service(&self, name: &str) -> Result<Service, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let reply: Option<Vec<(String, String, String)>> =
            proxy.call_with_flags(
                "UnmaskUnitFiles", 
                MethodFlags::AllowInteractiveAuth.into(), 
                &(vec![name], false)
            )?;
        reply.ok_or("No reply from UnmaskUnitFiles")?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        self.get_unit(name)
    }

    fn reload_daemon(&self) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        proxy.call_with_flags::<&str, (), ()>("Reload", MethodFlags::AllowInteractiveAuth.into(), &())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
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

fn wait_for_stable_service<Get, Sleep>(
    polling: PollingConfig,
    mut get_service: Get,
    mut sleep: Sleep,
) -> Result<Service, Box<dyn std::error::Error>>
where
    Get: FnMut() -> Result<Service, Box<dyn std::error::Error>>,
    Sleep: FnMut(Duration),
{
    let started = Instant::now();
    loop {
        let service = get_service()?;
        if !service.state().active().ends_with("ing") {
            return Ok(service);
        }
        if started.elapsed() >= polling.timeout {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("Timed out waiting for '{}' to finish", service.name()),
            )
            .into());
        }
        sleep(polling.interval);
    }
}

#[cfg(test)]
mod polling_tests {
    use super::*;
    use crate::domain::service_state::ServiceState;
    use std::collections::VecDeque;

    fn service(active: &str) -> Service {
        Service::new(
            "demo.service".into(),
            String::new(),
            ServiceState::new("loaded".into(), active.into(), String::new(), "enabled".into()),
        )
    }

    #[test]
    fn returns_when_service_reaches_stable_state() {
        let mut states = VecDeque::from([service("activating"), service("active")]);
        let mut sleeps = 0;

        let result = wait_for_stable_service(
            PollingConfig::new(Duration::from_secs(1), Duration::from_millis(1)),
            || Ok(states.pop_front().unwrap()),
            |_| sleeps += 1,
        )
        .unwrap();

        assert_eq!(result.state().active(), "active");
        assert_eq!(sleeps, 1);
    }

    #[test]
    fn returns_timeout_instead_of_polling_forever() {
        let result = wait_for_stable_service(
            PollingConfig::new(Duration::ZERO, Duration::ZERO),
            || Ok(service("deactivating")),
            |_| panic!("zero timeout must not sleep"),
        );

        let error = result.unwrap_err();
        assert!(error.to_string().contains("Timed out"));
    }

    #[test]
    fn propagates_errors_from_state_lookup() {
        let result = wait_for_stable_service(
            PollingConfig::default(),
            || Err("D-Bus unavailable".into()),
            |_| {},
        );

        assert_eq!(result.unwrap_err().to_string(), "D-Bus unavailable");
    }
}
