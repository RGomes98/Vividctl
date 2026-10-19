//! The Shell extension as files, and the setting that turns it on.
//!
//! The extension's two files are embedded in the binary, so the `vividctl` that installs it and
//! the extension it installs always belong together. GNOME Shell on Wayland only discovers a new
//! extension at login; writing the UUID into `enabled-extensions` is what makes it start then, and
//! it also starts the extension at once when the running shell already knows it.

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use crate::GnomeError;

/// The extension's UUID, which is also the name of its directory.
pub const UUID: &str = "vividctl@rgomes98.github.io";

/// The files of the extension, as embedded at build time.
const FILES: [(&str, &str); 2] = [
    ("extension.js", include_str!("../../../extensions/gnome/extension.js")),
    ("metadata.json", include_str!("../../../extensions/gnome/metadata.json")),
];

const SCHEMA: &str = "org.gnome.shell";
const ENABLED_KEY: &str = "enabled-extensions";
const DISABLE_KEY: &str = "disable-user-extensions";

/// Whether the extension's files on disk are the ones this build carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Files {
    /// There is no extension directory.
    Missing,
    /// The directory exists but a file is missing or differs from this build's.
    Outdated,
    /// Every file is identical to this build's.
    Current,
}

/// Where the extension goes: `$XDG_DATA_HOME/gnome-shell/extensions/<uuid>`, or under
/// `$HOME/.local/share` when `XDG_DATA_HOME` is unset, empty or relative (the XDG rule).
///
/// # Errors
///
/// [`GnomeError::Io`] when neither variable gives an absolute directory.
pub fn install_dir(xdg_data_home: Option<&str>, home: Option<&str>) -> Result<PathBuf, GnomeError> {
    let absolute = |value: Option<&str>| value.filter(|v| Path::new(v).is_absolute()).map(PathBuf::from);
    let data: PathBuf = absolute(xdg_data_home)
        .or_else(|| absolute(home).map(|h| h.join(".local/share")))
        .ok_or_else(|| GnomeError::Io("cannot tell where to install: neither XDG_DATA_HOME nor HOME is set".into()))?;
    Ok(data.join("gnome-shell/extensions").join(UUID))
}

/// Compares the directory with the embedded files.
#[must_use]
pub fn files_state(dir: &Path) -> Files {
    if !dir.is_dir() {
        return Files::Missing;
    }
    let same = |(name, content): &(&str, &str)| fs::read_to_string(dir.join(name)).is_ok_and(|text| text == *content);
    if FILES.iter().all(same) {
        Files::Current
    } else {
        Files::Outdated
    }
}

/// Writes the files, replacing what is there. Each file is written next to its place and renamed,
/// so a running shell never reads half a file.
///
/// # Errors
///
/// [`GnomeError::Io`] when the directory or a file cannot be written.
pub fn write_files(dir: &Path) -> Result<(), GnomeError> {
    let fail = |action: &str, path: &Path, error: &io::Error| {
        GnomeError::Io(format!("cannot {action} {}: {error}", path.display()))
    };
    fs::create_dir_all(dir).map_err(|e| fail("create", dir, &e))?;
    for (name, content) in FILES {
        let target: PathBuf = dir.join(name);
        let staging: PathBuf = dir.join(format!(".{name}.new"));
        fs::write(&staging, content).map_err(|e| fail("write", &staging, &e))?;
        fs::rename(&staging, &target).map_err(|e| fail("replace", &target, &e))?;
    }
    Ok(())
}

/// Deletes the extension's directory. Returns whether there was one.
///
/// # Errors
///
/// [`GnomeError::Io`] when it cannot be deleted, or when `dir` is not named after the extension
/// (a guard against deleting something else).
pub fn remove_files(dir: &Path) -> Result<bool, GnomeError> {
    if dir.file_name().and_then(|n| n.to_str()) != Some(UUID) {
        return Err(GnomeError::Io(format!(
            "refusing to delete {}: not the extension's directory",
            dir.display()
        )));
    }
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(GnomeError::Io(format!("cannot delete {}: {error}", dir.display()))),
    }
}

/// The GNOME settings the extension depends on.
pub trait Settings {
    /// The UUIDs in `org.gnome.shell enabled-extensions`.
    ///
    /// # Errors
    ///
    /// [`GnomeError::Io`] when the setting cannot be read or is not a list of strings.
    fn enabled_extensions(&mut self) -> Result<Vec<String>, GnomeError>;

