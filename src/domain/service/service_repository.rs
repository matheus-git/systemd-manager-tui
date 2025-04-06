use std::error::Error;
use super::service::Service;

pub trait ServiceRepository {
    fn list_services(&self) -> Result<Vec<Service>, Box<dyn Error>>;
    fn start_service(&self, name: &str) -> Result<(), Box<dyn Error>>;
    fn stop_service(&self, name: &str) -> Result<(), Box<dyn Error>>;
    fn restart_service(&self, name: &str) -> Result<(), Box<dyn Error>>;
    fn enable_service(&self, name: &str) -> Result<(), Box<dyn Error>>;
    fn disable_service(&self, name: &str) -> Result<(), Box<dyn Error>>;
}
