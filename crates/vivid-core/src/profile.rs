//! A saved set of per-monitor settings.

use std::collections::BTreeMap;

use crate::{OutputId, Vibrance};

/// The settings of one monitor. Digital Vibrance is the only control for now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Setting {
    /// The vibrance to apply.
    pub vibrance: Vibrance,
}

/// What to apply to which monitor. It is keyed by [`OutputId`], so it follows the monitor, not the
/// port or the position in a list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    settings: BTreeMap<OutputId, Setting>,
}

impl Profile {
    /// An empty profile.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the setting for a monitor, returning the previous one if there was any.
    pub fn set(&mut self, id: OutputId, setting: Setting) -> Option<Setting> {
        self.settings.insert(id, setting)
    }

    /// The setting for a monitor.
    #[must_use]
    pub fn get(&self, id: &OutputId) -> Option<&Setting> {
        self.settings.get(id)
    }

    /// Forgets a monitor, returning its setting if it had one.
    pub fn remove(&mut self, id: &OutputId) -> Option<Setting> {
        self.settings.remove(id)
    }

    /// The monitors in the profile, in a stable order.
    pub fn iter(&self) -> impl Iterator<Item = (&OutputId, &Setting)> {
        self.settings.iter()
    }

    /// How many monitors the profile has settings for.
    #[must_use]
    pub fn len(&self) -> usize {
        self.settings.len()
    }

    /// Whether the profile has no settings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MonitorSpec;

    fn id(serial: &str) -> OutputId {
        OutputId::new(MonitorSpec::new("PHL", "PHL 242V8", serial))
    }

    fn vibrance(value: i64) -> Setting {
        Setting {
            vibrance: Vibrance::clamped(value),
        }
    }

    #[test]
    fn a_new_profile_is_empty() {
        let profile: Profile = Profile::new();
        assert!(profile.is_empty());
        assert_eq!(profile.len(), 0);
    }

    #[test]
    fn setting_a_monitor_twice_replaces_it_and_returns_the_old_value() {
        let mut profile: Profile = Profile::new();
        assert_eq!(profile.set(id("0x1"), vibrance(100)), None);
        assert_eq!(profile.set(id("0x1"), vibrance(200)), Some(vibrance(100)));
        assert_eq!(profile.get(&id("0x1")), Some(&vibrance(200)));
        assert_eq!(profile.len(), 1);
    }

    #[test]
    fn monitors_are_independent() {
        let mut profile: Profile = Profile::new();
        profile.set(id("0x1"), vibrance(100));
        profile.set(id("0x2"), vibrance(-50));
        assert_eq!(profile.get(&id("0x2")), Some(&vibrance(-50)));
        assert_eq!(profile.remove(&id("0x1")), Some(vibrance(100)));
        assert_eq!(profile.get(&id("0x1")), None);
        assert_eq!(profile.len(), 1);
    }

    #[test]
    fn iteration_order_is_stable() {
        let mut profile: Profile = Profile::new();
        profile.set(id("0x2"), vibrance(2));
        profile.set(id("0x1"), vibrance(1));
        let serials: Vec<&str> = profile.iter().map(|(id, _)| id.spec().serial.as_str()).collect();
        assert_eq!(serials, ["0x1", "0x2"]);
    }

    #[test]
    fn the_default_setting_is_neutral() {
        assert!(Setting::default().vibrance.is_neutral());
    }
}
