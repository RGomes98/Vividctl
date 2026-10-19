use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Entry {
    vendor:   String,
    product:  String,
    serial:   String,
    vibrance: i16,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Config {
    version: u32,
    entries: Vec<Entry>,
}

fn load(text: &str) -> Result<Config, toml::de::Error> {
    toml::from_str(text)
}
use std::process::exit;

use lexopt::prelude::*;

const HELP: &str = "Digital Vibrance control

Usage: vividctl <COMMAND>

Commands:
  list    List outputs and the backend that controls each
  get     Show the current vibrance
  set     Set the vibrance, -1024 to 1023
  reset   Back to neutral
  doctor  Diagnose the system

Options:
  -h, --help     Print help
  -V, --version  Print version
";

#[derive(Debug)]
#[allow(dead_code)]
enum Command {
    List { json: bool },
    Get { output: Option<String> },
    Set { value: i16, output: Option<String> },
    Reset { output: Option<String> },
    Doctor,
}

fn fail(message: &str) -> ! {
    eprintln!("error: {message}\n\nUsage: vividctl <COMMAND>\n\nFor more information, try '--help'.");
    exit(2)
}

fn parse() -> Result<Command, lexopt::Error> {
    let mut parser = lexopt::Parser::from_env();
    let name = match parser.next()? {
        Some(Value(name)) => name.string()?,
        Some(Short('h') | Long("help")) => {
            print!("{HELP}");
            exit(0)
        }
        Some(Short('V') | Long("version")) => {
            println!("vividctl {}", env!("CARGO_PKG_VERSION"));
            exit(0)
        }
        Some(other) => return Err(other.unexpected()),
        None => {
            print!("{HELP}");
            exit(2)
        }
    };
    let (mut json, mut output, mut value) = (false, None, None);
    while let Some(arg) = parser.next()? {
        match arg {
            Long("json") if name == "list" => json = true,
            Long("output") if name != "list" && name != "doctor" => output = Some(parser.value()?.string()?),
            Value(v) if name == "set" && value.is_none() => value = Some(v.string()?),
            // lexopt reads `-300` as the short options `-3 -0 -0`. A negative vibrance is a normal
            // argument here, so glue the digits back together.
            Short(digit) if name == "set" && value.is_none() && digit.is_ascii_digit() => {
                let rest = parser
                    .optional_value()
                    .map(|r| r.string())
                    .transpose()?
                    .unwrap_or_default();
                value = Some(format!("-{digit}{rest}"));
            }
            Short('h') | Long("help") => {
                print!("{HELP}");
                exit(0)
            }
            other => return Err(other.unexpected()),
        }
    }
    Ok(match name.as_str() {
        "list" => Command::List { json },
        "get" => Command::Get { output },
        "set" => Command::Set {
            value: value.ok_or("missing <VALUE>")?.parse().map_err(|_| "invalid <VALUE>")?,
            output,
        },
        "reset" => Command::Reset { output },
        "doctor" => Command::Doctor,
        other => return Err(format!("unrecognized subcommand '{other}'").into()),
    })
}

fn main() {
    let _ = load(concat!(
        "version = 1\n",
        "[[entries]]\n",
        "vendor = \"PHL\"\n",
        "product = \"PHL 242V8\"\n",
        "serial = \"0x00008fac\"\n",
        "vibrance = 512\n"
    ));
    match parse() {
        Ok(command) => println!("{command:?}"),
        Err(error) => fail(&error.to_string()),
    }
}
