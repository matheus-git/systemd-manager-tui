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
