//! What a way of changing vibrance has to offer. NVKMS and the GNOME Shell extension implement it.

use std::fmt;

use crate::{BackendError, MonitorSpec, Vibrance};

/// An output as one backend names it. The meaning of the number is private to that backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutputHandle(pub u64);

impl fmt::Display for OutputHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// An output as a backend sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendOutput {
    /// How to refer to this output when talking to the backend.
    pub handle:    OutputHandle,
    /// The connector name in the backend's own naming (`HDMI-0` for NVKMS, `HDMI-1` for Mutter).
    pub connector: String,
    /// The monitor spec, built from the EDID the backend exposes.
    pub spec:      MonitorSpec,
    /// Whether a monitor is attached and active.
    pub connected: bool,
}

/// What a backend reports about itself after probing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capabilities {
    /// A line for humans: the driver or shell version and anything that explains behaviour.
    pub description:   String,
    /// Whether the value can be read back after being set.
    pub can_read_back: bool,
}

/// A way of changing the vibrance of outputs.
pub trait Backend {
    /// A short stable name, for messages and for choosing a backend.
    fn name(&self) -> &'static str;

    /// Checks that the backend can run here and says what it found.
    ///
    /// # Errors
    ///
    /// [`BackendError::Unavailable`] when it cannot run (driver not loaded, extension missing).
    fn probe(&mut self) -> Result<Capabilities, BackendError>;

    /// The outputs this backend can see.
    ///
    /// # Errors
    ///
    /// Whatever went wrong talking to the system.
    fn outputs(&mut self) -> Result<Vec<BackendOutput>, BackendError>;

    /// The current vibrance of an output.
    ///
    /// # Errors
    ///
    /// [`BackendError::UnknownOutput`] for a handle the backend does not know, or a system error.
    fn vibrance(&mut self, output: OutputHandle) -> Result<Vibrance, BackendError>;

    /// Sets the vibrance of an output.
    ///
    /// Implementations that can read the value back must do so and return
    /// [`BackendError::ReadbackMismatch`] when it differs.
    ///
    /// # Errors
    ///
    /// [`BackendError::UnknownOutput`], [`BackendError::Rejected`], or a system error.
    fn set_vibrance(&mut self, output: OutputHandle, value: Vibrance) -> Result<(), BackendError>;

    /// Puts an output back to neutral.
    ///
    /// # Errors
    ///
    /// The same as [`Backend::set_vibrance`].
    fn reset(&mut self, output: OutputHandle) -> Result<(), BackendError> {
        self.set_vibrance(output, Vibrance::NEUTRAL)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A backend that remembers values, used to exercise the trait and its default method.
    struct Memory {
        value:            Vibrance,
        corrupt_readback: bool,
    }

    impl Backend for Memory {
        fn name(&self) -> &'static str {
            "memory"
        }

        fn probe(&mut self) -> Result<Capabilities, BackendError> {
            Ok(Capabilities {
                description:   "in memory".into(),
                can_read_back: true,
            })
        }

        fn outputs(&mut self) -> Result<Vec<BackendOutput>, BackendError> {
            Ok(vec![BackendOutput {
                handle:    OutputHandle(1),
                connector: "MEM-1".into(),
                spec:      MonitorSpec::unknown(),
                connected: true,
            }])
        }

        fn vibrance(&mut self, output: OutputHandle) -> Result<Vibrance, BackendError> {
            if output == OutputHandle(1) {
                Ok(self.value)
            } else {
                Err(BackendError::UnknownOutput(output))
            }
        }

        fn set_vibrance(&mut self, output: OutputHandle, value: Vibrance) -> Result<(), BackendError> {
            if output != OutputHandle(1) {
                return Err(BackendError::UnknownOutput(output));
            }
            self.value = if self.corrupt_readback { Vibrance::MAX } else { value };
            if self.value == value {
                Ok(())
            } else {
                Err(BackendError::ReadbackMismatch {
                    expected: value,
                    actual:   self.value,
                })
            }
        }
    }

    #[test]
    fn reset_defaults_to_setting_neutral() {
        let mut backend: Memory = Memory {
            value:            Vibrance::MAX,
            corrupt_readback: false,
        };
        assert_eq!(backend.reset(OutputHandle(1)), Ok(()));
        assert_eq!(backend.vibrance(OutputHandle(1)), Ok(Vibrance::NEUTRAL));
    }

    #[test]
    fn a_backend_reports_unknown_outputs() {
        let mut backend: Memory = Memory {
            value:            Vibrance::NEUTRAL,
            corrupt_readback: false,
        };
        assert_eq!(
            backend.vibrance(OutputHandle(9)),
            Err(BackendError::UnknownOutput(OutputHandle(9)))
        );
    }

    #[test]
    fn a_failed_readback_is_an_error_with_both_values() {
        let mut backend: Memory = Memory {
            value:            Vibrance::NEUTRAL,
            corrupt_readback: true,
        };
        let wanted: Vibrance = Vibrance::clamped(10);
        assert_eq!(
            backend.set_vibrance(OutputHandle(1), wanted),
            Err(BackendError::ReadbackMismatch {
                expected: wanted,
                actual:   Vibrance::MAX,
            })
        );
    }

    #[test]
    fn the_trait_is_usable_as_an_object() {
        let mut backend: Box<dyn Backend> = Box::new(Memory {
            value:            Vibrance::NEUTRAL,
            corrupt_readback: false,
        });
        assert_eq!(backend.name(), "memory");
        assert_eq!(backend.outputs().map(|o| o.len()), Ok(1));
        assert_eq!(backend.probe().map(|c| c.can_read_back), Ok(true));
    }

    #[test]
    fn output_handles_print_with_a_hash() {
        assert_eq!(OutputHandle(42).to_string(), "#42");
    }
}
