//! Running the configured command: its process, its deadline, stopping it and
//! telling the failures apart.

use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

/// How long an agent may take. A review of a large PR is minutes, not seconds.
pub const TIMEOUT: Duration = Duration::from_mins(10);

/// How much of its answer is read, so that a runaway command cannot fill memory.
const MAX_ANSWER_BYTES: u64 = 4 * 1024 * 1024;

/// How long the rest of what a command printed is waited for once it has ended.
/// It has written everything by then, so this is only reached when it left a
/// process behind that holds its output open, and that one is not waited for.
const DRAIN: Duration = Duration::from_millis(500);

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("the command is empty; set `agent_review` in the config")]
    NoCommand,
    #[error("could not start `{program}`: {reason}")]
    NotStarted { program: String, reason: String },
    #[error("`{program}` did not answer within {} minutes", .after.as_secs() / 60)]
    TimedOut { program: String, after: Duration },
    #[error("`{program}` failed{}{}", exit(*.code), said(.stderr))]
    Failed {
        program: String,
        code: Option<i32>,
        stderr: String,
    },
    #[error("`{program}` answered with text that is not UTF-8")]
    NotText { program: String },
    #[error("the review was stopped")]
    Cancelled { program: String },
    #[error("there is no JSON in what `{program}` answered")]
    NoJson { program: String },
    #[error("what `{program}` answered cannot be used: {reason}")]
    Unusable { program: String, reason: String },
}

impl AgentError {
    /// The failure for the log: which one it was and how much was said, never
    /// what the command printed, which can repeat the PR it was given.
    pub fn for_log(&self) -> String {
        match self {
            Self::NoCommand => "no command".into(),
            Self::NotStarted { reason, .. } => format!("not started: {reason}"),
            Self::TimedOut { after, .. } => format!("timed out after {}s", after.as_secs()),
            Self::Failed { code, stderr, .. } => {
                format!("failed{} stderr_bytes={}", exit(*code), stderr.len())
            }
            Self::NotText { .. } => "answer not UTF-8".into(),
            Self::Cancelled { .. } => "stopped".into(),
            Self::NoJson { .. } => "no JSON in the answer".into(),
            Self::Unusable { reason, .. } => {
                format!("unusable answer reason_bytes={}", reason.len())
            }
        }
    }
}

fn exit(code: Option<i32>) -> String {
    code.map_or_else(String::new, |code| format!(" (exit {code})"))
}

fn said(stderr: &str) -> String {
    let line = stderr.lines().rev().find(|line| !line.trim().is_empty());
    line.map_or_else(String::new, |line| {
        let shown: String = line.chars().take(200).collect();
        format!(": {shown}")
    })
}

/// A way to tell a running agent to stop. Clones share it, and the command that is
/// running looks at it a few times a second.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// How many agent commands are running in this process. Leaving waits for it to
/// be none, so that no command goes on with nobody to read what it says.
static RUNNING: AtomicUsize = AtomicUsize::new(0);

/// One command running, for as long as it is held.
struct Running;

impl Running {
    fn start() -> Self {
        RUNNING.fetch_add(1, Ordering::SeqCst);
        Self
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        RUNNING.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Wait until no agent command is running, up to `timeout`; whether none is.
pub fn wait_until_stopped(timeout: Duration) -> bool {
    let started = Instant::now();
    while RUNNING.load(Ordering::SeqCst) > 0 {
        if started.elapsed() >= timeout {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    true
}

/// Run `command` with `input` on its standard input and return what it printed.
/// It is stopped, and its process ended, when `cancel` is asked to or the time is
/// up.
pub fn run(
    command: &[String],
    input: &str,
    timeout: Duration,
    cancel: &Cancel,
) -> Result<String, AgentError> {
    let (program, args) = command.split_first().ok_or(AgentError::NoCommand)?;
    // Asked to stop before it began, as when the reader leaves while the PR's diff
    // is still being read: nothing is started.
    if cancel.is_cancelled() {
        return Err(AgentError::Cancelled {
            program: program.clone(),
        });
    }
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AgentError::NotStarted {
            program: program.clone(),
            reason: e.to_string(),
        })?;
    let (Some(mut stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        let _ = child.kill();
        return Err(AgentError::NotStarted {
            program: program.clone(),
            reason: "its input or output was not available".into(),
        });
    };
    // Held until the process is gone, whichever way this ends.
    let _running = Running::start();
    let input = input.as_bytes().to_vec();
    // A command that stops reading its input closes the pipe; that is not an
    // error here, and the writing is not waited for: a process the command left
    // behind may hold the pipe and never read it.
    std::thread::spawn(move || stdin.write_all(&input));
    let out = Reading::of(stdout);
    let err = Reading::of(stderr);
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if cancel.is_cancelled() => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AgentError::Cancelled {
                    program: program.clone(),
                });
            }
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AgentError::TimedOut {
                    program: program.clone(),
                    after: timeout,
                });
            }
        }
    };
    let until = Instant::now() + DRAIN;
    let stdout = out.taken(until);
    let stderr = String::from_utf8_lossy(&err.taken(until)).into_owned();
    if !status.success() {
        return Err(AgentError::Failed {
            program: program.clone(),
            code: status.code(),
            stderr,
        });
    }
    String::from_utf8(stdout).map_err(|_not_utf8| AgentError::NotText {
        program: program.clone(),
    })
}

