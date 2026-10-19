use std::{env, process::exit};

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
    eprintln!("error: {message}\n\nFor more information, try '--help'.");
    exit(2)
}

fn parse(args: Vec<String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let Some(name) = args.next() else {
        print!("{HELP}");
        exit(2)
    };
    match name.as_str() {
        "-h" | "--help" => {
            print!("{HELP}");
            exit(0)
        }
        "-V" | "--version" => {
            println!("vividctl {}", env!("CARGO_PKG_VERSION"));
            exit(0)
        }
        _ => {}
    }
    let (mut json, mut output, mut value) = (false, None, None);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" if name == "list" => json = true,
            "--output" if name != "list" && name != "doctor" => {
                output = Some(args.next().ok_or("--output needs a value")?)
            }
            "-h" | "--help" => {
                print!("{HELP}");
                exit(0)
            }
            other if name == "set" && value.is_none() && !other.starts_with("--") => value = Some(other.to_owned()),
            other => return Err(format!("unexpected argument '{other}'")),
        }
    }
    match name.as_str() {
        "list" => Ok(Command::List { json }),
        "get" => Ok(Command::Get { output }),
        "set" => Ok(Command::Set {
            value: value.ok_or("missing <VALUE>")?.parse().map_err(|_| "invalid <VALUE>")?,
            output,
        }),
        "reset" => Ok(Command::Reset { output }),
        "doctor" => Ok(Command::Doctor),
        other => Err(format!("unrecognized subcommand '{other}'")),
    }
}

fn main() {
    match parse(env::args().skip(1).collect()) {
        Ok(command) => println!("{command:?}"),
        Err(message) => fail(&message),
    }
}
