use super::*;
use crate::test_support::FakeRepository;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::sync::mpsc;

fn test_app() -> App {
    let (event_tx, event_rx) = mpsc::channel();
    let manager = Rc::new(RefCell::new(ServicesManager::new(Box::new(
        FakeRepository::default(),
    ))));
    let table = TableServices::new(event_tx.clone(), manager.clone());
    let filter = Filter::new(event_tx.clone(), String::new());
    let log = ServiceLog::new(event_tx.clone(), manager.clone());
    let details = ServiceDetails::new(event_tx.clone(), manager.clone());

    App::new(
        event_tx, event_rx, table, filter, log, details, manager,
    )
}

#[test]
fn translates_known_dbus_errors() {
    let message = get_user_friendly_error(
        "org.freedesktop.DBus.Error.AccessDenied: rejected",
    );

    assert!(message.contains("Access denied"));
}

#[test]
fn preserves_unknown_errors() {
    assert_eq!(get_user_friendly_error("custom failure"), "custom failure");
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
    assert!(buffer
        .content()
        .iter()
        .any(|cell| cell.symbol() == "S" && cell.fg == Color::Cyan));
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

