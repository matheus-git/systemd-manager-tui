use crate::domain::service::Service;
use crate::domain::service_repository::ServiceRepository;
use crate::infrastructure::systemd_service_adapter::ConnectionType;
use std::error::Error;
use std::thread;
use std::time::Duration;
use std::collections::HashSet;

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
        let mut all = Vec::new();

        let mut services_runtime = self.repository.list_services(filter)?;
        let mut services_files = self.repository.list_service_files(filter)?;

        all.append(&mut services_runtime);
        all.append(&mut services_files);

        if filter {
            all.retain(|s| s.name().ends_with(".service"));
        }

        let mut seen = HashSet::new();
        all.retain(|s| seen.insert(s.name().to_string()));

        all.sort_by_key(|s| s.name().to_lowercase());

        Ok(all)
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

