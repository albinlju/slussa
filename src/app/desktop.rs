//! Desktop integration uses argument arrays and stdin, never interpolated shell commands.
use super::{
    App,
    action::{LinkAction, TaskResult},
    store::{LoadState, Notice},
};
use std::{
    io::{self, Write},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

impl App {
    pub(super) fn pr_link(&mut self, pr_id: u64, kind: LinkAction) {
        if self.state.store.link_pending {
            return;
        }
        let url = match &self.state.store.cache.prs {
            LoadState::Loaded(prs) => prs
                .iter()
                .find(|pr| pr.id == pr_id)
                .and_then(|pr| pr.url.clone()),
            _ => None,
        };
        let Some(url) = url else {
            return;
        };
        if let Err(message) = validate_url(&url) {
            self.state.store.notice = Some(Notice::new(message, true));
            return;
        }
        self.state.store.link_pending = true;
        self.state.store.notice = Some(Notice::new(
            format!(
                "{} PR #{pr_id}…",
                if kind == LinkAction::Open {
                    "Opening"
                } else {
                    "Copying link for"
                }
            ),
            false,
        ));
        let tx = self.results_tx.clone();
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || perform(kind, &url)).await;
            let result = match result {
                Ok(Ok(done)) => Ok(format!("PR #{pr_id}: {}", done.message())),
                Ok(Err(error)) => Err(format!("PR #{pr_id}: {error}")),
                Err(_) => Err(format!("PR #{pr_id}: desktop operation failed")),
            };
            let _ = tx.send(TaskResult::LinkFinished(result));
        });
    }
}

fn validate_url(value: &str) -> Result<(), String> {
    let invalid = || "This PR has no valid HTTP(S) link.".to_string();
    if value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(invalid());
    }
    let url = reqwest::Url::parse(value).map_err(|e| {
        tracing::debug!("rejected a PR link: {e}");
        invalid()
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid());
    }
    Ok(())
}

/// What a desktop action achieved. The terminal never confirms an OSC 52
/// write, so that outcome is reported as "sent", not "copied".
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Done {
    Opened,
    Copied,
    SentToTerminal,
}

impl Done {
    const fn message(self) -> &'static str {
        match self {
            Self::Opened => "opened in browser",
            Self::Copied => "link copied",
            Self::SentToTerminal => "link sent to terminal clipboard",
        }
    }
}

fn perform(kind: LinkAction, url: &str) -> Result<Done, String> {
    validate_url(url)?;
    let result = match kind {
        LinkAction::Open => open_browser(url).map(|()| Done::Opened),
        LinkAction::Copy => copy_link(url),
    };
    result.map_err(|error| {
        format!(
            "{}: {error}",
            if kind == LinkAction::Open {
                "Could not open browser"
            } else {
                "Could not copy link"
            }
        )
    })
}

fn run(command: &mut Command, input: &[u8], timeout: Duration) -> io::Result<()> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let written = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("Missing helper input"))
        .and_then(|mut stdin| stdin.write_all(input));
    if let Err(error) = written {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return Err(io::Error::other(format!(
                    "desktop helper exited with {status}"
                )));
            }
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(10)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(result.err().unwrap_or_else(|| {
                    io::Error::new(io::ErrorKind::TimedOut, "desktop helper timed out")
                }));
            }
        }
    }
}
const TIMEOUT: Duration = Duration::from_secs(5);

/// Copy with the platform helper, falling back to the terminal's clipboard
/// (OSC 52). Over SSH the helper would fill the *remote* machine's clipboard,
/// so the terminal goes first there.
fn copy_link(url: &str) -> io::Result<Done> {
    let remote = is_remote_session(
        std::env::var_os("SSH_CONNECTION").as_deref(),
        std::env::var_os("SSH_TTY").as_deref(),
    );
    if remote && write_osc52(url).is_ok() {
        return Ok(Done::SentToTerminal);
    }
    match copy_with_helper(url) {
        Ok(()) => Ok(Done::Copied),
        Err(error) => write_osc52(url)
            .map(|()| Done::SentToTerminal)
            .map_err(|osc52| {
                tracing::debug!("the terminal clipboard failed too: {osc52}");
                error
            }),
    }
}

fn is_remote_session(
    ssh_connection: Option<&std::ffi::OsStr>,
    ssh_tty: Option<&std::ffi::OsStr>,
) -> bool {
    [ssh_connection, ssh_tty]
        .into_iter()
        .flatten()
        .any(|value| !value.is_empty())
}

/// `ESC ] 52 ; c ; <base64> BEL` asks the terminal to set its clipboard. It
/// works through SSH and mosh, and through tmux with `set-clipboard on`.
fn osc52_sequence(text: &str) -> String {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    format!("\x1b]52;c;{encoded}\x07")
}

