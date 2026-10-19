//! The use cases shared by the command line and the GUI: list the outputs of every backend, name
//! one, read it and change it.

use vivid_core::{Backend, BackendError, BackendOutput, OutputId, Vibrance, assign_ids};
use vivid_nvkms::NvkmsBackend;

use crate::{SelectError, Selector};

/// One backend, ready or not.
pub struct Slot {
    name:  &'static str,
    state: State,
}

enum State {
    Ready {
        backend:     Box<dyn Backend>,
        description: String,
    },
    Unavailable(BackendError),
}

impl Slot {
    /// Probes `backend`. A backend that fails its probe becomes an unavailable slot that keeps the
    /// reason, so the user can be told why it is missing.
    #[must_use]
    pub fn ready(name: &'static str, mut backend: Box<dyn Backend>) -> Self {
        match backend.probe() {
            Ok(capabilities) => Self {
                name,
                state: State::Ready {
                    backend,
                    description: capabilities.description,
                },
            },
            Err(error) => Self::unavailable(name, error),
        }
    }

    /// A backend that could not even be created.
    #[must_use]
    pub const fn unavailable(name: &'static str, error: BackendError) -> Self {
        Self {
            name,
            state: State::Unavailable(error),
        }
    }
}

/// What a backend reported about itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendReport {
    /// The backend's name.
    pub name:   &'static str,
    /// What it found (a line for humans), or why it is not available.
    pub status: Result<String, BackendError>,
}

/// An output of some backend, with the identity it has now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Which backend sees it.
    pub backend: &'static str,
    slot:        usize,
    /// The output as the backend describes it.
    pub output:  BackendOutput,
    /// Its identity, for connected outputs.
    pub id:      Option<OutputId>,
}

/// A backend that failed while listing its outputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// The backend's name.
    pub backend: &'static str,
    /// What went wrong.
    pub error:   BackendError,
}

/// The outputs found, and the backends that failed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Listing {
    /// Outputs of every backend, in a stable order.
    pub entries:  Vec<Entry>,
    /// Backends that were ready but failed to list.
    pub problems: Vec<Problem>,
}

impl Listing {
    /// The outputs to act on.
    ///
    /// Without a selector, every connected output. With one, the outputs it names, which must be
    /// exactly one: a selector that matches several is reported as ambiguous instead of acting on
    /// all of them.
    ///
    /// # Errors
    ///
    /// [`SelectError`] when nothing is connected, nothing matches, or the match is ambiguous.
    pub fn select(&self, selector: Option<&Selector>, text: &str) -> Result<Vec<&Entry>, SelectError> {
        let Some(selector) = selector else {
            let connected: Vec<&Entry> = self.entries.iter().filter(|e| e.output.connected).collect();
            return if connected.is_empty() {
                Err(SelectError::NoConnectedOutputs)
            } else {
                Ok(connected)
            };
        };
        let matched: Vec<&Entry> = self.entries.iter().filter(|e| selector.matches(e)).collect();
        match matched.as_slice() {
            [] => Err(SelectError::NoMatch {
                selector: text.to_owned(),
            }),
            [_] => Ok(matched),
            several => Err(SelectError::Ambiguous {
                selector:   text.to_owned(),
                candidates: several.iter().map(|e| describe(e)).collect(),
            }),
        }
    }
}

/// How to name one output exactly: its identity when it has one, else backend and connector.
fn describe(entry: &Entry) -> String {
    entry
        .id
        .as_ref()
        .map_or_else(|| format!("{}/{}", entry.backend, entry.output.connector), ToString::to_string)
}

/// The result of acting on one output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome<T> {
    /// The backend that handled it.
    pub backend:   &'static str,
    /// The connector, in the backend's naming.
    pub connector: String,
    /// The identity of the output.
    pub id:        Option<OutputId>,
    /// What happened.
    pub result:    Result<T, BackendError>,
}

