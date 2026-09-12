use crate::domain::service::Service;
use crate::domain::service_repository::ServiceRepository;
use crate::domain::service_state::ServiceState;
use crate::infrastructure::systemd_service_adapter::ConnectionType;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeState {
    runtime_services: Vec<Service>,
    file_services: Vec<Service>,
    calls: Vec<String>,
    failures: HashSet<String>,
    log: String,
    unit_file: String,
    timestamp: u64,
}

#[derive(Clone, Default)]
pub struct FakeRepository {
    state: Arc<Mutex<FakeState>>,
}

impl FakeRepository {
    pub fn with_services(runtime: Vec<Service>, files: Vec<Service>) -> Self {
        let repository = Self::default();
        {
            let mut state = repository.state.lock().unwrap();
            state.runtime_services = runtime;
            state.file_services = files;
        }
        repository
    }

    pub fn with_content(log: &str, unit_file: &str, timestamp: u64) -> Self {
        let repository = Self::default();
        {
            let mut state = repository.state.lock().unwrap();
            state.log = log.to_string();
            state.unit_file = unit_file.to_string();
            state.timestamp = timestamp;
        }
        repository
    }

    pub fn fail(&self, operation: &str) {
        self.state
            .lock()
            .unwrap()
            .failures
            .insert(operation.to_string());
    }

    pub fn calls(&self) -> Vec<String> {
        self.state.lock().unwrap().calls.clone()
    }

    fn record(&self, operation: &str, name: &str) -> Result<(), Box<dyn Error>> {
        let mut state = self.state.lock().unwrap();
        state.calls.push(format!("{operation}:{name}"));
        if state.failures.contains(operation) {
            Err(format!("{operation} failed").into())
        } else {
            Ok(())
        }
    }

    fn service_result(&self, operation: &str, name: &str) -> Result<Service, Box<dyn Error>> {
        self.record(operation, name)?;
        Ok(service(name, "active", "enabled"))
    }
}

pub fn service(name: &str, active: &str, file: &str) -> Service {
    Service::new(
        name.to_string(),
        format!("Description for {name}"),
        ServiceState::new(
            "loaded".into(),
            active.into(),
            "running".into(),
            file.into(),
        ),
    )
}

impl ServiceRepository for FakeRepository {
    fn list_services(&self, filter: bool) -> Result<Vec<Service>, Box<dyn Error>> {
        self.record("list", &filter.to_string())?;
        Ok(self.state.lock().unwrap().runtime_services.clone())
    }

    fn unit_files_state(
        &self,
        services: Vec<Service>,
    ) -> Result<HashMap<String, String>, Box<dyn Error>> {
        self.record("states", &services.len().to_string())?;
        Ok(services
            .into_iter()
            .map(|s| (s.name().to_string(), s.state().file().to_string()))
            .collect())
    }

    fn list_service_files(&self) -> Result<Vec<Service>, Box<dyn Error>> {
        self.record("list_files", "")?;
        Ok(self.state.lock().unwrap().file_services.clone())
    }

    fn get_unit(&self, name: &str) -> Result<Service, Box<dyn Error>> {
        self.service_result("get", name)
    }
    fn get_service_log(&self, name: &str) -> Result<String, Box<dyn Error>> {
        self.record("log", name)?;
        Ok(self.state.lock().unwrap().log.clone())
    }
    fn start_service(&self, name: &str) -> Result<Service, Box<dyn Error>> {
        self.service_result("start", name)
    }
    fn stop_service(&self, name: &str) -> Result<Service, Box<dyn Error>> {
        self.service_result("stop", name)
    }
    fn restart_service(&self, name: &str) -> Result<Service, Box<dyn Error>> {
        self.service_result("restart", name)
    }
    fn enable_service(&self, name: &str) -> Result<Service, Box<dyn Error>> {
        self.service_result("enable", name)
    }
    fn disable_service(&self, name: &str) -> Result<Service, Box<dyn Error>> {
        self.service_result("disable", name)
    }
    fn mask_service(&self, name: &str) -> Result<Service, Box<dyn Error>> {
        self.service_result("mask", name)
    }
    fn unmask_service(&self, name: &str) -> Result<Service, Box<dyn Error>> {
        self.service_result("unmask", name)
    }
    fn reload_daemon(&self) -> Result<(), Box<dyn Error>> {
        self.record("reload", "")
    }
    fn change_connection(&mut self, connection_type: ConnectionType) -> Result<(), zbus::Error> {
        let name = match connection_type {
            ConnectionType::System => "system",
            ConnectionType::Session => "session",
        };
        let _ = self.record("connection", name);
        Ok(())
    }
    fn systemctl_cat(&self, name: &str) -> Result<String, Box<dyn Error>> {
        self.record("cat", name)?;
        Ok(self.state.lock().unwrap().unit_file.clone())
    }
    fn get_active_enter_timestamp(&self, name: &str) -> Result<u64, Box<dyn Error>> {
        self.record("timestamp", name)?;
        Ok(self.state.lock().unwrap().timestamp)
    }
}
