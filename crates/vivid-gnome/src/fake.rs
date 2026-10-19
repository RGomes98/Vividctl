//! A stand-in for the session bus: a fixed monitor list and an extension that keeps values in
//! memory and refuses what the real one refuses.

use std::collections::BTreeMap;

use crate::{ExtensionInfo, GnomeError, INTERFACE_VERSION, MutterMonitor, Transport};

/// The saturation range the extension accepts.
const ACCEPTED: std::ops::RangeInclusive<f64> = 0.0..=4.0;

/// An in-memory Mutter and extension.
#[derive(Clone, Debug)]
pub struct FakeShell {
    /// What `extension_info` answers; `None` simulates an extension that is not running.
    pub extension:  Option<ExtensionInfo>,
    /// What `monitors` answers.
    pub monitors:   Vec<MutterMonitor>,
    /// Makes every `set_saturation` fail with a bus error, to exercise failure paths.
    pub fail_sets:  bool,
    /// Stores this instead of what was set, to exercise the readback check.
    pub corrupt_to: Option<f64>,
    saturations:    BTreeMap<String, f64>,
}

impl FakeShell {
    /// A shell with a healthy extension and these monitors.
    #[must_use]
    pub fn new(monitors: Vec<MutterMonitor>) -> Self {
        Self {
            extension: Some(ExtensionInfo {
                version:       INTERFACE_VERSION,
                shell_version: "50.1".into(),
            }),
            monitors,
            fail_sets: false,
            corrupt_to: None,
            saturations: BTreeMap::new(),
        }
    }

    fn known(&self, connector: &str) -> Result<(), GnomeError> {
        if self.monitors.iter().any(|m| m.connector == connector) {
            Ok(())
        } else {
            Err(GnomeError::Rejected(format!("unknown monitor '{connector}'")))
        }
    }
}

impl Transport for FakeShell {
    fn extension_info(&mut self) -> Result<ExtensionInfo, GnomeError> {
        self.extension
            .clone()
            .ok_or_else(|| GnomeError::NoExtension("the name org.vividctl.Shell has no owner".into()))
    }

    fn monitors(&mut self) -> Result<Vec<MutterMonitor>, GnomeError> {
        Ok(self.monitors.clone())
    }

    fn set_saturation(&mut self, connector: &str, saturation: f64) -> Result<(), GnomeError> {
        if self.fail_sets {
            return Err(GnomeError::Bus("connection lost".into()));
        }
        self.known(connector)?;
        if !ACCEPTED.contains(&saturation) {
            return Err(GnomeError::Rejected(format!("saturation {saturation} is outside 0..4")));
        }
        self.saturations
            .insert(connector.to_owned(), self.corrupt_to.unwrap_or(saturation));
        Ok(())
    }

    fn saturation(&mut self, connector: &str) -> Result<f64, GnomeError> {
        self.known(connector)?;
        Ok(self.saturations.get(connector).copied().unwrap_or(1.0))
    }
}