/// Every backend, and what to do with their outputs.
pub struct Engine {
    slots: Vec<Slot>,
}

impl Engine {
    /// An engine over the given backends.
    #[must_use]
    pub const fn new(slots: Vec<Slot>) -> Self {
        Self { slots }
    }

    /// The backends of this system: NVKMS for outputs an NVIDIA GPU drives.
    #[must_use]
    pub fn system() -> Self {
        let nvkms: Slot = match NvkmsBackend::open_system() {
            Ok(backend) => Slot::ready("nvkms", Box::new(backend)),
            Err(error) => Slot::unavailable("nvkms", error),
        };
        Self::new(vec![nvkms])
    }

    /// What each backend found when it was probed.
    #[must_use]
    pub fn reports(&self) -> Vec<BackendReport> {
        self.slots
            .iter()
            .map(|slot| BackendReport {
                name:   slot.name,
                status: match &slot.state {
                    State::Ready { description, .. } => Ok(description.clone()),
                    State::Unavailable(error) => Err(error.clone()),
                },
            })
            .collect()
    }

    /// Asks every ready backend for its outputs and gives the connected ones their identity.
    ///
    /// A backend that fails is reported in [`Listing::problems`] and does not hide the others.
    pub fn listing(&mut self) -> Listing {
        let mut listing: Listing = Listing::default();
        for (index, slot) in self.slots.iter_mut().enumerate() {
            let State::Ready { backend, .. } = &mut slot.state else {
                continue;
            };
            match backend.outputs() {
                Ok(outputs) => {
                    listing.entries.extend(outputs.into_iter().map(|output| Entry {
                        backend: slot.name,
                        slot: index,
                        output,
                        id: None,
                    }));
                }
                Err(error) => listing.problems.push(Problem {
                    backend: slot.name,
                    error,
                }),
            }
        }
        listing
            .entries
            .sort_by(|a, b| (a.backend, &a.output.connector).cmp(&(b.backend, &b.output.connector)));

        // Identity is shared across backends, so collisions are resolved over all connected outputs.
        let names: Vec<String> = listing
            .entries
            .iter()
            .filter(|e| e.output.connected)
            .map(|e| format!("{}/{}", e.backend, e.output.connector))
            .collect();
        let connected: Vec<(&str, &vivid_core::MonitorSpec)> = names
            .iter()
            .map(String::as_str)
            .zip(
                listing
                    .entries
                    .iter()
                    .filter(|e| e.output.connected)
                    .map(|e| &e.output.spec),
            )
            .collect();
        let ids: Vec<OutputId> = assign_ids(connected);
        for (entry, id) in listing.entries.iter_mut().filter(|e| e.output.connected).zip(ids) {
            entry.id = Some(id);
        }
        listing
    }

    /// Reads the vibrance of each entry. A failure on one does not stop the others.
    pub fn get_all(&mut self, entries: &[&Entry]) -> Vec<Outcome<Vibrance>> {
        entries
            .iter()
            .map(|entry| self.act(entry, |backend, handle| backend.vibrance(handle)))
            .collect()
    }

    /// Sets the vibrance of each entry. A failure on one does not stop the others.
    pub fn set_all(&mut self, entries: &[&Entry], value: Vibrance) -> Vec<Outcome<()>> {
        entries
            .iter()
            .map(|entry| self.act(entry, |backend, handle| backend.set_vibrance(handle, value)))
            .collect()
    }

    fn act<T>(
        &mut self,
        entry: &Entry,
        action: impl FnOnce(&mut dyn Backend, vivid_core::OutputHandle) -> Result<T, BackendError>,
    ) -> Outcome<T> {
        let result: Result<T, BackendError> = if entry.output.connected {
            match self.slots.get_mut(entry.slot).map(|slot| &mut slot.state) {
                Some(State::Ready { backend, .. }) => action(backend.as_mut(), entry.output.handle),
                _ => Err(BackendError::Unavailable(format!("backend {} is not available", entry.backend))),
            }
        } else {
            // Say what is wrong instead of passing the request on to be rejected for no stated reason.
            Err(BackendError::Unsupported(format!(
                "no monitor is attached to {}",
                entry.output.connector
            )))
        };
        Outcome {
            backend: entry.backend,
            connector: entry.output.connector.clone(),
            id: entry.id.clone(),
            result,
        }
    }
}

