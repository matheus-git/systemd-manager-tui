use crate::domain::service::Service;
use crate::domain::service_repository::ServiceRepository;
use crate::infrastructure::systemd_service_adapter::ConnectionType;
use crate::terminal::components::list::{ListRequestContext, QueryUnitFile};
use std::collections::HashSet;
use std::error::Error;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

enum UnitFileQuery {
    Fetch {
        services: Vec<Service>,
        context: ListRequestContext,
        tx: Arc<Sender<QueryUnitFile>>,
    },
    Shutdown,
}

pub struct ServicesManager {
    repository: Arc<Mutex<Box<dyn ServiceRepository>>>,
    unit_file_query_tx: Sender<UnitFileQuery>,
    unit_file_query_handle: Option<JoinHandle<()>>,
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
        let repository = Arc::new(Mutex::new(repository));
        let (unit_file_query_tx, unit_file_query_rx) = mpsc::channel();
        let worker_repository = repository.clone();
        let unit_file_query_handle = Some(thread::spawn(move || {
            loop {
                let mut request = match unit_file_query_rx.recv() {
                    Ok(UnitFileQuery::Fetch {
                        services,
                        context,
                        tx,
                    }) => (services, context, tx),
                    Ok(UnitFileQuery::Shutdown) | Err(_) => break,
                };

                while let Ok(next) = unit_file_query_rx.try_recv() {
                    match next {
                        UnitFileQuery::Fetch {
                            services,
                            context,
                            tx,
                        } => request = (services, context, tx),
                        UnitFileQuery::Shutdown => return,
                    }
                }

                let (services, context, tx) = request;
                let result = worker_repository
                    .lock()
                    .map_err(|error| error.to_string())
                    .and_then(|repository| {
                        repository
                            .unit_files_state(services)
                            .map_err(|error| error.to_string())
                    });
                let message = match result {
                    Ok(states) => QueryUnitFile::Finished(context, states),
                    Err(error) => QueryUnitFile::Error(context, error),
                };
                let _ = tx.send(message);
            }
        }));

        Self {
            repository,
            unit_file_query_tx,
            unit_file_query_handle,
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

        if self
            .unit_file_query_tx
            .send(UnitFileQuery::Fetch {
                services: all.clone(),
                context,
                tx: tx.clone(),
            })
            .is_err()
        {
            let _ = tx.send(QueryUnitFile::Error(
                context,
                "Unit-file query worker is not available".to_string(),
            ));
        }

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

impl Drop for ServicesManager {
    fn drop(&mut self) {
        let _ = self.unit_file_query_tx.send(UnitFileQuery::Shutdown);
        // A read-only ListUnitFiles request may still be blocked in D-Bus. Its
        // repository and result sender are owned through Arc, so detaching it is
        // safe and avoids delaying process exit for an optional cache refresh.
        let _ = self.unit_file_query_handle.take();
    }
}
