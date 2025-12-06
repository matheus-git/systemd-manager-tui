use super::service_state::ServiceState;

#[derive(Clone, Debug)]
pub struct Service {
    name: String,
    description: String,
    state: ServiceState,
}

impl Service {
    pub fn new(name: String, description: String, state: ServiceState) -> Self {
        Service {
            name,
            description,
            state,
        }
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

    pub fn set_file_state(&mut self, file: String) {
        self.state.file = file;
    }
}
