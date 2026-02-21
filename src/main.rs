mod domain;
mod infrastructure;
mod terminal;
mod usecases;
use infrastructure::systemd_service_adapter::{ConnectionType, SystemdServiceAdapter};
use infrastructure::notifier::start_watchers;
use terminal::app::App;
use usecases::services_manager::ServicesManager;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use terminal::app::AppEvent;

use terminal::components::details::ServiceDetails;
use terminal::components::filter::Filter;
use terminal::components::list::TableServices;
use terminal::components::log::ServiceLog;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    
    let (event_tx, event_rx) = mpsc::channel::<AppEvent>();

    start_watchers();
    let systemd_adapter = SystemdServiceAdapter::new(ConnectionType::System)?;
    let usecase = Rc::new(RefCell::new(ServicesManager::new(Box::new(
        systemd_adapter
    ))));
    let table_services = TableServices::new(event_tx.clone(), usecase.clone());
    let filter = Filter::new(event_tx.clone());
    let service_log = ServiceLog::new(event_tx.clone(), usecase.clone());
    let details = ServiceDetails::new(event_tx.clone(), usecase.clone());

    let mut app = App::new(
        event_tx,
        event_rx,
        table_services,
        filter,
        service_log,
        details,
        usecase,
    );
    app.init();
    let result = app.run(terminal);
    ratatui::restore();
    result
}
