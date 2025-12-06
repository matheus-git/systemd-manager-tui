mod domain;
mod infrastructure;
mod terminal;
mod usecases;
use infrastructure::systemd_service_adapter::{ConnectionType, SystemdServiceAdapter};
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
use std::time::Instant;
use std::sync::Arc;
use logcast::init_on_addr;

fn main() -> color_eyre::Result<()> {
    init_on_addr("127.0.0.1:8080");
    let start = Instant::now();
    color_eyre::install()?;
    let terminal = ratatui::init();
    
    let (event_tx, event_rx) = mpsc::channel::<AppEvent>();

    let usecase = Rc::new(RefCell::new(ServicesManager::new(Box::new(
        SystemdServiceAdapter::new(ConnectionType::System)?,
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
    let _elapsed = start.elapsed();
    //panic!("Tempo de execução: {:.2?}", _elapsed);
    let result = app.run(terminal);
    ratatui::restore();
    result
}
