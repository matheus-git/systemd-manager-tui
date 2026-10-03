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

    pub fn is_template(&self) -> bool {
        self.name.split_once('@').is_some_and(|(prefix, suffix)| {
            !prefix.is_empty() && suffix.starts_with('.') && !suffix[1..].contains('.')
        })
    }

    /// `instance` is the identifier as written in a systemd unit name, including
    /// any systemd escapes. Do not escape it again (e.g. a literal '-' is valid).
    pub fn instantiate(&self, instance: &str) -> Result<Self, String> {
        if !self.is_template() {
            return Err("Select a template unit first".into());
        }
        if instance.is_empty() {
            return Err("Enter an instance name".into());
        }
        if !instance
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b":@-_.\\".contains(&byte))
        {
            return Err(
                "Use a systemd instance identifier; escape spaces or paths with systemd-escape"
                    .into(),
            );
        }
        let (prefix, suffix) = self.name.split_once('@').expect("validated template");
        let name = format!("{prefix}@{instance}{suffix}");
        if name.len() > 255 {
            return Err("The complete unit name must not exceed 255 bytes".into());
        }
        Ok(Self::new(
            name,
            self.description.clone(),
            ServiceState::default(),
        ))
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn state(&self) -> &ServiceState {
        &self.state
    }
}

#[cfg(test)]
mod tests;
