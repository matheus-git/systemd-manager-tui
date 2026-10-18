mod domain;
mod infrastructure;
mod keybindings;
mod terminal;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
mod usecases;
use domain::connection_type::ConnectionType;
use infrastructure::notifier::start_notifier;
use infrastructure::systemd_service_adapter::SystemdServiceAdapter;
use terminal::app::App;
use usecases::services_manager::ServicesManager;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
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

    /// Disable notifications for failed system and user units
    #[arg(short = 'n', long)]
    no_notifications: bool,

    /// Configuration file to read instead of the default
    /// ~/.config/systemd-manager-tui/config.toml
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Print a commented configuration file with every keybind at its default
    /// value, then exit
    #[arg(long)]
    print_default_config: bool,
}

#[derive(Clone)]
pub struct Config {
    pub filter: String,
    pub operation_timeout: Duration,
    pub failure_notifications: bool,
}

impl From<Args> for Config {
    fn from(args: Args) -> Self {
        Self {
            filter: args.filter.unwrap_or_default(),
            operation_timeout: Duration::from_secs(args.operation_timeout_secs),
            failure_notifications: !args.no_notifications,
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
    let parsed = Args::parse();

    if parsed.print_default_config {
        print!("{}", keybindings::DEFAULT_CONFIG_TEMPLATE);
        return Ok(());
    }

    // Keybindings are resolved before the terminal is taken over so a bad
    // configuration is reported on a normal screen instead of behind the TUI.
    let keybindings = match keybindings::load(parsed.config.as_deref()) {
        Ok(keybindings) => Arc::new(keybindings),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    let args: Config = parsed.into();
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

    if args.failure_notifications {
        start_notifier();
    }
    let mut systemd_adapter =
        SystemdServiceAdapter::new(ConnectionType::System, args.operation_timeout)?;
    systemd_adapter.set_completion_sender(completion_tx);
    let usecase = Rc::new(RefCell::new(ServicesManager::new(Box::new(
        systemd_adapter,
    ))));
    let table_services = TableServices::new(event_tx.clone(), usecase.clone(), keybindings.clone());
    let filter = Filter::new(event_tx.clone(), args.filter.clone(), keybindings.clone());
    let service_log = ServiceLog::new(event_tx.clone(), usecase.clone(), keybindings.clone());
    let details = ServiceDetails::new(event_tx.clone(), usecase.clone(), keybindings.clone());

    let mut app = App::new(
        event_tx,
        event_rx,
        table_services,
        filter,
        service_log,
        details,
        usecase,
        keybindings,
    );
    app.init(args);
    app.run(&mut terminal)
}
