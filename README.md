# Systemd manager tui

![rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)

Uma programa para gerenciamento de processos systemd através de uma interface tui. 

As operações são listagem, start, stop, restart, enable e disable de processos systemd usando a api d-bus. 

## Screenshots


## Usage

Deve ser executado com sudo ( or root ), recomenda-se a criação de um binário e adição de um alias no .bashrc ( por exemplo ).

### Executar em modo de desenvolvimento:
  ```
   sudo cargo run
  ```

### Criar binário:

1. Gere o binário
    ```
      cargo build --release
    ```
3. Executá-lo
    ```
      ./target/release/systemd-manager-tui
    ```

## Principais libs

- ratatui - 0.29.0
- zbus - 5.5.0

## Contributing

Contributions are welcome! Please open an issue or submit a pull request for any improvements or bug fixes.

## 📝 License

This project is open-source under the MIT License.
