//! Opt-in integration tests against the real systemd D-Bus API.
//!
//! These tests are ignored by the regular test suite because CI containers do
//! not normally expose systemd. Run them on a systemd-based Linux host with:
//!
//! `cargo test infrastructure::systemd_service_adapter::tests -- --ignored --test-threads=1`

use super::{
    ConnectionType, OperationTimeout, ServiceAction, SystemdJobFailed, SystemdServiceAdapter,
};
use crate::domain::service_repository::ServiceRepository;
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{self, Command, Output};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const INTEGRATION_TIMEOUT: Duration = Duration::from_secs(10);

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
        Self::start_with_command("/usr/bin/true")
    }

    fn start_with_command(command: &str) -> Result<Self, Box<dyn Error>> {
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
                command,
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
            .args(["--user", "unmask", "--", self.name.as_str()])
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "stop", "--", self.name.as_str()])
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "reset-failed", "--", self.name.as_str()])
            .output();
    }
}

struct InstalledUserUnit {
    name: String,
    source_path: PathBuf,
    mask_path: PathBuf,
}

impl InstalledUserUnit {
    fn new(command: &str) -> Result<Self, Box<dyn Error>> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let name = format!(
            "systemd-manager-tui-installed-{}-{unique}.service",
            process::id()
        );
        let source_dir = systemd_user_dir("user-shared")?;
        let mask_dir = systemd_user_dir("user-configuration")?;
        fs::create_dir_all(&source_dir)?;
        fs::create_dir_all(&mask_dir)?;
        let source_path = source_dir.join(&name);
        let mask_path = mask_dir.join(&name);
        fs::write(
            &source_path,
            format!(
                "[Unit]\nDescription=systemd-manager-tui integration fixture\n\
                 [Service]\nType=oneshot\nExecStart={command}\nRemainAfterExit=yes\n"
            ),
        )?;
        reload_user_daemon()?;
        Ok(Self {
            name,
            source_path,
            mask_path,
        })
    }
}

impl Drop for InstalledUserUnit {
    fn drop(&mut self) {
        let _ = Command::new("systemctl")
            .args(["--user", "unmask", "--", self.name.as_str()])
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "stop", "--", self.name.as_str()])
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "reset-failed", "--", self.name.as_str()])
            .output();
        let _ = fs::remove_file(&self.mask_path);
        let _ = fs::remove_file(&self.source_path);
        let _ = reload_user_daemon();
    }
}

fn systemd_user_dir(kind: &str) -> Result<PathBuf, Box<dyn Error>> {
    let output = Command::new("systemd-path").arg(kind).output()?;
    if !output.status.success() {
        return Err(command_error("systemd-path", &output).into());
    }
    let base = String::from_utf8(output.stdout)?.trim().to_string();
    Ok(PathBuf::from(base).join("systemd/user"))
}

fn reload_user_daemon() -> Result<(), Box<dyn Error>> {
    let output = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output()?;
    if !output.status.success() {
        return Err(command_error("systemctl --user daemon-reload", &output).into());
    }
    Ok(())
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

    let started = adapter.start_service(&unit.name)?;
    assert_eq!(started.state().active(), "active");

    let before_restart = adapter.get_active_enter_timestamp(&unit.name)?;
    let restarted = adapter.restart_service(&unit.name)?;
    let after_restart = adapter.get_active_enter_timestamp(&unit.name)?;

    assert_eq!(restarted.name(), unit.name);
    assert_eq!(restarted.state().active(), "active");
    assert!(!restarted.state().active().ends_with("ing"));
    assert!(after_restart >= before_restart);

    let stopped = adapter.stop_service(&unit.name)?;
    assert_eq!(stopped.state().active(), "inactive");
    Ok(())
}

#[test]
#[ignore = "requires a running systemd user manager and systemd-run"]
fn user_bus_reports_a_failed_job_result() -> Result<(), Box<dyn Error>> {
    let unit = InstalledUserUnit::new("/usr/bin/false")?;
    let adapter = SystemdServiceAdapter::new(ConnectionType::Session, INTEGRATION_TIMEOUT)?;

    let error = adapter.restart_service(&unit.name).unwrap_err();
    let failure = error
        .downcast_ref::<SystemdJobFailed>()
        .ok_or_else(|| io::Error::other(format!("expected SystemdJobFailed, got: {error}")))?;

    assert_eq!(failure.service, unit.name);
    assert_eq!(failure.action, ServiceAction::Restart);
    assert_eq!(failure.result, "failed");
    Ok(())
}

#[test]
#[ignore = "requires a running systemd user manager and systemd-run"]
fn masking_an_existing_unit_is_idempotent() -> Result<(), Box<dyn Error>> {
    let unit = InstalledUserUnit::new("/usr/bin/true")?;
    let adapter = SystemdServiceAdapter::new(ConnectionType::Session, INTEGRATION_TIMEOUT)?;

    let masked = adapter.mask_service(&unit.name)?;
    assert!(masked.state().file().starts_with("masked"));

    let masked_again = adapter.mask_service(&unit.name)?;
    assert!(masked_again.state().file().starts_with("masked"));

    let unmasked = adapter.unmask_service(&unit.name)?;
    assert!(!unmasked.state().file().starts_with("masked"));
    Ok(())
}

#[test]
#[ignore = "requires a running systemd user manager and systemd-run"]
fn timed_out_job_reports_its_later_completion() -> Result<(), Box<dyn Error>> {
    let unit = TransientUserUnit::start()?;
    let (sender, receiver) = mpsc::channel();
    let adapter = SystemdServiceAdapter::new_with_completion_sender(
        ConnectionType::Session,
        Duration::ZERO,
        Some(sender),
    )?;

    let error = adapter.restart_service(&unit.name).unwrap_err();
    assert!(error.downcast_ref::<OperationTimeout>().is_some());

    let completion = receiver.recv_timeout(Duration::from_secs(2))?;
    assert_eq!(completion.connection, ConnectionType::Session);
    assert_eq!(completion.service, unit.name);
    assert_eq!(completion.action, ServiceAction::Restart);
    assert_eq!(completion.result, "done");
    Ok(())
}

#[test]
#[ignore = "requires running systemd user and system managers"]
fn late_completion_keeps_the_connection_that_started_the_job() -> Result<(), Box<dyn Error>> {
    let unit = TransientUserUnit::start()?;
    let (sender, receiver) = mpsc::channel();
    let mut adapter = SystemdServiceAdapter::new_with_completion_sender(
        ConnectionType::Session,
        Duration::ZERO,
        Some(sender),
    )?;

    let error = adapter.restart_service(&unit.name).unwrap_err();
    assert!(error.downcast_ref::<OperationTimeout>().is_some());
    adapter.change_connection(ConnectionType::System)?;

    let completion = receiver.recv_timeout(Duration::from_secs(2))?;
    assert_eq!(completion.connection, ConnectionType::Session);
    assert_eq!(completion.service, unit.name);
    assert_eq!(completion.action, ServiceAction::Restart);
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
    assert!(message.contains("250ms"));
    assert!(message.contains("may still be in progress"));
}

#[test]
fn job_failure_identifies_the_operation_and_systemd_result() {
    let error = SystemdJobFailed {
        service: "demo.service".to_string(),
        action: ServiceAction::Start,
        result: "dependency".to_string(),
    };

    let message = error.to_string();
    assert!(message.contains("start"));
    assert!(message.contains("demo.service"));
    assert!(message.contains("dependency"));
}
