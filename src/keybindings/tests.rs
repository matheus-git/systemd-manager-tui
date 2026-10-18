use super::loader::parse;
use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn chord(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

#[test]
fn every_built_in_binding_parses() {
    for context in [
        Context::Global,
        Context::List,
        Context::Log,
        Context::UnitFile,
    ] {
        for (action, specs) in context.defaults() {
            for spec in *specs {
                assert!(
                    KeyBind::parse(spec).is_ok(),
                    "default binding '{spec}' for `{}` does not parse",
                    action.name()
                );
            }
        }
    }
}

#[test]
fn built_in_bindings_do_not_conflict() {
    let keybindings = KeyBindings::default();

    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Char('s'))),
        Some(Action::Start)
    );
    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Down)),
        Some(Action::MoveDown)
    );
    assert_eq!(
        keybindings.action(Context::Log, press(KeyCode::Char('a'))),
        Some(Action::ToggleAutoRefresh)
    );
    assert_eq!(
        keybindings.action(Context::UnitFile, press(KeyCode::Char('e'))),
        Some(Action::EditUnit)
    );
    assert_eq!(
        keybindings.action(
            Context::Global,
            chord(KeyCode::Char('c'), KeyModifiers::CONTROL)
        ),
        Some(Action::Quit)
    );
}

#[test]
fn the_shipped_template_only_restates_the_defaults() {
    let from_template = parse(DEFAULT_CONFIG_TEMPLATE).expect("template is valid");
    let defaults = KeyBindings::default();

    for context in [
        Context::Global,
        Context::List,
        Context::Log,
        Context::UnitFile,
    ] {
        for (action, _) in UserBindings::scopes(context)
            .iter()
            .flat_map(|scope| scope.defaults())
        {
            assert_eq!(
                from_template.keys(context, *action),
                defaults.keys(context, *action),
                "`{}` differs from its default in [{}]",
                action.name(),
                context.section()
            );
        }
    }
}

#[test]
fn an_empty_configuration_keeps_every_default() {
    assert_eq!(parse("").unwrap(), KeyBindings::default());
}

#[test]
fn a_character_override_replaces_the_default_character() {
    let keybindings = parse("[keybindings.list]\nstart = \"S\"\n").unwrap();

    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Char('S'))),
        Some(Action::Start)
    );
    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Char('s'))),
        None
    );
}

#[test]
fn a_freed_character_can_be_taken_by_another_action() {
    let keybindings =
        parse("[keybindings.list]\nstart = \"S\"\nstop = \"s\"\nview_log = \"x\"\n").unwrap();

    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Char('s'))),
        Some(Action::Stop)
    );
    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Char('x'))),
        Some(Action::ViewLog)
    );
}

#[test]
fn named_keys_survive_a_character_override() {
    let keybindings = parse("[keybindings]\nmove_down = \"n\"\n").unwrap();

    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Char('n'))),
        Some(Action::MoveDown)
    );
    // The arrow key is kept, the replaced character is not.
    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Down)),
        Some(Action::MoveDown)
    );
    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Char('j'))),
        None
    );
}

#[test]
fn the_first_key_listed_is_the_primary_one() {
    let keybindings = parse("[keybindings]\nmove_down = \"n\"\n").unwrap();

    assert_eq!(
        keybindings.primary_label(Context::List, Action::MoveDown),
        "n"
    );
    assert_eq!(keybindings.label(Context::List, Action::MoveDown), "n/↓");
}

#[test]
fn binding_a_named_key_elsewhere_takes_it_from_its_default_action() {
    let keybindings = parse("[keybindings]\npage_down = \"down\"\n").unwrap();

    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Down)),
        Some(Action::PageDown)
    );
    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Char('j'))),
        Some(Action::MoveDown)
    );
}

#[test]
fn unbind_drops_a_key_entirely() {
    let keybindings = parse("[keybindings]\nunbind = [\"down\", \"j\"]\n").unwrap();

    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Down)),
        None
    );
    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Char('j'))),
        None
    );
    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Up)),
        Some(Action::MoveUp)
    );
}

#[test]
fn an_empty_list_leaves_an_action_unbound() {
    let keybindings = parse("[keybindings.list]\nhelp = []\n").unwrap();

    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Char('?'))),
        None
    );
    assert_eq!(keybindings.keys(Context::List, Action::Help), Vec::new());
    assert_eq!(keybindings.label(Context::List, Action::Help), "unbound");
}

#[test]
fn the_same_key_may_mean_different_things_in_different_views() {
    let keybindings = parse("[keybindings.log]\ntoggle_auto_refresh = \"r\"\n").unwrap();

    assert_eq!(
        keybindings.action(Context::Log, press(KeyCode::Char('r'))),
        Some(Action::ToggleAutoRefresh)
    );
    assert_eq!(
        keybindings.action(Context::List, press(KeyCode::Char('r'))),
        Some(Action::Restart)
    );
}

#[test]
fn two_actions_on_one_key_is_rejected() {
    let error = parse("[keybindings.list]\nstart = \"g\"\nstop = \"g\"\n").unwrap_err();

    assert!(error.contains("'g' is bound to both"), "{error}");
    assert!(error.contains("start"), "{error}");
    assert!(error.contains("stop"), "{error}");
}

