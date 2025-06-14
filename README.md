# Systemd manager tui

![rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)

A program for managing systemd services through a TUI (Terminal User Interfaces).

This tool allows you to manage systemd services with ease. You can view logs, list services, view properties, and control their lifecycle—start, stop, restart, enable, and disable—using the D-Bus API. 

Additionally, it is possible to navigate between system and session units, choose to list either all units or only those of type 'service', and directly edit the selected unit's file.

## Screenshots
![screenshot_list](https://raw.githubusercontent.com/matheus-git/systemd-manager-tui/main/assets/screenshot_list.png)
View more [screenshots](https://github.com/matheus-git/systemd-manager-tui/blob/main/docs/screenshots.md)

## Usage

### Build binary
    cargo build --release
    
### Run binary *(use **`sudo`** if you intend to perform actions on **`system`** services)*
    ./target/release/systemd-manager-tui
    
## Main libraries

- ratatui - 0.29.0
- zbus - 5.5.0

## Weekly Updates

This project is actively maintained and updated every weekend.  

## Contributing

Contributions are welcome! Please open an issue or submit a pull request for any improvements or bug fixes.

## 📝 License

This project is open-source under the MIT License.
