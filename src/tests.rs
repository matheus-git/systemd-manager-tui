use super::{Args, Config};
use clap::Parser;
use std::path::Path;
use std::time::Duration;

#[test]
fn cli_uses_expected_defaults() {
    let args = Args::try_parse_from(["systemd-manager-tui"]).unwrap();
    let config = Config::from(args);

    assert!(config.filter.is_empty());
    assert_eq!(config.operation_timeout, Duration::from_secs(10));
    assert!(config.failure_notifications);
}

#[test]
fn cli_can_disable_failure_notifications() {
    let args = Args::try_parse_from(["systemd-manager-tui", "--no-notifications"]).unwrap();
    let config = Config::from(args);

    assert!(!config.failure_notifications);
}

#[test]
fn cli_accepts_short_no_notifications_flag() {
    let args = Args::try_parse_from(["systemd-manager-tui", "-n"]).unwrap();
    let config = Config::from(args);

    assert!(!config.failure_notifications);
}

#[test]
fn cli_accepts_operation_timeout() {
    let args =
        Args::try_parse_from(["systemd-manager-tui", "--operation-timeout-secs", "3"]).unwrap();
    let config = Config::from(args);

    assert_eq!(config.operation_timeout, Duration::from_secs(3));
}

#[test]
fn cli_rejects_zero_operation_timeout() {
    assert!(
        Args::try_parse_from(["systemd-manager-tui", "--operation-timeout-secs", "0"]).is_err()
    );
}

#[test]
fn cli_accepts_filter() {
    let args = Args::try_parse_from(["systemd-manager-tui", "--filter", "docker"]).unwrap();
    let config = Config::from(args);

    assert_eq!(config.filter, "docker");
}

#[test]
fn cli_accepts_short_filter_flag() {
    let args = Args::try_parse_from(["systemd-manager-tui", "-f", "ssh"]).unwrap();

    assert_eq!(args.filter.as_deref(), Some("ssh"));
}

#[test]
fn cli_rejects_unknown_arguments() {
    let result = Args::try_parse_from(["systemd-manager-tui", "--unknown"]);

    assert!(result.is_err());
}

#[test]
fn cli_accepts_a_configuration_file() {
    let args = Args::try_parse_from(["systemd-manager-tui", "--config", "/tmp/keys.toml"]).unwrap();

    assert_eq!(args.config.as_deref(), Some(Path::new("/tmp/keys.toml")));
}

#[test]
fn cli_defaults_to_no_explicit_configuration_file() {
    let args = Args::try_parse_from(["systemd-manager-tui"]).unwrap();

    assert!(args.config.is_none());
    assert!(!args.print_default_config);
}

#[test]
fn cli_can_print_the_default_configuration() {
    let args = Args::try_parse_from(["systemd-manager-tui", "--print-default-config"]).unwrap();

    assert!(args.print_default_config);
}

#[test]
fn the_printed_default_configuration_is_loadable() {
    let directory = std::env::temp_dir().join("systemd-manager-tui-config-test");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("config.toml");
    std::fs::write(&path, crate::keybindings::DEFAULT_CONFIG_TEMPLATE).unwrap();

    let keybindings = crate::keybindings::load(Some(&path));
    std::fs::remove_file(&path).unwrap();

    assert!(keybindings.is_ok(), "{:?}", keybindings.err());
}

#[test]
fn a_missing_explicit_configuration_file_is_an_error() {
    let path = std::env::temp_dir().join("systemd-manager-tui-does-not-exist.toml");

    let error = crate::keybindings::load(Some(&path)).unwrap_err();

    assert!(
        error.contains("Could not read the configuration file"),
        "{error}"
    );
}
