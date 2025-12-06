use crate::domain::service::Service;
use crate::domain::service_repository::ServiceRepository;
use crate::infrastructure::systemd_service_adapter::ConnectionType;
use crate::terminal::components::list::QueryUnitFile;
use std::error::Error;
use std::collections::HashSet;
use std::sync::mpsc::Sender;
use std::thread;
use std::sync::{Arc, Mutex};

pub struct ServicesManager {
    repository: Arc<Mutex<Box<dyn ServiceRepository>>>,
}

impl ServicesManager {
    pub fn new(repository: Box<dyn ServiceRepository>) -> Self {
        Self { repository: Arc::new(Mutex::new(repository)) }
    }

    pub fn start_service(&self, service: &Service) -> Result<Service, Box<dyn Error>> {
        let service = self.repository.lock().unwrap().start_service(service.name())?;
        Ok(service)
    }

    pub fn stop_service(&self, service: &Service) -> Result<Service, Box<dyn Error>> {
        let service = self.repository.lock().unwrap().stop_service(service.name())?;
        Ok(service)
    }

    pub fn restart_service(&self, service: &Service) -> Result<Service, Box<dyn Error>> {
        let service = self.repository.lock().unwrap().restart_service(service.name())?;
        Ok(service)
    }

    pub fn enable_service(&self, service: &Service) -> Result<Service, Box<dyn Error>> {
        let service = self.repository.lock().unwrap().enable_service(service.name())?;
        self.repository.lock().unwrap().reload_daemon()?;
        Ok(service)
    }

    pub fn disable_service(&self, service: &Service) -> Result<Service, Box<dyn Error>> {
        let service = self.repository.lock().unwrap().disable_service(service.name())?;
        self.repository.lock().unwrap().reload_daemon()?;
        Ok(service)
    }
    
    pub fn mask_service(&self, service: &Service) -> Result<Service, Box<dyn Error>> {
        let service = self.repository.lock().unwrap().mask_service(service.name())?;
        Ok(service)
    }

    pub fn unmask_service(&self, service: &Service) -> Result<Service, Box<dyn Error>> {
        let service = self.repository.lock().unwrap().unmask_service(service.name())?;
        Ok(service)
    }

    pub fn list_services(&self, filter: bool, tx: Arc<Sender<QueryUnitFile>>) -> Result<Vec<Service>, Box<dyn Error>> {
        let mut all = Vec::new();

        let mut services_runtime = self.repository.lock().unwrap().list_services(filter)?;

        all.append(&mut services_runtime);

        //if false {
        //    let mut services_files = self.repository.list_service_files(filter)?;
        //    all.append(&mut services_files);
        //    all.retain(|s| s.name().ends_with(".service"));
        //}

        let mut seen = HashSet::new();
        all.retain(|s| seen.insert(s.name().to_string()));

        all.sort_by_key(|s| s.name().to_lowercase());
        
        let repo = Arc::clone(&self.repository);

        thread::spawn(move || {
            let services_runtime = repo.lock().unwrap().list_services(filter).expect("dfsdfds");
            if let Ok(states) = repo.lock().unwrap().unit_files_state(services_runtime) {
                log::info!("states 1: {:?}", states);
                let _ = tx.send(QueryUnitFile::Finished(states));
            }
        });

        Ok(all)
    }

    pub fn units_file(&self, services: Vec<Service>, tx: Arc<Sender<QueryUnitFile>>) {
        let repo = Arc::clone(&self.repository);
        thread::spawn(move || {
            if let Ok(states) = repo.lock().unwrap().unit_files_state(services) {
                log::info!("states: {:?}", states);
                let _ = tx.send(QueryUnitFile::Finished(states));
            }
        });
    }

    pub fn get_log(&self, service: &Service) -> Result<String, Box<dyn Error>> {
        self.repository.lock().unwrap().get_service_log(service.name())
    }

    pub fn change_repository_connection(&mut self, connection_type: ConnectionType) -> Result<(), Box<dyn Error>> {
        self.repository.lock().unwrap().change_connection(connection_type)?;
        Ok(())
    }

    pub fn systemctl_cat(&self, service: &Service) -> Result<String, Box<dyn Error>> {
        self.repository.lock().unwrap().systemctl_cat(service.name())
    }
}

