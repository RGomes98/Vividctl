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
