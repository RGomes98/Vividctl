//! What each subcommand does, on top of the engine.
//!
//! Results go to the writer for stdout, diagnostics to the one for stderr. The exit status is 0
//! when everything asked for worked and 1 when anything failed, including a failure on one of
//! several outputs: the others are still done.

use std::io::{self, Write};

use vivid_core::{BackendError, Vibrance};
use vivid_engine::{BackendReport, Engine, Entry, Listing, Outcome, SelectError, Selector};

use crate::{cli::Command, render};

/// Runs a subcommand and returns the exit status.
pub fn run(command: &Command, engine: &mut Engine, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    // A write error means the other end closed (for example `| head`): stop quietly.
    execute(command, engine, out, err).unwrap_or(1)
}

fn execute(command: &Command, engine: &mut Engine, out: &mut dyn Write, err: &mut dyn Write) -> io::Result<u8> {
    let listing: Listing = engine.listing();
    for problem in &listing.problems {
        writeln!(err, "vividctl: warning: {}: {}", problem.backend, problem.error)?;
    }
    match command {
        Command::List { json } => list(engine, &listing, *json, out, err),
        Command::Get { output } => get(engine, &listing, output.as_deref(), out, err),
        Command::Set { value, output } => match Vibrance::new(i64::from(*value)) {
            Ok(vibrance) => set(engine, &listing, output.as_deref(), vibrance, out, err),
            Err(error) => {
                writeln!(err, "vividctl: error: {error}")?;
                Ok(2)
            }
        },
        Command::Reset { output } => set(engine, &listing, output.as_deref(), Vibrance::NEUTRAL, out, err),
    }
}

fn monitor(entry: &Entry) -> String {
    entry.id.as_ref().map_or_else(|| "-".to_owned(), ToString::to_string)
}

fn labelled<T>(outcome: &Outcome<T>) -> (String, String) {
    (
        outcome.connector.clone(),
        outcome.id.as_ref().map_or_else(|| "-".to_owned(), ToString::to_string),
    )
}

/// The entries a command acts on, or the reason there are none, already printed.
fn targets<'a>(
    engine: &Engine,
    listing: &'a Listing,
    output: Option<&str>,
    err: &mut dyn Write,
) -> io::Result<Option<Vec<&'a Entry>>> {
    let selector: Option<Selector> = output.map(Selector::parse);
    match listing.select(selector.as_ref(), output.unwrap_or("")) {
        Ok(entries) => Ok(Some(entries)),
        Err(error) => {
            writeln!(err, "vividctl: error: {error}")?;
            if error == SelectError::NoConnectedOutputs {
                for BackendReport { name, status } in engine.reports() {
                    if let Err(reason) = status {
                        writeln!(err, "  {name} is unavailable: {reason}")?;
                    }
                }
            }
            Ok(None)
        }
    }
}

