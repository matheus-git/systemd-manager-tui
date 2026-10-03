use super::*;

#[test]
fn exposes_service_data() {
    let state = ServiceState::new(
        "loaded".into(),
        "active".into(),
        "running".into(),
        "enabled".into(),
    );
    let service = Service::new("demo.service".into(), "Demo service".into(), state);

    assert_eq!(service.name(), "demo.service");
    assert_eq!(service.description(), "Demo service");
    assert_eq!(service.state().active(), "active");
}

#[test]
fn distinguishes_templates_from_existing_instances() {
    for (name, expected) in [
        ("demo@.service", true),
        ("demo@.timer", true),
        ("demo@one.service", false),
        ("demo.service", false),
        ("demo@one@.service", false),
    ] {
        let unit = Service::new(name.into(), String::new(), ServiceState::default());
        assert_eq!(unit.is_template(), expected, "{name}");
    }
}

#[test]
fn instantiates_without_reescaping_identifiers_or_copying_template_state() {
    let template = Service::new(
        "demo@.service".into(),
        "Template".into(),
        ServiceState::new(
            "loaded".into(),
            "inactive".into(),
            "dead".into(),
            "disabled".into(),
        ),
    );
    for name in ["one", "tenant-1", "a:b.c_d", "user@host", r"a\x20b"] {
        let instance = template.instantiate(name).unwrap();
        assert_eq!(instance.name(), format!("demo@{name}.service"));
        assert_eq!(instance.description(), "Template");
        assert!(instance.state().file().is_empty());
    }
    for name in ["", "a b", "a/b", "é", "a\n"] {
        assert!(template.instantiate(name).is_err());
    }
    let longest = "a".repeat(255 - template.name().len());
    assert!(template.instantiate(&longest).is_ok());
    assert!(template.instantiate(&(longest + "a")).is_err());
    assert!(
        template
            .instantiate("one")
            .unwrap()
            .instantiate("two")
            .is_err()
    );
}
