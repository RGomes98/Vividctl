use std::process::exit;

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

fn parse() -> Result<Command, pico_args::Error> {
    let mut args = pico_args::Arguments::from_env();
    if args.contains(["-h", "--help"]) {
        print!("{HELP}");
        exit(0)
    }
    if args.contains(["-V", "--version"]) {
        println!("vividctl {}", env!("CARGO_PKG_VERSION"));
        exit(0)
    }
    let Some(name) = args.subcommand()? else {
        print!("{HELP}");
        exit(2)
    };
    let command = match name.as_str() {
        "list" => Command::List {
            json: args.contains("--json"),
        },
        "get" => Command::Get {
            output: args.opt_value_from_str("--output")?,
        },
        "set" => {
            let output = args.opt_value_from_str("--output")?;
            Command::Set {
                value: args.free_from_str()?,
                output,
            }
        }
        "reset" => Command::Reset {
            output: args.opt_value_from_str("--output")?,
        },
        "doctor" => Command::Doctor,
        other => {
            eprintln!("error: unrecognized subcommand '{other}'\n\nFor more information, try '--help'.");
            exit(2)
        }
    };
    let rest = args.finish();
    if !rest.is_empty() {
        eprintln!("error: unexpected argument {:?}\n\nFor more information, try '--help'.", rest);
        exit(2)
    }
    Ok(command)
}

fn main() {
    match parse() {
        Ok(command) => println!("{command:?}"),
        Err(error) => {
            eprintln!("error: {error}\n\nFor more information, try '--help'.");
            exit(2)
        }
    }
}
