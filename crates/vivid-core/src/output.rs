//! Output identity, as decided in `docs/adr/0003-output-identity.md`.

use std::{collections::BTreeMap, fmt, num::NonZeroU32};

/// The three strings that identify a monitor, built from its EDID with the same rule Mutter uses.
///
/// - `vendor`: the 3-letter manufacturer code
/// - `product`: the monitor name descriptor, or `0x%04x` of the product code
/// - `serial`: the serial descriptor, or `0x%08x` of the numeric serial
///
/// The connector name is not part of it: the sources name the same connector differently and it
/// changes with the port.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonitorSpec {
    /// The 3-letter manufacturer code, or `unknown`.
    pub vendor:  String,
    /// The monitor name, or the product code in hex, or `unknown`.
    pub product: String,
    /// The serial string, or the numeric serial in hex, or `unknown`.
    pub serial:  String,
}

impl MonitorSpec {
    /// Builds a spec from its three parts.
    #[must_use]
    pub fn new(vendor: impl Into<String>, product: impl Into<String>, serial: impl Into<String>) -> Self {
        Self {
            vendor:  vendor.into(),
            product: product.into(),
            serial:  serial.into(),
        }
    }

    /// The spec of an output that has no usable EDID.
    #[must_use]
    pub fn unknown() -> Self {
        Self::new("unknown", "unknown", "unknown")
    }
}

impl fmt::Display for MonitorSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.vendor, self.product, self.serial)
    }
}

/// How well the EDID can tell two units of the same monitor model apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerialQuality {
    /// A serial descriptor, or a numeric serial that is not a known factory default.
    Usable,
    /// Only a factory-default numeric serial: identical monitors will collide.
    Sentinel,
    /// No EDID at all.
    Missing,
}

/// What a saved profile keys on: the monitor spec, plus a slot when several connected outputs
/// share a spec.
///
/// A slot exists only while a collision exists. See [`assign_ids`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutputId {
    spec: MonitorSpec,
    slot: Option<NonZeroU32>,
}

impl OutputId {
    /// An id for a monitor with no collision.
    #[must_use]
    pub const fn new(spec: MonitorSpec) -> Self {
        Self { spec, slot: None }
    }

    /// An id for one of several outputs that share a spec. Slots start at 1.
    #[must_use]
    pub const fn with_slot(spec: MonitorSpec, slot: NonZeroU32) -> Self {
        Self { spec, slot: Some(slot) }
    }

    /// The monitor spec.
    #[must_use]
    pub const fn spec(&self) -> &MonitorSpec {
        &self.spec
    }

    /// The collision slot, if the spec is shared by more than one connected output.
    #[must_use]
    pub fn slot(&self) -> Option<u32> {
        self.slot.map(NonZeroU32::get)
    }
}

/// The readable form, `vendor:product:serial` or `vendor:product:serial#slot`. It is for display
/// only: product names can contain `:`, so storage keeps the three fields apart.
impl fmt::Display for OutputId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.spec.fmt(f)?;
        if let Some(slot) = self.slot {
            write!(f, "#{slot}")?;
        }
        Ok(())
    }
}

