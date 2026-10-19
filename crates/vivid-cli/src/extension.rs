//! `vividctl extension`: put the GNOME Shell extension in place, check it, take it out.
//!
//! The text is built by pure functions from the engine's reports, so it is tested without a
//! system; `run` only connects them to the real installer.

use std::io::{self, Write};

use vivid_core::BackendError;
use vivid_engine::{
    BackendReport, Engine,
    extension::{Change, ExtensionInstaller, ExtensionStatus, Files, InstallReport, RemoveReport},
};

use crate::cli::ExtensionAction;

/// What the gnome backend said when it was probed: its description when the extension runs.
type Running = Result<String, BackendError>;

/// Runs one action and returns the exit status.
pub fn run(action: &ExtensionAction, engine: &Engine, out: &mut dyn Write, err: &mut dyn Write) -> io::Result<u8> {
    let running: Running = engine
        .reports()
        .into_iter()
        .find(|report| report.name == "gnome")
        .map_or_else(
            || Err(BackendError::Unavailable("not built with the gnome feature".into())),
            |BackendReport { status, .. }| status,
        );
    let mut installer: ExtensionInstaller<_> = match ExtensionInstaller::system() {
        Ok(installer) => installer,
        Err(error) => {
            writeln!(err, "vividctl: error: {error}")?;
            return Ok(1);
        }
    };
    let result: Result<(Vec<String>, bool), BackendError> = match action {
        ExtensionAction::Install => installer
            .install()
            .map(|report| (install_lines(&report, &running), true)),
        ExtensionAction::Status => installer.status().map(|status| status_lines(&status, &running)),
        ExtensionAction::Remove => installer.remove().map(|report| (remove_lines(report, &running), true)),
    };
    match result {
        Ok((lines, ok)) => {
            for line in lines {
                writeln!(out, "{line}")?;
            }
            Ok(u8::from(!ok))
        }
        Err(error) => {
            writeln!(err, "vividctl: error: {error}")?;
            Ok(1)
        }
    }
}

const POLICY_HINT: &str =
    "GNOME has user extensions turned off: run `gsettings set org.gnome.shell disable-user-extensions false`.";

fn install_lines(report: &InstallReport, running: &Running) -> Vec<String> {
    let dir: String = report.dir.display().to_string();
    let mut lines: Vec<String> = vec![match report.change {
        Change::Created => format!("installed the extension in {dir}"),
        Change::Updated => format!("updated the extension in {dir}"),
        Change::Unchanged => format!("the extension in {dir} is already up to date"),
    }];
    lines.push(if report.newly_enabled {
        "enabled it".to_owned()
    } else {
        "it was already enabled".to_owned()
    });
    if !report.user_extensions_allowed {
        lines.push(POLICY_HINT.to_owned());
    } else if running.is_ok() && report.change == Change::Unchanged {
        lines.push("it is running: nothing more to do".to_owned());
    } else if running.is_ok() {
        lines.push("log out and back in to load the new version; the running one stays until then".to_owned());
    } else {
        lines.push(
            "log out and back in once: GNOME on Wayland only loads new extensions at login. Then `vividctl list` shows the gnome backend"
                .to_owned(),
        );
    }
    lines
}

fn status_lines(status: &ExtensionStatus, running: &Running) -> (Vec<String>, bool) {
    let files: &str = match status.files {
        Files::Current => "current",
        Files::Outdated => "differs from this vividctl (run `vividctl extension install`)",
        Files::Missing => "not installed (run `vividctl extension install`)",
    };
    let mut lines: Vec<String> = vec![
        format!("directory: {}", status.dir.display()),
        format!("files: {files}"),
        format!("enabled: {}", if status.enabled { "yes" } else { "no" }),
        format!(
            "user extensions: {}",
            if status.user_extensions_allowed {
                "allowed"
            } else {
                "turned off"
            }
        ),
        match running {
            Ok(description) => format!("running: yes ({description})"),
            Err(error) => format!("running: no ({error})"),
        },
    ];
    if !status.user_extensions_allowed {
        lines.push(POLICY_HINT.to_owned());
    }
    let ok: bool =
        status.files == Files::Current && status.enabled && status.user_extensions_allowed && running.is_ok();
    (lines, ok)
}

