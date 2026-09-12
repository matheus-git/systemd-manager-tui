use super::*;
use crate::test_support::{FakeRepository, service};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;

fn test_app() -> App {
    test_app_with_repository(FakeRepository::default())
}

fn test_app_with_repository(repository: FakeRepository) -> App {
    let (event_tx, event_rx) = mpsc::channel();
    let manager = Rc::new(RefCell::new(ServicesManager::new(Box::new(repository))));
    let table = TableServices::new(event_tx.clone(), manager.clone());
    let filter = Filter::new(event_tx.clone(), String::new());
    let log = ServiceLog::new(event_tx.clone());
    let details = ServiceDetails::new(event_tx.clone());

    App::new(event_tx, event_rx, table, filter, log, details, manager)
}

#[test]
fn translates_known_dbus_errors() {
    let message = get_user_friendly_error("org.freedesktop.DBus.Error.AccessDenied: rejected");

    assert!(message.contains("Access denied"));
}

#[test]
fn preserves_unknown_errors() {
    assert_eq!(get_user_friendly_error("custom failure"), "custom failure");
}

#[test]
fn actions_navigate_between_all_screens() {
    let mut app = test_app();

    app.handle_event(AppEvent::Action(Actions::GoDetails))
        .unwrap();
    assert!(matches!(app.status, Status::Details));

    app.handle_event(AppEvent::Action(Actions::GoLog)).unwrap();
    assert!(matches!(app.status, Status::Log));

    app.handle_event(AppEvent::Action(Actions::GoList)).unwrap();
    assert!(matches!(app.status, Status::List));
}

#[test]
fn error_event_opens_overlay_and_next_key_closes_it() {
    let mut app = test_app();

    app.handle_event(AppEvent::Error("failure".into())).unwrap();
    assert_eq!(app.error_message.as_deref(), Some("failure"));

    app.handle_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Esc,
        KeyModifiers::NONE,
    )))
    .unwrap();
    assert!(app.error_message.is_none());
}

#[test]
fn error_overlay_is_rendered_by_test_backend() {
    let mut app = test_app();
    app.handle_event(AppEvent::Error(
        "org.freedesktop.DBus.Error.AccessDenied".into(),
    ))
    .unwrap();
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| app.draw_overlays(frame, frame.area()))
        .unwrap();

    let screen = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(screen.contains("ERROR"));
    assert!(screen.contains("Access denied"));
    assert!(screen.contains("Press any key to dismiss"));
}

#[test]
fn help_overlay_can_be_opened_and_closed() {
    let mut app = test_app();

    app.handle_event(AppEvent::Action(Actions::ShowHelp))
        .unwrap();
    assert!(app.show_help);

    app.handle_event(AppEvent::Key(KeyEvent::new(
        KeyCode::Char('?'),
        KeyModifiers::NONE,
    )))
    .unwrap();
    assert!(!app.show_help);
}

#[test]
fn ctrl_c_stops_the_application() {
    let mut app = test_app();

    let effects = app
        .handle_event(AppEvent::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )))
        .unwrap();

    assert!(!app.running);
    assert!(effects.is_empty());
    assert!(app.event_rx.try_recv().is_err());
}

#[test]
fn ctrl_z_returns_suspend_effect_without_touching_terminal() {
    let mut app = test_app();

    let effects = app
        .handle_event(AppEvent::Key(KeyEvent::new(
            KeyCode::Char('z'),
            KeyModifiers::CONTROL,
        )))
        .unwrap();

    assert_eq!(effects, [AppEffect::Suspend]);
}

#[test]
fn edit_action_returns_effect_for_selected_unit() {
    let mut app = test_app();
    app.table_service.services = vec![service("demo.service", "active", "enabled")];
    app.table_service.refresh("");

    let effects = app
        .handle_event(AppEvent::Action(Actions::EditCurrentService))
        .unwrap();

    assert_eq!(effects, [AppEffect::EditUnit("demo.service".into())]);
}

#[test]
fn log_worker_returns_result_through_event_queue() {
    let fake = FakeRepository::with_content("journal output", "", 0);
    let mut app = test_app_with_repository(fake);
    let unit = service("demo.service", "active", "enabled");
    app.status = Status::Log;
    app.latest_log_request_id = 1;
    app.table_service.services = vec![unit.clone()];
    app.table_service.refresh("");

    app.spawn_log_worker(1, unit);
    let event = app.event_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    app.handle_event(event).unwrap();

    let backend = TestBackend::new(50, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| app.service_log.render(frame, frame.area()))
        .unwrap();
    let screen = terminal.backend().to_string();
    assert!(screen.contains("demo.service log"));
    assert!(screen.contains("journal output"));
}

