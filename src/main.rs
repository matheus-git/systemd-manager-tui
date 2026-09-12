mod domain;
mod infrastructure;
mod terminal;
#[cfg(test)]
mod test_support;
mod usecases;
use infrastructure::notifier::start_notifier;
use infrastructure::systemd_service_adapter::{
    ConnectionType, PollingConfig, SystemdServiceAdapter,
};
use terminal::app::App;
use usecases::services_manager::ServicesManager;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use terminal::app::AppEvent;

use terminal::components::details::ServiceDetails;
use terminal::components::filter::Filter;
use terminal::components::list::TableServices;
use terminal::components::log::ServiceLog;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Filter text applied on startup
    #[arg(short, long)]
    filter: Option<String>,

    /// Maximum time to wait for a systemd job to finish
    #[arg(long, default_value_t = 30)]
    operation_timeout_seconds: u64,
}

#[derive(Clone)]
pub struct Config {
    pub filter: String,
    pub operation_timeout_seconds: u64,
}

impl From<Args> for Config {
    fn from(args: Args) -> Self {
        Self {
            filter: args.filter.unwrap_or_default(),
            operation_timeout_seconds: args.operation_timeout_seconds,
        }
    }
}

struct TerminalRestoreGuard;

impl Drop for TerminalRestoreGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let args: Config = Args::parse().into();
    let terminal = ratatui::init();
    let _terminal_restore_guard = TerminalRestoreGuard;

    let (event_tx, event_rx) = mpsc::channel::<AppEvent>();

    start_notifier();
    let polling = PollingConfig::new(
        Duration::from_secs(args.operation_timeout_seconds),
        Duration::from_millis(100),
    );
    let systemd_adapter =
        SystemdServiceAdapter::with_polling_config(ConnectionType::System, polling)?;
    let usecase = Rc::new(RefCell::new(ServicesManager::new(Box::new(
        systemd_adapter,
    ))));
    let table_services = TableServices::new(event_tx.clone(), usecase.clone());
    let filter = Filter::new(event_tx.clone(), args.filter.clone());
    let service_log = ServiceLog::new(event_tx.clone());
    let details = ServiceDetails::new(event_tx.clone());

    let mut app = App::new(
        event_tx,
        event_rx,
        table_services,
        filter,
        service_log,
        details,
        usecase,
    );
    app.init(args);
    app.run(terminal)
}
