mod executor;
mod protocol;
mod suite;

use std::{
    error::Error,
    io::{self, Write},
    num::NonZeroU64,
    path::PathBuf,
    process::ExitCode,
    time::Duration,
};

use clap::Parser;
use executor::{Execution, Failure};
use protocol::{CaseResult, Outcome, Report, Status, Summary};

#[derive(Parser)]
#[command(name = "r8-eval", version, about = "Run JavaScript tests against r8")]
struct Cli {
    /// JavaScript test file or directory to execute recursively.
    #[arg(required_unless_present = "worker", conflicts_with = "worker")]
    suite: Option<PathBuf>,
    #[arg(long, hide = true)]
    worker: bool,
    /// Directory containing Test262 harness scripts.
    #[arg(long, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/evals/harness"))]
    harness: PathBuf,
    /// Per-variant r8 worker timeout in milliseconds.
    #[arg(long, default_value = "2000")]
    timeout_ms: NonZeroU64,
}

fn evaluate(cli: Cli) -> Result<Report, Box<dyn Error>> {
    let suite_path = cli.suite.ok_or("Missing test suite.")?;
    let suite = suite::load(&suite_path)?;
    let mut results = Vec::new();
    let mut summary = Summary::default();
    for case in suite.cases {
        if let Some(reason) = suite::unsupported(&case) {
            summary.record(Status::Skip);
            results.push(CaseResult {
                case,
                status: Status::Skip,
                actual: None,
                diagnostic: Some(reason),
                stderr: String::new(),
                duration_ms: 0,
            });
            continue;
        }
        let execution = match suite::request(&case, &cli.harness) {
            Ok(request) => executor::run(&request, Duration::from_millis(cli.timeout_ms.get())),
            Err(message) => Execution {
                outcome: Ok(Outcome::HarnessError { message }),
                stderr: String::new(),
                duration_ms: 0,
            },
        };
        let (status, actual, diagnostic) = match execution.outcome {
            Ok(actual) => {
                let (status, diagnostic) = match &actual {
                    Outcome::Unsupported { reason } => (Status::Skip, Some(reason.clone())),
                    Outcome::HarnessError { message } => {
                        (Status::HarnessError, Some(message.clone()))
                    }
                    _ if case.matches(&actual) => (Status::Pass, None),
                    Outcome::Completed {} => (
                        Status::Fail,
                        Some("Expected an exception, but execution completed.".into()),
                    ),
                    Outcome::Error { message, .. } => (Status::Fail, Some(message.clone())),
                };
                (status, Some(actual), diagnostic)
            }
            Err(Failure::Timeout(message)) => (Status::Timeout, None, Some(message)),
            Err(Failure::Crash(message)) => (Status::Crash, None, Some(message)),
            Err(Failure::HarnessError(message)) => (Status::HarnessError, None, Some(message)),
        };
        summary.record(status);
        results.push(CaseResult {
            case,
            status,
            actual,
            diagnostic,
            stderr: execution.stderr,
            duration_ms: execution.duration_ms,
        });
    }
    Ok(Report {
        suite: suite_path.display().to_string(),
        files: suite.files,
        harness: cli.harness.display().to_string(),
        engine: "r8",
        timeout_ms: cli.timeout_ms.get(),
        summary,
        results,
    })
}

fn execute(cli: Cli) -> Result<ExitCode, Box<dyn Error>> {
    if cli.worker {
        executor::worker()?;
        return Ok(ExitCode::SUCCESS);
    }
    let report = evaluate(cli)?;
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    writeln!(stdout)?;
    eprintln!(
        "{}: {}/{} passed, {} skipped",
        report.suite, report.summary.pass, report.summary.total, report.summary.skip
    );
    Ok(if report.summary.pass == report.summary.total {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn main() -> ExitCode {
    match execute(Cli::parse()) {
        Ok(status) => status,
        Err(error) => {
            eprintln!("Evaluation error: {error}");
            ExitCode::from(2)
        }
    }
}
