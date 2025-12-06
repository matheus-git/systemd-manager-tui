use crate::infrastructure::systemd_service_adapter::ConnectionType;

use super::service::Service;
use std::error::Error;
use std::collections::HashMap;

pub trait ServiceRepository: Send + Sync {
    fn list_services(&self, filter: bool) -> Result<Vec<Service>, Box<dyn Error>>;
    fn unit_files_state(&self, services: Vec<Service>) -> Result<HashMap<String, String>, Box<dyn Error>>;
    fn list_service_files(&self) -> Result<Vec<Service>, Box<dyn Error>>;
    fn get_unit(&self, name: &str) -> Result<Service, Box<dyn Error>>;
    fn get_service_log(&self, name: &str) -> Result<String, Box<dyn Error>>;
    fn start_service(&self, name: &str) -> Result<Service, Box<dyn Error>>;
    fn stop_service(&self, name: &str) -> Result<Service, Box<dyn Error>>;
    fn restart_service(&self, name: &str) -> Result<Service, Box<dyn Error>>;
    fn enable_service(&self, name: &str) -> Result<Service, Box<dyn Error>>;
    fn disable_service(&self, name: &str) -> Result<Service, Box<dyn Error>>;
    fn mask_service(&self, name: &str) -> Result<Service, Box<dyn Error>>;
    fn unmask_service(&self, name: &str) -> Result<Service, Box<dyn Error>>;
    fn reload_daemon(&self) -> Result<(), Box<dyn std::error::Error>>;
    fn change_connection(&mut self, connection_type: ConnectionType) -> Result<(), zbus::Error>;
    fn systemctl_cat(&self, name: &str) -> Result<String, Box<dyn Error>>;
}
