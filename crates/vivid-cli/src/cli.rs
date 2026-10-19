//! The command line, as `clap` reads it.

use clap::{Parser, Subcommand};

/// Digital Vibrance control.
///
/// This version controls the outputs that an NVIDIA GPU drives. Name a monitor with `--output`
/// by its identity (the `VENDOR:PRODUCT:SERIAL` that `list` shows, which follows the monitor
/// from port to port) or by its connector name. Without `--output`, every connected output is
/// used.
#[derive(Parser, Debug)]
#[command(name = "vividctl", version)]
pub struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// The subcommands.
#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum Command {
    /// List the outputs, the backend that controls each, and their current vibrance
    List {
        /// Print JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Show the current vibrance
    Get {
        /// The monitor: its identity or a connector name. Default: every connected output
        #[arg(long, value_name = "MONITOR")]
        output: Option<String>,
    },
    /// Set the vibrance: -1024 (washed out) to 1023 (saturated), 0 is neutral
    Set {
        /// The vibrance
        #[arg(allow_hyphen_values = true, value_parser = clap::value_parser!(i16).range(-1024..=1023))]
        value:  i16,
        /// The monitor: its identity or a connector name. Default: every connected output
        #[arg(long, value_name = "MONITOR")]
        output: Option<String>,
    },
    /// Put the vibrance back to neutral (0)
    Reset {
        /// The monitor: its identity or a connector name. Default: every connected output
        #[arg(long, value_name = "MONITOR")]
        output: Option<String>,
    },
    /// Install, check or remove the GNOME Shell extension that controls the outputs NVIDIA does not drive
    #[cfg(feature = "gnome")]
    Extension {
        /// What to do with the extension
        #[command(subcommand)]
        action: ExtensionAction,
    },
}

/// What to do with the GNOME Shell extension.
#[cfg(feature = "gnome")]
#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum ExtensionAction {
    /// Copy the extension that comes with this vividctl into your user directory and enable it
    Install,
    /// Say whether the extension is installed, enabled and running
    Status,
    /// Disable the extension and delete its files
    Remove,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    fn parse(args: &[&str]) -> Result<Command, clap::Error> {
        Cli::try_parse_from(std::iter::once("vividctl").chain(args.iter().copied())).map(|cli| cli.command)
    }

    /// The command, or `None` when the arguments are rejected.
    fn read(args: &[&str]) -> Option<Command> {
        parse(args).ok()
    }

    #[cfg(feature = "gnome")]
    #[test]
    fn the_extension_subcommands_are_read() {
        for (word, action) in [
            ("install", ExtensionAction::Install),
            ("status", ExtensionAction::Status),
            ("remove", ExtensionAction::Remove),
        ] {
            assert_eq!(read(&["extension", word]), Some(Command::Extension { action }));
        }
        assert_eq!(read(&["extension"]), None, "the action is required");
        assert_eq!(read(&["extension", "bogus"]), None);
    }

    #[test]
    fn the_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn subcommands_and_flags_are_read() {
        assert_eq!(read(&["list"]), Some(Command::List { json: false }));
        assert_eq!(read(&["list", "--json"]), Some(Command::List { json: true }));
        assert_eq!(read(&["get"]), Some(Command::Get { output: None }));
        assert_eq!(
            read(&["get", "--output", "HDMI-0"]),
            Some(Command::Get {
                output: Some("HDMI-0".into()),
            })
        );
        assert_eq!(
            read(&["reset", "--output", "PHL:PHL 242V8:0x00008fac"]),
            Some(Command::Reset {
                output: Some("PHL:PHL 242V8:0x00008fac".into()),
            })
        );
    }

    #[test]
    fn negative_vibrance_is_a_value_not_an_option() {
        assert_eq!(
            read(&["set", "-300"]),
            Some(Command::Set {
                value:  -300,
                output: None,
            })
        );
        assert_eq!(
            read(&["set", "-1024", "--output", "X"]),
            Some(Command::Set {
                value:  -1024,
                output: Some("X".into()),
            })
        );
        assert_eq!(
            read(&["set", "--output", "X", "1023"]),
            Some(Command::Set {
                value:  1023,
                output: Some("X".into()),
            })
        );
    }

    #[test]
    fn values_outside_the_range_are_usage_errors() {
        for value in ["1024", "-1025", "99999", "abc", "1.5"] {
            assert!(parse(&["set", value]).is_err(), "{value}");
        }
        assert!(parse(&["set"]).is_err());
    }

    #[test]
    fn unknown_commands_and_flags_are_usage_errors_with_a_suggestion() {
        let error: String = parse(&["lst"]).expect_err("unknown subcommand").to_string();
        assert!(error.contains("similar subcommand exists: 'list'"), "{error}");
        let error: String = parse(&["get", "--outptu", "X"]).expect_err("unknown flag").to_string();
        assert!(error.contains("similar argument exists: '--output'"), "{error}");
    }
}
