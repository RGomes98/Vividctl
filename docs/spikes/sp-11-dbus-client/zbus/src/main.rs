//! Reads the monitors Mutter reports, over the session bus, with zbus (blocking API).

use std::collections::HashMap;

use zbus::{blocking::Connection, zvariant::OwnedValue};

type Mode = (String, i32, i32, f64, f64, Vec<f64>, HashMap<String, OwnedValue>);
type Monitor = ((String, String, String, String), Vec<Mode>, HashMap<String, OwnedValue>);
type LogicalMonitor = (
    i32,
    i32,
    f64,
    u32,
    bool,
    Vec<(String, String, String, String)>,
    HashMap<String, OwnedValue>,
);
type State = (u32, Vec<Monitor>, Vec<LogicalMonitor>, HashMap<String, OwnedValue>);

fn main() -> zbus::Result<()> {
    let connection = Connection::session()?;
    let reply = connection.call_method(
        Some("org.gnome.Mutter.DisplayConfig"),
        "/org/gnome/Mutter/DisplayConfig",
        Some("org.gnome.Mutter.DisplayConfig"),
        "GetCurrentState",
        &(),
    )?;
    let (_serial, monitors, _logical, _properties): State = reply.body().deserialize()?;
    for ((connector, vendor, product, serial), _, _) in monitors {
        println!("{connector}  {vendor}:{product}:{serial}");
    }
    Ok(())
}
