use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;

use crate::domain::service::Service;
use crate::domain::service_state::ServiceState;
use crate::domain::service_repository::ServiceRepository;

type SystemdUnit = (String, String, String, String, String, String, OwnedObjectPath, u32, String, OwnedObjectPath);

pub struct SystemdServiceAdapter;

impl SystemdServiceAdapter {
    fn manager_proxy(&self) -> Result<Proxy<'_>, Box<dyn std::error::Error>> {
        let connection = Connection::system()?;
        let proxy = Proxy::new(
            &connection,
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
        )?;
        Ok(proxy)
    }

    pub fn reload_daemon(&self) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        proxy.call::<&str, (), ()>("Reload", &())?; 
        Ok(())
    }
}

impl ServiceRepository for SystemdServiceAdapter {
    fn start_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let _job: OwnedObjectPath = proxy.call::<&str, (&str, &str), OwnedObjectPath>("StartUnit", &(name, "replace"))?;
        Ok(())
    }

    fn stop_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let _job: OwnedObjectPath = proxy.call::<&str, (&str, &str), OwnedObjectPath>("StopUnit", &(name, "replace"))?;
        Ok(())
    }

    fn restart_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let _job: OwnedObjectPath = proxy.call::<&str, (&str, &str), OwnedObjectPath>("RestartUnit", &(name, "replace"))?;
        Ok(())
    }

    fn enable_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let (_carries_install_info, _changes): (bool, Vec<(String, String, String)>) = proxy
            .call::<&str, (Vec<&str>, bool, bool), (bool, Vec<(String, String, String)>)>(
                "EnableUnitFiles",
                &(vec![name], false, true),
            )?;

        Ok(())
    }

    fn disable_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let _changes: Vec<(String, String, String)> = proxy
            .call::<&str, (Vec<&str>, bool), Vec<(String, String, String)>>(
                "DisableUnitFiles",
                &(vec![name], false),
            )?;
        Ok(())
    }

    fn list_services(&self) -> Result<Vec<Service>, Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;

        let units: Vec<SystemdUnit> = proxy.call("ListUnits", &())?;

        let services = units
            .into_iter()
            .filter(|(name, ..)| name.ends_with(".service"))
            .map(|(name, description, load_state, active_state, sub_state, _followed, _object_path, _job_id, _job_type, _job_object)| {
                let state: String = proxy
                    .call("GetUnitFileState", &name)
                    .unwrap_or_else(|_| "unknown".into());
                
                let service_state = ServiceState::new(load_state, active_state, sub_state, state);

                Service::new(name, description, service_state)
            })
            .collect();

        Ok(services)
    }

    fn get_service_log(&self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let output = std::process::Command::new("journalctl")
            .arg("-eu")
            .arg(name)
            .arg("--no-pager")
            .output()?;

        let log = if output.status.success() {
            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            String::from_utf8_lossy(&output.stderr).to_string()
        };

        Ok(log)
    }
}

