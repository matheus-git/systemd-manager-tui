use libc;

mod infrastructure;
mod domain;
mod usecases;
mod terminal;
use terminal::terminal::App;

fn main() -> color_eyre::Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("❌ This application must be run with sudo (as root).");
        std::process::exit(1);
    }
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = App::new().run(terminal);
    ratatui::restore();
    result
}
