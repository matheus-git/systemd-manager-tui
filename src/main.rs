mod domain;
mod infrastructure;
mod terminal;
mod usecases;
use terminal::app::App;
use infrastructure::systemd_service_adapter::SystemdServiceAdapter;
use usecases::services_manager::ServicesManager;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let usecase = ServicesManager::new(Box::new(SystemdServiceAdapter::default()));
    let mut app = App::new(usecase);
    app.init();
    let result = app.run(terminal);
    ratatui::restore();
    result
}