#[test]
fn details_worker_returns_result_through_event_queue() {
    let fake = FakeRepository::with_content("", "[Service]\nExecStart=/bin/true", 0);
    let mut app = test_app_with_repository(fake);
    let unit = service("demo.service", "active", "enabled");
    app.status = Status::Details;
    app.latest_details_request_id = 1;
    app.table_service.services = vec![unit.clone()];
    app.table_service.refresh("");
    app.details.update(unit.clone());

    app.spawn_details_worker(1, unit);
    let event = app.event_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    app.handle_event(event).unwrap();

    let backend = TestBackend::new(50, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| app.details.render(frame, frame.area()))
        .unwrap();
    assert!(terminal.backend().to_string().contains("ExecStart"));
}

#[test]
fn stale_log_result_cannot_replace_current_service_log() {
    let mut app = test_app();
    let old_service = service("old.service", "active", "enabled");
    let current_service = service("current.service", "active", "enabled");
    app.table_service.services = vec![old_service, current_service];
    app.table_service.refresh("");
    app.table_service.set_selected_index(1);
    app.status = Status::Log;
    app.latest_log_request_id = 2;

    app.handle_event(AppEvent::Action(Actions::LogLoaded(
        2,
        Ok(("current.service".into(), "current output".into())),
    )))
    .unwrap();
    app.handle_event(AppEvent::Action(Actions::LogLoaded(
        1,
        Ok(("old.service".into(), "stale output".into())),
    )))
    .unwrap();

    let backend = TestBackend::new(60, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| app.service_log.render(frame, frame.area()))
        .unwrap();
    let screen = terminal.backend().to_string();
    assert!(screen.contains("current.service log"));
    assert!(screen.contains("current output"));
    assert!(!screen.contains("stale output"));
}

#[test]
fn stale_details_result_cannot_replace_current_unit_file() {
    let mut app = test_app();
    let old_service = service("old.service", "active", "enabled");
    let current_service = service("current.service", "active", "enabled");
    app.table_service.services = vec![old_service.clone(), current_service.clone()];
    app.table_service.refresh("");
    app.table_service.set_selected_index(1);
    app.status = Status::Details;
    app.latest_details_request_id = 2;
    app.details.update(current_service.clone());

    app.handle_event(AppEvent::Action(Actions::DetailsLoaded(
        2,
        current_service,
        Ok("[Service]\nExecStart=/current".into()),
    )))
    .unwrap();
    app.handle_event(AppEvent::Action(Actions::DetailsLoaded(
        1,
        old_service,
        Ok("[Service]\nExecStart=/stale".into()),
    )))
    .unwrap();

    let backend = TestBackend::new(60, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| app.details.render(frame, frame.area()))
        .unwrap();
    let screen = terminal.backend().to_string();
    assert!(screen.contains("current.service file"));
    assert!(screen.contains("ExecStart=/current"));
    assert!(!screen.contains("ExecStart=/stale"));
}

#[test]
fn service_worker_executes_operation_and_returns_updated_service() {
    let fake = FakeRepository::default();
    let observer = fake.clone();
    let mut app = test_app_with_repository(fake);
    let unit = service("demo.service", "inactive", "disabled");
    app.table_service.services = vec![unit.clone()];
    app.table_service.refresh("");
    app.pending_service_action_request_id = Some(1);

    app.spawn_service_action_worker(1, 0, unit, ServiceAction::Start);
    let event = app.event_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    app.handle_event(event).unwrap();

    assert_eq!(observer.calls(), ["start:demo.service"]);
    assert_eq!(app.table_service.services[0].state().active(), "active");
    assert!(!app.has_pending_service_action());
}

#[test]
fn pending_service_action_blocks_connection_change() {
    let mut app = test_app();
    app.table_service.services = vec![service("demo.service", "inactive", "disabled")];
    app.table_service.refresh("");

    let action_effects = app
        .handle_event(AppEvent::Action(Actions::ServiceAction(
            ServiceAction::Start,
        )))
        .unwrap();
    let connection_effects = app
        .handle_event(AppEvent::Key(KeyEvent::new(
            KeyCode::Right,
            KeyModifiers::NONE,
        )))
        .unwrap();

    assert!(matches!(
        action_effects.as_slice(),
        [AppEffect::RunServiceAction {
            request_id: 1,
            connection_request_id: 0,
            ..
        }]
    ));
    assert!(app.has_pending_service_action());
    assert!(connection_effects.is_empty());
    assert_eq!(app.selected_tab_index, 0);
    assert_eq!(app.latest_connection_request_id, 0);
}

