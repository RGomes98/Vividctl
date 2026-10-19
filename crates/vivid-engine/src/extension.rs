//! Installing, checking and removing the GNOME Shell extension that `vivid-gnome` talks to.
//!
//! The extension's files come embedded in the binary (`vivid_gnome::extension`), so the version
//! that is installed is the version the `vividctl` in hand was built with.

use std::{
    env,
    path::{Path, PathBuf},
};

use vivid_core::BackendError;
pub use vivid_gnome::extension::Files;
use vivid_gnome::extension::{Gsettings, Settings, UUID, files_state, install_dir, remove_files, write_files};

/// What `install` did to the files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// There was nothing before.
    Created,
    /// The files differed from this build's and were replaced.
    Updated,
    /// The files were already this build's.
    Unchanged,
}

/// The outcome of [`ExtensionInstaller::install`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallReport {
    /// Where the extension is.
    pub dir:                     PathBuf,
    /// What happened to the files.
    pub change:                  Change,
    /// Whether the extension was added to the enabled list now (false: it already was).
    pub newly_enabled:           bool,
    /// Whether GNOME allows extensions of the user at all. When false, the extension cannot start.
    pub user_extensions_allowed: bool,
}

/// What [`ExtensionInstaller::status`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionStatus {
    /// Where the extension is, or would be.
    pub dir:                     PathBuf,
    /// Whether the files are the ones this build carries.
    pub files:                   Files,
    /// Whether the extension is in the enabled list.
    pub enabled:                 bool,
    /// Whether GNOME allows extensions of the user at all.
    pub user_extensions_allowed: bool,
}

/// The outcome of [`ExtensionInstaller::remove`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemoveReport {
    /// Whether there were files to delete.
    pub removed_files: bool,
    /// Whether the extension was in the enabled list.
    pub was_enabled:   bool,
}

/// Puts the extension in place and switches it on.
pub struct ExtensionInstaller<S: Settings> {
    dir:      PathBuf,
    settings: S,
}

impl ExtensionInstaller<Gsettings> {
    /// An installer for this user: their data directory and the real GNOME settings.
    ///
    /// # Errors
    ///
    /// [`BackendError::Io`] when the home directory cannot be determined.
    pub fn system() -> Result<Self, BackendError> {
        let data: Option<String> = env::var("XDG_DATA_HOME").ok();
        let home: Option<String> = env::var("HOME").ok();
        Ok(Self::new(install_dir(data.as_deref(), home.as_deref())?, Gsettings))
    }
}

impl<S: Settings> ExtensionInstaller<S> {
    /// An installer for `dir` with the given settings.
    #[must_use]
    pub const fn new(dir: PathBuf, settings: S) -> Self {
        Self { dir, settings }
    }

    /// Where the extension is, or would be.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Writes the files if they differ and adds the extension to the enabled list, keeping every
    /// other entry. Running it again changes nothing.
    ///
    /// # Errors
    ///
    /// [`BackendError::Io`] when the files or the settings cannot be written. The enabled list is
    /// only touched after the files are in place, and not at all when it cannot be read.
    pub fn install(&mut self) -> Result<InstallReport, BackendError> {
        let before: Files = files_state(&self.dir);
        let mut enabled: Vec<String> = self.settings.enabled_extensions()?;
        let user_extensions_allowed: bool = self.settings.user_extensions_allowed()?;
        if before != Files::Current {
            write_files(&self.dir)?;
        }
        let newly_enabled: bool = !enabled.iter().any(|uuid| uuid == UUID);
        if newly_enabled {
            enabled.push(UUID.to_owned());
            self.settings.set_enabled_extensions(&enabled)?;
        }
        Ok(InstallReport {
            dir: self.dir.clone(),
            change: match before {
                Files::Missing => Change::Created,
                Files::Outdated => Change::Updated,
                Files::Current => Change::Unchanged,
            },
            newly_enabled,
            user_extensions_allowed,
        })
    }

    /// Looks at the files and the settings without changing anything.
    ///
    /// # Errors
    ///
    /// [`BackendError::Io`] when the settings cannot be read.
    pub fn status(&mut self) -> Result<ExtensionStatus, BackendError> {
        Ok(ExtensionStatus {
            dir:                     self.dir.clone(),
            files:                   files_state(&self.dir),
            enabled:                 self.settings.enabled_extensions()?.iter().any(|uuid| uuid == UUID),
            user_extensions_allowed: self.settings.user_extensions_allowed()?,
        })
    }

    /// Takes the extension out of the enabled list and deletes its files.
    ///
    /// # Errors
    ///
    /// [`BackendError::Io`] when the settings or the files cannot be changed.
    pub fn remove(&mut self) -> Result<RemoveReport, BackendError> {
        let enabled: Vec<String> = self.settings.enabled_extensions()?;
        let was_enabled: bool = enabled.iter().any(|uuid| uuid == UUID);
        if was_enabled {
            let kept: Vec<String> = enabled.into_iter().filter(|uuid| uuid != UUID).collect();
            self.settings.set_enabled_extensions(&kept)?;
        }
        Ok(RemoveReport {
            removed_files: remove_files(&self.dir)?,
            was_enabled,
        })
    }
}

#[cfg(test)]
// Tests unwrap what they just built.
#[allow(clippy::expect_used)]
mod tests {
    use std::fs;

