# Systemd manager tui

![rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)

A TUI application for managing systemd services.

This tool allows you to manage systemd services with ease. You can view logs, list services, view properties, edit unit files, and control their lifecycle: start, stop, restart, mask, unmask, enable, and disable using the D-Bus API. It also supports Vim-like navigation. It is possible to navigate between system and user units and choose to list only running services or all units.

## Quick Preview

![screenshot_list](https://raw.githubusercontent.com/matheus-git/systemd-manager-tui/main/assets/systemd-manager-tui.gif)

View [screenshots](https://github.com/matheus-git/systemd-manager-tui/blob/main/docs/screenshots.md)

## Usage

```bash
systemd-manager-tui

# With a filter
systemd-manager-tui -f docker
```

## Install

After installation, you can create an `alias` to make it easier to use.

### Ubuntu (recommended)

    sudo dpkg -i ./systemd-manager-tui_x.x.x-x_amd64.deb 

Download the .deb file from [Releases](https://github.com/matheus-git/systemd-manager-tui/releases)

### Fedora (recommended)

    sudo dnf install ./systemd-manager-tui_x.x.x-x_x86_64.rpm
    
Download the .rpm file from [Releases](https://github.com/matheus-git/systemd-manager-tui/releases)

### Building packages

Build native and cross-compiled binaries, DEBs, and RPMs with:

```sh
./build.sh
```

The script selects the RPM architecture and binary asset automatically. To regenerate packages
from binaries that already exist under `target/<triple>/release/`, use:

```sh
./build.sh --rpm-only
./build.sh --deb-only
./build.sh --packages-only
```

To build or package only one architecture, combine an option with `--target`:

```sh
./build.sh --rpm-only --target aarch64-unknown-linux-musl
./build.sh --skip-native --target x86_64-unknown-linux-musl
```

### Arch linux

    yay -S systemd-manager-tui

https://aur.archlinux.org/packages/systemd-manager-tui

### NixOS
    nix run github:matheus-git/systemd-manager-tui

NixOS with flakes [Read here](docs/flakes.md)
### Binary

    chmod +x systemd-manager-tui
    ./systemd-manager-tui
Download binary from [Releases](https://github.com/matheus-git/systemd-manager-tui/releases)

### Cargo

    cargo install --locked systemd-manager-tui

## Main libraries

- ratatui - 0.29.0
- zbus - 5.5.0

## Testing

Run the regular test suite with:

```bash
cargo test
```

The regular suite includes unit, application-flow, and Ratatui rendering tests using `TestBackend`, so it does not require an interactive terminal or a running systemd instance.

Real integration tests against the system and user D-Bus buses are available as an opt-in suite. Run them on a systemd-based Linux host where the current user has an active user manager and both `systemctl` and `systemd-run` are available:

```bash
cargo test infrastructure::systemd_service_adapter::tests -- --ignored --test-threads=1
```

The integration suite reads real system units and creates an isolated transient user service to exercise lifecycle operations. The temporary service is stopped and cleaned up automatically. These tests are ignored during a regular `cargo test` so environments without systemd, such as many CI containers, continue to work.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request for any improvements or bug fixes.

## 📝 License

This project is open-source under the MIT License.
