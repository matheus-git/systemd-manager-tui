use zbus::blocking::{Connection, MessageIterator};
use zbus::MatchRule;
use zbus::message::Type;
use zbus::zvariant::{Value, OwnedValue};
use zbus::Error;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

const SLEEP_DURATION: u64 = 300;

pub fn start_notifier() {
    thread::spawn(|| {
        let connection = match Connection::system() {
            Ok(connection) => connection,
            Err(_e) => return
        };

        let notifier = match Notifier::new(connection) {
            Ok(notifier) => notifier,
            Err(_e) => {
                return;
            }
        };

        if let Err(_e) = notifier.watch_failed_services() {
        }
    });

    thread::spawn(|| {
         let connection = match Connection::session() {
            Ok(connection) => connection,
            Err(_e) => return
        };
        
        let notifier = match Notifier::new(connection) {
            Ok(notifier) => notifier,
            Err(_e) => {
                return;
            }
        };

        if let Err(_e) = notifier.watch_failed_services() {
        }
    });
}

pub struct Notifier {
    connection: Connection
}

impl Notifier {
    pub fn new(connection: Connection) -> Result<Self, Error> {
        Ok(Self {
            connection
        })
    }

    pub fn watch_failed_services(&self) -> Result<(), Box<dyn std::error::Error>> {
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender("org.freedesktop.systemd1")?
            .interface("org.freedesktop.DBus.Properties")?
            .member("PropertiesChanged")?
            .build();

        let mut iter = MessageIterator::for_match_rule(
            rule,
            &self.connection,
            Some(64),
        )?;

        loop {
            let msg = match iter.next() {
                Some(Ok(m)) => m,
                _ => {
                    thread::sleep(Duration::from_millis(SLEEP_DURATION));
                    continue
                },
            };

            let (interface, changed, _invalidated): (
                String,
                HashMap<String, OwnedValue>,
                Vec<String>,
            ) = msg.body().deserialize()?;

            if interface != "org.freedesktop.systemd1.Unit" {
                continue;
            }

            if let Some(state_val) = changed.get("ActiveState")
                && let Ok(state) = <&str>::try_from(state_val) 
                    && state == "failed" 
                        && let Some(path) = msg.header().path() {
                            let name = decode_unit_path(path.as_str());
                            self.send_notification(&format!("{} {}", name, state))?;
            }
        }
    }
    
    fn send_notification(
        &self,
        summary: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let body = "Systemd reported a failure";

        let notification_connection = Connection::session()?;
        notification_connection.call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                "systemd-manager-tui",
                0u32,
                "dialog-error",
                summary,
                body,
                Vec::<&str>::new(),
                HashMap::<&str, &Value>::new(),
                5000i32,
            ),
        )?;

        Ok(())
    }
}

fn decode_unit_path(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);

    let mut out = String::with_capacity(name.len());
    let bytes = name.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'_' && i + 2 < bytes.len() 
            && let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) 
                && let Ok(val) = u8::from_str_radix(hex, 16) {
                    out.push(val as char);
                    i += 3;
                    continue;
        }

        out.push(bytes[i] as char);
        i += 1;
    }

    out
}
