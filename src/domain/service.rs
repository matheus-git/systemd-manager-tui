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

    pub fn is_template(&self) -> bool {
        self.name
            .strip_suffix(".service")
            .is_some_and(|stem| stem.ends_with('@'))
    }

    pub fn instantiate(&self, instance: &str) -> Result<Self, String> {
        let prefix = self
            .name
            .strip_suffix("@.service")
            .ok_or_else(|| format!("'{}' is not a service template", self.name))?;
        let escaped_instance = escape_instance(instance)?;
        let name = format!("{prefix}@{escaped_instance}.service");

        if name.len() > 255 {
            return Err("Instantiated unit name exceeds systemd's 255-byte limit".to_string());
        }

        Ok(Self::new(
            name,
            self.description.clone(),
            self.state.clone(),
        ))
    }
}

fn escape_instance(instance: &str) -> Result<String, String> {
    if instance.is_empty() {
        return Err("Instance name cannot be empty".to_string());
    }

    let mut escaped = String::with_capacity(instance.len());
    for (index, byte) in instance.bytes().enumerate() {
        let allowed = byte.is_ascii_alphanumeric()
            || matches!(byte, b':' | b'_' | b'.' | b'-');

        if byte == b'/' {
            escaped.push('-');
        } else if allowed && !(index == 0 && byte == b'.') {
            escaped.push(char::from(byte));
        } else {
            escaped.push_str(&format!("\\x{byte:02x}"));
        }
    }

    Ok(escaped)
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

    #[test]
    fn recognizes_only_uninstantiated_service_templates() {
        let state = ServiceState::new("loaded".into(), "inactive".into(), "dead".into(), "disabled".into());

        assert!(Service::new("worker@.service".into(), String::new(), state.clone()).is_template());
        assert!(!Service::new("worker@one.service".into(), String::new(), state.clone()).is_template());
        assert!(!Service::new("worker.service".into(), String::new(), state).is_template());
    }

    #[test]
    fn instantiates_template_and_preserves_metadata() {
        let state = ServiceState::new("loaded".into(), "inactive".into(), "dead".into(), "disabled".into());
        let template = Service::new("worker@.service".into(), "Worker".into(), state);

        let instance = template.instantiate("queue-1").unwrap();

        assert_eq!(instance.name(), "worker@queue-1.service");
        assert_eq!(instance.description(), "Worker");
        assert_eq!(instance.state(), template.state());
    }

    #[test]
    fn escapes_instance_for_a_systemd_unit_name() {
        let state = ServiceState::new("loaded".into(), "inactive".into(), "dead".into(), "disabled".into());
        let template = Service::new("worker@.service".into(), String::new(), state);

        assert_eq!(
            template.instantiate("queue one/path@").unwrap().name(),
            r"worker@queue\x20one-path\x40.service"
        );
        assert_eq!(
            template.instantiate(".hidden").unwrap().name(),
            r"worker@\x2ehidden.service"
        );
        assert_eq!(
            template.instantiate("café").unwrap().name(),
            r"worker@caf\xc3\xa9.service"
        );
    }

    #[test]
    fn rejects_empty_instance_and_non_template_service() {
        let state = ServiceState::new("loaded".into(), "inactive".into(), "dead".into(), "disabled".into());
        let template = Service::new("worker@.service".into(), String::new(), state.clone());
        let regular = Service::new("worker.service".into(), String::new(), state);

        assert_eq!(template.instantiate("").unwrap_err(), "Instance name cannot be empty");
        assert!(regular.instantiate("one").unwrap_err().contains("not a service template"));
    }

    #[test]
    fn rejects_instantiated_unit_name_above_systemd_limit() {
        let state = ServiceState::new("loaded".into(), "inactive".into(), "dead".into(), "disabled".into());
        let template = Service::new("worker@.service".into(), String::new(), state);

        let maximum_instance_length = 255 - template.name().len();
        assert_eq!(
            template
                .instantiate(&"a".repeat(maximum_instance_length))
                .unwrap()
                .name()
                .len(),
            255
        );
        assert_eq!(
            template
                .instantiate(&"a".repeat(maximum_instance_length + 1))
                .unwrap_err(),
            "Instantiated unit name exceeds systemd's 255-byte limit"
        );
    }
}
