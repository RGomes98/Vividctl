//! The real binary, from the outside. None of these tests changes a vibrance: every `set` and
//! `reset` names a monitor that does not exist, so they are safe on a machine with an NVIDIA GPU.

// Integration tests expect the binary to run and fail loudly when it does not.
#![allow(clippy::expect_used)]

use std::process::{Command, Output};

fn vividctl(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vividctl"))
        .args(args)
        .output()
        .expect("the binary runs")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn help_lists_every_subcommand() {
    let output: Output = vividctl(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let help: String = text(&output.stdout);
    for subcommand in ["list", "get", "set", "reset"] {
        assert!(help.contains(subcommand), "{help}");
    }
    assert!(help.contains("identity"), "the help explains how to name a monitor: {help}");
}

#[test]
fn every_subcommand_has_its_own_help() {
    for subcommand in ["list", "get", "set", "reset"] {
        let output: Output = vividctl(&[subcommand, "--help"]);
        assert_eq!(output.status.code(), Some(0), "{subcommand}");
        assert!(text(&output.stdout).contains("Usage: vividctl"), "{subcommand}");
    }
}

#[test]
fn the_version_is_printed() {
    let output: Output = vividctl(&["--version"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(text(&output.stdout).starts_with("vividctl "), "{}", text(&output.stdout));
}

#[test]
fn a_bad_command_line_exits_with_2_and_nothing_on_stdout() {
    for args in [
        &["bogus"][..],
        &["set"],
        &["set", "abc"],
        &["set", "5000"],
        &["set", "-1025"],
        &["get", "--outptu", "X"],
        &[],
    ] {
        let output: Output = vividctl(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
    }
}

#[test]
fn a_negative_vibrance_is_accepted_by_the_parser() {
    // Runs to the point of resolving the monitor, which does not exist, so nothing is changed.
    let output: Output = vividctl(&["set", "-300", "--output", "NO:SUCH:0x0"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stderr).contains("no output matches"), "{}", text(&output.stderr));
}

#[test]
fn naming_a_monitor_that_does_not_exist_is_an_error_for_every_command() {
    for args in [
        &["get", "--output", "NO:SUCH:0x0"][..],
        &["set", "5", "--output", "NO:SUCH:0x0"],
        &["reset", "--output", "NO:SUCH:0x0"],
    ] {
        let output: Output = vividctl(args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
    }
}

#[test]
fn list_works_with_or_without_an_nvidia_gpu() {
    let output: Output = vividctl(&["list"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(text(&output.stdout).starts_with("nvkms: "), "{}", text(&output.stdout));
}

#[test]
fn list_json_is_one_line_of_json_with_both_sections() {
    let output: Output = vividctl(&["list", "--json"]);
    assert_eq!(output.status.code(), Some(0));
    let line: String = text(&output.stdout);
    assert!(
        line.starts_with("{\"backends\":[") && line.contains("],\"outputs\":["),
        "{line}"
    );
    assert_eq!(line.trim_end().matches('\n').count(), 0);
}

/// `vividctl extension` against a throw-away data directory and a `gsettings` stand-in, so the
/// test never touches the real GNOME settings or the user's extensions.
#[cfg(feature = "gnome")]
mod extension {
    use std::{
        env, fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
    };

    use super::*;

    struct Sandbox {
        root: PathBuf,
    }

    impl Sandbox {
        fn new(name: &str) -> Self {
            let root: PathBuf = env::temp_dir().join(format!("vividctl-cli-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("bin")).expect("creates the sandbox");
            let stub: PathBuf = root.join("bin/gsettings");
            // `get SCHEMA KEY` and `set SCHEMA KEY VALUE`, with the value kept in a file.
            fs::write(
                &stub,
                "#!/bin/sh\nstate=\"$SANDBOX/state\"\ncase \"$1 $3\" in\n  'get enabled-extensions') cat \"$state\" 2>/dev/null || echo '@as []';;\n  'get disable-user-extensions') echo false;;\n  'set enabled-extensions') printf '%s\\n' \"$4\" > \"$state\";;\nesac\n",
            )
            .expect("writes the stub");
            fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).expect("makes it executable");
            Self { root }
        }

        fn run(&self, args: &[&str]) -> Output {
            let path: String = format!("{}:{}", self.root.join("bin").display(), env::var("PATH").unwrap_or_default());
            Command::new(env!("CARGO_BIN_EXE_vividctl"))
                .args(args)
                .env("PATH", path)
                .env("SANDBOX", &self.root)
                .env("XDG_DATA_HOME", self.root.join("data"))
                // No session bus: the gnome backend is then unavailable, as before a first login.
                .env_remove("DBUS_SESSION_BUS_ADDRESS")
                .output()
                .expect("the binary runs")
        }

        fn extension_dir(&self) -> PathBuf {
            self.root
                .join("data/gnome-shell/extensions/vividctl@rgomes98.github.io")
        }

        fn enabled(&self) -> String {
            fs::read_to_string(self.root.join("state")).unwrap_or_default()
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .expect("lists")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn install_status_and_remove_work_end_to_end() {
        let sandbox: Sandbox = Sandbox::new("lifecycle");

        let before: Output = sandbox.run(&["extension", "status"]);
        assert_eq!(before.status.code(), Some(1), "nothing is installed yet");
        assert!(text(&before.stdout).contains("not installed"), "{}", text(&before.stdout));

        let install: Output = sandbox.run(&["extension", "install"]);
        assert_eq!(install.status.code(), Some(0), "{}", text(&install.stderr));
        assert!(
            text(&install.stdout).contains("installed the extension"),
            "{}",
            text(&install.stdout)
        );
        assert_eq!(files(&sandbox.extension_dir()), ["extension.js", "metadata.json"]);
        assert!(
            sandbox.enabled().contains("vividctl@rgomes98.github.io"),
            "{}",
            sandbox.enabled()
        );

        let again: Output = sandbox.run(&["extension", "install"]);
        assert!(text(&again.stdout).contains("already up to date"), "{}", text(&again.stdout));
        assert_eq!(
            sandbox.enabled().matches("vividctl@").count(),
            1,
            "enabled once: {}",
            sandbox.enabled()
        );

        let status: Output = sandbox.run(&["extension", "status"]);
        let report: String = text(&status.stdout);
        assert!(report.contains("files: current") && report.contains("enabled: yes"), "{report}");
        assert_eq!(status.status.code(), Some(1), "not running without a shell: {report}");

        let remove: Output = sandbox.run(&["extension", "remove"]);
        assert_eq!(remove.status.code(), Some(0), "{}", text(&remove.stderr));
        assert!(!sandbox.extension_dir().exists());
        assert!(!sandbox.enabled().contains("vividctl@"), "{}", sandbox.enabled());
    }

    #[test]
    fn a_stale_copy_is_updated() {
        let sandbox: Sandbox = Sandbox::new("stale");
        sandbox.run(&["extension", "install"]);
        fs::write(sandbox.extension_dir().join("extension.js"), "// old").expect("edits");
        let status: Output = sandbox.run(&["extension", "status"]);
        assert!(
            text(&status.stdout).contains("differs from this vividctl"),
            "{}",
            text(&status.stdout)
        );
        let update: Output = sandbox.run(&["extension", "install"]);
        assert!(
            text(&update.stdout).contains("updated the extension"),
            "{}",
            text(&update.stdout)
        );
    }

    #[test]
    fn the_extension_command_needs_an_action() {
        let output: Output = vividctl(&["extension"]);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(output.stdout, Vec::<u8>::new());
    }
}
