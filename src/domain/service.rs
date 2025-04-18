use super::service_state::ServiceState;

#[derive(Clone)]
pub struct Service {
   name: String,
   description: String,
   state: ServiceState
} 

impl Service {
    pub fn new(name: String, description: String, state: ServiceState) -> Self {
        Service { name, description, state }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn state(&self) -> &ServiceState {
        &self.state
    }
}
