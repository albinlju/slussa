use crate::providers::error::FetchError;
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[allow(clippy::duration_suboptimal_units)]
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub(super) fn run_gh(args: &[&str]) -> Result<Vec<u8>, FetchError> {
    run_gh_stdin(args, &[])
}

pub(super) fn run_gh_stdin(args: &[&str], stdin: &[u8]) -> Result<Vec<u8>, FetchError> {
    let mut command = Command::new("gh");
    command.args(args);
    run_command(&mut command, stdin, REQUEST_TIMEOUT)
}

fn run_command(
    command: &mut Command,
    input: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, FetchError> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| FetchError::GhMissing)?;
    let mut stdin = take_pipe(child.stdin.take(), "stdin")?;
    let input = input.to_vec();
    let writer = background(move || stdin.write_all(&input));
    let mut stdout = take_pipe(child.stdout.take(), "stdout")?;
    let out = background(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let mut stderr = take_pipe(child.stderr.take(), "stderr")?;
    let err = background(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                // Do not wait for pipe readers: an inherited descriptor in a
                // credential-helper child must not hold the UI operation open.
                return Err(FetchError::Timeout);
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(FetchError::Network(error.to_string()));
            }
        }
    };
    let deadline = started + timeout;
    let stdout = receive(&out, deadline)?;
    let stderr = receive(&err, deadline)?;
    if !status.success() {
        return Err(FetchError::GhFailed {
            code: status.code(),
            stderr: format!(
                "{} {}",
                String::from_utf8_lossy(&stderr).trim(),
                String::from_utf8_lossy(&stdout).trim()
            )
            .trim()
            .into(),
        });
    }
    receive(&writer, deadline)?;
    Ok(stdout)
}

/// `Stdio::piped()` guarantees the handle exists; report the impossible case
/// as an error instead of panicking.
fn take_pipe<T>(pipe: Option<T>, name: &str) -> Result<T, FetchError> {
    pipe.ok_or_else(|| FetchError::Network(format!("gh {name} was not available")))
}

fn background<T: Send + 'static>(
    work: impl FnOnce() -> std::io::Result<T> + Send + 'static,
) -> std::sync::mpsc::Receiver<std::io::Result<T>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    rx
}

fn receive<T>(
    rx: &std::sync::mpsc::Receiver<std::io::Result<T>>,
    deadline: Instant,
) -> Result<T, FetchError> {
    rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| FetchError::Timeout)?
        .map_err(|e| FetchError::Network(e.to_string()))
}

pub(super) fn run_gh_json<T: serde::de::DeserializeOwned>(args: &[&str]) -> Result<T, FetchError> {
    let stdout = run_gh(args)?;
    serde_json::from_slice(&stdout).map_err(|e| {
        let sample: String = String::from_utf8_lossy(&stdout).chars().take(200).collect();
        tracing::warn!("gh json parse failed: {e} (first 200B: {sample})");
        FetchError::ParseFailed(e.to_string())
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn hung_process_times_out_and_large_bidirectional_io_does_not_deadlock() {
        let start = Instant::now();
        assert!(matches!(
            run_command(
                Command::new("sh").args(["-c", "sleep 2"]),
                &[],
                Duration::from_millis(40)
            ),
            Err(FetchError::Timeout)
        ));
        assert!(start.elapsed() < Duration::from_secs(1));
        let input = vec![b'x'; 256 * 1024];
        assert_eq!(
            run_command(&mut Command::new("cat"), &input, Duration::from_secs(2)).unwrap(),
            input
        );
    }
}
