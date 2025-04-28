use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use ratatui::style::{Modifier, Style, Color};
use ratatui::widgets::{Paragraph, Block, Borders};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::Frame;
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;
use std::time::Duration;

use std::rc::Rc;
use std::cell::RefCell;

use super::components::list::TableServices;
use super::components::filter::Filter;
use super::components::log::ServiceLog;
use super::components::details::ServiceDetails;

#[derive(PartialEq)]
enum Status {
    List,
    Log,
    Details
}

pub enum Actions {
    RefreshLog,
    RefreshDetails,
    GoList,
    GoLog,
    GoDetails,
    Updatelog((String, String)),
    UpdateDetails,
    Filter(String),
    UpdateIgnoreListKeys(bool)
}

pub enum AppEvent {
    Key(KeyEvent),
    Action(Actions),
}

fn spawn_key_event_listener(event_tx: Sender<AppEvent>) {
    thread::spawn(move || {
        loop {
            if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(Event::Key(key_event)) = event::read() {
                    if key_event.kind == KeyEventKind::Press && event_tx.send(AppEvent::Key(key_event)).is_err() {                            break;
                    }
                }
            }
        }
    });
}
pub struct App<'a> { 
    running: bool,
    status: Status,
    table_service: Rc<RefCell<TableServices<'a>>>,
    filter: Rc<RefCell<Filter>>,
    service_log: Rc<RefCell<ServiceLog<'a>>>,
    details: Rc<RefCell<ServiceDetails>>,
    event_rx: Receiver<AppEvent>,
    event_tx: Sender<AppEvent>,
}