    /// Replaces `enabled-extensions`.
    ///
    /// # Errors
    ///
    /// [`GnomeError::Io`] when the setting cannot be written.
    fn set_enabled_extensions(&mut self, uuids: &[String]) -> Result<(), GnomeError>;

    /// Whether extensions of the user may run at all (`disable-user-extensions` is false).
    ///
    /// # Errors
    ///
    /// [`GnomeError::Io`] when the setting cannot be read.
    fn user_extensions_allowed(&mut self) -> Result<bool, GnomeError>;
}

/// The real settings, through the `gsettings` command.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gsettings;

impl Gsettings {
    fn run(arguments: &[&str]) -> Result<String, GnomeError> {
        let output = Command::new("gsettings").args(arguments).output().map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                GnomeError::Io("the gsettings command was not found: is this a GNOME system?".into())
            } else {
                GnomeError::Io(format!("cannot run gsettings: {error}"))
            }
        })?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        } else {
            Err(GnomeError::Io(format!(
                "gsettings {} failed: {}",
                arguments.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            )))
        }
    }
}

impl Settings for Gsettings {
    fn enabled_extensions(&mut self) -> Result<Vec<String>, GnomeError> {
        parse_string_list(&Self::run(&["get", SCHEMA, ENABLED_KEY])?)
    }

    fn set_enabled_extensions(&mut self, uuids: &[String]) -> Result<(), GnomeError> {
        Self::run(&["set", SCHEMA, ENABLED_KEY, &format_string_list(uuids)]).map(drop)
    }

    fn user_extensions_allowed(&mut self) -> Result<bool, GnomeError> {
        Ok(Self::run(&["get", SCHEMA, DISABLE_KEY])? != "true")
    }
}

/// Reads a `GVariant` array of strings as `gsettings get` prints it: `['a', 'b']`, `[]` or `@as []`.
///
/// # Errors
///
/// [`GnomeError::Io`] when the text is not such an array. The caller must then not write the
/// setting back, or it would drop the extensions the user has.
pub fn parse_string_list(text: &str) -> Result<Vec<String>, GnomeError> {
    let bad = || GnomeError::Io(format!("cannot read {ENABLED_KEY} as a list of strings: {text}"));
    let body: &str = text.trim().trim_start_matches("@as").trim();
    let mut chars = body
        .strip_prefix('[')
        .and_then(|b| b.strip_suffix(']'))
        .ok_or_else(bad)?
        .chars()
        .peekable();
    let mut items: Vec<String> = Vec::new();
    loop {
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        let Some(quote) = chars.next() else { return Ok(items) };
        if quote != '\'' && quote != '"' {
            return Err(bad());
        }
        let mut item: String = String::new();
        loop {
            match chars.next().ok_or_else(bad)? {
                '\\' => item.push(chars.next().ok_or_else(bad)?),
                c if c == quote => break,
                c => item.push(c),
            }
        }
        items.push(item);
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        match chars.next() {
            Some(',') => {}
            None => return Ok(items),
            Some(_) => return Err(bad()),
        }
    }
}

/// Writes a list of strings as a `GVariant` array that `gsettings set` accepts.
#[must_use]
pub fn format_string_list(items: &[String]) -> String {
    let quoted: Vec<String> = items
        .iter()
        .map(|item| format!("'{}'", item.replace('\\', "\\\\").replace('\'', "\\'")))
        .collect();
    format!("[{}]", quoted.join(", "))
}

