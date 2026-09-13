//! Opt-in integration tests against the real systemd D-Bus API.
//!
//! These tests are ignored by the regular test suite because CI containers do
//! not normally expose systemd. Run them on a systemd-based Linux host with:
//!
//! `cargo test infrastructure::systemd_service_adapter::tests -- --ignored --test-threads=1`

use super::{ConnectionType, SystemdServiceAdapter};
use crate::domain::service_repository::ServiceRepository;
use std::error::Error;
use std::io;
use std::process::{self, Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

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
    let adapter = SystemdServiceAdapter::new(ConnectionType::System)?;
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
    let adapter = SystemdServiceAdapter::new(ConnectionType::System)?;
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
    let adapter = SystemdServiceAdapter::new(ConnectionType::Session)?;

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
