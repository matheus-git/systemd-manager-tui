//! Opt-in integration tests against the real systemd D-Bus API.
//!
//! These tests are ignored by the regular test suite because CI containers do
//! not normally expose systemd. Run them on a systemd-based Linux host with:
//!
//! `cargo test infrastructure::systemd_service_adapter::tests -- --ignored --test-threads=1`

use super::{
    ConnectionType, OperationTimeout, ServiceAction, SystemdServiceAdapter, wait_for_operation,
};
use crate::domain::service::Service;
use crate::domain::service_repository::ServiceRepository;
use crate::domain::service_state::ServiceState;
use std::error::Error;
use std::io;
use std::process::{self, Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const INTEGRATION_TIMEOUT: Duration = Duration::from_secs(10);

fn service_with_active_state(active: &str) -> Service {
    Service::new(
        "demo.service".to_string(),
        String::new(),
        ServiceState::new(
            "loaded".to_string(),
            active.to_string(),
            String::new(),
            String::new(),
        ),
    )
}

fn command_error(command: &str, output: &Output) -> io::Error {
    let stderr = String::from_utf8_lossy(&output.stderr);
    io::Error::other(format!(
        "{command} failed with status {}: {}",
        output.status,
        stderr.trim()
    ))
}

struct TransientUserUnit {
    name: String,
}

impl TransientUserUnit {
    fn start() -> Result<Self, Box<dyn Error>> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let name = format!(
            "systemd-manager-tui-integration-{}-{unique}.service",
            process::id()
        );
        let unit = Self { name };

        let output = Command::new("systemd-run")
            .args([
                "--user",
                "--unit",
                unit.name.as_str(),
                "--property",
                "Type=oneshot",
                "--property",
                "RemainAfterExit=yes",
                "--quiet",
                "--",
                "/usr/bin/true",
            ])
            .output()?;

        if !output.status.success() {
            return Err(command_error("systemd-run", &output).into());
        }

        Ok(unit)
    }
}

impl Drop for TransientUserUnit {
    fn drop(&mut self) {
        let _ = Command::new("systemctl")
            .args(["--user", "stop", "--", self.name.as_str()])
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "reset-failed", "--", self.name.as_str()])
            .output();
    }
}

#[test]
#[ignore = "requires access to a running systemd system bus"]
fn system_bus_lists_services_and_reads_a_real_unit() -> Result<(), Box<dyn Error>> {
    let adapter = SystemdServiceAdapter::new(ConnectionType::System, INTEGRATION_TIMEOUT)?;
    let services = adapter.list_services(false)?;

    let listed = services
        .first()
        .ok_or_else(|| io::Error::other("systemd returned no loaded service units"))?;
    let loaded = adapter.get_unit(listed.name())?;

    assert_eq!(loaded.name(), listed.name());
    assert!(!loaded.state().load().is_empty());
    assert!(!loaded.state().active().is_empty());
    assert!(!loaded.state().sub().is_empty());
    Ok(())
}

#[test]
#[ignore = "requires access to a running systemd system bus"]
fn system_bus_lists_unit_files_and_resolves_their_states() -> Result<(), Box<dyn Error>> {
    let adapter = SystemdServiceAdapter::new(ConnectionType::System, INTEGRATION_TIMEOUT)?;
    let services: Vec<_> = adapter
        .list_service_files()?
        .into_iter()
        .filter(|service| service.name().ends_with(".service"))
        .take(8)
        .collect();

    assert!(
        !services.is_empty(),
        "systemd returned no service unit files"
    );
    let states = adapter.unit_files_state(services.clone())?;

    for service in services {
        let state = states
            .get(service.name())
            .unwrap_or_else(|| panic!("missing unit-file state for {}", service.name()));
        assert!(!state.is_empty(), "empty state for {}", service.name());
    }
    Ok(())
}

#[test]
#[ignore = "requires a running systemd user manager and systemd-run"]
fn user_bus_controls_an_isolated_transient_service() -> Result<(), Box<dyn Error>> {
    let unit = TransientUserUnit::start()?;
    let adapter = SystemdServiceAdapter::new(ConnectionType::Session, INTEGRATION_TIMEOUT)?;

    let loaded = adapter.get_unit(&unit.name)?;
    assert_eq!(loaded.name(), unit.name);
    assert_eq!(loaded.state().load(), "loaded");
    assert_eq!(loaded.state().active(), "active");

    let before_restart = adapter.get_active_enter_timestamp(&unit.name)?;
    let restarted = adapter.restart_service(&unit.name)?;
    let after_restart = adapter.get_active_enter_timestamp(&unit.name)?;

    assert_eq!(restarted.name(), unit.name);
    assert_eq!(restarted.state().active(), "active");
    assert!(!restarted.state().active().ends_with("ing"));
    assert!(after_restart >= before_restart);
    Ok(())
}

#[test]
fn operation_timeout_identifies_context_and_systemd_uncertainty() {
    let error = OperationTimeout {
        service: "demo.service".to_string(),
        action: ServiceAction::Restart,
        timeout: Duration::from_millis(250),
    };

    let message = error.to_string();
    assert!(message.contains("demo.service"));
    assert!(message.contains("restart"));
    assert!(message.contains("0.2s"));
    assert!(message.contains("may still be in progress"));
}

#[test]
fn wait_for_operation_returns_after_service_settles() {
    let mut polls = 0;
    let service = wait_for_operation(
        "demo.service",
        ServiceAction::Start,
        Duration::from_secs(1),
        Duration::ZERO,
        || {
            polls += 1;
            Ok(service_with_active_state(if polls == 1 {
                "activating"
            } else {
                "active"
            }))
        },
    )
    .unwrap();

    assert_eq!(polls, 2);
    assert_eq!(service.state().active(), "active");
}

#[test]
fn wait_for_operation_stops_at_the_configured_timeout() {
    let error = wait_for_operation(
        "demo.service",
        ServiceAction::Stop,
        Duration::ZERO,
        Duration::ZERO,
        || Ok(service_with_active_state("deactivating")),
    )
    .unwrap_err();
    let timeout = error.downcast_ref::<OperationTimeout>().unwrap();

    assert_eq!(timeout.service, "demo.service");
    assert_eq!(timeout.action, ServiceAction::Stop);
    assert_eq!(timeout.timeout, Duration::ZERO);
}
