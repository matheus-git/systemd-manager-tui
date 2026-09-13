use super::*;
use crate::domain::service_repository::ServiceRepository;
use crate::domain::service_state::ServiceState;
use crate::infrastructure::systemd_service_adapter::ConnectionType;
use crate::test_support::FakeRepository;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::collections::HashMap;
use std::sync::mpsc;

struct EmptyRepository;

impl ServiceRepository for EmptyRepository {
    fn list_services(&self, _: bool) -> Result<Vec<Service>, Box<dyn Error>> {
        Ok(vec![])
    }
    fn unit_files_state(&self, _: Vec<Service>) -> Result<HashMap<String, String>, Box<dyn Error>> {
        Ok(HashMap::new())
    }
    fn list_service_files(&self) -> Result<Vec<Service>, Box<dyn Error>> {
        Ok(vec![])
    }
    fn get_unit(&self, _: &str) -> Result<Service, Box<dyn Error>> {
        Err("not found".into())
    }
    fn get_service_log(&self, _: &str) -> Result<String, Box<dyn Error>> {
        Ok(String::new())
    }
    fn start_service(&self, _: &str) -> Result<Service, Box<dyn Error>> {
        Err("unsupported".into())
    }
    fn stop_service(&self, _: &str) -> Result<Service, Box<dyn Error>> {
        Err("unsupported".into())
    }
    fn restart_service(&self, _: &str) -> Result<Service, Box<dyn Error>> {
        Err("unsupported".into())
    }
    fn enable_service(&self, _: &str) -> Result<Service, Box<dyn Error>> {
        Err("unsupported".into())
    }
    fn disable_service(&self, _: &str) -> Result<Service, Box<dyn Error>> {
        Err("unsupported".into())
    }
    fn mask_service(&self, _: &str) -> Result<Service, Box<dyn Error>> {
        Err("unsupported".into())
    }
    fn unmask_service(&self, _: &str) -> Result<Service, Box<dyn Error>> {
        Err("unsupported".into())
    }
    fn reload_daemon(&self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn change_connection(&mut self, _: ConnectionType) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn systemctl_cat(&self, _: &str) -> Result<String, Box<dyn Error>> {
        Ok(String::new())
    }
    fn get_active_enter_timestamp(&self, _: &str) -> Result<u64, Box<dyn Error>> {
        Ok(0)
    }
}

fn table_with_repository(
    repository: impl ServiceRepository + 'static,
) -> (TableServices, mpsc::Receiver<AppEvent>) {
    let (sender, receiver) = mpsc::channel();
    let manager = ServicesManager::new(Box::new(repository));
    (
        TableServices::new(sender, Rc::new(RefCell::new(manager))),
        receiver,
    )
}

fn table_with_receiver() -> (TableServices, mpsc::Receiver<AppEvent>) {
    table_with_repository(EmptyRepository)
}

fn table() -> TableServices {
    table_with_receiver().0
}

fn service(name: &str, active: &str) -> Service {
    Service::new(
        name.to_string(),
        String::new(),
        ServiceState::new(
            "loaded".into(),
            active.into(),
            "running".into(),
            "enabled".into(),
        ),
    )
}

#[test]
fn navigation_on_empty_list_clears_selection() {
    let mut table = table();

    table.select_next();
    table.select_previous();
    table.select_page_down();
    table.select_page_up();

    assert_eq!(table.table_state.selected(), None);
}

#[test]
fn navigation_wraps_in_both_directions() {
    let mut table = table();
    table.filtered_services = vec![
        service("a.service", "active"),
        service("b.service", "inactive"),
    ];
    table.table_state.select(Some(1));

    table.select_next();
    assert_eq!(table.table_state.selected(), Some(0));

    table.select_previous();
    assert_eq!(table.table_state.selected(), Some(1));
}

#[test]
fn filter_combines_name_and_active_state() {
    let mut table = table();
    table.active_filter_state = ActiveFilterState::Active;
    let services = vec![
        service("alpha.service", "active"),
        service("beta.service", "active"),
        service("alpha.timer", "inactive"),
    ];

    let filtered = table.filter("alpha", &services);

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name(), "alpha.service");
}

#[test]
fn file_state_resolution_prefers_loaded_state_map() {
    let loading = Service::new(
        "demo.service".into(),
        String::new(),
        ServiceState::new(
            "loaded".into(),
            "active".into(),
            "running".into(),
            LOADING_PLACEHOLDER.into(),
        ),
    );
    let states = HashMap::from([("demo.service".to_string(), "enabled".to_string())]);

    assert_eq!(resolve_file(&loading, Some(&states)), "enabled");
}

#[test]
fn explicit_service_file_state_wins_over_loaded_state_map() {
    let unit = service("demo.service", "active");
    let states = HashMap::from([("demo.service".to_string(), "masked".to_string())]);

    assert_eq!(resolve_file(&unit, Some(&states)), "enabled");
}