#[test]
fn stale_service_action_result_cannot_repopulate_a_changed_connection() {
    let mut app = test_app();
    app.pending_service_action_request_id = Some(1);
    app.latest_connection_request_id = 2;
    app.connection_pending = true;
    app.table_service.begin_connection_change();

    app.handle_event(AppEvent::Action(Actions::ServiceActionFinished(
        1,
        1,
        Ok(service("old.service", "active", "enabled")),
    )))
    .unwrap();

    assert!(!app.has_pending_service_action());
    assert!(app.table_service.services.is_empty());
    assert!(app.table_service.ignore_key_events);
}

#[test]
fn refresh_action_produces_worker_effect_even_with_empty_list() {
    let mut app = test_app();

    let effects = app
        .handle_event(AppEvent::Action(Actions::ServiceAction(
            ServiceAction::RefreshAll,
        )))
        .unwrap();

    assert!(matches!(
        effects.as_slice(),
        [AppEffect::RefreshServices { .. }]
    ));
}

#[test]
fn changing_tab_produces_async_connection_effect() {
    let mut app = test_app();
    app.table_service.services = vec![service("system.service", "active", "enabled")];
    app.table_service.refresh("");

    let effects = app
        .handle_event(AppEvent::Key(KeyEvent::new(
            KeyCode::Right,
            KeyModifiers::NONE,
        )))
        .unwrap();

    assert_eq!(app.selected_tab_index, 1);
    assert!(app.connection_pending);
    assert!(app.table_service.services.is_empty());
    assert!(app.table_service.get_selected_service().is_none());
    assert!(app.table_service.ignore_key_events);
    assert_eq!(
        effects,
        [AppEffect::ChangeConnection(ConnectionRequest {
            id: 1,
            target: ConnectionType::Session,
        })]
    );
}

#[test]
fn connection_worker_processes_rapid_tab_changes_in_request_order() {
    let fake = FakeRepository::default();
    let observer = fake.clone();
    let mut app = test_app_with_repository(fake);

    let first = app.next_connection_request(ConnectionType::Session);
    let second = app.next_connection_request(ConnectionType::System);
    app.connection_tx.send(first).unwrap();
    app.connection_tx.send(second).unwrap();

    let first_result = app.event_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    let second_result = app.event_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(app.handle_event(first_result).unwrap().is_empty());
    assert!(matches!(
        app.handle_event(second_result).unwrap().as_slice(),
        [AppEffect::RefreshServices { .. }]
    ));
    assert_eq!(
        observer.calls(),
        ["connection:session", "connection:system"]
    );
}

#[test]
fn stale_connection_result_cannot_trigger_a_refresh() {
    let mut app = test_app();
    app.latest_connection_request_id = 2;

    let stale_effects = app
        .handle_event(AppEvent::Action(Actions::ConnectionChanged(
            1,
            ConnectionType::Session,
            Ok(()),
        )))
        .unwrap();
    assert_eq!(app.active_connection, ConnectionType::Session);
    let current_effects = app
        .handle_event(AppEvent::Action(Actions::ConnectionChanged(
            2,
            ConnectionType::System,
            Ok(()),
        )))
        .unwrap();

    assert!(stale_effects.is_empty());
    assert!(matches!(
        current_effects.as_slice(),
        [AppEffect::RefreshServices { .. }]
    ));
    assert_eq!(app.active_connection, ConnectionType::System);
}

#[test]
fn failed_latest_connection_restores_last_successful_intermediate_target() {
    let mut app = test_app();
    app.latest_connection_request_id = 2;
    app.selected_tab_index = 0;
    app.connection_pending = true;

    let intermediate_effects = app
        .handle_event(AppEvent::Action(Actions::ConnectionChanged(
            1,
            ConnectionType::Session,
            Ok(()),
        )))
        .unwrap();
    let latest_effects = app
        .handle_event(AppEvent::Action(Actions::ConnectionChanged(
            2,
            ConnectionType::System,
            Err("system connection failed".into()),
        )))
        .unwrap();

    assert!(intermediate_effects.is_empty());
    assert_eq!(app.active_connection, ConnectionType::Session);
    assert_eq!(app.selected_tab_index, 1);
    assert_eq!(
        app.error_message.as_deref(),
        Some("system connection failed")
    );
    assert!(matches!(
        latest_effects.as_slice(),
        [AppEffect::RefreshServices {
            connection_request_id: Some(2),
            ..
        }]
    ));
}

