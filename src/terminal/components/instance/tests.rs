use super::*;
use crate::test_support::service;
use ratatui::{Terminal, backend::TestBackend};

fn prompt() -> InstancePrompt {
    InstancePrompt::new(service("worker@.service", "inactive", "disabled"), "Start")
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn invalid_input_stays_open_and_can_be_corrected() {
    let mut prompt = prompt();
    assert!(matches!(
        prompt.on_key(key(KeyCode::Enter)),
        PromptResult::Editing
    ));
    assert!(prompt.error.is_some());
    prompt.on_key(key(KeyCode::Char('a')));
    prompt.on_key(key(KeyCode::Char('/')));
    assert!(matches!(
        prompt.on_key(key(KeyCode::Enter)),
        PromptResult::Editing
    ));
    prompt.on_key(key(KeyCode::Backspace));
    let PromptResult::Submitted(unit) = prompt.on_key(key(KeyCode::Enter)) else {
        panic!("expected instance");
    };
    assert_eq!(unit.name(), "worker@a.service");
}

#[test]
fn editing_handles_unicode_and_cursor_navigation() {
    let mut prompt = prompt();
    for c in "aéb".chars() {
        prompt.on_key(key(KeyCode::Char(c)));
    }
    prompt.on_key(key(KeyCode::Left));
    prompt.on_key(key(KeyCode::Backspace));
    prompt.on_key(key(KeyCode::Char('x')));
    prompt.on_key(key(KeyCode::Home));
    prompt.on_key(key(KeyCode::Delete));
    prompt.on_key(key(KeyCode::End));
    prompt.on_key(key(KeyCode::Char('z')));
    assert_eq!(prompt.input, "xbz");
    assert!(matches!(
        prompt.on_key(key(KeyCode::Esc)),
        PromptResult::Cancelled
    ));
}

#[test]
fn renders_target_action_and_validation_error() {
    let mut prompt = prompt();
    prompt.on_key(key(KeyCode::Char('a')));
    let mut terminal = Terminal::new(TestBackend::new(90, 20)).unwrap();
    terminal
        .draw(|frame| prompt.render(frame, frame.area()))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Start instance"));
    assert!(text.contains("worker@a.service"));
    assert!(text.contains("Esc: cancel"));
    prompt.on_key(key(KeyCode::Backspace));
    prompt.on_key(key(KeyCode::Enter));
    terminal
        .draw(|frame| prompt.render(frame, frame.area()))
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Enter an instance name"));
    let mut tiny = Terminal::new(TestBackend::new(2, 2)).unwrap();
    tiny.draw(|frame| prompt.render(frame, frame.area()))
        .unwrap();
}
