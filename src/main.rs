use std::{
    error::Error,
    io::{self, Write},
    process::ExitCode,
};

use clap::Parser;
use r8::Value;

#[derive(Parser)]
#[command(
    name = "r8",
    version,
    about = "Execute the r8 ECMAScript arithmetic subset"
)]
struct Cli {
    /// Evaluate a Script and print its non-undefined completion value.
    #[arg(short = 'e', long = "eval", allow_hyphen_values = true)]
    source: String,
}

fn execute(cli: Cli) -> Result<(), Box<dyn Error>> {
    let value = r8::eval(&cli.source)?;
    if value != Value::Undefined {
        writeln!(io::stdout().lock(), "{value}")?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match execute(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "{error}");
            ExitCode::from(1)
        }
    }
}
