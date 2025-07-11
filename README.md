# Systemd manager tui

![rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)

A program for managing systemd services through a TUI (Terminal User Interfaces).

This tool allows you to manage systemd services with ease. You can view logs, list services, view properties, and control their lifecycle—start, stop, restart, enable, and disable—using the D-Bus API. 

Additionally, it is possible to navigate between system and session units, choose to list either all units or only those of type 'service', and directly edit the selected unit's file. It also supports Vim-like navigation.

## Screenshots
![screenshot_list](https://raw.githubusercontent.com/matheus-git/systemd-manager-tui/main/assets/systemd-manager-tui.gif)
View [screenshots](https://github.com/matheus-git/systemd-manager-tui/blob/main/docs/screenshots.md)

## Install

After installation, you can create an `alias` to make it easier to use.

### Ubuntu (recommended)
    sudo dpkg -i systemd-manager-tui_x.x.x-x_amd64.deb
Download the .deb file from Releases

### Arch linux (recommended)
    yay -S systemd-manager-tui
https://aur.archlinux.org/packages/systemd-manager-tui

### Binary
    chmod +x systemd-manager-tui
    ./systemd-manager-tui
Download binary from Releases

### Cargo
    cargo install --locked systemd-manager-tui
        
## Main libraries

- ratatui - 0.29.0
- zbus - 5.5.0

## Contributing

Contributions are welcome! Please open an issue or submit a pull request for any improvements or bug fixes.

## 📝 License

This project is open-source under the MIT License.
