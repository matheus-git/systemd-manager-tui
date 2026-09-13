#[derive(Clone, Default, Debug)]
pub struct ServiceState {
    load: String,
    active: String,
    sub: String,
    pub file: String,
}

impl ServiceState {
    pub fn new(load: String, active: String, sub: String, file: String) -> Self {
        ServiceState {
            load,
            active,
            sub,
            file,
        }
    }

    pub fn load(&self) -> &str {
        &self.load
    }

    pub fn active(&self) -> &str {
        &self.active
    }

    pub fn sub(&self) -> &str {
        &self.sub
    }

    pub fn file(&self) -> &str {
        &self.file
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_all_state_fields() {
        let state = ServiceState::new("loaded".into(), "active".into(), "running".into(), "enabled".into());

        assert_eq!(state.load(), "loaded");
        assert_eq!(state.active(), "active");
        assert_eq!(state.sub(), "running");
        assert_eq!(state.file(), "enabled");
    }
}
