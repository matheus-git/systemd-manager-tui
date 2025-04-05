mod infrastructure;
mod domain;
mod usecases;
mod terminal;
use terminal::terminal::App;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
        let result = App::new().run(terminal);
    ratatui::restore();
    result
}