fn remove_lines(report: RemoveReport, running: &Running) -> Vec<String> {
    let mut lines: Vec<String> = vec![
        if report.removed_files {
            "deleted the extension's files"
        } else {
            "there were no extension files"
        }
        .to_owned(),
        if report.was_enabled {
            "disabled it"
        } else {
            "it was not enabled"
        }
        .to_owned(),
    ];
    if running.is_ok() {
        lines.push("log out and back in to unload it from this session; it keeps working until then".to_owned());
    }
    lines
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn report(change: Change, newly_enabled: bool, allowed: bool) -> InstallReport {
        InstallReport {
            dir: PathBuf::from("/home/u/.local/share/gnome-shell/extensions/vividctl@x"),
            change,
            newly_enabled,
            user_extensions_allowed: allowed,
        }
    }

    // The helper stands for a Result so the tests can swap it with `not_running`.
    #[allow(clippy::unnecessary_wraps)]
    fn running() -> Running {
        Ok("GNOME Shell 50.1 extension, interface 1".into())
    }

    fn not_running() -> Running {
        Err(BackendError::Unavailable("the extension is not running".into()))
    }

    fn joined(lines: &[String]) -> String {
        lines.join("\n")
    }

    #[test]
    fn a_first_install_tells_the_user_to_log_out_once() {
        let text: String = joined(&install_lines(&report(Change::Created, true, true), &not_running()));
        assert!(text.contains("installed the extension in /home/u"), "{text}");
        assert!(text.contains("enabled it"), "{text}");
        assert!(text.contains("log out and back in once"), "{text}");
    }

    #[test]
    fn repeating_an_install_on_a_running_extension_says_there_is_nothing_to_do() {
        let text: String = joined(&install_lines(&report(Change::Unchanged, false, true), &running()));
        assert!(
            text.contains("already up to date") && text.contains("already enabled"),
            "{text}"
        );
        assert!(text.contains("nothing more to do") && !text.contains("log out"), "{text}");
    }

    #[test]
    fn updating_a_running_extension_says_the_old_one_stays_until_login() {
        let text: String = joined(&install_lines(&report(Change::Updated, false, true), &running()));
        assert!(
            text.contains("updated the extension") && text.contains("running one stays"),
            "{text}"
        );
    }

    #[test]
    fn turned_off_user_extensions_are_explained_with_the_command_to_fix_them() {
        let text: String = joined(&install_lines(&report(Change::Created, true, false), &not_running()));
        assert!(text.contains("disable-user-extensions false"), "{text}");
        assert!(
            !text.contains("log out and back in once"),
            "logging in again would not help: {text}"
        );
    }

    fn status(files: Files, enabled: bool, allowed: bool) -> ExtensionStatus {
        ExtensionStatus {
            dir: PathBuf::from("/d"),
            files,
            enabled,
            user_extensions_allowed: allowed,
        }
    }

    #[test]
    fn status_is_ok_only_when_everything_is_in_place() {
        let (lines, ok) = status_lines(&status(Files::Current, true, true), &running());
        assert!(ok);
        assert!(joined(&lines).contains("running: yes (GNOME Shell 50.1"), "{lines:?}");
        for (files, enabled, allowed, up) in [
            (Files::Missing, true, true, true),
            (Files::Outdated, true, true, true),
            (Files::Current, false, true, true),
            (Files::Current, true, false, true),
            (Files::Current, true, true, false),
        ] {
            let running: Running = if up { self::running() } else { not_running() };
            assert!(
                !status_lines(&status(files, enabled, allowed), &running).1,
                "{files:?} {enabled} {allowed} {up}"
            );
        }
    }

    #[test]
    fn status_says_what_to_run_for_each_problem() {
        let text: String = joined(&status_lines(&status(Files::Missing, false, true), &not_running()).0);
        assert!(text.contains("not installed (run `vividctl extension install`)"), "{text}");
        assert!(
            text.contains("enabled: no") && text.contains("running: no (backend unavailable"),
            "{text}"
        );
        let outdated: String = joined(&status_lines(&status(Files::Outdated, true, true), &running()).0);
        assert!(outdated.contains("differs from this vividctl"), "{outdated}");
    }

    #[test]
    fn remove_says_what_was_done_and_when_the_session_still_has_it() {
        let done = RemoveReport {
            removed_files: true,
            was_enabled:   true,
        };
        let text: String = joined(&remove_lines(done, &running()));
        assert!(
            text.contains("deleted") && text.contains("disabled it") && text.contains("keeps working until then"),
            "{text}"
        );
        let nothing = RemoveReport {
            removed_files: false,
            was_enabled:   false,
        };
        let text: String = joined(&remove_lines(nothing, &not_running()));
        assert!(
            text.contains("no extension files") && text.contains("not enabled") && !text.contains("log out"),
            "{text}"
        );
    }
}
