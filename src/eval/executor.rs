use std::{
    fs::File,
    io::{self, Read, Seek, Write},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::protocol::{Outcome, Request};

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

fn execute_r8(_request: &Request) -> Outcome {
    // Connect the real r8 parser and VM here when the engine library exists.
    Outcome::Unsupported {
        reason: "r8 parser and VM are not implemented yet.".into(),
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
