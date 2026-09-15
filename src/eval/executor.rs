use std::{
    fs::File,
    io::{self, Read, Seek, Write},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::protocol::{Outcome, Phase, Request};

const OUTPUT_LIMIT: u64 = 1024 * 1024;

pub enum Failure {
    Timeout(String),
    Crash(String),
    HarnessError(String),
}

pub struct Execution {
    pub outcome: Result<Outcome, Failure>,
    pub stderr: String,
    pub duration_ms: u128,
}

enum Stop {
    Exited(ExitStatus),
    Timeout,
    OutputLimit,
}

pub fn run(request: &Request, timeout: Duration) -> Execution {
    let started = Instant::now();
    let (outcome, stderr) = match capture(request, timeout) {
        Ok((stop, stdout, stderr)) => {
            let outcome = match stop {
                Stop::Exited(status) if status.success() => serde_json::from_slice(&stdout)
                    .map_err(|error| {
                        Failure::HarnessError(format!("Invalid r8 worker response: {error}"))
                    }),
                Stop::Exited(status) => {
                    Err(Failure::Crash(format!("r8 worker exited with {status}")))
                }
                Stop::Timeout => Err(Failure::Timeout(format!(
                    "r8 worker exceeded timeout of {} ms",
                    timeout.as_millis()
                ))),
                Stop::OutputLimit => Err(Failure::HarnessError(format!(
                    "r8 worker output limit exceeded ({OUTPUT_LIMIT} bytes per stream)"
                ))),
            };
            (outcome, String::from_utf8_lossy(&stderr).into_owned())
        }
        Err(error) => (
            Err(Failure::HarnessError(format!(
                "r8 worker I/O failed: {error}"
            ))),
            String::new(),
        ),
    };
    Execution {
        outcome,
        stderr,
        duration_ms: started.elapsed().as_millis(),
    }
}

pub fn worker() -> io::Result<()> {
    let outcome = match serde_json::from_reader(io::stdin().lock()) {
        Ok(request) => execute_r8(&request),
        Err(error) => Outcome::HarnessError {
            message: format!("Invalid r8 worker request: {error}"),
        },
    };
    let mut stdout = io::stdout().lock();
    serde_json::to_writer(&mut stdout, &outcome)?;
    writeln!(stdout)
}

fn execute_r8(request: &Request) -> Outcome {
    let script = match r8::Script::parse(&request.source) {
        Ok(script) => script,
        Err(error) => return language_outcome(error, Phase::Parse, &request.filename),
    };
    if request.parse_only {
        return Outcome::Completed {};
    }
    let mut context = r8::Context::default();
    for helper in &request.harness {
        if let Err(error) =
            r8::Script::parse(&helper.source).and_then(|script| context.run(&script))
        {
            let message = format!("r8 harness {}: {error}", helper.filename);
            return match error.kind {
                r8::ErrorKind::Unsupported => Outcome::Unsupported { reason: message },
                _ => Outcome::HarnessError { message },
            };
        }
    }
    match context.run(&script) {
        Ok(_) => Outcome::Completed {},
        Err(error) => language_outcome(error, Phase::Runtime, &request.filename),
    }
}

fn language_outcome(error: r8::Error, phase: Phase, filename: &str) -> Outcome {
    let message = format!("r8: {filename}: {error}");
    match error.kind {
        r8::ErrorKind::Syntax | r8::ErrorKind::Reference => Outcome::Error {
            phase,
            name: Some(
                match error.kind {
                    r8::ErrorKind::Syntax => "SyntaxError",
                    _ => "ReferenceError",
                }
                .into(),
            ),
            message,
        },
        r8::ErrorKind::Unsupported => Outcome::Unsupported { reason: message },
        r8::ErrorKind::ResourceLimit => Outcome::HarnessError { message },
    }
}

fn capture(request: &Request, timeout: Duration) -> io::Result<(Stop, Vec<u8>, Vec<u8>)> {
    let mut input = tempfile::tempfile()?;
    serde_json::to_writer(&mut input, request)?;
    input.rewind()?;
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    let mut child = Command::new(std::env::current_exe()?)
        .arg("--worker")
        .stdin(Stdio::from(input))
        .stdout(Stdio::from(stdout.try_clone()?))
        .stderr(Stdio::from(stderr.try_clone()?))
        .spawn()?;
    let stop = wait_for_child(&mut child, timeout, &stdout, &stderr);
    if stop.is_err() {
        let _ = terminate(&mut child);
    }
    Ok((stop?, read_output(&mut stdout)?, read_output(&mut stderr)?))
}

fn wait_for_child(
    child: &mut Child,
    timeout: Duration,
    stdout: &File,
    stderr: &File,
) -> io::Result<Stop> {
    let started = Instant::now();
    loop {
        let status = child.try_wait()?;
        if stdout.metadata()?.len() > OUTPUT_LIMIT || stderr.metadata()?.len() > OUTPUT_LIMIT {
            if status.is_none() {
                terminate(child)?;
            }
            return Ok(Stop::OutputLimit);
        }
        if let Some(status) = status {
            return Ok(Stop::Exited(status));
        }
        if started.elapsed() >= timeout {
            terminate(child)?;
            return Ok(Stop::Timeout);
        }
        thread::sleep(Duration::from_millis(5).min(timeout.saturating_sub(started.elapsed())));
    }
}

fn terminate(child: &mut Child) -> io::Result<()> {
    if let Err(error) = child.kill()
        && child.try_wait()?.is_none()
    {
        return Err(error);
    }
    child.wait()?;
    Ok(())
}

fn read_output(file: &mut File) -> io::Result<Vec<u8>> {
    file.rewind()?;
    let mut output = Vec::new();
    file.take(OUTPUT_LIMIT).read_to_end(&mut output)?;
    Ok(output)
}
