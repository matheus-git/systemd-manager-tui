use super::{Args, Config};
use clap::Parser;

#[test]
fn cli_uses_expected_defaults() {
    let args = Args::try_parse_from(["systemd-manager-tui"]).unwrap();
    let config = Config::from(args);

    assert!(config.filter.is_empty());
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
