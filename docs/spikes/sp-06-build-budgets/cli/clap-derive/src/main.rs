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
    println!("{:?}", Cli::parse().command);
}
