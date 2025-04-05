use crate::{domain::service::service_repository::ServiceRepository, infrastructure::systemd_service_adapter::SystemdServiceAdapter};
use crate::domain::service::service::Service;

pub fn list_services() -> Result<Vec<Service>, Box<dyn std::error::Error>> {
    SystemdServiceAdapter.list_services()
}

