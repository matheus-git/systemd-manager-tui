# Architecture

In this section, I’ll explain how the project is structured. The architectural styles used include DDD (Domain-Driven Design), Hexagonal Architecture, [component architeture](https://ratatui.rs/concepts/application-patterns/component-architecture/) and [Elm Architecture](https://ratatui.rs/concepts/application-patterns/the-elm-architecture/).

    src
    ├── infrastructure            # Files responsible for fetching data from external sources
    ├── domain                    # Business rules and core logic
    ├── usecase                   # Application use cases
    ├── terminal                  # TUI rendering logic
    │   └── components            # Components to be rendered on the terminal
    └── main.rs              

## Infrastructure

This layer is responsible for data retrieval from external systems or services. It may return entities to be processed by the business logic layer. It often contains adapters to technologies such as system APIs, databases, or D-Bus

## Domain

Defines the core business logic of the application. This layer is completely isolated from technical concerns and represents the heart of the software — including entities, value objects, domain services, and business rules.

## Usecase

Contains the application's use cases — it acts as the bridge between the UI and the business rules. It knows how to perform specific actions and orchestrate domain objects and infrastructure when necessary.

## Terminal

Responsible for rendering the TUI (terminal user interface). It handles UI events and triggers the appropriate use cases. It knows what needs to be done, but not how to do it — delegating the execution to the usecase layer.

This is the most code-heavy section, so let’s break it down in detail below.

    terminal
    ├── components
    │   ├── list.rs
    │   ├── details.rs└
    │   ├── filter.rs
    │   ├── log.rs
    └── app.rs   

As mentioned earlier, this project follows the Elm Architecture — it's entirely event-driven.

app.rs
  ````
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
                ...
            },
            ...
            AppEvent::Action(Actions::GoList) => self.status = Status::List,
            AppEvent::Action(Actions::Updatelog(log)) => {
                 self.service_log.borrow_mut().update(log.0, log.1);
            },
            ...
        }
    }

  ````

Whenever an event is triggered — whether it's a key press or a programmatic action — the app responds accordingly and then redraws the terminal. These responses usually involve updating the state of a component. Since app.rs holds a shared reference (Rc<RefCell< T >>), it can directly call methods to update components with new data (or update itself). The render method should be kept as simple as possible — its only job is to reflect the current state visually.

log.rs
  ````
  pub struct ServiceLog<'a> {
      log_paragraph: Option<Paragraph<'a>>,
      log_block: Option<Block<'a>>,
      border_color: BorderColor,
      service_name: String,
      scroll: u16,
      sender: Sender<AppEvent>,
      auto_refresh: Arc<Mutex<bool>>
  }
  ...
  pub fn render(&mut self, frame: &mut Frame, area: Rect) {
      if self.log_paragraph.is_none() || self.log_block.is_none(){
          self.render_loading(frame, area);
          return;
      }

      let log_block = self.log_block.clone().unwrap();
      let paragraph = self.log_paragraph.clone().unwrap()
          .scroll((self.scroll, 0))
          .block(log_block);
      
      frame.render_widget(paragraph, area);
  }
  ...
  ````

The methods responsible for updating data run in background threads. Once these threads complete their tasks, they trigger events that cause the terminal UI to redraw — this helps keep the interface responsive and prevents it from freezing.

log.rs

    pub fn fetch_log_and_dispatch(&mut self, service: Service){
        let event_tx = self.sender.clone();
        thread::spawn(move|| {
            if let Ok(log) = ServicesManager::get_log(&service) {
                event_tx.send(AppEvent::Action(Actions::Updatelog((service.name().to_string(),log))))
                    .expect("Failed to send Updatelog event");
            }
        });
    }

***

If you have any questions or suggestions for improvements regarding this section, feel free to open an issue. Thank you!
