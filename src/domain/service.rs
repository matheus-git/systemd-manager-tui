use super::service_state::ServiceState;

#[derive(Clone, Debug, PartialEq, Eq)]
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_service_data() {
        let state = ServiceState::new("loaded".into(), "active".into(), "running".into(), "enabled".into());
        let service = Service::new("demo.service".into(), "Demo service".into(), state);

        assert_eq!(service.name(), "demo.service");
        assert_eq!(service.description(), "Demo service");
        assert_eq!(service.state().active(), "active");
    }
}