impl App<'_> {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel::<AppEvent>();
        Self {
            running: true,
            status: Status::List,
            table_service: Rc::new(RefCell::new(TableServices::new(event_tx.clone()))),
            filter: Rc::new(RefCell::new(Filter::new(event_tx.clone()))),
            service_log: Rc::new(RefCell::new(ServiceLog::new(event_tx.clone()))),
            details: Rc::new(RefCell::new(ServiceDetails::new(event_tx.clone()))),
            event_rx,
            event_tx
        }
    }

    pub fn init(&mut self) {
        spawn_key_event_listener(self.event_tx.clone());
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;

        let table_service = Rc::clone(&self.table_service);
        let filter = Rc::clone(&self.filter);
        let log = Rc::clone(&self.service_log);
        let details = Rc::clone(&self.details);


        while self.running {
            match self.status {
                Status::Log => self.draw_log_status(&mut terminal, &log)?,
                Status::List => self.draw_list_status(&mut terminal, &filter, &table_service)?,
                Status::Details => self.draw_details_status(&mut terminal, &details)?
            } 

            match self.event_rx.recv()? {
                AppEvent::Key(key) => match self.status {
                    Status::Log => {
                        self.on_key_event(key);
                        self.service_log.borrow_mut().on_key_event(key)
                    },
                    Status::List => {
                        self.on_key_event(key);
                        self.table_service.borrow_mut().on_key_event(key);
                        self.filter.borrow_mut().on_key_event(key);
                    },
                    Status::Details => {
                        self.on_key_event(key);
                        self.details.borrow_mut().on_key_event(key);
                    }
                },
                AppEvent::Action(Actions::UpdateIgnoreListKeys(bool)) => {
                    self.table_service.borrow_mut().set_ignore_key_events(bool);
                }
                AppEvent::Action(Actions::Filter(input)) => {
                    self.table_service.borrow_mut().set_selected_index(0);
                    self.table_service.borrow_mut().refresh(input); 
                },
                AppEvent::Action(Actions::Updatelog(log)) => {
                    self.service_log.borrow_mut().update(log.0, log.1);
                },
                AppEvent::Action(Actions::RefreshLog) => {
                    if self.status == Status::Log {
                        if let Some(service) = self.table_service.borrow_mut().get_selected_service() {
                            self.service_log.borrow_mut().fetch_log_and_dispatch(service.clone());
                        }
                    }
                },
                AppEvent::Action(Actions::GoLog) => {
                    self.status = Status::Log;
                    self.event_tx.send(AppEvent::Action(Actions::RefreshLog)).unwrap();
                    self.service_log.borrow_mut().start_auto_refresh();
                },
                AppEvent::Action(Actions::GoList) => self.status = Status::List,
                AppEvent::Action(Actions::UpdateDetails) => {
                },
                AppEvent::Action(Actions::RefreshDetails) => {
                    if self.status == Status::Details {
                        self.details.borrow_mut().fetch_log_and_dispatch();
                    }
                },
                AppEvent::Action(Actions::GoDetails) => {
                    if let Some(service) = self.table_service.borrow_mut().get_selected_service() {
                        self.details.borrow_mut().update(service.clone());
                    }
                    self.event_tx.send(AppEvent::Action(Actions::RefreshDetails)).unwrap();
                    self.status = Status::Details;
                    self.details.borrow_mut().start_auto_refresh();
                }
            }
        }

        Ok(())
    }

    fn draw_details_status(&mut self,  terminal: &mut DefaultTerminal, service_details: &Rc<RefCell<ServiceDetails>>)-> Result<()> {
        let mut service_details = service_details.borrow_mut();
        terminal.draw(|frame| {
            let area = frame.area();

            let [list_box, help_area_box] = Layout::vertical([
                Constraint::Min(0),     
                Constraint::Max(7),  
            ])
                .areas(area);

            service_details.render(frame, list_box);
            self.draw_shortcuts(frame, help_area_box, service_details.shortcuts());                
        })?;

        Ok(())
    }

    fn draw_log_status(&mut self,  terminal: &mut DefaultTerminal, service_log: &Rc<RefCell<ServiceLog>>)-> Result<()> {
        let mut service_log = service_log.borrow_mut();
        terminal.draw(|frame| {
            let area = frame.area();

            let [list_box, help_area_box] = Layout::vertical([
                Constraint::Min(0),     
                Constraint::Max(7),  
            ])
                .areas(area);

            service_log.render(frame, list_box);
            self.draw_shortcuts(frame, help_area_box, service_log.shortcuts());                
        })?;

        Ok(())
    }

    fn draw_list_status(&mut self, terminal: &mut DefaultTerminal, filter: &Rc<RefCell<Filter>>, table_service: &Rc<RefCell<TableServices>>)-> Result<()>{
        let filter = filter.borrow_mut();
        let mut table = table_service.borrow_mut();
        terminal.draw(|frame| {
            let area = frame.area();

            let [filter_box, list_box, help_area_box] = Layout::vertical([
                Constraint::Length(4),    
                Constraint::Min(10),     
                Constraint::Max(7),  
            ])
                .areas(area);

            filter.draw(frame, filter_box);
            table.render(frame, list_box);
            self.draw_shortcuts(frame, help_area_box, table.shortcuts());                
        })?;

        Ok(())
    }

    fn draw_shortcuts(&mut self, frame: &mut Frame, help_area: Rect, shortcuts: Vec<Line<'_>>) {
        let mut help_text: Vec<Line<'_>> = Vec::new(); 
        let shortcuts_lens = shortcuts.len();

        help_text.extend(shortcuts); 

        if shortcuts_lens > 0 {
            help_text.push(Line::raw(""));
            if help_area.width > 125 {
                help_text.push(Line::raw(""));
            }
        }

        help_text.push(
            Line::from(vec![
                Span::styled("Exit", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw(": Ctrl + c"),
            ])
        );

        let help_block = Paragraph::new(help_text)
            .block(Block::default().title("Shortcuts").borders(Borders::ALL))
            .wrap(ratatui::widgets::Wrap { trim: true });

        frame.render_widget(help_block, help_area);
    }

    fn on_key_event(&mut self, key: KeyEvent) {
        if let (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) = (key.modifiers, key.code) { 
            self.quit() 
        }   
    }

    fn quit(&mut self) {
        self.running = false;
    }
}
