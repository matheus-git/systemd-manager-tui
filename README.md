# Systemd manager tui

![rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)

A program for managing systemd services through a TUI (Terminal User Interfaces).

This tool allows you to manage systemd services with ease. You can view logs, list services, view properties, and control their lifecycle—start, stop, restart, enable, and disable—using the D-Bus API. 

Additionally, it is possible to navigate between system services (sudo) and session services (user).

## Screenshots
![screenshot_list](assets/screenshot_list.png?v=2)
View more [screenshots](docs/screenshots.md)

## Usage

### Build binary
    cargo build --release
### Manage *system* services
    sudo ./target/release/systemd-manager-tui
### Manage *session* services
    ./target/release/systemd-manager-tui

## Architecture

See the architecture [here](docs/architecture.md).

## Request New Features or Properties

There are many possible actions and pieces of information that can be retrieved, so I’ve implemented the ones I found most relevant. If you’d like more to be added, feel free to open an issue with your request! You can check all available methods and properties on D-Bus [here](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.systemd1.html).

## Main libraries

- ratatui - 0.29.0
- zbus - 5.5.0

## Future Improvements

- Show loading indicator when switching tabs
- Remotely manage services.

## Weekly Updates

This project is actively maintained and updated every weekend.  

## Contributing

Contributions are welcome! Please open an issue or submit a pull request for any improvements or bug fixes.

## Contributors

<a href="https://github.com/matheus-git/systemd-manager-tui/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=matheus-git/systemd-manager-tui" />
</a>

Made with [contrib.rocks](https://contrib.rocks).

## 📝 License

This project is open-source under the MIT License.
