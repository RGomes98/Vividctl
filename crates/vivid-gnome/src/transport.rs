//! The calls the backend needs, as a trait, so everything above them can be tested without a bus.

use vivid_core::MonitorSpec;

use crate::GnomeError;

/// The interface version of `org.vividctl.Shell1` this build understands.
pub const INTERFACE_VERSION: u32 = 1;

/// A monitor as Mutter reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutterMonitor {
    /// Mutter's connector name (`eDP-1`, `HDMI-1`, `Meta-0`), the key the extension takes.
    pub connector: String,
    /// Vendor, product and serial as Mutter derived them from the EDID (ADR-0003).
    pub spec:      MonitorSpec,
}

/// What the extension says about itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionInfo {
    /// The interface version.
    pub version:       u32,
    /// The running GNOME Shell version.
    pub shell_version: String,
}

/// Mutter's monitor list and the extension's calls. One implementation talks to the session bus
/// ([`crate::SessionBus`]); [`crate::fake::FakeShell`] stands in for it in tests.
pub trait Transport {
    /// The extension's interface version and the Shell version.
    ///
    /// # Errors
    ///
    /// [`GnomeError::NoExtension`] when the extension does not own its bus name.
    fn extension_info(&mut self) -> Result<ExtensionInfo, GnomeError>;

    /// The monitors Mutter currently drives.
    ///
    /// # Errors
    ///
    /// [`GnomeError::NoMutter`] when Mutter does not answer, [`GnomeError::BadReply`] on a reply of
    /// the wrong shape.
    fn monitors(&mut self) -> Result<Vec<MutterMonitor>, GnomeError>;

    /// Sets the saturation of the monitor on `connector`. `1.0` is neutral.
    ///
    /// # Errors
    ///
    /// [`GnomeError::Rejected`] for an unknown connector or a value out of range.
    fn set_saturation(&mut self, connector: &str, saturation: f64) -> Result<(), GnomeError>;

    /// The saturation the extension holds for `connector`.
    ///
    /// # Errors
    ///
    /// [`GnomeError::Rejected`] for an unknown connector.
    fn saturation(&mut self, connector: &str) -> Result<f64, GnomeError>;
}
