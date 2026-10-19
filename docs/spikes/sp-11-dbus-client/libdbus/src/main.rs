//! Reads the monitors Mutter reports, over the session bus, with the `dbus` crate.

use std::{collections::HashMap, time::Duration};

use dbus::{
    arg::{RefArg, Variant},
    blocking::Connection,
};

type Props = HashMap<String, Variant<Box<dyn RefArg>>>;
type Mode = (String, i32, i32, f64, f64, Vec<f64>, Props);
type Monitor = ((String, String, String, String), Vec<Mode>, Props);
type LogicalMonitor = (i32, i32, f64, u32, bool, Vec<(String, String, String, String)>, Props);
type State = (u32, Vec<Monitor>, Vec<LogicalMonitor>, Props);

fn main() -> Result<(), dbus::Error> {
    let connection = Connection::new_session()?;
    let proxy = connection.with_proxy(
        "org.gnome.Mutter.DisplayConfig",
        "/org/gnome/Mutter/DisplayConfig",
        Duration::from_secs(5),
    );
    let (_serial, monitors, _logical, _props): State =
        proxy.method_call("org.gnome.Mutter.DisplayConfig", "GetCurrentState", ())?;
    for ((connector, vendor, product, serial), _, _) in monitors {
        println!("{connector}  {vendor}:{product}:{serial}");
    }
    Ok(())
}
