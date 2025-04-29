use crate::domain::service::Service;
use crate::{
    domain::service_repository::ServiceRepository,
    infrastructure::systemd_service_adapter::SystemdServiceAdapter,
};
use std::error::Error;
use std::thread;
use std::time::Duration;

const SLEEP_DURATION: u64 = 200;

pub struct ServicesManager;

impl ServicesManager {
    pub fn start_service(service: &Service) -> Result<(), Box<dyn Error>> {
        SystemdServiceAdapter.start_service(service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        Ok(())
    }

    pub fn stop_service(service: &Service) -> Result<(), Box<dyn Error>> {
        SystemdServiceAdapter.stop_service(service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        Ok(())
    }

    pub fn restart_service(service: &Service) -> Result<(), Box<dyn Error>> {
        SystemdServiceAdapter.restart_service(service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        Ok(())
    }

    pub fn enable_service(service: &Service) -> Result<(), Box<dyn Error>> {
        SystemdServiceAdapter.enable_service(service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        SystemdServiceAdapter.reload_daemon()?;
        Ok(())
    }

    pub fn disable_service(service: &Service) -> Result<(), Box<dyn Error>> {
        SystemdServiceAdapter.disable_service(service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        SystemdServiceAdapter.reload_daemon()?;
        Ok(())
    }

    pub fn list_services() -> Result<Vec<Service>, Box<dyn Error>> {
        let mut services = SystemdServiceAdapter.list_services()?;
        services.sort_by_key(|a| a.name().to_lowercase());
        Ok(services)
    }

    pub fn update_properties(service: &mut Service) -> Result<(), Box<dyn Error>> {
        let service_property = SystemdServiceAdapter.get_service_property(service.name())?;
        let _ = &service.update_properties(service_property);
        Ok(())
    }

    pub fn get_log(service: &Service) -> Result<String, Box<dyn Error>> {
        let log = SystemdServiceAdapter.get_service_log(service.name())?;
        Ok(log)
    }
}