#[test]
fn renders_service_table_to_test_backend() {
    let mut table = table();
    table.filtered_services = vec![service("demo.service", "active")];
    table.table_state.select(Some(0));
    let backend = TestBackend::new(90, 6);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| table.render(frame, frame.area()))
        .unwrap();

    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("Name"));
    assert!(rendered.contains("demo.service"));
    assert!(rendered.contains("active (running)"));
    assert!(rendered.contains("enabled"));
}

#[test]
fn active_filter_state_cycles_through_every_option() {
    let state = ActiveFilterState::All;

    let state = state.next();
    assert!(state == ActiveFilterState::Active);
    assert_eq!(state.as_str(), "active");
    let state = state.next();
    assert!(state == ActiveFilterState::Inactive);
    assert_eq!(state.as_str(), "inactive");
    let state = state.next();
    assert!(state == ActiveFilterState::Failed);
    assert_eq!(state.as_str(), "failed");
    assert!(state.next() == ActiveFilterState::All);
}

#[test]
fn each_active_state_filter_selects_only_matching_services() {
    let mut table = table();
    let services = vec![
        service("active.service", "active"),
        service("inactive.service", "inactive"),
        service("failed.service", "failed"),
    ];

    for (filter_state, expected_name) in [
        (ActiveFilterState::Active, "active.service"),
        (ActiveFilterState::Inactive, "inactive.service"),
        (ActiveFilterState::Failed, "failed.service"),
    ] {
        table.active_filter_state = filter_state;
        let filtered = table.filter("", &services);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name(), expected_name);
    }
}

#[test]
fn refresh_repairs_an_out_of_bounds_selection() {
    let mut table = table();
    table.services = vec![service("demo.service", "active")];
    table.table_state.select(Some(99));

    table.refresh("");

    assert_eq!(table.table_state.selected(), Some(0));
    assert_eq!(table.get_selected_service().unwrap().name(), "demo.service");
}

#[test]
fn service_shortcut_dispatches_action_and_locks_input() {
    let (mut table, receiver) = table_with_receiver();

    table.on_key_event(KeyEvent::new(
        KeyCode::Char('s'),
        crossterm::event::KeyModifiers::NONE,
    ));

    assert!(table.ignore_key_events);
    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Action(Actions::ServiceAction(ServiceAction::Start))
    ));
    assert!(table.shortcuts().is_empty());
}

#[test]
fn selected_row_uses_distinct_enabled_and_disabled_colors() {
    let mut table = table();
    table.filtered_services = vec![service("demo.service", "active")];
    table.table_state.select(Some(0));
    let backend = TestBackend::new(90, 6);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| table.render(frame, frame.area()))
        .unwrap();
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.bg == Color::Blue)
    );

    table.set_ignore_key_events(true);
    let backend = TestBackend::new(90, 6);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| table.render(frame, frame.area()))
        .unwrap();
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.bg == Color::DarkGray)
    );
}

#[test]
fn timestamp_update_is_applied_only_to_current_selection() {
    let mut table = table();
    table.selected_service_name = Some("current.service".into());

    table.update_timestamp("old.service".into(), Some(10));
    assert!(table.active_enter_timestamp.is_none());

    table.update_timestamp("current.service".into(), Some(20));
    assert_eq!(table.active_enter_timestamp, Some(20));
    assert!(table.has_active_runtime());
}

#[test]
fn successful_service_action_updates_row_and_unlocks_input() {
    let fake = FakeRepository::default();
    let observer = fake.clone();
    let (mut table, _receiver) = table_with_repository(fake);
    let unit = service("demo.service", "inactive");
    table.services = vec![unit.clone()];
    table.filtered_services = vec![unit];
    table.table_state.select(Some(0));
    table.set_ignore_key_events(true);

    table.act_on_selected_service(&ServiceAction::Start);

    assert_eq!(observer.calls(), ["start:demo.service"]);
    assert_eq!(table.services[0].state().active(), "active");
    assert!(!table.ignore_key_events);
}

#[test]
fn failed_service_action_reports_error_and_unlocks_input() {
    let fake = FakeRepository::default();
    fake.fail("start");
    let observer = fake.clone();
    let (mut table, receiver) = table_with_repository(fake);
    let unit = service("broken.service", "inactive");
    table.services = vec![unit.clone()];
    table.filtered_services = vec![unit];
    table.table_state.select(Some(0));
    table.set_ignore_key_events(true);

    table.act_on_selected_service(&ServiceAction::Start);

    assert!(matches!(
        receiver.recv().unwrap(),
        AppEvent::Error(message) if message == "start failed"
    ));
    assert!(observer.calls().contains(&"list:false".to_string()));
    assert!(!table.ignore_key_events);
}

