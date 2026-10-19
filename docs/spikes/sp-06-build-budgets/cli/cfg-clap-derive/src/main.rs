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
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "vividctl", version, about = "Digital Vibrance control")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List outputs and the backend that controls each
    List {
        /// Print JSON
        #[arg(long)]
        json: bool,
    },
    /// Show the current vibrance
    Get {
        /// Monitor identity
        #[arg(long)]
        output: Option<String>,
    },
    /// Set the vibrance, -1024 to 1023
    Set {
        #[arg(allow_hyphen_values = true)]
        value:  i16,
        /// Monitor identity
        #[arg(long)]
        output: Option<String>,
    },
    /// Back to neutral
    Reset {
        /// Monitor identity
        #[arg(long)]
        output: Option<String>,
    },
    /// Diagnose the system
    Doctor,
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
    println!("{:?}", Cli::parse().command);
}
