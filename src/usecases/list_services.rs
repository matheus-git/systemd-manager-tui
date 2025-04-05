use zbus::{blocking::Connection, blocking::Proxy};
use std::error::Error;
use zvariant::OwnedObjectPath;


pub fn list_services() -> Result<Vec<Vec<String>>, Box<dyn Error>> {
    let connection = Connection::system()?;

    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    )?;

    let units: Vec<(
        String,         // unit name
        String,         // description
        String,         // load state
        String,         // active state
        String,         // sub state
        String,         // followed
        OwnedObjectPath,// object path
        u32,            // job id
        String,         // job type
        OwnedObjectPath // job object path
    )> = proxy.call("ListUnits", &())?;

    let services = units
        .into_iter()
        .filter(|(name, ..)| name.ends_with(".service"))
        .map(|(name, description, load, active, sub, ..)| {
            vec![name, description, load, active, sub]
        })
        .collect();

    Ok(services)
}

