//! Desktop integration uses argument arrays and stdin, never interpolated shell commands.
use super::{
    App,
    effect::{LinkAction, LinkTarget, TaskResult},
    store::Notice,
};
use crate::domain::pr::PrId;
use std::{
    io::{self, Write},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

impl App {
    pub(super) fn pr_link(&mut self, pr_id: PrId, kind: LinkAction) {
        let prs = self.state.store.cache.prs.loaded();
        let pr = prs.and_then(|prs| prs.iter().find(|pr| pr.id == pr_id));
        let Some(url) = pr.and_then(|pr| pr.url.clone()) else {
            return;
        };
        self.start_link(LinkTarget::Pr(pr_id), kind, &url);
    }

    /// Open or copy `url`, one link at a time, off the UI thread.
    pub(super) fn start_link(&mut self, target: LinkTarget, kind: LinkAction, url: &str) {
        if self.state.store.link_pending {
            return;
        }
        let url = match WebUrl::parse(url) {
            Ok(url) => url,
            Err(error) => {
                self.state.store.notice = Some(Notice::error(format!("{target}: {error}")));
                return;
            }
        };
        self.state.store.link_pending = true;
        self.state.store.notice = Some(Notice::info(format!(
            "{} {target}…",
            if kind == LinkAction::Open {
                "Opening"
            } else {
                "Copying link for"
            }
        )));
        self.spawn_fetch(
            move || perform(kind, &url),
            move |returned| TaskResult::LinkFinished {
                target,
                result: returned.unwrap_or_else(|panic| {
                    tracing::error!("desktop worker panicked: {panic}");
                    Err(LinkError::WorkerPanicked)
                }),
            },
        );
    }
}

/// A link that is safe to hand to a browser or a clipboard helper: HTTP(S),
/// with a host, without credentials or control characters. `parse` is the
/// only way to make one, so the helpers below take nothing else.
#[derive(Debug)]
struct WebUrl(String);

/// Why a link action did nothing. The text is what the notice says.
#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("There is no valid HTTP(S) link.")]
    InvalidUrl,
    #[error("Could not open browser: {0}")]
    Open(io::Error),
    #[error("Could not copy link: {0}")]
    Copy(io::Error),
    #[error("desktop operation failed")]
    WorkerPanicked,
}

impl WebUrl {
    fn parse(value: &str) -> Result<Self, LinkError> {
        if value.len() > 4096 || value.chars().any(char::is_control) {
            return Err(LinkError::InvalidUrl);
        }
        let url = reqwest::Url::parse(value).map_err(|e| {
            tracing::debug!("rejected a PR link: {e}");
            LinkError::InvalidUrl
        })?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(LinkError::InvalidUrl);
        }
        Ok(Self(value.to_owned()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// What a desktop action achieved. The terminal never confirms an OSC 52
/// write, so that outcome is reported as "sent", not "copied".
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LinkDone {
    Opened,
    Copied,
    SentToTerminal,
}

impl LinkDone {
    pub const fn message(self) -> &'static str {
        match self {
            Self::Opened => "opened in browser",
            Self::Copied => "link copied",
            Self::SentToTerminal => "link sent to terminal clipboard",
        }
    }
}

fn perform(kind: LinkAction, url: &WebUrl) -> Result<LinkDone, LinkError> {
    let url = url.as_str();
    match kind {
        LinkAction::Open => open_browser(url)
            .map(|()| LinkDone::Opened)
            .map_err(LinkError::Open),
        LinkAction::Copy => copy_link(url).map_err(LinkError::Copy),
    }
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
fn copy_link(url: &str) -> io::Result<LinkDone> {
    let remote = is_remote_session(
        std::env::var_os("SSH_CONNECTION").as_deref(),
        std::env::var_os("SSH_TTY").as_deref(),
    );
    if remote && write_osc52(url).is_ok() {
        return Ok(LinkDone::SentToTerminal);
    }
    match copy_with_helper(url) {
        Ok(()) => Ok(LinkDone::Copied),
        Err(error) => write_osc52(url)
            .map(|()| LinkDone::SentToTerminal)
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
            assert_eq!(WebUrl::parse(url).unwrap().as_str(), url);
        }
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "--help",
            "https://user:secret@host/pr/1",
            "https://host/pr/1\n",
            "",
        ] {
            assert!(WebUrl::parse(url).is_err(), "{url}");
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
        assert_eq!(LinkDone::Copied.message(), "link copied");
        assert_eq!(
            LinkDone::SentToTerminal.message(),
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
