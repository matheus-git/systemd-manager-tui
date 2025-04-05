use std::error::Error;
use super::service::Service;

pub trait ServiceRepository {
    fn list_services(&self) -> Result<Vec<Service>, Box<dyn Error>>;
}