/// Written to the controlling terminal rather than stdout, because ratatui
/// owns stdout. Fire and forget: terminals that disable OSC 52 ignore it.
#[cfg(unix)]
fn write_osc52(text: &str) -> io::Result<()> {
    let mut tty = std::fs::OpenOptions::new().write(true).open("/dev/tty")?;
    tty.write_all(osc52_sequence(text).as_bytes())?;
    tty.flush()
}

#[cfg(not(unix))]
fn write_osc52(_text: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "no controlling terminal device",
    ))
}

#[cfg(target_os = "macos")]
fn open_browser(url: &str) -> io::Result<()> {
    run(Command::new("open").arg(url), &[], TIMEOUT)
}
#[cfg(target_os = "macos")]
fn copy_with_helper(url: &str) -> io::Result<()> {
    run(&mut Command::new("pbcopy"), url.as_bytes(), TIMEOUT)
}

#[cfg(target_os = "windows")]
fn open_browser(url: &str) -> io::Result<()> {
    run(
        Command::new("rundll32.exe").args(["url.dll,FileProtocolHandler", url]),
        &[],
        TIMEOUT,
    )
}
#[cfg(target_os = "windows")]
fn copy_with_helper(url: &str) -> io::Result<()> {
    run(
        Command::new("powershell.exe").args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Console]::InputEncoding = [System.Text.UTF8Encoding]::new(); $text = [Console]::In.ReadToEnd(); Set-Clipboard -Value $text",
        ]),
        url.as_bytes(),
        TIMEOUT,
    )
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn open_browser(url: &str) -> io::Result<()> {
    run(Command::new("xdg-open").arg(url), &[], TIMEOUT)
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn copy_with_helper(url: &str) -> io::Result<()> {
    let helpers: &[(&str, &[&str])] = &[
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
    ];
    for (name, args) in helpers {
        if run(Command::new(name).args(*args), url.as_bytes(), TIMEOUT).is_ok() {
            return Ok(());
        }
    }
    Err(io::Error::other(
        "No clipboard helper succeeded. A graphical session with wl-copy, xclip or xsel is required.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_hosted_and_self_hosted_web_links_but_rejects_other_handlers() {
        for url in [
            "https://github.com/team/repo/pull/42",
            "https://code.example.test/bitbucket/projects/TEAM/repos/repo/pull-requests/42",
            "http://intranet:7990/projects/X/repos/r/pull-requests/1",
        ] {
            assert!(validate_url(url).is_ok());
        }
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "--help",
            "https://user:secret@host/pr/1",
            "https://host/pr/1\n",
            "",
        ] {
            assert!(validate_url(url).is_err(), "{url}");
        }
    }
    #[test]
    fn osc52_sequence_wraps_base64_payload() {
        assert_eq!(osc52_sequence("hello"), "\x1b]52;c;aGVsbG8=\x07");
        assert_eq!(osc52_sequence(""), "\x1b]52;c;\x07");
    }
    #[test]
    fn ssh_environment_selects_the_terminal_clipboard() {
        use std::ffi::OsStr;
        assert!(!is_remote_session(None, None));
        assert!(!is_remote_session(Some(OsStr::new("")), None));
        assert!(is_remote_session(
            Some(OsStr::new("1.2.3.4 22 5.6.7.8 22")),
            None
        ));
        assert!(is_remote_session(None, Some(OsStr::new("/dev/pts/3"))));
    }
    #[test]
    fn outcomes_are_reported_honestly() {
        assert_eq!(Done::Copied.message(), "link copied");
        assert_eq!(
            Done::SentToTerminal.message(),
            "link sent to terminal clipboard"
        );
    }
    #[cfg(unix)]
    #[test]
    fn helper_receives_literal_input_and_failures_and_timeouts_are_reported() {
        let path = std::env::temp_dir().join(format!(
            "slussa-link-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        let value = "https://example.com/pr/42?q=$(echo%20oops)&x='quoted'";
        run(
            Command::new("/bin/sh")
                .args(["-c", "cat > \"$1\"", "helper"])
                .arg(&path),
            value.as_bytes(),
            TIMEOUT,
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), value);
        std::fs::remove_file(path).unwrap();
        assert!(run(Command::new("/bin/sh").args(["-c", "exit 7"]), &[], TIMEOUT).is_err());
        assert_eq!(
            run(
                Command::new("/bin/sleep").arg("1"),
                &[],
                Duration::from_millis(20)
            )
            .unwrap_err()
            .kind(),
            io::ErrorKind::TimedOut
        );
    }
}
