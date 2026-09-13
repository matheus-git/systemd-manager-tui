use super::{Args, Config};
use clap::Parser;
use std::time::Duration;

#[test]
fn cli_uses_expected_defaults() {
    let args = Args::try_parse_from(["systemd-manager-tui"]).unwrap();
    let config = Config::from(args);

    assert!(config.filter.is_empty());
    assert_eq!(config.operation_timeout, Duration::from_secs(10));
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
