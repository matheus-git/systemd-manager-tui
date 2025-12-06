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
