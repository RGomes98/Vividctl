//! The session bus, through the system libdbus (ADR-0007).

use std::{collections::HashMap, time::Duration};

use dbus::{
    Error as DbusError,
    arg::{RefArg, Variant},
    blocking::{Connection, Proxy, stdintf::org_freedesktop_dbus::Properties},
};
use vivid_core::MonitorSpec;

use crate::{ExtensionInfo, GnomeError, MutterMonitor, Transport};

const TIMEOUT: Duration = Duration::from_secs(5);

const MUTTER_NAME: &str = "org.gnome.Mutter.DisplayConfig";
const MUTTER_PATH: &str = "/org/gnome/Mutter/DisplayConfig";

const SHELL_NAME: &str = "org.vividctl.Shell";
const SHELL_PATH: &str = "/org/vividctl/Shell";
const SHELL_INTERFACE: &str = "org.vividctl.Shell1";

type Props = HashMap<String, Variant<Box<dyn RefArg>>>;
type Mode = (String, i32, i32, f64, f64, Vec<f64>, Props);
type Monitor = ((String, String, String, String), Vec<Mode>, Props);
type LogicalMonitor = (i32, i32, f64, u32, bool, Vec<(String, String, String, String)>, Props);
/// `GetCurrentState`: serial, monitors, logical monitors, properties.
type State = (u32, Vec<Monitor>, Vec<LogicalMonitor>, Props);

/// The real [`Transport`]: one connection to the session bus.
pub struct SessionBus {
    connection: Connection,
}

impl SessionBus {
    /// Connects to the session bus.
    ///
    /// # Errors
    ///
    /// [`GnomeError::Bus`] when there is no session bus (no `DBUS_SESSION_BUS_ADDRESS`, no login
    /// session).
    pub fn connect() -> Result<Self, GnomeError> {
        Connection::new_session()
            .map(|connection| Self { connection })
            .map_err(|e| GnomeError::Bus(describe(&e)))
    }

    fn mutter(&self) -> Proxy<'_, &Connection> {
        self.connection.with_proxy(MUTTER_NAME, MUTTER_PATH, TIMEOUT)
    }

    fn shell(&self) -> Proxy<'_, &Connection> {
        self.connection.with_proxy(SHELL_NAME, SHELL_PATH, TIMEOUT)
    }
}

impl Transport for SessionBus {
    fn extension_info(&mut self) -> Result<ExtensionInfo, GnomeError> {
        let shell: Proxy<'_, &Connection> = self.shell();
        let version: u32 = shell.get(SHELL_INTERFACE, "Version").map_err(|e| extension_error(&e))?;
        let shell_version: String = shell
            .get(SHELL_INTERFACE, "ShellVersion")
            .map_err(|e| extension_error(&e))?;
        Ok(ExtensionInfo { version, shell_version })
    }

    fn monitors(&mut self) -> Result<Vec<MutterMonitor>, GnomeError> {
        let (_serial, monitors, _logical, _props): State = self
            .mutter()
            .method_call(MUTTER_NAME, "GetCurrentState", ())
            .map_err(|e| mutter_error(&e))?;
        Ok(monitors
            .into_iter()
            .map(|((connector, vendor, product, serial), _modes, _props)| MutterMonitor {
                connector,
                spec: MonitorSpec::new(vendor, product, serial),
            })
            .collect())
    }

    fn set_saturation(&mut self, connector: &str, saturation: f64) -> Result<(), GnomeError> {
        self.shell()
            .method_call(SHELL_INTERFACE, "SetSaturation", (connector, saturation))
            .map_err(|e| extension_error(&e))
    }

    fn saturation(&mut self, connector: &str) -> Result<f64, GnomeError> {
        let (saturation,): (f64,) = self
            .shell()
            .method_call(SHELL_INTERFACE, "GetSaturation", (connector,))
            .map_err(|e| extension_error(&e))?;
        Ok(saturation)
    }
}

fn describe(error: &DbusError) -> String {
    match (error.name(), error.message()) {
        (Some(name), Some(message)) => format!("{name}: {message}"),
        (Some(name), None) => name.to_owned(),
        _ => "unknown D-Bus error".to_owned(),
    }
}

/// A failure of a call to Mutter: an absent service means this is not GNOME.
fn mutter_error(error: &DbusError) -> GnomeError {
    match error.name() {
        Some("org.freedesktop.DBus.Error.ServiceUnknown" | "org.freedesktop.DBus.Error.NameHasNoOwner") => {
            GnomeError::NoMutter(describe(error))
        }
        Some("org.freedesktop.DBus.Error.InvalidArgs" | "org.freedesktop.DBus.Error.InvalidSignature") => {
            GnomeError::BadReply(describe(error))
        }
        _ => GnomeError::Bus(describe(error)),
    }
}

/// A failure of a call to the extension. The shell activates nothing for this name, so an unowned
/// name means the extension is not running; a refused argument is the extension saying no.
fn extension_error(error: &DbusError) -> GnomeError {
    match error.name() {
        Some("org.freedesktop.DBus.Error.ServiceUnknown" | "org.freedesktop.DBus.Error.NameHasNoOwner") => {
            GnomeError::NoExtension(describe(error))
        }
        Some("org.freedesktop.DBus.Error.InvalidArgs") => GnomeError::Rejected(
            error
                .message()
                .map_or_else(|| "invalid arguments".to_owned(), ToOwned::to_owned),
        ),
        Some("org.freedesktop.DBus.Error.UnknownMethod" | "org.freedesktop.DBus.Error.UnknownProperty") => {
            GnomeError::BadReply(describe(error))
        }
        _ => GnomeError::Bus(describe(error)),
    }
}
