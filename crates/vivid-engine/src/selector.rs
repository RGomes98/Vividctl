//! How the user names a monitor.

use vivid_core::{MonitorSpec, OutputId};

use crate::Entry;

/// What the user typed after `--output`.
///
/// A monitor identity (`vendor:product:serial`, optionally ending in `#slot`) follows the monitor
/// wherever it is plugged. A connector name (`HDMI-0`) is a convenience: it names a port, and
/// ports are named differently by each backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selector {
    /// A monitor, by the identity that [`OutputId`] prints.
    Monitor {
        /// The monitor spec.
        spec: MonitorSpec,
        /// The collision slot, when the user gave one.
        slot: Option<u32>,
    },
    /// A connector, by name, ignoring case.
    Connector(String),
}

impl Selector {
    /// Reads what the user typed.
    ///
    /// Text with at least two colons is a monitor identity: the vendor up to the first colon, the
    /// serial after the last one, and the product in between (product names may contain colons).
    /// A trailing `#N` is the collision slot. Anything else is a connector name.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let (body, slot) = match text.rsplit_once('#') {
            Some((body, digits)) => match digits.parse::<u32>() {
                Ok(slot) if slot > 0 => (body, Some(slot)),
                _ => (text, None),
            },
            None => (text, None),
        };
        match body
            .split_once(':')
            .and_then(|(vendor, rest)| Some((vendor, rest.rsplit_once(':')?)))
        {
            Some((vendor, (product, serial))) if !vendor.is_empty() && !serial.is_empty() => Self::Monitor {
                spec: MonitorSpec::new(vendor, product, serial),
                slot,
            },
            _ => Self::Connector(text.to_owned()),
        }
    }

    pub(crate) fn matches(&self, entry: &Entry) -> bool {
        match self {
            Self::Connector(name) => entry.output.connector.eq_ignore_ascii_case(name),
            Self::Monitor { spec, slot } => entry
                .id
                .as_ref()
                .is_some_and(|id: &OutputId| id.spec() == spec && slot.is_none_or(|wanted| id.slot() == Some(wanted))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_monitor_identity_is_recognised() {
        assert_eq!(
            Selector::parse("PHL:PHL 242V8:0x00008fac"),
            Selector::Monitor {
                spec: MonitorSpec::new("PHL", "PHL 242V8", "0x00008fac"),
                slot: None,
            }
        );
    }

    #[test]
    fn a_trailing_slot_is_split_off() {
        assert_eq!(
            Selector::parse("DEL:U2720Q:0x01010101#2"),
            Selector::Monitor {
                spec: MonitorSpec::new("DEL", "U2720Q", "0x01010101"),
                slot: Some(2),
            }
        );
    }

    #[test]
    fn a_product_name_may_contain_colons() {
        assert_eq!(
            Selector::parse("ABC:Model: X:SN1"),
            Selector::Monitor {
                spec: MonitorSpec::new("ABC", "Model: X", "SN1"),
                slot: None,
            }
        );
    }

    #[test]
    fn a_connector_name_is_the_fallback() {
        assert_eq!(Selector::parse("HDMI-0"), Selector::Connector("HDMI-0".into()));
        assert_eq!(Selector::parse("eDP-1"), Selector::Connector("eDP-1".into()));
    }

    #[test]
    fn text_that_only_looks_like_an_identity_is_a_connector() {
        for text in ["a:b", ":x:y", "a:b:", "HDMI-0#1", "x#0", "x#-1", "x#abc", ""] {
            assert!(matches!(Selector::parse(text), Selector::Connector(_)), "{text:?}");
        }
    }

    #[test]
    fn a_zero_after_the_hash_is_part_of_the_serial_not_a_slot() {
        // Slots start at 1, so `#0` is just text in the serial.
        assert_eq!(
            Selector::parse("A:B:C#0"),
            Selector::Monitor {
                spec: MonitorSpec::new("A", "B", "C#0"),
                slot: None,
            }
        );
    }
}
