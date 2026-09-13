mod domain;
mod infrastructure;
mod terminal;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
mod usecases;
use infrastructure::notifier::start_notifier;
use infrastructure::systemd_service_adapter::{ConnectionType, SystemdServiceAdapter};
use terminal::app::App;
use usecases::services_manager::ServicesManager;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use terminal::app::{Actions, AppEvent};

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

    /// Maximum time to wait for a systemd service operation to settle
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..))]
    operation_timeout_secs: u64,
}

#[derive(Clone)]
pub struct Config {
    pub filter: String,
    pub operation_timeout: Duration,
}

impl From<Args> for Config {
    fn from(args: Args) -> Self {
        Self {
            filter: args.filter.unwrap_or_default(),
            operation_timeout: Duration::from_secs(args.operation_timeout_secs),
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
    let mut terminal = ratatui::init();
    let _terminal_restore_guard = TerminalRestoreGuard;

    let (event_tx, event_rx) = mpsc::channel::<AppEvent>();
    let (completion_tx, completion_rx) = mpsc::channel();
    let completion_event_tx = event_tx.clone();
    std::thread::spawn(move || {
        while let Ok(completion) = completion_rx.recv() {
            if completion_event_tx
                .send(AppEvent::Action(Actions::OperationCompleted(completion)))
                .is_err()
            {
                break;
            }
        }
    });

    start_notifier();
    let mut systemd_adapter =
        SystemdServiceAdapter::new(ConnectionType::System, args.operation_timeout)?;
    systemd_adapter.set_completion_sender(completion_tx);
    let usecase = Rc::new(RefCell::new(ServicesManager::new(Box::new(
        systemd_adapter,
    ))));
    let table_services = TableServices::new(event_tx.clone(), usecase.clone());
    let filter = Filter::new(event_tx.clone(), args.filter.clone());
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
    app.init(args);
    app.run(&mut terminal)
}
