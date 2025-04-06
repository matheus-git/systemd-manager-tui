use crate::{domain::service::service_repository::ServiceRepository, infrastructure::systemd_service_adapter::SystemdServiceAdapter};
use crate::domain::service::service::Service;

pub fn list_services() -> Result<Vec<Service>, Box<dyn std::error::Error>> {
    let mut services = SystemdServiceAdapter.list_services()?;
    services.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(services)
}


