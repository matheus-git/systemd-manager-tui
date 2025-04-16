use crate::{domain::service::service_repository::ServiceRepository, infrastructure::systemd_service_adapter::SystemdServiceAdapter};
use crate::domain::service::service::Service;
use std::time::Duration;
use std::thread;
use std::error::Error;

const SLEEP_DURATION: u64 = 200;

pub struct ServicesManager;

impl ServicesManager {
    pub fn start_service(service: &Service) -> Result<(), Box<dyn Error>> {
        SystemdServiceAdapter.start_service(&service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        Ok(())
    }

    pub fn stop_service(service: &Service) -> Result<(), Box<dyn Error>> {
        SystemdServiceAdapter.stop_service(&service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        Ok(())
    }

    pub fn restart_service(service: &Service) -> Result<(), Box<dyn Error>> {
        SystemdServiceAdapter.restart_service(&service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        Ok(())
    }

    pub fn enable_service(service: &Service) -> Result<(), Box<dyn Error>> {
        SystemdServiceAdapter.enable_service(&service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        SystemdServiceAdapter.reload_daemon()?;
        Ok(())
    }

    pub fn disable_service(service: &Service) -> Result<(), Box<dyn Error>> {
        SystemdServiceAdapter.disable_service(&service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        SystemdServiceAdapter.reload_daemon()?;
        Ok(())
    }

    pub fn list_services() -> Result<Vec<Service>, Box<dyn Error>> {
        let mut services = SystemdServiceAdapter.list_services()?;
        services.sort_by(|a, b| a.name().to_lowercase().cmp(&b.name().to_lowercase()));
        Ok(services)
    }   

    pub fn get_log(service: &Service) -> Result<String, Box<dyn Error>> {
        let log = SystemdServiceAdapter.get_service_log(&service.name())?;
        Ok(log)
    }
}
