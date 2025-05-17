mod domain;
mod infrastructure;
mod terminal;
mod usecases;
use terminal::app::App;
use infrastructure::systemd_service_adapter::{SystemdServiceAdapter, ConnectionType};
use usecases::services_manager::ServicesManager;

use std::sync::mpsc;
use std::cell::RefCell;
use std::rc::Rc;

use terminal::app::AppEvent;

use terminal::components::details::ServiceDetails;
use terminal::components::filter::Filter;
use terminal::components::list::TableServices;
use terminal::components::log::ServiceLog;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();

    let (event_tx, event_rx) = mpsc::channel::<AppEvent>();

    let connection_type = match unsafe {libc::geteuid()}  {
        0 => ConnectionType::System,
        _ => ConnectionType::Session
    };
    let usecase = Rc::new(ServicesManager::new(Box::new(SystemdServiceAdapter::new(connection_type)?)));
    let table_services = TableServices::new(event_tx.clone(), usecase.clone());
    let filter = Filter::new(event_tx.clone());
    let service_log = ServiceLog::new(event_tx.clone(), usecase.clone());
    let details = ServiceDetails::new(event_tx.clone(), usecase.clone());

    let mut app = App::new(
        event_tx,
        event_rx,
        Rc::new(RefCell::new(table_services)),
        Rc::new(RefCell::new(filter)),
        Rc::new(RefCell::new(service_log)),
        Rc::new(RefCell::new(details)),
        usecase
    );
    app.init();
    let result = app.run(terminal);
    ratatui::restore();
    result
}
