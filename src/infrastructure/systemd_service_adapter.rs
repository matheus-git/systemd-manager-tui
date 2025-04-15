use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;

use crate::domain::service::service::Service;
use crate::domain::service::service_repository::ServiceRepository;

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
        proxy.call_method::<&str, ()>("Reload", &())?; 
        Ok(())
    }
    pub fn get_service_log(&self, service_name: &str) -> Result<(Service, String), Box<dyn std::error::Error>> {
        let connection = Connection::system()?;

        let proxy = Proxy::new(
            &connection,
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
        )?;

        let units: Vec<(
            String,         // name
            String,         // description
            String,         // load_state
            String,         // active_state
            String,         // sub_state
            String,         // followed
            OwnedObjectPath,// object_path
            u32,            // job_id
            String,         // job_type
            OwnedObjectPath // job_object
        )> = proxy.call("ListUnits", &())?;

        let unit = units.into_iter()
            .find(|(name, ..)| name == service_name)
            .ok_or_else(|| format!("Serviço '{}' não encontrado.", service_name))?;

        let file_state: String = proxy
            .call("GetUnitFileState", &service_name)
            .unwrap_or_else(|_| "unknown".into());

        let service = Service {
            name: unit.0,
            description: unit.1,
            load_state: unit.2,
            active_state: unit.3,
            sub_state: unit.4,
            followed: unit.5,
            object_path: unit.6,
            job_id: unit.7,
            job_type: unit.8,
            job_object: unit.9,
            file_state,
        };

        let output = std::process::Command::new("journalctl")
            .arg("-eu")
            .arg(service_name)
            .arg("--no-pager")
            .output()?;

        let log = if output.status.success() {
            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            String::from_utf8_lossy(&output.stderr).to_string()
        };

        Ok((service, log))
    }
}

impl ServiceRepository for SystemdServiceAdapter {
    fn start_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        proxy.call::<&str, (&str, &str), ()>("StartUnit", &(name, "replace"))?;
        Ok(())
    }

    fn stop_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        proxy.call::<&str, (&str, &str), ()>("StopUnit", &(name, "replace"))?;
        Ok(())
    }

    fn restart_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        proxy.call::<&str, (&str, &str), ()>("RestartUnit", &(name, "replace"))?;
        Ok(())
    }

    fn enable_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let args: (&[&str], bool, bool) = (&[name], false, true);
        proxy.call::<_, _, ()>("EnableUnitFiles", &args)?;
        Ok(())
    }

    fn disable_service(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let proxy = self.manager_proxy()?;
        let args: (&[&str], bool) = (&[name], false);
        proxy.call::<_, _, ()>("DisableUnitFiles", &args)?;
        Ok(())
    }

    fn list_services(&self) -> Result<Vec<Service>, Box<dyn std::error::Error>> {
        let connection = Connection::system()?;

        let proxy = Proxy::new(
            &connection,
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
        )?;

        let units: Vec<(
            String,         // name
            String,         // description
            String,         // load_state
            String,         // active_state
            String,         // sub_state
            String,         // followed
            OwnedObjectPath,// object_path
            u32,            // job_id
            String,         // job_type
            OwnedObjectPath // job_object
        )> = proxy.call("ListUnits", &())?;

        let services = units
            .into_iter()
            .filter(|(name, ..)| name.ends_with(".service"))
            .map(|(name, description, load_state, active_state, sub_state, followed, object_path, job_id, job_type, job_object)| {
                let state: String = proxy
                    .call("GetUnitFileState", &name)
                    .unwrap_or_else(|_| "unknown".into());

                Service {
                    name,
                    description,
                    load_state,
                    active_state,
                    sub_state,
                    followed,
                    file_state: state,
                    object_path,
                    job_id,
                    job_type,
                    job_object,
                }
            })
            .collect();

        Ok(services)
    }

}

