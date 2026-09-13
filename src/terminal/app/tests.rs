use super::*;
use crate::test_support::FakeRepository;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::sync::mpsc;

fn test_app() -> App {
    test_app_with_repository(FakeRepository::default()).0
}

fn test_app_with_repository(repository: FakeRepository) -> (App, FakeRepository) {
    let (event_tx, event_rx) = mpsc::channel();
    let observer = repository.clone();
    let manager = Rc::new(RefCell::new(ServicesManager::new(Box::new(repository))));
    let table = TableServices::new(event_tx.clone(), manager.clone());
    let filter = Filter::new(event_tx.clone(), String::new());
    let log = ServiceLog::new(event_tx.clone(), manager.clone());
    let details = ServiceDetails::new(event_tx.clone(), manager.clone());

    (
        App::new(event_tx, event_rx, table, filter, log, details, manager),
        observer,
    )
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
fn connection_change_commits_active_context_only_after_success() {
    let (mut app, observer) = test_app_with_repository(FakeRepository::default());

    app.update_connection_and_reset(1);

    assert_eq!(app.active_connection, ConnectionType::Session);
    assert_eq!(app.selected_tab_index, 1);
    assert_eq!(observer.calls(), ["connection:session"]);
}

#[test]
fn failed_connection_change_preserves_previous_active_context() {
    let repository = FakeRepository::default();
    repository.fail("connection");
    let (mut app, observer) = test_app_with_repository(repository);

    app.update_connection_and_reset(1);

    assert_eq!(app.active_connection, ConnectionType::System);
    assert_eq!(app.selected_tab_index, 0);
    assert_eq!(observer.calls(), ["connection:session"]);
    assert!(matches!(app.event_rx.recv().unwrap(), AppEvent::Error(_)));
}

#[test]
fn translates_each_supported_dbus_error_family() {
    for (error, expected) in [
        (
            "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired",
            "permission",
        ),
        ("org.freedesktop.DBus.Error.ServiceUnknown", "not available"),
        ("org.freedesktop.DBus.Error.NoReply", "did not respond"),
        ("org.freedesktop.systemd1.NoSuchUnit", "doesn't exist"),
    ] {
        assert!(get_user_friendly_error(error).contains(expected));
    }
}

#[test]
fn help_popup_is_rendered_with_sections_and_highlighted_title() {
    let app = test_app();
    let backend = TestBackend::new(100, 45);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| app.draw_help_popup(frame, frame.area()))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("SYSTEMD MANAGER TUI - HELP"));
    assert!(rendered.contains("Navigation:"));
    assert!(rendered.contains("Service Control:"));
    assert!(rendered.contains("Ctrl+c - Quit"));
    assert!(
        buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() == "S" && cell.fg == Color::Cyan)
    );
}

#[test]
fn shortcuts_panel_renders_context_actions_and_global_exit() {
    let app = test_app();
    let backend = TestBackend::new(70, 7);
    let mut terminal = Terminal::new(backend).unwrap();
    let shortcuts = vec![Line::from("Custom action: x")];

    terminal
        .draw(|frame| app.draw_shortcuts(frame, frame.area(), &shortcuts))
        .unwrap();

    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("Shortcuts"));
    assert!(rendered.contains("Custom action: x"));
    assert!(rendered.contains("Exit: Ctrl + c"));
}

#[test]
fn ctrl_c_hides_the_cursor_and_stops_the_app() {
    let mut app = test_app();
    let backend = TestBackend::new(20, 5);
    let expected_hidden_backend = backend.clone();
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.show_cursor().unwrap();

    let handled = app
        .handle_quit_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut terminal,
        )
        .unwrap();

    assert!(handled);
    assert!(!app.running);
    assert_eq!(terminal.backend(), &expected_hidden_backend);
}

#[test]
fn ctrl_shift_c_also_uses_the_controlled_shutdown_path() {
    let mut app = test_app();
    let backend = TestBackend::new(20, 5);
    let expected_hidden_backend = backend.clone();
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.show_cursor().unwrap();

    let handled = app
        .handle_quit_key(
            KeyEvent::new(
                KeyCode::Char('C'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            &mut terminal,
        )
        .unwrap();

    assert!(handled);
    assert!(!app.running);
    assert_eq!(terminal.backend(), &expected_hidden_backend);
}

#[test]
fn plain_c_does_not_stop_the_app_or_change_cursor_visibility() {
    let mut app = test_app();
    let backend = TestBackend::new(20, 5);
    let mut visible_backend = backend.clone();
    visible_backend.show_cursor().unwrap();
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.show_cursor().unwrap();

    let handled = app
        .handle_quit_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
            &mut terminal,
        )
        .unwrap();

    assert!(!handled);
    assert!(app.running);
    assert_eq!(terminal.backend(), &visible_backend);
}
