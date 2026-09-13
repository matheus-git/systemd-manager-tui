use super::*;
use crate::test_support::{service, FakeRepository};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::sync::mpsc;

fn details_with(repository: FakeRepository) -> (ServiceDetails, mpsc::Receiver<AppEvent>) {
    let (sender, receiver) = mpsc::channel();
    let manager = ServicesManager::new(Box::new(repository));
    (ServiceDetails::new(sender, Rc::new(RefCell::new(manager))), receiver)
}

fn rendered_text(details: &mut ServiceDetails, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| details.render(frame, frame.area())).unwrap();
    terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn fetches_and_renders_a_unit_file() {
    let fake = FakeRepository::with_content("", "[Unit]\nDescription=Demo\n# comment", 0);
    let observer = fake.clone();
    let (mut details, _receiver) = details_with(fake);
    details.update(service("demo.service", "active", "enabled"));

    details.fetch_unit_file();
    let screen = rendered_text(&mut details, 60, 8);

    assert!(screen.contains("demo.service file"));
    assert!(screen.contains("[Unit]"));
    assert!(screen.contains("Description=Demo"));
    assert!(screen.contains("# comment"));
    assert_eq!(observer.calls(), ["cat:demo.service"]);
}

#[test]
fn reports_repository_error_as_app_event() {
    let fake = FakeRepository::default();
    fake.fail("cat");
    let (mut details, receiver) = details_with(fake);
    details.update(service("broken.service", "active", "enabled"));

    details.fetch_unit_file();

    assert!(matches!(receiver.recv().unwrap(), AppEvent::Error(message) if message == "cat failed"));
}

#[test]
fn navigation_updates_scroll_and_emits_actions() {
    let (mut details, receiver) = details_with(FakeRepository::default());
    details.on_key_event(KeyEvent::new(KeyCode::PageDown, crossterm::event::KeyModifiers::NONE));
    assert_eq!(details.scroll, 10);

    details.on_key_event(KeyEvent::new(KeyCode::Char('e'), crossterm::event::KeyModifiers::NONE));
    assert!(matches!(receiver.recv().unwrap(), AppEvent::Action(Actions::EditCurrentService)));
}

#[test]
fn unit_file_rendering_applies_systemd_syntax_colors() {
    let fake = FakeRepository::with_content(
        "",
        "[Service]\nDescription=Demo\n# comment\n; another comment",
        0,
    );
    let (mut details, _receiver) = details_with(fake);
    details.update(service("demo.service", "active", "enabled"));
    details.fetch_unit_file();
    let backend = TestBackend::new(60, 8);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| details.render(frame, frame.area()))
        .unwrap();

    let cells = terminal.backend().buffer().content();
    assert!(cells
        .iter()
        .any(|cell| cell.symbol() == "[" && cell.fg == Color::LightBlue));
    assert!(cells
        .iter()
        .any(|cell| cell.symbol() == "D" && cell.fg == Color::Yellow));
    assert!(cells
        .iter()
        .any(|cell| cell.symbol() == "#" && cell.fg == Color::Green));
    assert!(cells
        .iter()
        .any(|cell| cell.symbol() == ";" && cell.fg == Color::Green));
}

#[test]
fn leaving_details_clears_content_and_returns_to_list() {
    let fake = FakeRepository::with_content("", "[Service]\nExecStart=/bin/true", 0);
    let (mut details, receiver) = details_with(fake);
    details.update(service("demo.service", "active", "enabled"));
    details.fetch_unit_file();
    details.scroll = 12;

    details.on_key_event(KeyEvent::new(
        KeyCode::Esc,
        crossterm::event::KeyModifiers::NONE,
    ));

    assert!(details.service.is_none());
    assert!(details.unit_file.is_empty());
    assert_eq!(details.scroll, 0);
    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::GoList)
    ));
}

#[test]
fn switching_from_details_clears_content_and_opens_log() {
    let fake = FakeRepository::with_content("", "old content", 0);
    let (mut details, receiver) = details_with(fake);
    details.update(service("demo.service", "active", "enabled"));
    details.fetch_unit_file();

    details.on_key_event(KeyEvent::new(
        KeyCode::Right,
        crossterm::event::KeyModifiers::NONE,
    ));

    assert!(details.service.is_none());
    assert!(details.unit_file.is_empty());
    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::GoLog)
    ));
}

#[test]
fn fetching_without_selected_service_is_a_noop() {
    let fake = FakeRepository::default();
    let observer = fake.clone();
    let (mut details, receiver) = details_with(fake);

    details.fetch_unit_file();

    assert!(details.unit_file.is_empty());
    assert!(observer.calls().is_empty());
    assert!(receiver.try_recv().is_err());
}

#[test]
fn upward_scrolling_saturates_at_zero() {
    let (mut details, _receiver) = details_with(FakeRepository::default());

    details.on_key_event(KeyEvent::new(
        KeyCode::Up,
        crossterm::event::KeyModifiers::NONE,
    ));
    details.on_key_event(KeyEvent::new(
        KeyCode::PageUp,
        crossterm::event::KeyModifiers::NONE,
    ));

    assert_eq!(details.scroll, 0);
}

