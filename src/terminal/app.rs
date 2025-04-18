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

#[derive(PartialEq)]
enum Status {
    List,
    Details
}

#[derive(PartialEq)]
pub enum Actions {
    RefreshLog,
    GoList,
    GoLog,
    Updatelog((String, String))
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
    filter: Rc<RefCell<Filter<'a>>>,
    service_log: Rc<RefCell<ServiceLog<'a>>>,
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
            filter: Rc::new(RefCell::new(Filter::new())),
            service_log: Rc::new(RefCell::new(ServiceLog::new(event_tx.clone()))),
            event_rx,
            event_tx
        }
    }

    pub fn init(&mut self) {
        self.filter.borrow_mut().set_table_service(Rc::clone(&self.table_service));
        spawn_key_event_listener(self.event_tx.clone());
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;

        let table_service = Rc::clone(&self.table_service);
        let filter = Rc::clone(&self.filter);
        let service_details = Rc::clone(&self.service_log);


        while self.running {
            match self.status {
                Status::Details => self.draw_details_status(&mut terminal, &service_details)?,
                Status::List => self.draw_list_status(&mut terminal, &filter, &table_service)?
            } 

            match self.event_rx.recv()? {
                AppEvent::Key(key) => match self.status {
                    Status::Details => {
                        self.on_key_event(key);
                        self.service_log.borrow_mut().on_key_event(key)
                    },
                    Status::List => {
                        self.on_key_event(key);
                        self.table_service.borrow_mut().on_key_event(key);
                        self.filter.borrow_mut().on_key_event(key);
                    }
                },
                AppEvent::Action(Actions::Updatelog(log)) => {
                    self.service_log.borrow_mut().update(log.0, log.1);
                },
                AppEvent::Action(Actions::RefreshLog) => {
                    if self.status == Status::Details {
                        if let Some(service) = self.table_service.borrow_mut().get_selected_service() {
                            self.service_log.borrow_mut().fetch_log_and_dispatch(service.clone());
                        }
                    }
                },
                AppEvent::Action(Actions::GoLog) => {
                    self.status = Status::Details;
                    self.event_tx.send(AppEvent::Action(Actions::RefreshLog)).unwrap();
                    self.service_log.borrow_mut().start_auto_refresh();
                },
                AppEvent::Action(Actions::GoList) => self.status = Status::List,
            }
        }

        Ok(())
    }

    fn draw_details_status(&mut self,  terminal: &mut DefaultTerminal, service_details: &Rc<RefCell<ServiceLog>>)-> Result<()> {
        terminal.draw(|frame| {
            let area = frame.area();

            let [list_box, help_area_box] = Layout::vertical([
                Constraint::Min(0),     
                Constraint::Length(6),  
            ])
                .areas(area);

            service_details.borrow_mut().render(frame, list_box);
            service_details.borrow_mut().draw_shortcuts(frame, help_area_box);                
        })?;

        Ok(())
    }

    fn draw_list_status(&mut self, terminal: &mut DefaultTerminal, filter: &Rc<RefCell<Filter>>, table_service: &Rc<RefCell<TableServices>>)-> Result<()>{
        terminal.draw(|frame| {
            let area = frame.area();

            let [filter_box, list_box, help_area_box] = Layout::vertical([
                Constraint::Length(4),    
                Constraint::Min(10),     
                Constraint::Length(7),  
            ])
                .areas(area);

            filter.borrow_mut().draw(frame, filter_box);
            table_service.borrow_mut().render(frame, list_box);
            self.draw_shortcuts(frame, help_area_box);                
        })?;

        Ok(())
    }


    fn draw_shortcuts(&mut self, frame: &mut Frame, help_area: Rect){
        let help_text = vec![
            Line::from(vec![
                Span::styled("Actions on the selected service", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("Navigate: ↑/↓ | Start: s | Stop: x | Restart: r | Enable: e | Disable: d | Refresh all: u | View logs: v"),
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled("Exit", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw(": Ctrl + c"),
            ]),
        ];

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
