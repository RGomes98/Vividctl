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
use clap::{Arg, ArgAction, Command};

fn output_arg() -> Arg {
    Arg::new("output")
        .long("output")
        .help("Monitor identity")
        .value_name("OUTPUT")
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
    let matches = Command::new("vividctl")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Digital Vibrance control")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("list")
                .about("List outputs and the backend that controls each")
                .arg(
                    Arg::new("json")
                        .long("json")
                        .help("Print JSON")
                        .action(ArgAction::SetTrue),
                ),
        )
        .subcommand(Command::new("get").about("Show the current vibrance").arg(output_arg()))
        .subcommand(
            Command::new("set")
                .about("Set the vibrance, -1024 to 1023")
                .arg(
                    Arg::new("value")
                        .required(true)
                        .allow_hyphen_values(true)
                        .value_parser(clap::value_parser!(i16)),
                )
                .arg(output_arg()),
        )
        .subcommand(Command::new("reset").about("Back to neutral").arg(output_arg()))
        .subcommand(Command::new("doctor").about("Diagnose the system"))
        .get_matches();
    let output = |m: &clap::ArgMatches| m.get_one::<String>("output").cloned();
    match matches.subcommand() {
        Some(("list", m)) => println!("List {{ json: {} }}", m.get_flag("json")),
        Some(("get", m)) => println!("Get {{ output: {:?} }}", output(m)),
        Some(("set", m)) => println!(
            "Set {{ value: {}, output: {:?} }}",
            m.get_one::<i16>("value").copied().unwrap_or(0),
            output(m)
        ),
        Some(("reset", m)) => println!("Reset {{ output: {:?} }}", output(m)),
        Some(("doctor", _)) => println!("Doctor"),
        _ => {}
    }
}