#[test]
fn taking_over_a_default_character_without_moving_it_is_rejected() {
    let error = parse("[keybindings.list]\nstart = \"j\"\n").unwrap_err();

    assert!(error.contains("move_down"), "{error}");
    assert!(error.contains("default key"), "{error}");
}

#[test]
fn a_global_binding_is_checked_against_every_view() {
    let error = parse("[keybindings]\nquit = \"q\"\n").unwrap_err();

    assert!(error.contains("keybindings.log"), "{error}");
    assert!(error.contains("back"), "{error}");
}

#[test]
fn unbinding_and_binding_the_same_key_is_rejected() {
    let error = parse("[keybindings.list]\nunbind = [\"g\"]\nstart = \"g\"\n").unwrap_err();

    assert!(error.contains("unbind"), "{error}");
}

#[test]
fn an_unknown_action_names_the_valid_ones() {
    let error = parse("[keybindings.list]\nstat = \"s\"\n").unwrap_err();

    assert!(error.contains("unknown action `stat`"), "{error}");
    assert!(error.contains("start"), "{error}");
}

#[test]
fn an_action_used_in_the_wrong_section_says_where_it_belongs() {
    let error = parse("[keybindings.list]\nedit = \"e\"\n").unwrap_err();

    assert!(error.contains("keybindings.unit_file"), "{error}");
}

#[test]
fn an_unknown_section_is_rejected() {
    let error = parse("[keybindings.detials]\nedit = \"e\"\n").unwrap_err();

    assert!(error.contains("unknown section"), "{error}");
}

#[test]
fn an_unknown_key_name_is_rejected() {
    let error = parse("[keybindings.list]\nstart = \"supr\"\n").unwrap_err();

    assert!(error.contains("unknown key 'supr'"), "{error}");
}

#[test]
fn a_wrong_value_type_is_rejected() {
    let error = parse("[keybindings.list]\nstart = 3\n").unwrap_err();

    assert!(error.contains("must be a key"), "{error}");
}

#[test]
fn an_unknown_top_level_section_is_rejected() {
    let error = parse("[keybinding]\nquit = \"ctrl+q\"\n").unwrap_err();

    assert!(error.contains("keybinding"), "{error}");
}

#[test]
fn modifiers_are_parsed_and_matched() {
    let keybindings = parse("[keybindings]\nsuspend = \"alt+shift+f5\"\n").unwrap();

    assert_eq!(
        keybindings.action(
            Context::Global,
            chord(KeyCode::F(5), KeyModifiers::ALT | KeyModifiers::SHIFT)
        ),
        Some(Action::Suspend)
    );
    assert_eq!(
        keybindings.action(Context::Global, chord(KeyCode::F(5), KeyModifiers::ALT)),
        None
    );
}

#[test]
fn a_plain_binding_does_not_match_a_chord() {
    let keybindings = KeyBindings::default();

    assert_eq!(
        keybindings.action(Context::List, chord(KeyCode::Char('s'), KeyModifiers::ALT)),
        None
    );
}

#[test]
fn a_chord_ignores_the_case_reported_by_the_terminal() {
    let keybindings = KeyBindings::default();

    for modifiers in [
        KeyModifiers::CONTROL,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    ] {
        for code in [KeyCode::Char('c'), KeyCode::Char('C')] {
            assert_eq!(
                keybindings.action(Context::Global, chord(code, modifiers)),
                Some(Action::Quit),
                "{code:?} with {modifiers:?} should quit"
            );
        }
    }
}

#[test]
fn shifted_characters_stay_distinct() {
    let keybindings = parse("[keybindings.list]\nstart = \"S\"\n").unwrap();

    assert_eq!(
        keybindings.action(
            Context::List,
            chord(KeyCode::Char('S'), KeyModifiers::SHIFT)
        ),
        Some(Action::Start)
    );
}

#[test]
fn a_release_event_resolves_like_a_press() {
    let keybindings = KeyBindings::default();
    let mut release = press(KeyCode::Char('s'));
    release.kind = KeyEventKind::Release;

    assert_eq!(
        keybindings.action(Context::List, release),
        Some(Action::Start)
    );
}

#[test]
fn key_names_round_trip_to_display_text() {
    for (spec, display) in [
        ("ctrl+c", "Ctrl+c"),
        ("CTRL+C", "Ctrl+c"),
        ("alt+f", "Alt+f"),
        ("down", "↓"),
        ("pagedown", "PageDown"),
        ("page_up", "PageUp"),
        ("esc", "Esc"),
        ("space", "Space"),
        ("f12", "F12"),
        ("+", "+"),
        ("ctrl++", "Ctrl++"),
        ("?", "?"),
    ] {
        assert_eq!(KeyBind::parse(spec).unwrap().to_string(), display);
    }
}

#[test]
fn an_unknown_modifier_is_rejected() {
    let error = KeyBind::parse("hyper+x").unwrap_err();

    assert!(error.contains("unknown modifier 'hyper'"), "{error}");
}

#[test]
fn an_empty_specification_is_rejected() {
    assert!(KeyBind::parse("   ").is_err());
}