#[cfg(test)]
// Tests build fixtures with expect and index into small vectors.
#[allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::collections::BTreeMap;

    use vivid_core::{Capabilities, MonitorSpec, OutputHandle};

    use super::*;

    /// A backend with fixed outputs that stores values.
    struct Fake {
        name:         &'static str,
        outputs:      Vec<BackendOutput>,
        values:       BTreeMap<u64, Vibrance>,
        fail_set_on:  Option<u64>,
        fail_listing: bool,
    }

    impl Fake {
        fn new(name: &'static str, outputs: Vec<(u64, &str, MonitorSpec, bool)>) -> Self {
            Self {
                name,
                outputs: outputs
                    .into_iter()
                    .map(|(handle, connector, spec, connected)| BackendOutput {
                        handle: OutputHandle(handle),
                        connector: connector.into(),
                        spec,
                        connected,
                    })
                    .collect(),
                values: BTreeMap::new(),
                fail_set_on: None,
                fail_listing: false,
            }
        }
    }

    impl Backend for Fake {
        fn name(&self) -> &'static str {
            self.name
        }
        fn probe(&mut self) -> Result<Capabilities, BackendError> {
            Ok(Capabilities {
                description:   format!("{} fake", self.name),
                can_read_back: true,
            })
        }
        fn outputs(&mut self) -> Result<Vec<BackendOutput>, BackendError> {
            if self.fail_listing {
                Err(BackendError::Io("boom".into()))
            } else {
                Ok(self.outputs.clone())
            }
        }
        fn vibrance(&mut self, output: OutputHandle) -> Result<Vibrance, BackendError> {
            self.outputs
                .iter()
                .find(|o| o.handle == output)
                .ok_or(BackendError::UnknownOutput(output))?;
            Ok(self.values.get(&output.0).copied().unwrap_or_default())
        }
        fn set_vibrance(&mut self, output: OutputHandle, value: Vibrance) -> Result<(), BackendError> {
            if self.fail_set_on == Some(output.0) {
                return Err(BackendError::Rejected("no".into()));
            }
            self.values.insert(output.0, value);
            Ok(())
        }
    }

    fn phl() -> MonitorSpec {
        MonitorSpec::new("PHL", "PHL 242V8", "0x00008fac")
    }

    fn twin() -> MonitorSpec {
        MonitorSpec::new("DEL", "U2720Q", "0x01010101")
    }

    fn engine(fake: Fake) -> Engine {
        Engine::new(vec![Slot::ready("fake", Box::new(fake))])
    }

    #[test]
    fn the_listing_has_every_output_and_identities_for_the_connected_ones() {
        let mut engine = engine(Fake::new(
            "fake",
            vec![(1, "HDMI-0", phl(), true), (2, "DP-0", MonitorSpec::unknown(), false)],
        ));
        let listing: Listing = engine.listing();
        assert_eq!(listing.entries.len(), 2);
        assert_eq!(listing.entries[0].output.connector, "DP-0");
        assert_eq!(listing.entries[0].id, None);
        assert_eq!(
            listing.entries[1].id.as_ref().map(ToString::to_string).as_deref(),
            Some("PHL:PHL 242V8:0x00008fac")
        );
    }

    #[test]
    fn identical_monitors_get_slots_by_connector_order() {
        let mut engine = engine(Fake::new("fake", vec![(1, "HDMI-0", twin(), true), (2, "DP-1", twin(), true)]));
        let listing: Listing = engine.listing();
        let slots: Vec<(String, Option<u32>)> = listing
            .entries
            .iter()
            .map(|e| (e.output.connector.clone(), e.id.as_ref().and_then(OutputId::slot)))
            .collect();
        assert_eq!(slots, [("DP-1".to_owned(), Some(1)), ("HDMI-0".to_owned(), Some(2))]);
    }

    #[test]
    fn without_a_selector_every_connected_output_is_targeted() {
        let mut engine = engine(Fake::new(
            "fake",
            vec![(1, "HDMI-0", phl(), true), (2, "DP-0", MonitorSpec::unknown(), false)],
        ));
        let listing: Listing = engine.listing();
        let targets: Vec<&Entry> = listing.select(None, "").expect("one connected");
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].output.connector, "HDMI-0");
    }

    #[test]
    fn nothing_connected_is_reported_not_silently_ignored() {
        let mut engine = engine(Fake::new("fake", vec![(2, "DP-0", MonitorSpec::unknown(), false)]));
        assert_eq!(engine.listing().select(None, ""), Err(SelectError::NoConnectedOutputs));
        let mut empty = Engine::new(vec![]);
        assert_eq!(empty.listing().select(None, ""), Err(SelectError::NoConnectedOutputs));
    }

    #[test]
    fn a_monitor_identity_or_a_connector_name_selects_the_output() {
        let mut engine = engine(Fake::new("fake", vec![(1, "HDMI-0", phl(), true), (2, "DP-1", twin(), true)]));
        let listing: Listing = engine.listing();
        for text in ["PHL:PHL 242V8:0x00008fac", "hdmi-0"] {
            let found = listing.select(Some(&Selector::parse(text)), text).expect("matches");
            assert_eq!(found.len(), 1);
            assert_eq!(found[0].output.connector, "HDMI-0");
        }
    }

    #[test]
    fn a_selector_that_matches_nothing_says_so() {
        let mut engine = engine(Fake::new("fake", vec![(1, "HDMI-0", phl(), true)]));
        let listing: Listing = engine.listing();
        assert_eq!(
            listing.select(Some(&Selector::parse("DP-9")), "DP-9"),
            Err(SelectError::NoMatch {
                selector: "DP-9".into(),
            })
        );
    }

    #[test]
    fn identical_monitors_need_a_slot_to_be_told_apart() {
        let mut engine = engine(Fake::new("fake", vec![(1, "HDMI-0", twin(), true), (2, "DP-1", twin(), true)]));
        let listing: Listing = engine.listing();
        let text: &str = "DEL:U2720Q:0x01010101";
        match listing.select(Some(&Selector::parse(text)), text) {
            Err(SelectError::Ambiguous { candidates, .. }) => {
                assert_eq!(candidates, ["DEL:U2720Q:0x01010101#1", "DEL:U2720Q:0x01010101#2"]);
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
        let with_slot: &str = "DEL:U2720Q:0x01010101#2";
        let found = listing
            .select(Some(&Selector::parse(with_slot)), with_slot)
            .expect("a slot names one");
        assert_eq!(found[0].output.connector, "HDMI-0");
    }

    #[test]
    fn setting_and_reading_go_through_the_right_backend() {
        let mut engine = engine(Fake::new("fake", vec![(1, "HDMI-0", phl(), true)]));
        let listing: Listing = engine.listing();
        let targets = listing.select(None, "").expect("connected");
        let value: Vibrance = Vibrance::clamped(512);
        assert!(engine.set_all(&targets, value)[0].result.is_ok());
        assert_eq!(engine.get_all(&targets)[0].result, Ok(value));
    }

    #[test]
    fn an_output_without_a_monitor_is_refused_with_a_clear_reason_and_the_backend_is_not_asked() {
        let mut engine = engine(Fake::new("fake", vec![(2, "DP-0", MonitorSpec::unknown(), false)]));
        let listing: Listing = engine.listing();
        let named = Selector::parse("DP-0");
        let targets = listing
            .select(Some(&named), "DP-0")
            .expect("a named output is selectable even when disconnected");
        let read = engine.get_all(&targets).remove(0).result;
        let write = engine.set_all(&targets, Vibrance::MAX).remove(0).result;
        assert_eq!(read, Err(BackendError::Unsupported("no monitor is attached to DP-0".into())));
        assert_eq!(write, Err(BackendError::Unsupported("no monitor is attached to DP-0".into())));
    }

    #[test]
    fn a_failure_on_one_output_does_not_stop_the_others() {
        let mut fake: Fake = Fake::new(
            "fake",
            vec![
                (1, "DP-0", phl(), true),
                (2, "DP-1", MonitorSpec::new("AOC", "X", "0x2"), true),
                (3, "DP-2", MonitorSpec::new("AOC", "Y", "0x3"), true),
            ],
        );
        fake.fail_set_on = Some(2);
        let mut engine = engine(fake);
        let listing: Listing = engine.listing();
        let targets = listing.select(None, "").expect("connected");
        let outcomes = engine.set_all(&targets, Vibrance::clamped(10));
        let results: Vec<bool> = outcomes.iter().map(|o| o.result.is_ok()).collect();
        assert_eq!(results, [true, false, true]);
        assert!(matches!(outcomes[1].result, Err(BackendError::Rejected(_))));
        assert_eq!(engine.get_all(&targets)[2].result, Ok(Vibrance::clamped(10)));
    }

    #[test]
    fn an_unavailable_backend_is_reported_with_its_reason_and_hides_nothing() {
        let down: Slot = Slot::unavailable("nvkms", BackendError::Unavailable("module not loaded".into()));
        let up: Slot = Slot::ready("fake", Box::new(Fake::new("fake", vec![(1, "HDMI-0", phl(), true)])));
        let mut engine: Engine = Engine::new(vec![down, up]);
        let reports: Vec<BackendReport> = engine.reports();
        assert_eq!(reports[0].name, "nvkms");
        assert!(matches!(reports[0].status, Err(BackendError::Unavailable(_))));
        assert_eq!(reports[1].status, Ok("fake fake".to_owned()));
        assert_eq!(engine.listing().entries.len(), 1);
    }

    #[test]
    fn a_backend_that_fails_to_list_is_a_problem_not_a_crash() {
        let mut broken: Fake = Fake::new("fake", vec![]);
        broken.fail_listing = true;
        let working: Slot = Slot::ready("other", Box::new(Fake::new("other", vec![(1, "HDMI-0", phl(), true)])));
        let mut engine: Engine = Engine::new(vec![Slot::ready("fake", Box::new(broken)), working]);
        let listing: Listing = engine.listing();
        assert_eq!(listing.problems.len(), 1);
        assert_eq!(listing.problems[0].backend, "fake");
        assert_eq!(listing.entries.len(), 1);
    }

    #[test]
    fn a_backend_whose_probe_fails_becomes_unavailable() {
        struct Dead;
        impl Backend for Dead {
            fn name(&self) -> &'static str {
                "dead"
            }
            fn probe(&mut self) -> Result<Capabilities, BackendError> {
                Err(BackendError::Unavailable("no driver".into()))
            }
            fn outputs(&mut self) -> Result<Vec<BackendOutput>, BackendError> {
                Ok(vec![])
            }
            fn vibrance(&mut self, output: OutputHandle) -> Result<Vibrance, BackendError> {
                Err(BackendError::UnknownOutput(output))
            }
            fn set_vibrance(&mut self, output: OutputHandle, _: Vibrance) -> Result<(), BackendError> {
                Err(BackendError::UnknownOutput(output))
            }
        }
        let engine: Engine = Engine::new(vec![Slot::ready("dead", Box::new(Dead))]);
        assert_eq!(engine.reports()[0].status, Err(BackendError::Unavailable("no driver".into())));
    }
}
