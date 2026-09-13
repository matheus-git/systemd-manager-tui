use super::*;
use crate::infrastructure::systemd_service_adapter::ConnectionType;
use crate::test_support::{FakeRepository, service};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::sync::mpsc;

fn request_context(name: &str) -> ServiceRequestContext {
    ServiceRequestContext {
        connection: ConnectionType::System,
        service_name: name.to_string(),
    }
}

fn log_with(repository: FakeRepository) -> (ServiceLog, mpsc::Receiver<AppEvent>) {
    let (sender, receiver) = mpsc::channel();
    let manager = ServicesManager::new(Box::new(repository));
    (
        ServiceLog::new(sender, Rc::new(RefCell::new(manager))),
        receiver,
    )
}

fn rendered_text(log: &mut ServiceLog, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| log.render(frame, frame.area()))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn empty_log_renders_loading_state() {
    let (mut log, _receiver) = log_with(FakeRepository::default());

    assert!(rendered_text(&mut log, 40, 8).contains("Loading..."));
}

#[test]
fn renders_service_name_and_latest_log_lines() {
    let (mut log, _receiver) = log_with(FakeRepository::default());
    log.update("demo.service".into(), "first\nsecond\nthird".into());

    let screen = rendered_text(&mut log, 40, 5);

    assert!(screen.contains("demo.service log"));
    assert!(screen.contains("first"));
    assert!(screen.contains("third"));
}

#[test]
fn fetch_dispatches_log_update_event() {
    let fake = FakeRepository::with_content("journal output", "", 0);
    let observer = fake.clone();
    let (mut log, receiver) = log_with(fake);
    let unit = service("demo.service", "active", "enabled");

    log.fetch_log_and_dispatch(request_context(unit.name()));

    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::UpdateLog(context, content))
            if context == request_context("demo.service") && content == "journal output"
    ));
    assert_eq!(observer.calls(), ["log:demo.service"]);
}

#[test]
fn log_failure_is_reported_without_panicking() {
    let fake = FakeRepository::default();
    fake.fail("log");
    let (mut log, receiver) = log_with(fake);

    log.fetch_log_and_dispatch(request_context("broken.service"));

    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Error(message)
            if message.contains("broken.service") && message.contains("log failed")
    ));
}

#[test]
fn auto_refresh_changes_border_and_shortcut_label() {
    let (mut log, _receiver) = log_with(FakeRepository::default());

    log.set_auto_refresh(true);

    assert!(matches!(log.border_color, BorderColor::Orange));
    let shortcuts = log.shortcuts();
    assert!(
        shortcuts
            .iter()
            .any(|line| line.to_string().contains("Disable auto-refresh"))
    );
    log.set_auto_refresh(false);
}

#[test]
fn auto_refresh_state_is_visible_in_rendered_border_color() {
    let (mut log, _receiver) = log_with(FakeRepository::default());
    log.update("demo.service".into(), "journal output".into());
    log.set_auto_refresh(true);
    let backend = TestBackend::new(40, 5);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| log.render(frame, frame.area()))
        .unwrap();

    assert_eq!(
        terminal.backend().buffer().content()[0].fg,
        Color::Rgb(255, 165, 0)
    );
    log.set_auto_refresh(false);
}

#[test]
fn auto_refresh_owns_exactly_one_stoppable_worker() {
    let (mut log, _receiver) = log_with(FakeRepository::default());
    let toggle = KeyEvent::new(KeyCode::Char('a'), crossterm::event::KeyModifiers::NONE);

    log.on_key_event(toggle);
    assert!(log.auto_refresh_worker.is_some());
    assert!(*log.auto_refresh.lock().unwrap());

    log.on_key_event(toggle);
    assert!(log.auto_refresh_worker.is_none());
    assert!(!*log.auto_refresh.lock().unwrap());
}

#[test]
fn scrolling_changes_the_visible_log_window() {
    let (mut log, _receiver) = log_with(FakeRepository::default());
    log.update(
        "demo.service".into(),
        "line1\nline2\nline3\nline4\nline5\nline6".into(),
    );

    let latest = rendered_text(&mut log, 40, 4);
    assert!(latest.contains("line5"));
    assert!(latest.contains("line6"));
    assert!(!latest.contains("line4"));

    log.on_key_event(KeyEvent::new(
        KeyCode::Up,
        crossterm::event::KeyModifiers::NONE,
    ));
    let older = rendered_text(&mut log, 40, 4);
    assert!(older.contains("line4"));
    assert!(older.contains("line5"));
    assert!(!older.contains("line6"));
}

#[test]
fn leaving_log_resets_state_and_returns_to_list() {
    let (mut log, receiver) = log_with(FakeRepository::default());
    log.update("demo.service".into(), "journal output".into());
    log.set_auto_refresh(true);
    log.scroll = 10;

    log.on_key_event(KeyEvent::new(
        KeyCode::Esc,
        crossterm::event::KeyModifiers::NONE,
    ));

    assert!(log.log.is_empty());
    assert_eq!(log.scroll, 0);
    assert!(!*log.auto_refresh.lock().unwrap());
    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::GoList)
    ));
}

#[test]
fn switching_from_log_resets_content_and_opens_details() {
    let (mut log, receiver) = log_with(FakeRepository::default());
    log.update("demo.service".into(), "journal output".into());
    log.scroll = 3;

    log.on_key_event(KeyEvent::new(
        KeyCode::Right,
        crossterm::event::KeyModifiers::NONE,
    ));

    assert!(log.log.is_empty());
    assert_eq!(log.scroll, 0);
    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::GoDetails)
    ));
}

#[test]
fn downward_scrolling_saturates_at_zero() {
    let (mut log, _receiver) = log_with(FakeRepository::default());

    log.on_key_event(KeyEvent::new(
        KeyCode::Down,
        crossterm::event::KeyModifiers::NONE,
    ));
    log.on_key_event(KeyEvent::new(
        KeyCode::PageDown,
        crossterm::event::KeyModifiers::NONE,
    ));

    assert_eq!(log.scroll, 0);
}

#[test]
fn long_log_lines_wrap_to_available_width() {
    let (mut log, _receiver) = log_with(FakeRepository::default());
    log.update("demo.service".into(), "abcdefghijklmnopqrst".into());

    let screen = rendered_text(&mut log, 12, 4);

    assert!(screen.contains("abcdefghij"));
    assert!(screen.contains("klmnopqrst"));
}