/// Gives an [`OutputId`] to each connected output.
///
/// `outputs` yields `(connector name, spec)` pairs. The ids come back in the same order. Outputs
/// that share a spec get slots `1..n` in sorted connector-name order; an output with a unique
/// spec gets no slot, so an unrelated monitor coming or going never changes anyone's slot.
#[must_use]
pub fn assign_ids<'a>(outputs: impl IntoIterator<Item = (&'a str, &'a MonitorSpec)>) -> Vec<OutputId> {
    let items: Vec<(usize, &str, &MonitorSpec)> = outputs
        .into_iter()
        .enumerate()
        .map(|(index, (connector, spec))| (index, connector, spec))
        .collect();

    let mut totals: BTreeMap<&MonitorSpec, u32> = BTreeMap::new();
    for (_, _, spec) in &items {
        *totals.entry(spec).or_insert(0) += 1;
    }

    let mut by_connector: Vec<(usize, &str, &MonitorSpec)> = items.clone();
    by_connector.sort_by(|a, b| a.1.cmp(b.1));

    let mut seen: BTreeMap<&MonitorSpec, u32> = BTreeMap::new();
    let mut slots: Vec<(usize, Option<NonZeroU32>)> = Vec::with_capacity(items.len());
    for (index, _, spec) in by_connector {
        let position: &mut u32 = seen.entry(spec).or_insert(0);
        *position += 1;
        let shared: bool = totals.get(spec).copied().unwrap_or(0) > 1;
        slots.push((index, if shared { NonZeroU32::new(*position) } else { None }));
    }
    slots.sort_by_key(|(index, _)| *index);

    items
        .into_iter()
        .zip(slots)
        .map(|((_, _, spec), (_, slot))| OutputId {
            spec: spec.clone(),
            slot,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(model: &str, serial: &str) -> MonitorSpec {
        MonitorSpec::new("DEL", model, serial)
    }

    #[test]
    fn distinct_monitors_get_distinct_ids_without_slots() {
        let (a, b) = (spec("A", "0x11"), spec("A", "0x22"));
        let ids: Vec<OutputId> = assign_ids([("DP-1", &a), ("HDMI-A-1", &b)]);
        assert_ne!(ids.first(), ids.get(1));
        assert!(ids.iter().all(|id| id.slot().is_none()));
    }

    #[test]
    fn identical_twins_get_slots_in_connector_order() {
        let twin: MonitorSpec = spec("U2720Q", "0x01010101");
        let ids: Vec<OutputId> = assign_ids([("HDMI-A-1", &twin), ("DP-1", &twin)]);
        // DP-1 sorts before HDMI-A-1, so it is slot 1 even though it was listed second.
        assert_eq!(ids.get(1).and_then(OutputId::slot), Some(1));
        assert_eq!(ids.first().and_then(OutputId::slot), Some(2));
    }

    #[test]
    fn slots_do_not_change_when_an_unrelated_monitor_comes_and_goes() {
        let twin: MonitorSpec = spec("U2720Q", "0x0");
        let other: MonitorSpec = MonitorSpec::new("PHL", "P", "0x77");
        let without: Vec<OutputId> = assign_ids([("DP-1", &twin), ("DP-2", &twin)]);
        let with: Vec<OutputId> = assign_ids([("DP-1", &twin), ("DP-2", &twin), ("HDMI-A-1", &other)]);
        assert_eq!(without.first(), with.first());
        assert_eq!(without.get(1), with.get(1));
        assert_eq!(with.get(2).and_then(OutputId::slot), None);
    }

    #[test]
    fn the_same_monitor_on_another_port_keeps_its_id() {
        let monitor: MonitorSpec = MonitorSpec::new("PHL", "PHL 242V8", "0x00008fac");
        assert_eq!(assign_ids([("HDMI-A-1", &monitor)]), assign_ids([("DP-3", &monitor)]));
    }

    #[test]
    fn three_identical_outputs_get_slots_one_to_three() {
        let twin: MonitorSpec = spec("X", "0x1");
        let ids: Vec<OutputId> = assign_ids([("DP-3", &twin), ("DP-1", &twin), ("DP-2", &twin)]);
        let slots: Vec<Option<u32>> = ids.iter().map(OutputId::slot).collect();
        assert_eq!(slots, [Some(3), Some(1), Some(2)]);
    }

    #[test]
    fn no_outputs_gives_no_ids() {
        assert_eq!(assign_ids(std::iter::empty()), Vec::<OutputId>::new());
    }

    #[test]
    fn display_form_is_readable_and_marks_the_slot() {
        let monitor: MonitorSpec = MonitorSpec::new("PHL", "PHL 242V8", "0x00008fac");
        assert_eq!(OutputId::new(monitor.clone()).to_string(), "PHL:PHL 242V8:0x00008fac");
        let slot: NonZeroU32 = NonZeroU32::MIN;
        assert_eq!(OutputId::with_slot(monitor, slot).to_string(), "PHL:PHL 242V8:0x00008fac#1");
    }

    #[test]
    fn outputs_without_an_edid_share_the_unknown_spec() {
        let unknown: MonitorSpec = MonitorSpec::unknown();
        let ids: Vec<OutputId> = assign_ids([("A", &unknown), ("B", &unknown)]);
        assert_eq!(ids.iter().map(OutputId::slot).collect::<Vec<_>>(), [Some(1), Some(2)]);
        assert_eq!(unknown.to_string(), "unknown:unknown:unknown");
    }
}
