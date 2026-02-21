# Systemd manager tui

![rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)

A TUI application for managing systemd services.

This tool allows you to manage systemd services with ease. You can view logs, list services, view properties, edit unit files, and control their lifecycle: start, stop, restart, mask, unmask, enable, and disable using the D-Bus API. It also supports Vim-like navigation. It is possible to navigate between system and user units and choose to list only running services or all units.

Additionally, it runs a background notifier that monitors systemd events and sends desktop notifications whenever a unit enters a failed state, helping you quickly detect service failures without constantly watching the interface.

## Quick Preview

![screenshot_list](https://raw.githubusercontent.com/matheus-git/systemd-manager-tui/main/assets/systemd-manager-tui.gif)

View [screenshots](https://github.com/matheus-git/systemd-manager-tui/blob/main/docs/screenshots.md)

## Install

After installation, you can create an `alias` to make it easier to use.

### Ubuntu (recommended)

    sudo dpkg -i ./systemd-manager-tui_x.x.x-x_amd64.deb 

Download the .deb file from [Releases](https://github.com/matheus-git/systemd-manager-tui/releases)

### Fedora (recommended)

    sudo dnf install ./systemd-manager-tui_x.x.x-x_x86_64.rpm
    
Download the .rpm file from [Releases](https://github.com/matheus-git/systemd-manager-tui/releases)

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

## Contributing

Contributions are welcome! Please open an issue or submit a pull request for any improvements or bug fixes.

## 📝 License

This project is open-source under the MIT License.
