use super::*;

#[test]
fn exposes_all_state_fields() {
    let state = ServiceState::new(
        "loaded".into(),
        "active".into(),
        "running".into(),
        "enabled".into(),
    );

    assert_eq!(state.load(), "loaded");
    assert_eq!(state.active(), "active");
    assert_eq!(state.sub(), "running");
    assert_eq!(state.file(), "enabled");
}