/// What a pipe of the command has given so far, and word of when it has given
/// everything. Kept as it comes, so that what was printed can be taken without
/// waiting for a pipe that is never closed.
struct Reading {
    bytes: Arc<Mutex<Vec<u8>>>,
    ended: mpsc::Receiver<()>,
}

impl Reading {
    fn of(pipe: impl Read + Send + 'static) -> Self {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let (end, ended) = mpsc::channel();
        let kept = Arc::clone(&bytes);
        std::thread::spawn(move || {
            let mut pipe = pipe.take(MAX_ANSWER_BYTES);
            let mut chunk = [0; 8192];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(read) => kept
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .extend(chunk.iter().take(read)),
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            let _ = end.send(());
        });
        Self { bytes, ended }
    }

    /// What was read, waiting until `until` for the pipe to end.
    fn taken(self, until: Instant) -> Vec<u8> {
        let wait = until.saturating_duration_since(Instant::now());
        if self.ended.recv_timeout(wait).is_err() {
            tracing::warn!("an agent command left its output open; taking what it had printed");
        }
        std::mem::take(&mut *self.bytes.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run that nobody is going to stop.
    fn run(command: &[String], input: &str, timeout: Duration) -> Result<String, AgentError> {
        super::run(command, input, timeout, &Cancel::default())
    }

    #[cfg(unix)]
    fn sh(script: &str) -> Vec<String> {
        vec!["sh".into(), "-c".into(), script.into()]
    }

    #[cfg(unix)]
    #[test]
    fn a_command_is_given_the_text_and_what_it_prints_comes_back() {
        let answer = run(&sh("cat"), "the prompt", Duration::from_secs(5)).unwrap();
        assert_eq!(answer, "the prompt");
        // A command that does not read its input is not an error.
        let answer = run(
            &sh("echo ok"),
            &"x".repeat(1_000_000),
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(answer.trim(), "ok");
    }

    #[cfg(unix)]
    #[test]
    fn a_command_that_fails_is_told_with_what_it_said_last() {
        let error = run(
            &sh("echo first >&2; echo it went wrong >&2; exit 3"),
            "",
            Duration::from_secs(5),
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "`sh` failed (exit 3): it went wrong");
    }

    #[test]
    fn the_log_is_told_which_failure_it_was_and_not_what_the_command_printed() {
        let failed = AgentError::Failed {
            program: "agent".into(),
            code: Some(3),
            stderr: "the secret title".into(),
        };
        assert_eq!(failed.for_log(), "failed (exit 3) stderr_bytes=16");
        let unusable = AgentError::Unusable {
            program: "agent".into(),
            reason: "invalid type: string \"the secret comment\"".into(),
        };
        assert!(!unusable.for_log().contains("secret"));
    }

    #[cfg(unix)]
    #[test]
    fn a_command_that_takes_too_long_is_stopped() {
        let started = Instant::now();
        let error = run(&sh("sleep 5"), "", Duration::from_millis(60)).unwrap_err();
        assert!(matches!(error, AgentError::TimedOut { .. }), "{error}");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_command_that_is_missing_or_empty_is_said_so() {
        let error = run(
            &["no-such-agent-program".into()],
            "",
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("could not start `no-such-agent-program`"),
            "{error}"
        );
        assert!(matches!(
            run(&[], "", Duration::from_secs(1)),
            Err(AgentError::NoCommand)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn an_answer_that_is_not_text_is_refused() {
        let error = run(&sh("printf '\\377\\376'"), "", Duration::from_secs(5)).unwrap_err();
        assert!(matches!(error, AgentError::NotText { .. }), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn a_process_the_command_left_behind_with_its_output_open_is_not_waited_for() {
        let started = Instant::now();
        let answer = run(
            &sh("sleep 4 & echo the answer; echo a warning >&2"),
            "",
            Duration::from_secs(30),
        )
        .unwrap();
        assert_eq!(answer.trim(), "the answer");
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "the command had ended: what it printed is the answer"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_command_that_is_asked_to_stop_is_stopped_and_its_process_ended() {
        let cancel = Cancel::default();
        let asking = cancel.clone();
        let asker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(80));
            asking.cancel();
        });
        let started = Instant::now();
        let error = super::run(&sh("sleep 30"), "", Duration::from_secs(60), &cancel).unwrap_err();
        asker.join().unwrap();
        assert!(matches!(error, AgentError::Cancelled { .. }), "{error}");
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "stopped, not waited for"
        );
        // The process is gone when it returns: leaving would not wait for it.
        assert!(wait_until_stopped(Duration::from_secs(2)));
    }

    #[cfg(unix)]
    #[test]
    fn a_stop_asked_for_before_the_command_starts_stops_it_at_once() {
        let cancel = Cancel::default();
        cancel.cancel();
        let error = super::run(&sh("sleep 30"), "", Duration::from_secs(60), &cancel).unwrap_err();
        assert!(matches!(error, AgentError::Cancelled { .. }), "{error}");
    }
}