    use vivid_gnome::fake::FakeSettings;

    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let root: PathBuf = env::temp_dir().join(format!("vividctl-engine-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            Self(root)
        }

        fn dir(&self) -> PathBuf {
            self.0.join(UUID)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn installer(scratch: &Scratch, settings: FakeSettings) -> ExtensionInstaller<FakeSettings> {
        ExtensionInstaller::new(scratch.dir(), settings)
    }

    #[test]
    fn a_first_install_writes_the_files_and_enables_the_extension() {
        let scratch: Scratch = Scratch::new("first");
        let mut installer = installer(&scratch, FakeSettings::default());
        let report: InstallReport = installer.install().expect("installs");
        assert_eq!(report.change, Change::Created);
        assert!(report.newly_enabled && report.user_extensions_allowed);
        assert_eq!(files_state(&scratch.dir()), Files::Current);
        assert_eq!(installer.settings.enabled, [UUID]);
    }

    #[test]
    fn installing_again_changes_nothing() {
        let scratch: Scratch = Scratch::new("again");
        let mut installer = installer(&scratch, FakeSettings::default());
        installer.install().expect("installs");
        let report: InstallReport = installer.install().expect("installs again");
        assert_eq!(report.change, Change::Unchanged);
        assert!(!report.newly_enabled);
        assert_eq!(installer.settings.enabled, [UUID], "no duplicate entry");
    }

    #[test]
    fn other_extensions_the_user_has_are_kept_and_in_order() {
        let scratch: Scratch = Scratch::new("others");
        let settings: FakeSettings = FakeSettings {
            enabled: vec!["dash-to-dock@micxgx.gmail.com".into(), "blur@x.org".into()],
            ..FakeSettings::default()
        };
        let mut installer = installer(&scratch, settings);
        installer.install().expect("installs");
        assert_eq!(
            installer.settings.enabled,
            ["dash-to-dock@micxgx.gmail.com", "blur@x.org", UUID]
        );
        installer.remove().expect("removes");
        assert_eq!(installer.settings.enabled, ["dash-to-dock@micxgx.gmail.com", "blur@x.org"]);
    }

    #[test]
    fn an_old_copy_is_replaced_and_reported_as_updated() {
        let scratch: Scratch = Scratch::new("update");
        fs::create_dir_all(scratch.dir()).expect("creates");
        fs::write(scratch.dir().join("extension.js"), "// old").expect("writes");
        let mut installer = installer(&scratch, FakeSettings::default());
        assert_eq!(installer.install().expect("installs").change, Change::Updated);
        assert_eq!(files_state(&scratch.dir()), Files::Current);
    }

    #[test]
    fn a_disabled_but_present_extension_is_enabled_without_rewriting_files() {
        let scratch: Scratch = Scratch::new("reenable");
        write_files(&scratch.dir()).expect("writes");
        let mut installer = installer(&scratch, FakeSettings::default());
        let report: InstallReport = installer.install().expect("installs");
        assert_eq!((report.change, report.newly_enabled), (Change::Unchanged, true));
    }

    #[test]
    fn globally_disabled_user_extensions_are_reported_not_overridden() {
        let scratch: Scratch = Scratch::new("policy");
        let settings: FakeSettings = FakeSettings {
            user_allowed: false,
            ..FakeSettings::default()
        };
        let mut installer = installer(&scratch, settings);
        assert!(!installer.install().expect("installs").user_extensions_allowed);
        assert!(!installer.settings.user_allowed, "the user's policy is theirs to change");
    }

    #[test]
    fn unreadable_settings_stop_the_install_before_any_file_is_written() {
        let scratch: Scratch = Scratch::new("broken");
        let settings: FakeSettings = FakeSettings {
            broken: true,
            ..FakeSettings::default()
        };
        let mut installer = installer(&scratch, settings);
        assert!(matches!(installer.install(), Err(BackendError::Io(_))));
        assert_eq!(files_state(&scratch.dir()), Files::Missing);
    }

    #[test]
    fn status_reports_files_and_setting_separately() {
        let scratch: Scratch = Scratch::new("status");
        let mut installer = installer(&scratch, FakeSettings::default());
        let before: ExtensionStatus = installer.status().expect("reads");
        assert_eq!((before.files, before.enabled), (Files::Missing, false));
        installer.install().expect("installs");
        let after: ExtensionStatus = installer.status().expect("reads");
        assert_eq!((after.files, after.enabled), (Files::Current, true));
        assert_eq!(after.dir, scratch.dir());
    }

    #[test]
    fn status_changes_nothing() {
        let scratch: Scratch = Scratch::new("readonly");
        let mut installer = installer(&scratch, FakeSettings::default());
        installer.status().expect("reads");
        assert_eq!(files_state(&scratch.dir()), Files::Missing);
        assert_eq!(installer.settings.enabled, Vec::<String>::new());
    }

    #[test]
    fn remove_deletes_files_and_the_setting_and_says_what_it_did() {
        let scratch: Scratch = Scratch::new("remove");
        let mut installer = installer(&scratch, FakeSettings::default());
        installer.install().expect("installs");
        assert_eq!(
            installer.remove().expect("removes"),
            RemoveReport {
                removed_files: true,
                was_enabled:   true,
            }
        );
        assert_eq!(files_state(&scratch.dir()), Files::Missing);
        assert_eq!(
            installer.remove().expect("removes again"),
            RemoveReport {
                removed_files: false,
                was_enabled:   false,
            }
        );
    }
}