fn list(
    engine: &mut Engine,
    listing: &Listing,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> io::Result<u8> {
    let mut failed: bool = false;
    // Read the vibrance of every connected output once.
    let readings: Vec<Option<Result<Vibrance, BackendError>>> = listing
        .entries
        .iter()
        .map(|entry| {
            entry
                .output
                .connected
                .then(|| engine.get_all(&[entry]).into_iter().next().map(|o| o.result))
                .flatten()
        })
        .collect();
    for (entry, reading) in listing.entries.iter().zip(&readings) {
        if let Some(Err(error)) = reading {
            writeln!(err, "vividctl: {}: {error}", entry.output.connector)?;
            failed = true;
        }
    }
    let reports: Vec<BackendReport> = engine.reports();
    if json {
        write_json(listing, &readings, &reports, out)?;
    } else {
        write_human(listing, &readings, &reports, out)?;
    }
    Ok(u8::from(failed))
}

fn write_human(
    listing: &Listing,
    readings: &[Option<Result<Vibrance, BackendError>>],
    reports: &[BackendReport],
    out: &mut dyn Write,
) -> io::Result<()> {
    for report in reports {
        match &report.status {
            Ok(description) => writeln!(out, "{}: {description}", report.name)?,
            Err(error) => writeln!(out, "{}: unavailable: {error}", report.name)?,
        }
    }
    if listing.entries.is_empty() {
        return Ok(());
    }
    let rows: Vec<Vec<String>> = listing
        .entries
        .iter()
        .zip(readings)
        .map(|(entry, reading)| {
            vec![
                entry.backend.to_owned(),
                entry.output.connector.clone(),
                if entry.output.connected {
                    "connected"
                } else {
                    "disconnected"
                }
                .to_owned(),
                match reading {
                    Some(Ok(value)) => value.to_string(),
                    Some(Err(_)) => "error".to_owned(),
                    None => "-".to_owned(),
                },
                monitor(entry),
            ]
        })
        .collect();
    writeln!(out)?;
    write!(
        out,
        "{}",
        render::table(&["BACKEND", "CONNECTOR", "STATE", "VIBRANCE", "MONITOR"], &rows)
    )
}

fn write_json(
    listing: &Listing,
    readings: &[Option<Result<Vibrance, BackendError>>],
    reports: &[BackendReport],
    out: &mut dyn Write,
) -> io::Result<()> {
    let backends: Vec<String> = reports
        .iter()
        .map(|report| match &report.status {
            Ok(description) => format!(
                "{{\"name\":{},\"status\":\"ready\",\"description\":{}}}",
                render::json_string(report.name),
                render::json_string(description)
            ),
            Err(error) => format!(
                "{{\"name\":{},\"status\":\"unavailable\",\"reason\":{}}}",
                render::json_string(report.name),
                render::json_string(&error.to_string())
            ),
        })
        .collect();
    let outputs: Vec<String> = listing
        .entries
        .iter()
        .zip(readings)
        .map(|(entry, reading)| {
            let identity: String = entry.id.as_ref().map_or_else(
                || "null".to_owned(),
                |id| {
                    let spec = id.spec();
                    format!(
                        "{{\"vendor\":{},\"product\":{},\"serial\":{},\"slot\":{}}}",
                        render::json_string(&spec.vendor),
                        render::json_string(&spec.product),
                        render::json_string(&spec.serial),
                        id.slot().map_or_else(|| "null".to_owned(), |slot| slot.to_string())
                    )
                },
            );
            let (vibrance, error) = match reading {
                Some(Ok(value)) => (value.to_string(), "null".to_owned()),
                Some(Err(error)) => ("null".to_owned(), render::json_string(&error.to_string())),
                None => ("null".to_owned(), "null".to_owned()),
            };
            format!(
                "{{\"backend\":{},\"connector\":{},\"connected\":{},\"monitor\":{identity},\"vibrance\":{vibrance},\"error\":{error}}}",
                render::json_string(entry.backend),
                render::json_string(&entry.output.connector),
                entry.output.connected
            )
        })
        .collect();
    writeln!(
        out,
        "{{\"backends\":[{}],\"outputs\":[{}]}}",
        backends.join(","),
        outputs.join(",")
    )
}

fn get(
    engine: &mut Engine,
    listing: &Listing,
    output: Option<&str>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> io::Result<u8> {
    let Some(entries) = targets(engine, listing, output, err)? else {
        return Ok(1);
    };
    let mut failed: bool = false;
    for outcome in engine.get_all(&entries) {
        let (connector, id) = labelled(&outcome);
        match outcome.result {
            Ok(value) => writeln!(out, "{connector}  {id}  {value}")?,
            Err(error) => {
                writeln!(err, "vividctl: {connector}: {error}")?;
                failed = true;
            }
        }
    }
    Ok(u8::from(failed))
}

fn set(
    engine: &mut Engine,
    listing: &Listing,
    output: Option<&str>,
    value: Vibrance,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> io::Result<u8> {
    let Some(entries) = targets(engine, listing, output, err)? else {
        return Ok(1);
    };
    let mut failed: bool = false;
    for outcome in engine.set_all(&entries, value) {
        let (connector, id) = labelled(&outcome);
        match outcome.result {
            Ok(()) => writeln!(out, "{connector}  {id}  vibrance set to {value}")?,
            Err(error) => {
                writeln!(err, "vividctl: {connector}: {error}")?;
                failed = true;
            }
        }
    }
    Ok(u8::from(failed))
}

#[cfg(test)]
// Tests build fixtures with expect and index into small vectors.
#[allow(clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

    use vivid_core::{Backend, BackendOutput, Capabilities, MonitorSpec, OutputHandle};
    use vivid_engine::Slot;

    use super::*;

    #[derive(Default)]
    struct Shared {
        values:        BTreeMap<u64, Vibrance>,
        reject_set_on: Option<u64>,
        fail_listing:  bool,
    }

    struct Fake {
        outputs: Vec<BackendOutput>,
        shared:  Rc<RefCell<Shared>>,
    }

    impl Backend for Fake {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn probe(&mut self) -> Result<Capabilities, BackendError> {
            Ok(Capabilities {
                description:   "fake driver 1.0".into(),
                can_read_back: true,
            })
        }
        fn outputs(&mut self) -> Result<Vec<BackendOutput>, BackendError> {
            if self.shared.borrow().fail_listing {
                Err(BackendError::Io("device vanished".into()))
            } else {
                Ok(self.outputs.clone())
            }
        }
        fn vibrance(&mut self, output: OutputHandle) -> Result<Vibrance, BackendError> {
            Ok(self.shared.borrow().values.get(&output.0).copied().unwrap_or_default())
        }
        fn set_vibrance(&mut self, output: OutputHandle, value: Vibrance) -> Result<(), BackendError> {
            if self.shared.borrow().reject_set_on == Some(output.0) {
                return Err(BackendError::Rejected("the driver said no".into()));
            }
            self.shared.borrow_mut().values.insert(output.0, value);
            Ok(())
        }
    }

    fn output(handle: u64, connector: &str, spec: MonitorSpec, connected: bool) -> BackendOutput {
        BackendOutput {
            handle: OutputHandle(handle),
            connector: connector.into(),
            spec,
            connected,
        }
    }

    fn phl() -> MonitorSpec {
        MonitorSpec::new("PHL", "PHL 242V8", "0x00008fac")
    }

    fn engine(outputs: Vec<BackendOutput>, shared: &Rc<RefCell<Shared>>) -> Engine {
        Engine::new(vec![Slot::ready(
            "fake",
            Box::new(Fake {
                outputs,
                shared: Rc::clone(shared),
            }),
        )])
    }

    /// Runs a command and returns (status, stdout, stderr).
    fn go(command: &Command, engine: &mut Engine) -> (u8, String, String) {
        let (mut out, mut err): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
        let status: u8 = run(command, engine, &mut out, &mut err);
        (
            status,
            String::from_utf8(out).expect("utf-8"),
            String::from_utf8(err).expect("utf-8"),
        )
    }

    fn one_monitor() -> (Engine, Rc<RefCell<Shared>>) {
        let shared: Rc<RefCell<Shared>> = Rc::default();
        (
            engine(
                vec![
                    output(1, "HDMI-0", phl(), true),
                    output(2, "DP-0", MonitorSpec::unknown(), false),
                ],
                &shared,
            ),
            shared,
        )
    }

    #[test]
    fn list_shows_the_backend_and_a_table_with_the_vibrance_and_the_monitor() {
        let (mut engine, shared) = one_monitor();
        shared.borrow_mut().values.insert(1, Vibrance::clamped(512));
        let (status, out, err) = go(&Command::List { json: false }, &mut engine);
        assert_eq!((status, err.as_str()), (0, ""));
        assert!(out.starts_with("fake: fake driver 1.0\n\n"), "{out}");
        assert!(out.contains("BACKEND  CONNECTOR  STATE         VIBRANCE  MONITOR"), "{out}");
        assert!(
            out.contains("fake     HDMI-0     connected     512       PHL:PHL 242V8:0x00008fac"),
            "{out}"
        );
        assert!(out.contains("fake     DP-0       disconnected  -         -"), "{out}");
    }

    #[test]
    fn list_json_has_the_backends_and_every_output() {
        let (mut engine, _) = one_monitor();
        let (status, out, _) = go(&Command::List { json: true }, &mut engine);
        assert_eq!(status, 0);
        assert!(
            out.starts_with(
                "{\"backends\":[{\"name\":\"fake\",\"status\":\"ready\",\"description\":\"fake driver 1.0\"}]"
            ),
            "{out}"
        );
        assert!(out.contains("\"connector\":\"HDMI-0\",\"connected\":true,\"monitor\":{\"vendor\":\"PHL\",\"product\":\"PHL 242V8\",\"serial\":\"0x00008fac\",\"slot\":null},\"vibrance\":0"), "{out}");
        assert!(
            out.contains("\"connector\":\"DP-0\",\"connected\":false,\"monitor\":null,\"vibrance\":null"),
            "{out}"
        );
        assert_eq!(out.matches('\n').count(), 1, "one line of JSON");
    }

    #[test]
    fn list_json_escapes_what_a_monitor_reports() {
        let shared: Rc<RefCell<Shared>> = Rc::default();
        let tricky: MonitorSpec = MonitorSpec::new("ABC", "Model \"Q\"", "S\\N");
        let mut engine: Engine = engine(vec![output(1, "HDMI-0", tricky, true)], &shared);
        let (_, out, _) = go(&Command::List { json: true }, &mut engine);
        assert!(out.contains("\"product\":\"Model \\\"Q\\\"\",\"serial\":\"S\\\\N\""), "{out}");
    }

    #[test]
    fn list_says_when_a_backend_is_unavailable_and_still_succeeds() {
        let mut engine: Engine = Engine::new(vec![Slot::unavailable(
            "nvkms",
            BackendError::Unavailable("the NVIDIA kernel module is not loaded".into()),
        )]);
        let (status, out, _) = go(&Command::List { json: false }, &mut engine);
        assert_eq!(status, 0);
        assert_eq!(
            out,
            "nvkms: unavailable: backend unavailable: the NVIDIA kernel module is not loaded\n"
        );
        let (_, json, _) = go(&Command::List { json: true }, &mut engine);
        assert!(
            json.contains(
                "\"status\":\"unavailable\",\"reason\":\"backend unavailable: the NVIDIA kernel module is not loaded\""
            ),
            "{json}"
        );
    }

    #[test]
    fn get_prints_every_connected_output_by_default_and_one_when_named() {
        let (mut engine, shared) = one_monitor();
        shared.borrow_mut().values.insert(1, Vibrance::clamped(-300));
        let (status, out, _) = go(&Command::Get { output: None }, &mut engine);
        assert_eq!((status, out.as_str()), (0, "HDMI-0  PHL:PHL 242V8:0x00008fac  -300\n"));
        let (_, by_id, _) = go(
            &Command::Get {
                output: Some("PHL:PHL 242V8:0x00008fac".into()),
            },
            &mut engine,
        );
        let (_, by_name, _) = go(
            &Command::Get {
                output: Some("hdmi-0".into()),
            },
            &mut engine,
        );
        assert_eq!(by_id, out);
        assert_eq!(by_name, out);
    }

    #[test]
    fn a_read_that_fails_exits_with_1_and_prints_nothing_on_stdout() {
        let (mut engine, _) = one_monitor();
        // DP-0 exists in the listing but has no monitor attached, so it cannot be read.
        let (status, out, err) = go(
            &Command::Get {
                output: Some("DP-0".into()),
            },
            &mut engine,
        );
        assert_eq!((status, out.as_str()), (1, ""));
        assert!(
            err.contains("vividctl: DP-0: not supported: no monitor is attached to DP-0"),
            "{err}"
        );
    }

    #[test]
    fn set_changes_every_connected_output_by_default_and_reports_it() {
        let (mut engine, shared) = one_monitor();
        let (status, out, err) = go(
            &Command::Set {
                value:  512,
                output: None,
            },
            &mut engine,
        );
        assert_eq!((status, err.as_str()), (0, ""));
        assert_eq!(out, "HDMI-0  PHL:PHL 242V8:0x00008fac  vibrance set to 512\n");
        assert_eq!(shared.borrow().values.get(&1), Some(&Vibrance::clamped(512)));
        assert_eq!(shared.borrow().values.get(&2), None, "a disconnected output is not touched");
    }

    #[test]
    fn reset_puts_the_vibrance_back_to_zero() {
        let (mut engine, shared) = one_monitor();
        shared.borrow_mut().values.insert(1, Vibrance::MAX);
        let (status, out, _) = go(
            &Command::Reset {
                output: Some("HDMI-0".into()),
            },
            &mut engine,
        );
        assert_eq!(status, 0);
        assert!(out.contains("vibrance set to 0"), "{out}");
        assert_eq!(shared.borrow().values.get(&1), Some(&Vibrance::NEUTRAL));
    }

    #[test]
    fn an_unknown_monitor_is_an_error_and_nothing_is_changed() {
        let (mut engine, shared) = one_monitor();
        let (status, out, err) = go(
            &Command::Set {
                value:  100,
                output: Some("NO:SUCH:0x0".into()),
            },
            &mut engine,
        );
        assert_eq!((status, out.as_str()), (1, ""));
        assert!(err.contains("no output matches \"NO:SUCH:0x0\""), "{err}");
        assert!(shared.borrow().values.is_empty());
    }

    #[test]
    fn a_failure_on_one_output_is_reported_and_the_others_are_still_done() {
        let shared: Rc<RefCell<Shared>> = Rc::default();
        shared.borrow_mut().reject_set_on = Some(2);
        let mut engine: Engine = engine(
            vec![
                output(1, "DP-0", phl(), true),
                output(2, "DP-1", MonitorSpec::new("AOC", "X", "0x2"), true),
                output(3, "DP-2", MonitorSpec::new("AOC", "Y", "0x3"), true),
            ],
            &shared,
        );
        let (status, out, err) = go(
            &Command::Set {
                value:  50,
                output: None,
            },
            &mut engine,
        );
        assert_eq!(status, 1);
        assert_eq!(out.lines().count(), 2, "{out}");
        assert!(err.contains("vividctl: DP-1: request rejected: the driver said no"), "{err}");
        assert_eq!(shared.borrow().values.get(&3), Some(&Vibrance::clamped(50)));
    }

    #[test]
    fn with_nothing_to_act_on_the_error_says_why_the_backend_is_missing() {
        let mut engine: Engine = Engine::new(vec![Slot::unavailable(
            "nvkms",
            BackendError::Unavailable("module not loaded".into()),
        )]);
        let (status, out, err) = go(
            &Command::Set {
                value:  10,
                output: None,
            },
            &mut engine,
        );
        assert_eq!((status, out.as_str()), (1, ""));
        assert!(err.contains("no connected output can be controlled"), "{err}");
        assert!(
            err.contains("nvkms is unavailable: backend unavailable: module not loaded"),
            "{err}"
        );
    }

    #[test]
    fn a_backend_that_fails_to_list_is_a_warning_and_the_command_goes_on() {
        let shared: Rc<RefCell<Shared>> = Rc::default();
        shared.borrow_mut().fail_listing = true;
        let mut engine: Engine = engine(vec![], &shared);
        let (status, _, err) = go(&Command::List { json: false }, &mut engine);
        assert_eq!(status, 0);
        assert!(err.contains("vividctl: warning: fake: i/o error: device vanished"), "{err}");
    }

    #[test]
    fn a_closed_output_pipe_ends_the_run_quietly() {
        struct Closed;
        impl Write for Closed {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let (mut engine, _) = one_monitor();
        let mut err: Vec<u8> = Vec::new();
        assert_eq!(run(&Command::List { json: false }, &mut engine, &mut Closed, &mut err), 1);
        assert_eq!(err, Vec::<u8>::new());
    }
}
