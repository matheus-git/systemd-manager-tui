use crate::domain::service::Service;
use crate::domain::service_repository::ServiceRepository;
use crate::infrastructure::systemd_service_adapter::ConnectionType;
use crate::terminal::components::list::{ListRequestContext, QueryUnitFile};
use std::collections::HashSet;
use std::error::Error;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

pub struct ServicesManager {
    repository: Arc<Mutex<Box<dyn ServiceRepository>>>,
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests;

impl ServicesManager {
    fn repository(&self) -> Result<MutexGuard<'_, Box<dyn ServiceRepository>>, Box<dyn Error>> {
        self.repository
            .lock()
            .map_err(|error| format!("Service repository lock was poisoned: {error}").into())
    }

    pub fn new(repository: Box<dyn ServiceRepository>) -> Self {
        Self {
            repository: Arc::new(Mutex::new(repository)),
        }
    }

    pub fn start_service(&self, service: &Service) -> Result<Service, Box<dyn Error>> {
        let service = self.repository()?.start_service(service.name())?;
        Ok(service)
    }

    pub fn stop_service(&self, service: &Service) -> Result<Service, Box<dyn Error>> {
        let service = self.repository()?.stop_service(service.name())?;
        Ok(service)
    }

    pub fn restart_service(&self, service: &Service) -> Result<Service, Box<dyn Error>> {
        let service = self.repository()?.restart_service(service.name())?;
        Ok(service)
    }

    pub fn enable_service(&self, service: &Service) -> Result<Service, Box<dyn Error>> {
        let service = self.repository()?.enable_service(service.name())?;
        self.repository()?.reload_daemon()?;
        Ok(service)
    }

    pub fn disable_service(&self, service: &Service) -> Result<Service, Box<dyn Error>> {
        let service = self.repository()?.disable_service(service.name())?;
        self.repository()?.reload_daemon()?;
        Ok(service)
    }

    pub fn mask_service(&self, service: &Service) -> Result<Service, Box<dyn Error>> {
        let service = self.repository()?.mask_service(service.name())?;
        Ok(service)
    }

    pub fn unmask_service(&self, service: &Service) -> Result<Service, Box<dyn Error>> {
        let service = self.repository()?.unmask_service(service.name())?;
        Ok(service)
    }

    pub fn list_services(
        &self,
        filter: bool,
        context: ListRequestContext,
        tx: Arc<Sender<QueryUnitFile>>,
    ) -> Result<Vec<Service>, Box<dyn Error>> {
        let mut all = Vec::new();

        let mut services_runtime = self.repository()?.list_services(filter)?;
        all.append(&mut services_runtime);

        let mut seen = HashSet::new();
        #[allow(clippy::explicit_iter_loop)]
        for s in all.iter() {
            seen.insert(s.name().to_string());
        }

        if filter {
            let services_files = self.repository()?.list_service_files()?;
            #[allow(clippy::explicit_iter_loop)]
            for s in services_files.iter() {
                if seen.insert(s.name().to_string()) {
                    all.push(s.clone());
                }
            }
        }

        all.sort_by_key(|a| a.name().to_ascii_lowercase());

        let repo = Arc::clone(&self.repository);
        thread::spawn(move || {
            let result = repo
                .lock()
                .map_err(|error| error.to_string())
                .and_then(|repo| {
                    let services = repo
                        .list_services(filter)
                        .map_err(|error| error.to_string())?;
                    repo.unit_files_state(services)
                        .map_err(|error| error.to_string())
                });
            let message = match result {
                Ok(states) => QueryUnitFile::Finished(context, states),
                Err(error) => QueryUnitFile::Error(context, error),
            };
            let _ = tx.send(message);
        });

        Ok(all)
    }

    pub fn get_log(&self, service_name: &str) -> Result<String, Box<dyn Error>> {
        self.repository()?.get_service_log(service_name)
    }

    pub fn change_repository_connection(
        &mut self,
        connection_type: ConnectionType,
    ) -> Result<(), Box<dyn Error>> {
        self.repository()?.change_connection(connection_type)?;
        Ok(())
    }

    pub fn systemctl_cat(&self, service: &Service) -> Result<String, Box<dyn Error>> {
        self.repository()?.systemctl_cat(service.name())
    }

    pub fn repository_handle(&self) -> Arc<Mutex<Box<dyn ServiceRepository>>> {
        Arc::clone(&self.repository)
    }
}
