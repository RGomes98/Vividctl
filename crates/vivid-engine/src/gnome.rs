//! The GNOME Shell backend: vibrance in, saturation out.
//!
//! `vivid-gnome` speaks saturation and knows nothing of vibrance; `vivid-color` knows the mapping
//! and nothing of the bus. This adapter joins them, so neither crate depends on the other.

use vivid_color::{saturation_for, vibrance_for};
use vivid_core::{Backend, BackendError, BackendOutput, Capabilities, OutputHandle, Vibrance};
use vivid_gnome::{GnomeError, INTERFACE_VERSION, Transport};

/// The outputs Mutter drives, controlled through the Shell extension.
pub struct GnomeBackend<T: Transport> {
    transport:  T,
    /// The connector behind each handle given out by the last call to [`Backend::outputs`].
    connectors: Vec<String>,
}

impl<T: Transport> GnomeBackend<T> {
    /// A backend over `transport`.
    #[must_use]
    pub const fn new(transport: T) -> Self {
        Self {
            transport,
            connectors: Vec::new(),
        }
    }

    fn connector(&self, output: OutputHandle) -> Result<&str, BackendError> {
        usize::try_from(output.0)
            .ok()
            .and_then(|index| self.connectors.get(index))
            .map(String::as_str)
            .ok_or(BackendError::UnknownOutput(output))
    }
}

impl<T: Transport> Backend for GnomeBackend<T> {
    fn name(&self) -> &'static str {
        "gnome"
    }

    fn probe(&mut self) -> Result<Capabilities, BackendError> {
        let info = self.transport.extension_info()?;
        if info.version != INTERFACE_VERSION {
            return Err(GnomeError::VersionMismatch {
                found:     info.version,
                supported: INTERFACE_VERSION,
            }
            .into());
        }
        Ok(Capabilities {
            description:   format!("GNOME Shell {} extension, interface {}", info.shell_version, info.version),
            can_read_back: true,
        })
    }

    fn outputs(&mut self) -> Result<Vec<BackendOutput>, BackendError> {
        let monitors = self.transport.monitors()?;
        self.connectors = monitors.iter().map(|m| m.connector.clone()).collect();
        Ok(monitors
            .into_iter()
            .zip(0_u64..)
            .map(|(monitor, handle)| BackendOutput {
                handle:    OutputHandle(handle),
                connector: monitor.connector,
                spec:      monitor.spec,
                // Mutter lists only monitors that are attached and active.
                connected: true,
            })
            .collect())
    }

    fn vibrance(&mut self, output: OutputHandle) -> Result<Vibrance, BackendError> {
        let connector: String = self.connector(output)?.to_owned();
        Ok(vibrance_for(self.transport.saturation(&connector)?))
    }

    fn set_vibrance(&mut self, output: OutputHandle, value: Vibrance) -> Result<(), BackendError> {
        let connector: String = self.connector(output)?.to_owned();
        self.transport.set_saturation(&connector, saturation_for(value))?;
        let actual: Vibrance = vibrance_for(self.transport.saturation(&connector)?);
        if actual == value {
            Ok(())
        } else {
            Err(BackendError::ReadbackMismatch {
                expected: value,
                actual,
            })
        }
    }
}

