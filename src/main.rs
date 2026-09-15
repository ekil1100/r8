use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::{ArgGroup, Parser};
use r8::{MAX_SOURCE_BYTES, Value};

#[derive(Parser)]
#[command(
    name = "r8",
    version,
    about = "Execute the r8 ECMAScript subset",
    group(ArgGroup::new("input").required(true).args(["source", "file"]))
)]
struct Cli {
    /// Evaluate inline source and print its non-undefined completion value.
    #[arg(short = 'e', long = "eval", allow_hyphen_values = true)]
    source: Option<String>,
    /// Execute a UTF-8 Script file and print its non-undefined completion value.
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,
}

fn execute(cli: Cli) -> Result<(), Box<dyn Error>> {
    let source = match cli.source {
        Some(source) => source,
        None => {
            let path = cli.file.expect("Clap requires exactly one input");
            let mut bytes = Vec::new();
            File::open(&path)
                .and_then(|file| {
                    file.take(MAX_SOURCE_BYTES as u64 + 1)
                        .read_to_end(&mut bytes)
                })
                .map_err(|error| format!("{}: {error}", path.display()))?;
            if bytes.len() > MAX_SOURCE_BYTES {
                return Err(format!(
                    "{}: ResourceLimit: Source exceeds the 1 MiB limit.",
                    path.display()
                )
                .into());
            }
            String::from_utf8(bytes)
                .map_err(|error| format!("{}: Invalid UTF-8 source: {error}", path.display()))?
        }
    };
    let value = r8::eval(&source)?;
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