#[cfg(test)]
// Tests unwrap what they just built and index small vectors.
#[allow(clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir: PathBuf = std::env::temp_dir().join(format!("vividctl-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join(UUID)
    }

    #[test]
    fn the_embedded_extension_is_the_one_in_the_repository() {
        let metadata: &str = FILES.iter().find(|(n, _)| *n == "metadata.json").expect("embedded").1;
        assert!(metadata.contains(UUID), "the UUID constant must match metadata.json");
        let code: &str = FILES.iter().find(|(n, _)| *n == "extension.js").expect("embedded").1;
        assert!(code.contains("org.vividctl.Shell"));
    }

    #[test]
    fn the_install_dir_follows_the_xdg_rule() {
        let expected = |base: &str| PathBuf::from(base).join("gnome-shell/extensions").join(UUID);
        assert_eq!(install_dir(Some("/data"), Some("/home/u")).expect("ok"), expected("/data"));
        assert_eq!(
            install_dir(None, Some("/home/u")).expect("ok"),
            expected("/home/u/.local/share")
        );
        // An empty or relative XDG_DATA_HOME is ignored, as the specification says.
        assert_eq!(
            install_dir(Some(""), Some("/home/u")).expect("ok"),
            expected("/home/u/.local/share")
        );
        assert_eq!(
            install_dir(Some("rel"), Some("/home/u")).expect("ok"),
            expected("/home/u/.local/share")
        );
        assert!(install_dir(None, None).is_err());
        assert!(install_dir(None, Some("relative-home")).is_err());
    }

    #[test]
    fn writing_then_comparing_says_current_and_a_change_says_outdated() {
        let dir: PathBuf = temp_dir("write");
        assert_eq!(files_state(&dir), Files::Missing);
        write_files(&dir).expect("writes");
        assert_eq!(files_state(&dir), Files::Current);
        fs::write(dir.join("extension.js"), "// old").expect("edits");
        assert_eq!(files_state(&dir), Files::Outdated);
        write_files(&dir).expect("rewrites");
        assert_eq!(files_state(&dir), Files::Current);
        fs::remove_file(dir.join("metadata.json")).expect("removes");
        assert_eq!(files_state(&dir), Files::Outdated);
        let _ = fs::remove_dir_all(dir.parent().expect("parent"));
    }

    #[test]
    fn writing_leaves_no_staging_files_behind() {
        let dir: PathBuf = temp_dir("staging");
        write_files(&dir).expect("writes");
        let mut names: Vec<String> = fs::read_dir(&dir)
            .expect("lists")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["extension.js", "metadata.json"]);
        let _ = fs::remove_dir_all(dir.parent().expect("parent"));
    }

    #[test]
    fn removing_deletes_the_directory_and_says_whether_there_was_one() {
        let dir: PathBuf = temp_dir("remove");
        assert_eq!(remove_files(&dir), Ok(false));
        write_files(&dir).expect("writes");
        assert_eq!(remove_files(&dir), Ok(true));
        assert!(!dir.exists());
        let _ = fs::remove_dir_all(dir.parent().expect("parent"));
    }

    #[test]
    fn removing_refuses_a_directory_not_named_after_the_extension() {
        let dir: PathBuf = std::env::temp_dir().join(format!("vividctl-test-{}-guard", std::process::id()));
        fs::create_dir_all(&dir).expect("creates");
        assert!(matches!(remove_files(&dir), Err(GnomeError::Io(_))));
        assert!(dir.is_dir(), "the directory must still be there");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn gvariant_lists_are_read_the_way_gsettings_prints_them() {
        let list = |text: &str| parse_string_list(text).expect("parses");
        assert_eq!(list("@as []"), Vec::<String>::new());
        assert_eq!(list("[]"), Vec::<String>::new());
        assert_eq!(list("['a']"), ["a"]);
        assert_eq!(list("['a@b.org', 'c']"), ["a@b.org", "c"]);
        assert_eq!(list("[\"a\", 'b']"), ["a", "b"]);
        assert_eq!(list("['it\\'s', 'back\\\\slash']"), ["it's", "back\\slash"]);
        assert_eq!(list("  ['a' ,  'b'  ]  "), ["a", "b"]);
    }

    #[test]
    fn malformed_lists_are_errors_so_nothing_is_written_back() {
        for text in [
            "",
            "a",
            "['a'",
            "['a' 'b']",
            "[a]",
            "['a',, 'b']x",
            "[1, 2]",
            "['unterminated]",
        ] {
            assert!(parse_string_list(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn formatting_round_trips_through_parsing() {
        for items in [
            vec![],
            vec!["a".to_owned()],
            vec!["it's".to_owned(), "x\\y".to_owned(), "a@b.c".to_owned()],
        ] {
            assert_eq!(parse_string_list(&format_string_list(&items)).expect("parses"), items);
        }
        assert_eq!(format_string_list(&[]), "[]");
        assert_eq!(format_string_list(&["a".into(), "b".into()]), "['a', 'b']");
    }
}
