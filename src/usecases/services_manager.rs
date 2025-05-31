use crate::domain::service::Service;
use crate::domain::service_repository::ServiceRepository;
use crate::infrastructure::systemd_service_adapter::ConnectionType;
use std::error::Error;
use std::thread;
use std::time::Duration;

const SLEEP_DURATION: u64 = 200;

pub struct ServicesManager {
    repository: Box<dyn ServiceRepository>,
}

impl ServicesManager {
    pub fn new(repository: Box<dyn ServiceRepository>) -> Self {
        Self { repository }
    }

    pub fn start_service(&self, service: &Service) -> Result<(), Box<dyn Error>> {
        self.repository.start_service(service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        Ok(())
    }

    pub fn stop_service(&self, service: &Service) -> Result<(), Box<dyn Error>> {
        self.repository.stop_service(service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        Ok(())
    }

    pub fn restart_service(&self, service: &Service) -> Result<(), Box<dyn Error>> {
        self.repository.restart_service(service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        Ok(())
    }

    pub fn enable_service(&self, service: &Service) -> Result<(), Box<dyn Error>> {
        self.repository.enable_service(service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        self.repository.reload_daemon()?;
        Ok(())
    }

    pub fn disable_service(&self, service: &Service) -> Result<(), Box<dyn Error>> {
        self.repository.disable_service(service.name())?;
        thread::sleep(Duration::from_millis(SLEEP_DURATION));
        self.repository.reload_daemon()?;
        Ok(())
    }

    pub fn list_services(&self, filter: bool) -> Result<Vec<Service>, Box<dyn Error>> {
        let mut services = self.repository.list_services(filter)?;
        services.sort_by_key(|a| a.name().to_lowercase());
        Ok(services)
    }

    #[allow(dead_code)]
    pub fn update_properties(&self, service: &mut Service) -> Result<(), Box<dyn Error>> {
        let props = self.repository.get_service_property(service.name())?;
        service.update_properties(props);
        Ok(())
    }

    pub fn get_log(&self, service: &Service) -> Result<String, Box<dyn Error>> {
        self.repository.get_service_log(service.name())
    }

    pub fn change_repository_connection(&mut self, connection_type: ConnectionType) -> Result<(), Box<dyn Error>> {
        self.repository.change_connection(connection_type)?;
        Ok(())
    }

    pub fn systemctl_cat(&self, service: &Service) -> Result<String, Box<dyn Error>> {
        self.repository.systemctl_cat(service.name())
    }
}