#[test]
fn service_actions_are_ignored_while_connection_is_pending() {
    let mut app = test_app();
    app.connection_pending = true;
    app.table_service.set_ignore_key_events(true);

    let effects = app
        .handle_event(AppEvent::Action(Actions::ServiceAction(
            ServiceAction::Start,
        )))
        .unwrap();

    assert!(effects.is_empty());
    assert!(app.connection_pending);
    assert!(app.table_service.ignore_key_events);
}

#[test]
fn pending_connection_accepts_only_its_own_service_list() {
    let mut app = test_app();
    app.latest_connection_request_id = 2;
    app.latest_services_request_id = 2;
    app.connection_pending = true;
    app.table_service.begin_connection_change();

    app.handle_event(AppEvent::Action(Actions::ServicesLoaded(
        1,
        Ok(vec![service("old.service", "active", "enabled")]),
        String::new(),
        Some(1),
    )))
    .unwrap();
    app.handle_event(AppEvent::Action(Actions::ServicesLoaded(
        1,
        Ok(vec![service("unrelated.service", "active", "enabled")]),
        String::new(),
        None,
    )))
    .unwrap();

    assert!(app.connection_pending);
    assert!(app.table_service.services.is_empty());
    assert!(app.table_service.ignore_key_events);

    app.handle_event(AppEvent::Action(Actions::ServicesLoaded(
        2,
        Ok(vec![service("current.service", "active", "enabled")]),
        String::new(),
        Some(2),
    )))
    .unwrap();

    assert!(!app.connection_pending);
    assert_eq!(app.table_service.services[0].name(), "current.service");
    assert!(!app.table_service.ignore_key_events);
}

#[test]
fn stale_refresh_cannot_overwrite_newer_service_list() {
    let mut app = test_app();
    app.latest_services_request_id = 2;

    app.handle_event(AppEvent::Action(Actions::ServicesLoaded(
        2,
        Ok(vec![service("new.service", "active", "enabled")]),
        "new".into(),
        None,
    )))
    .unwrap();
    app.handle_event(AppEvent::Action(Actions::ServicesLoaded(
        1,
        Ok(vec![service("old.service", "active", "enabled")]),
        "old".into(),
        None,
    )))
    .unwrap();

    assert_eq!(app.table_service.services[0].name(), "new.service");
    assert_eq!(app.table_service.refresh_parameters().1, "new");
}

#[test]
fn stale_unit_file_states_cannot_overwrite_newer_states() {
    let mut app = test_app();
    let unit = service("demo.service", "active", "disabled");
    app.latest_services_request_id = 2;

    app.handle_event(AppEvent::Action(Actions::UnitFileStatesLoaded(
        2,
        None,
        Ok(HashMap::from([(
            "demo.service".to_string(),
            "enabled".to_string(),
        )])),
    )))
    .unwrap();
    app.handle_event(AppEvent::Action(Actions::UnitFileStatesLoaded(
        1,
        None,
        Ok(HashMap::from([(
            "demo.service".to_string(),
            "masked".to_string(),
        )])),
    )))
    .unwrap();

    assert_eq!(app.table_service.file_state_for(&unit), "enabled");
}

#[test]
fn failed_connection_restores_active_tab_and_refreshes_before_unlocking() {
    let mut app = test_app();
    app.selected_tab_index = 1;
    app.latest_connection_request_id = 1;
    app.connection_pending = true;
    app.table_service.begin_connection_change();

    let effects = app
        .handle_event(AppEvent::Action(Actions::ConnectionChanged(
            1,
            ConnectionType::Session,
            Err("connection failed".into()),
        )))
        .unwrap();

    assert_eq!(app.selected_tab_index, 0);
    assert!(app.connection_pending);
    assert!(app.table_service.ignore_key_events);
    assert_eq!(app.error_message.as_deref(), Some("connection failed"));
    assert!(matches!(
        effects.as_slice(),
        [AppEffect::RefreshServices {
            connection_request_id: Some(1),
            ..
        }]
    ));
}
