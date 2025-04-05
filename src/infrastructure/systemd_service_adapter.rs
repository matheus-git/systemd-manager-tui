use zbus::blocking::Connection;
use zbus::blocking::Proxy;
use zbus::zvariant::OwnedObjectPath;

use crate::domain::service::service::Service;
use crate::domain::service::service_repository::ServiceRepository;

pub struct SystemdServiceAdapter;

impl ServiceRepository for SystemdServiceAdapter {
    fn list_services(&self) -> Result<Vec<Service>, Box<dyn std::error::Error>> {
        let connection = Connection::system()?;

        let proxy = Proxy::new(
            &connection,
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
        )?;

        let units: Vec<(
            String,         // unit name
            String,         // description
            String,         // load state
            String,         // active state
            String,         // sub state
            String,         // followed
            OwnedObjectPath,// object path
            u32,            // job id
            String,         // job type
            OwnedObjectPath // job object path
        )> = proxy.call("ListUnits", &())?;

        let services = units
            .into_iter()
            .filter(|(name, ..)| name.ends_with(".service"))
            .map(|(name, description, load_state, active_state, sub_state, followed, object_path, job_id, job_type, job_object)| {
                Service {
                    name,
                    description,
                    load_state,
                    active_state,
                    sub_state,
                    followed,
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