#[cfg(test)]
// Tests build fixtures with expect and index into small vectors.
#[allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use vivid_core::MonitorSpec;
    use vivid_gnome::{ExtensionInfo, MutterMonitor, fake::FakeShell};

    use super::*;

    fn monitor(connector: &str, serial: &str) -> MutterMonitor {
        MutterMonitor {
            connector: connector.into(),
            spec:      MonitorSpec::new("DEL", "Dell Inspiron panel", serial),
        }
    }

    fn backend() -> GnomeBackend<FakeShell> {
        let mut backend = GnomeBackend::new(FakeShell::new(vec![monitor("eDP-1", "0x1"), monitor("HDMI-1", "0x2")]));
        backend.outputs().expect("fake lists monitors");
        backend
    }

    #[test]
    fn probe_describes_the_shell_and_the_interface() {
        let capabilities = GnomeBackend::new(FakeShell::new(vec![])).probe().expect("healthy");
        assert_eq!(capabilities.description, "GNOME Shell 50.1 extension, interface 1");
        assert!(capabilities.can_read_back);
    }

    #[test]
    fn probe_fails_with_guidance_when_the_extension_is_not_running() {
        let mut shell = FakeShell::new(vec![]);
        shell.extension = None;
        let error = GnomeBackend::new(shell).probe().expect_err("no extension");
        assert!(
            matches!(&error, BackendError::Unavailable(m) if m.contains("install.sh")),
            "{error}"
        );
    }

    #[test]
    fn probe_refuses_an_interface_it_does_not_understand() {
        let mut shell = FakeShell::new(vec![]);
        shell.extension = Some(ExtensionInfo {
            version:       INTERFACE_VERSION + 1,
            shell_version: "99".into(),
        });
        let error = GnomeBackend::new(shell).probe().expect_err("newer interface");
        assert!(
            matches!(&error, BackendError::Unavailable(m) if m.contains("update the extension")),
            "{error}"
        );
    }

    #[test]
    fn outputs_are_the_monitors_mutter_lists_and_all_connected() {
        let mut backend = GnomeBackend::new(FakeShell::new(vec![monitor("eDP-1", "0x1"), monitor("HDMI-1", "0x2")]));
        let outputs = backend.outputs().expect("lists");
        assert_eq!(
            outputs.iter().map(|o| o.connector.as_str()).collect::<Vec<_>>(),
            ["eDP-1", "HDMI-1"]
        );
        assert!(outputs.iter().all(|o| o.connected));
        assert_ne!(outputs[0].handle, outputs[1].handle);
    }

    #[test]
    fn a_set_value_is_read_back_unchanged() {
        let mut backend = backend();
        let panel = OutputHandle(0);
        for level in [-1024, -300, -1, 0, 1, 512, 1023] {
            let value = Vibrance::new(level).expect("in range");
            backend.set_vibrance(panel, value).expect("accepted");
            assert_eq!(backend.vibrance(panel), Ok(value), "level {level}");
        }
    }

    #[test]
    fn setting_one_monitor_leaves_the_other_alone() {
        let mut backend = backend();
        backend.set_vibrance(OutputHandle(0), Vibrance::MAX).expect("accepted");
        assert_eq!(backend.vibrance(OutputHandle(1)), Ok(Vibrance::NEUTRAL));
    }

    #[test]
    fn reset_sends_the_neutral_saturation() {
        let mut backend = backend();
        backend.set_vibrance(OutputHandle(0), Vibrance::MAX).expect("accepted");
        backend.reset(OutputHandle(0)).expect("reset");
        assert_eq!(backend.vibrance(OutputHandle(0)), Ok(Vibrance::NEUTRAL));
        assert!((backend.transport.saturation("eDP-1").expect("reads") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_handle_nobody_gave_out_is_unknown() {
        let mut backend = backend();
        assert_eq!(
            backend.vibrance(OutputHandle(7)),
            Err(BackendError::UnknownOutput(OutputHandle(7)))
        );
        assert_eq!(
            backend.set_vibrance(OutputHandle(u64::MAX), Vibrance::NEUTRAL),
            Err(BackendError::UnknownOutput(OutputHandle(u64::MAX)))
        );
    }

    #[test]
    fn handles_are_unknown_until_the_outputs_are_listed() {
        let mut backend = GnomeBackend::new(FakeShell::new(vec![monitor("eDP-1", "0x1")]));
        assert_eq!(
            backend.vibrance(OutputHandle(0)),
            Err(BackendError::UnknownOutput(OutputHandle(0)))
        );
    }

    #[test]
    fn a_value_the_extension_stored_differently_is_reported() {
        let mut backend = backend();
        backend.transport.corrupt_to = Some(1.0);
        let wanted = Vibrance::new(500).expect("in range");
        assert_eq!(
            backend.set_vibrance(OutputHandle(0), wanted),
            Err(BackendError::ReadbackMismatch {
                expected: wanted,
                actual:   Vibrance::NEUTRAL,
            })
        );
    }

    #[test]
    fn a_bus_failure_is_an_i_o_error() {
        let mut backend = backend();
        backend.transport.fail_sets = true;
        assert!(matches!(
            backend.set_vibrance(OutputHandle(0), Vibrance::MAX),
            Err(BackendError::Io(_))
        ));
    }
}