#[test]
fn toggle_mask_chooses_operation_from_loaded_file_state() {
    let fake = FakeRepository::default();
    let observer = fake.clone();
    let (mut table, _receiver) = table_with_repository(fake);
    let unit = service("demo.service", "inactive");
    table.services = vec![unit.clone()];
    table.filtered_services = vec![unit];
    table.table_state.select(Some(0));
    table
        .states
        .lock()
        .unwrap()
        .insert("demo.service".into(), "masked".into());

    table.act_on_selected_service(&ServiceAction::ToggleMask);

    let calls = observer.calls();
    assert!(calls.contains(&"unmask:demo.service".to_string()));
    assert!(!calls.contains(&"mask:demo.service".to_string()));
}

#[test]
fn rendered_rows_expose_active_state_and_loading_styles() {
    let mut table = table();
    table.filtered_services = vec![
        service("active.service", "active"),
        service("starting.service", "activating"),
        service("inactive.service", "inactive"),
        service("failed.service", "failed"),
        Service::new(
            "loading.service".into(),
            String::new(),
            ServiceState::new(
                "loaded".into(),
                "active".into(),
                String::new(),
                LOADING_PLACEHOLDER.into(),
            ),
        ),
    ];
    table.table_state.select(None);
    let backend = TestBackend::new(100, 10);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| table.render(frame, frame.area()))
        .unwrap();

    let cells = terminal.backend().buffer().content();
    for color in [Color::Green, Color::Yellow, Color::DarkGray, Color::Red] {
        assert!(cells.iter().any(|cell| cell.fg == color));
    }
    assert!(cells.iter().any(|cell| {
        cell.symbol() == "L"
            && cell.modifier.contains(Modifier::ITALIC)
            && cell.modifier.contains(Modifier::DIM)
    }));
}

#[test]
fn every_list_shortcut_dispatches_its_expected_action() {
    fn assert_action(key: char, expected: fn(Actions) -> bool) {
        let (mut table, receiver) = table_with_receiver();
        table.on_key_event(KeyEvent::new(
            KeyCode::Char(key),
            crossterm::event::KeyModifiers::NONE,
        ));
        match receiver.recv().unwrap() {
            AppEvent::Action(action) => assert!(expected(action), "wrong action for key {key}"),
            AppEvent::Key(_) | AppEvent::Error(_) => panic!("wrong event for key {key}"),
        }
    }

    assert_action('r', |action| {
        matches!(action, Actions::ServiceAction(ServiceAction::Restart))
    });
    assert_action('x', |action| {
        matches!(action, Actions::ServiceAction(ServiceAction::Stop))
    });
    assert_action('e', |action| {
        matches!(action, Actions::ServiceAction(ServiceAction::Enable))
    });
    assert_action('d', |action| {
        matches!(action, Actions::ServiceAction(ServiceAction::Disable))
    });
    assert_action('u', |action| {
        matches!(action, Actions::ServiceAction(ServiceAction::RefreshAll))
    });
    assert_action('f', |action| {
        matches!(action, Actions::ServiceAction(ServiceAction::ToggleFilter))
    });
    assert_action('m', |action| {
        matches!(action, Actions::ServiceAction(ServiceAction::ToggleMask))
    });
    assert_action('?', |action| matches!(action, Actions::ShowHelp));
    assert_action('c', |action| matches!(action, Actions::GoDetails));
    assert_action('v', |action| matches!(action, Actions::GoLog));
}

#[test]
fn page_navigation_wraps_for_lists_larger_than_jump() {
    let mut table = table();
    table.filtered_services = (0..11)
        .map(|index| service(&format!("{index}.service"), "active"))
        .collect();
    table.table_state.select(Some(0));

    table.select_page_up();
    assert_eq!(table.table_state.selected(), Some(1));
    table.select_page_down();
    assert_eq!(table.table_state.selected(), Some(0));
}

#[test]
fn active_service_runtime_is_rendered_instead_of_raw_state() {
    let mut table = table();
    table.filtered_services = vec![service("demo.service", "active")];
    table.table_state.select(Some(0));
    table.selected_service_name = Some("demo.service".into());
    table.last_timestamp_fetch = Some(Instant::now());
    table.active_enter_timestamp = Some(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
            - 65_000_000,
    );
    let backend = TestBackend::new(90, 6);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| table.render(frame, frame.area()))
        .unwrap();

    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("Uptime: 1m"));
    assert!(!rendered.contains("active (running)"));
}

#[test]
fn toggle_mask_masks_an_unmasked_service() {
    let fake = FakeRepository::default();
    let observer = fake.clone();
    let (mut table, _receiver) = table_with_repository(fake);
    let unit = service("demo.service", "inactive");
    table.services = vec![unit.clone()];
    table.filtered_services = vec![unit];
    table.table_state.select(Some(0));
    table
        .states
        .lock()
        .unwrap()
        .insert("demo.service".into(), "enabled".into());

    table.act_on_selected_service(&ServiceAction::ToggleMask);

    let calls = observer.calls();
    assert!(calls.contains(&"mask:demo.service".to_string()));
    assert!(!calls.contains(&"unmask:demo.service".to_string()));
}
