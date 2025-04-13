# Systemd manager tui

![rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)

A program for managing systemd services through a TUI (Text User Interface).

The available operations include viewing logs, listing, starting, stopping, restarting, enabling, and disabling systemd services using the D-Bus API.

## Screenshots
![screenshot1](assets/screeshot1.png)
![screenshot2](assets/screeshot2.png)

## Usage

Must be run as sudo (or root).

**It's recommended to build a binary since it's simpler to run with sudo than to configure sudo to run "sudo cargo run".**

### Run in development mode
  ```
   sudo cargo run
  ```

### Build binary

1. Build the binary
    ```
      cargo build --release
    ```
3. Run it ( opcional )
    ```
      sudo target/release/systemd-manager-tui
    ```

## Main libraries

- ratatui - 0.29.0
- zbus - 5.5.0

## Future Improvements

- Auto-refresh logs
- Monitor CPU and memory usage per service
- View detailed information for each service
- Display user-specific systemd services
- Remotely manage services

## Weekly Updates

This project is actively maintained and updated every weekend.  

## Contributing

Contributions are welcome! Please open an issue or submit a pull request for any improvements or bug fixes.

## 📝 License

This project is open-source under the MIT License.
