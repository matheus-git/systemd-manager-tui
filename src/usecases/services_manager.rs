use crate::{domain::service::service_repository::ServiceRepository, infrastructure::systemd_service_adapter::SystemdServiceAdapter};
use crate::domain::service::service::Service;
use std::time::Duration;
use std::thread;

const SLEEP_DURATION: u64 = 500;

pub struct ServicesManager;

impl ServicesManager {
    pub fn start_service(name: &str) {
        let _ = SystemdServiceAdapter.start_service(name);
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
    }

    pub fn stop_service(name: &str) {
        let _ = SystemdServiceAdapter.stop_service(name);
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
    }

    pub fn restart_service(name: &str) {
        let _ = SystemdServiceAdapter.restart_service(name);
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
    }

    pub fn enable_service(name: &str) {
        let _ = SystemdServiceAdapter.enable_service(name);
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        let _ = SystemdServiceAdapter.reload_daemon();
    }

    pub fn disable_service(name: &str) {
        let _ = SystemdServiceAdapter.disable_service(name);
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        let _ = SystemdServiceAdapter.reload_daemon();
    }

    pub fn list_services() -> Result<Vec<Service>, Box<dyn std::error::Error>> {
        let mut services = SystemdServiceAdapter.list_services()?;
        services.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(services)
    }   
}



