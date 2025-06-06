use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;
use zbus::Error;
use std::process::Command;
use std::io::{self};
use crate::domain::service::Service;
use crate::domain::service_property::{ServiceProperty, SASBTTUII};
use crate::domain::service_repository::ServiceRepository;
use crate::domain::service_state::ServiceState;

/// Represents a systemd unit as returned by the D-Bus ListUnits method.
/// Each tuple element corresponds to a specific property of the unit:
///
/// 1. name - The unit name (e.g., "nginx.service")
/// 2. description - Human-readable description of the service
/// 3. load_state - Service load state (e.g., "loaded", "not-found")
/// 4. active_state - Active state (e.g., "active", "inactive", "failed")
/// 5. sub_state - Sub-state (e.g., "running", "dead", "exited")
/// 6. followed - Followed unit name
/// 7. object_path - D-Bus object path to the unit
/// 8. job_id - Job ID if there's a job queued for this unit
/// 9. job_type - Type of pending job if any
/// 10. job_object - D-Bus object path to the job object
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
                    // Chamada D-Bus para pegar estado do arquivo da unidade
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

    fn get_service_property(
        &self,
        name: &str,
    ) -> Result<ServiceProperty, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;

        let unit_path: OwnedObjectPath = proxy.call("GetUnit", &(name))?;

        let service_proxy = Proxy::new(
            &self.connection,
            "org.freedesktop.systemd1",
            unit_path.as_str(),
            "org.freedesktop.systemd1.Service",
        )?;

        let exec_start: Vec<SASBTTUII> = service_proxy.get_property("ExecStart")?;
        let exec_start_pre: Vec<SASBTTUII> = service_proxy.get_property("ExecStartPre")?;
        let exec_start_post: Vec<SASBTTUII> = service_proxy.get_property("ExecStartPost")?;
        let exec_stop: Vec<SASBTTUII> = service_proxy.get_property("ExecStop")?;
        let exec_stop_post: Vec<SASBTTUII> = service_proxy.get_property("ExecStopPost")?;

        let exec_main_pid: u32 = service_proxy.get_property("ExecMainPID")?;
        let exec_main_start_timestamp: u64 =
        service_proxy.get_property("ExecMainStartTimestamp")?;
        let exec_main_exit_timestamp: u64 = service_proxy.get_property("ExecMainExitTimestamp")?;
        let exec_main_code: i32 = service_proxy.get_property("ExecMainCode")?;
        let exec_main_status: i32 = service_proxy.get_property("ExecMainStatus")?;

        let main_pid: u32 = service_proxy.get_property("MainPID")?;
        let control_pid: u32 = service_proxy.get_property("ControlPID")?;

        let restart: String = service_proxy.get_property("Restart")?;
        let restart_usec: u64 = service_proxy.get_property("RestartUSec")?;

        let status_text: String = service_proxy.get_property("StatusText")?;
        let result: String = service_proxy.get_property("Result")?;

        let user: String = service_proxy.get_property("User")?;
        let group: String = service_proxy.get_property("Group")?;

        let limit_cpu: u64 = service_proxy.get_property("LimitCPU")?;
        let limit_nofile: u64 = service_proxy.get_property("LimitNOFILE")?;
        let limit_nproc: u64 = service_proxy.get_property("LimitNPROC")?;
        let limit_memlock: u64 = service_proxy.get_property("LimitMEMLOCK")?;
        let memory_limit: u64 = service_proxy.get_property("MemoryLimit")?;
        let cpu_shares: u64 = service_proxy.get_property("CPUShares")?;

        Ok(ServiceProperty::new(
            exec_start,
            exec_start_pre,
            exec_start_post,
            exec_stop,
            exec_stop_post,
            exec_main_pid,
            exec_main_start_timestamp,
            exec_main_exit_timestamp,
            exec_main_code,
            exec_main_status,
            main_pid,
            control_pid,
            restart,
            restart_usec,
            status_text,
            result,
            user,
            group,
            limit_cpu,
            limit_nofile,
            limit_nproc,
            limit_memlock,
            memory_limit,
            cpu_shares,
        ))
    }
}
