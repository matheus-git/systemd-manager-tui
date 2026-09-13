use super::*;
use ratatui::Terminal;
use ratatui::backend::{Backend, TestBackend};
use std::sync::mpsc::{self, Receiver};

fn filter_with_input(input: &str) -> (Filter, Receiver<AppEvent>) {
    let (sender, receiver) = mpsc::channel();
    (Filter::new(sender, input.to_string()), receiver)
}

#[test]
fn renders_filter_and_cursor_to_test_backend() {
    let (mut filter, _receiver) = filter_with_input("docker");
    filter.input_mode = InputMode::Editing;
    let backend = TestBackend::new(40, 4);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| filter.draw(frame, frame.area()))
        .unwrap();

    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("docker"));
    assert_eq!(
        terminal.backend_mut().get_cursor_position().unwrap(),
        Position::new(7, 2)
    );
}

#[test]
fn entering_edit_mode_locks_list_input() {
    let (mut filter, receiver) = filter_with_input("");

    filter.on_key_event(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));

    assert!(filter.input_mode == InputMode::Editing);
    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::UpdateIgnoreListKeys(true))
    ));
}

#[test]
fn editing_dispatches_live_filter_and_submit_unlocks_list() {
    let (mut filter, receiver) = filter_with_input("docker");
    filter.input_mode = InputMode::Editing;

    filter.on_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    assert_eq!(filter.input, "dockerd");
    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::Filter(value)) if value == "dockerd"
    ));

    filter.on_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(filter.input_mode == InputMode::Normal);
    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::Filter(value)) if value == "dockerd"
    ));
    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::UpdateIgnoreListKeys(false))
    ));
}

#[test]
fn ascii_word_and_range_editing_commands_preserve_cursor_invariants() {
    let (mut filter, _receiver) = filter_with_input("alpha beta");

    filter.character_index = filter.input.len();
    filter.move_cursor_prev_word();
    assert_eq!(filter.character_index, 6);
    filter.move_cursor_next_word();
    assert_eq!(filter.character_index, 10);

    filter.delete_prev_word();
    assert_eq!(filter.input, "alpha ");
    assert_eq!(filter.character_index, 6);

    filter.input = "alpha beta".into();
    filter.character_index = 5;
    filter.delete_from_cursor();
    assert_eq!(filter.input, "alpha");

    filter.input = "alpha beta".into();
    filter.character_index = 6;
    filter.delete_to_start();
    assert_eq!(filter.input, "beta");
    assert_eq!(filter.character_index, 0);
}

#[test]
fn editing_mode_changes_visual_help_and_input_color() {
    let (mut filter, _receiver) = filter_with_input("docker");
    filter.input_mode = InputMode::Editing;
    let backend = TestBackend::new(50, 4);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| filter.draw(frame, frame.area()))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let rendered = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("Esc"));
    assert!(rendered.contains("Enter"));
    assert!(
        buffer
            .content()
            .iter()
            .any(|cell| cell.symbol() == "d" && cell.fg == Color::Yellow)
    );
}

#[test]
fn escape_in_normal_mode_clears_filter_and_unlocks_list() {
    let (mut filter, receiver) = filter_with_input("docker");

    filter.on_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert!(filter.input.is_empty());
    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::Filter(value)) if value.is_empty()
    ));
    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::UpdateIgnoreListKeys(false))
    ));
}

#[test]
fn escape_in_edit_mode_preserves_text_and_stops_editing() {
    let (mut filter, receiver) = filter_with_input("docker");
    filter.input_mode = InputMode::Editing;

    filter.on_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert_eq!(filter.input, "docker");
    assert!(filter.input_mode == InputMode::Normal);
    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::UpdateIgnoreListKeys(false))
    ));
    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::Filter(value)) if value == "docker"
    ));
}

#[test]
fn release_events_do_not_edit_or_dispatch_filter() {
    let (mut filter, receiver) = filter_with_input("docker");
    filter.input_mode = InputMode::Editing;
    let mut key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    key.kind = KeyEventKind::Release;

    filter.on_key_event(key);

    assert_eq!(filter.input, "docker");
    assert!(receiver.try_recv().is_err());
}

#[test]
fn backspace_deletes_character_before_cursor() {
    let (mut filter, receiver) = filter_with_input("abcd");
    filter.input_mode = InputMode::Editing;
    filter.character_index = 2;

    filter.on_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

    assert_eq!(filter.input, "acd");
    assert_eq!(filter.character_index, 1);
    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::Filter(value)) if value == "acd"
    ));
}
