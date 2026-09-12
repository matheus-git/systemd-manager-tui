use crate::domain::service::Service;
use crate::domain::service_repository::ServiceRepository;
use crate::infrastructure::systemd_service_adapter::ConnectionType;
use crate::terminal::components::list::QueryUnitFile;
use std::error::Error;
use std::collections::HashSet;
use std::sync::mpsc::Sender;
use std::thread;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct ServicesManager {
    repository: Arc<Mutex<Box<dyn ServiceRepository>>>,
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::test_support::{service, FakeRepository};
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn lists_sorts_and_deduplicates_runtime_and_file_units() {
        let fake = FakeRepository::with_services(
            vec![
                service("zeta.service", "active", "enabled"),
                service("alpha.service", "active", "enabled"),
            ],
            vec![
                service("alpha.service", "inactive", "disabled"),
                service("beta.service", "inactive", "disabled"),
            ],
        );
        let manager = ServicesManager::new(Box::new(fake));
        let (sender, receiver) = mpsc::channel();

        let services = manager.list_services(true, Arc::new(sender)).unwrap();

        let names: Vec<_> = services.iter().map(Service::name).collect();
        assert_eq!(names, ["alpha.service", "beta.service", "zeta.service"]);
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(1)),
            Ok(QueryUnitFile::Finished(_))
        ));
    }

    #[test]
    fn service_only_listing_does_not_add_unit_files() {
        let fake = FakeRepository::with_services(
            vec![service("runtime.service", "active", "enabled")],
            vec![service("file-only.service", "inactive", "disabled")],
        );
        let manager = ServicesManager::new(Box::new(fake));
        let (sender, _receiver) = mpsc::channel();

        let services = manager.list_services(false, Arc::new(sender)).unwrap();

        assert_eq!(services.len(), 1);
        assert_eq!(services[0].name(), "runtime.service");
    }

    #[test]
    fn background_state_errors_are_returned_to_the_caller() {
        let fake = FakeRepository::with_services(
            vec![service("demo.service", "active", "enabled")],
            vec![],
        );
        fake.fail("states");
        let manager = ServicesManager::new(Box::new(fake));
        let (sender, receiver) = mpsc::channel();

        manager.list_services(false, Arc::new(sender)).unwrap();

        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(1)),
            Ok(QueryUnitFile::Error(error)) if error == "states failed"
        ));
    }

    #[test]
    fn enable_and_disable_reload_the_daemon() {
        let fake = FakeRepository::default();
        let observer = fake.clone();
        let manager = ServicesManager::new(Box::new(fake));
        let unit = service("demo.service", "inactive", "disabled");

        manager.enable_service(&unit).unwrap();
        manager.disable_service(&unit).unwrap();

        assert_eq!(
            observer.calls(),
            [
                "enable:demo.service",
                "reload:",
                "disable:demo.service",
                "reload:",
            ]
        );
    }

    #[test]
    fn delegates_all_lifecycle_operations() {
        let fake = FakeRepository::default();
        let observer = fake.clone();
        let manager = ServicesManager::new(Box::new(fake));
        let unit = service("demo.service", "inactive", "disabled");

        manager.start_service(&unit).unwrap();
        manager.stop_service(&unit).unwrap();
        manager.restart_service(&unit).unwrap();
        manager.mask_service(&unit).unwrap();
        manager.unmask_service(&unit).unwrap();

        assert_eq!(
            observer.calls(),
            [
                "start:demo.service",
                "stop:demo.service",
                "restart:demo.service",
                "mask:demo.service",
                "unmask:demo.service",
            ]
        );
    }

    #[test]
    fn propagates_repository_errors_without_reloading() {
        let fake = FakeRepository::default();
        fake.fail("enable");
        let observer = fake.clone();
        let manager = ServicesManager::new(Box::new(fake));
        let unit = service("broken.service", "inactive", "disabled");

        let error = manager.enable_service(&unit).unwrap_err();

        assert_eq!(error.to_string(), "enable failed");
        assert_eq!(observer.calls(), ["enable:broken.service"]);
    }

    #[test]
    fn delegates_log_unit_file_timestamp_and_connection() {
        let fake = FakeRepository::with_content("journal output", "[Service]\nExecStart=/bin/true", 42);
        let observer = fake.clone();
        let mut manager = ServicesManager::new(Box::new(fake));
        let unit = service("demo.service", "active", "enabled");

        assert_eq!(manager.get_log(&unit).unwrap(), "journal output");
        assert!(manager.systemctl_cat(&unit).unwrap().contains("ExecStart"));
        manager.change_repository_connection(ConnectionType::Session).unwrap();
        assert_eq!(
            manager.repository_handle().lock().unwrap().get_active_enter_timestamp(unit.name()).unwrap(),
            42
        );

        assert_eq!(
            observer.calls(),
            ["log:demo.service", "cat:demo.service", "connection:session", "timestamp:demo.service"]
        );
    }
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

        let mut seen = HashSet::new();
        #[allow(clippy::explicit_iter_loop)]
        for s in all.iter() {
            seen.insert(s.name().to_string());
        }

        if filter {
            let services_files = self.repository.lock().unwrap().list_service_files()?;
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
                    let services = repo.list_services(filter).map_err(|error| error.to_string())?;
                    repo.unit_files_state(services).map_err(|error| error.to_string())
                });
            let message = match result {
                Ok(states) => QueryUnitFile::Finished(states),
                Err(error) => QueryUnitFile::Error(error),
            };
            let _ = tx.send(message);
        });

        Ok(all)
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

    pub fn repository_handle(&self) -> Arc<Mutex<Box<dyn ServiceRepository>>> {
        Arc::clone(&self.repository)
    }
}
