//! The `gh` CLI, replaced for the tests: `FakeGh` and the lock that makes the
//! installed fake one at a time.

use super::temp_dir::TempDir;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::{Mutex, MutexGuard, PoisonError},
};

#[derive(Clone)]
enum GhOverride {
    Script(PathBuf),
    Missing,
}

static GH_OVERRIDE: Mutex<Option<GhOverride>> = Mutex::new(None);
static GH_LOCK: Mutex<()> = Mutex::new(());

/// The command the provider runs instead of `gh`. With no fake installed it
/// is a program that does not exist, so a test that forgot its fake fails with
/// "gh is missing" instead of calling the real `gh`.
pub fn gh_command() -> Command {
    let installed = GH_OVERRIDE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    match installed {
        Some(GhOverride::Script(path)) => {
            let mut command = Command::new("/bin/sh");
            command.arg(path);
            command
        }
        Some(GhOverride::Missing) | None => Command::new("/nonexistent/slussa-test-gh"),
    }
}

const GH_SCRIPT: &str = r#"dir=$(dirname "$0")
printf '%s\n' "$*" >> "$dir/calls.log"
printf '%s\n' "${GH_REPO-}" >> "$dir/repos.log"
cat >> "$dir/stdin.log"
i=0
while [ -f "$dir/n$i" ]; do
  if printf '%s' "$*" | grep -qF -f "$dir/n$i"; then
    used=$(cat "$dir/u$i" 2>/dev/null || echo 0)
    max=$(cat "$dir/m$i")
    if [ "$max" -eq 0 ] || [ "$used" -lt "$max" ]; then
      echo $((used + 1)) > "$dir/u$i"
      cat "$dir/o$i"
      cat "$dir/e$i" >&2
      exit "$(cat "$dir/c$i")"
    fi
  fi
  i=$((i + 1))
done
echo "fake gh: no rule for: $*" >&2
exit 97
"#;

struct Rule {
    needle: String,
    stdout: String,
    stderr: String,
    code: i32,
    /// How many calls the rule answers; 0 means unlimited.
    max: u32,
}

/// A scripted `gh`. Rules are tried in order; the first whose needle occurs in
/// the joined arguments (and that still has uses left) answers. A call no rule
/// matches exits with status 97.
#[derive(Default)]
pub struct FakeGh {
    rules: Vec<Rule>,
}

impl FakeGh {
    pub fn new() -> Self {
        Self::default()
    }

    fn rule(mut self, needle: &str, stdout: &str, stderr: &str, code: i32, max: u32) -> Self {
        self.rules.push(Rule {
            needle: needle.to_owned(),
            stdout: stdout.to_owned(),
            stderr: stderr.to_owned(),
            code,
            max,
        });
        self
    }

    /// Always answer matching calls with `stdout`.
    pub fn on(self, needle: &str, stdout: &str) -> Self {
        self.rule(needle, stdout, "", 0, 0)
    }

    /// Answer the first matching call with `stdout`, then fall through.
    pub fn once(self, needle: &str, stdout: &str) -> Self {
        self.rule(needle, stdout, "", 0, 1)
    }

    /// Always fail matching calls.
    pub fn fail(self, needle: &str, code: i32, stderr: &str) -> Self {
        self.rule(needle, "", stderr, code, 0)
    }

    /// Fail the first matching call, then fall through.
    pub fn fail_once(self, needle: &str, code: i32, stderr: &str) -> Self {
        self.rule(needle, "", stderr, code, 1)
    }

    /// Make this fake the process's `gh` until the guard is dropped.
    pub fn install(self) -> InstalledGh {
        let lock = GH_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let dir = TempDir::new("gh");
        for (i, rule) in self.rules.iter().enumerate() {
            let write = |prefix: &str, text: &str| {
                fs::write(dir.path().join(format!("{prefix}{i}")), text).expect("write rule");
            };
            write("n", &format!("{}\n", rule.needle));
            write("o", &rule.stdout);
            write("e", &rule.stderr);
            write("c", &rule.code.to_string());
            write("m", &rule.max.to_string());
        }
        let script = dir.path().join("gh");
        fs::write(&script, GH_SCRIPT).expect("write script");
        *GH_OVERRIDE.lock().unwrap_or_else(PoisonError::into_inner) =
            Some(GhOverride::Script(script));
        InstalledGh {
            dir: Some(dir),
            _lock: lock,
        }
    }
}

/// While alive, provider calls to `gh` go to the fake. Dropping it leaves no
/// `gh` at all and lets the next gh-using test run.
pub struct InstalledGh {
    dir: Option<TempDir>,
    _lock: MutexGuard<'static, ()>,
}

impl InstalledGh {
    /// Hold the `gh` slot with nothing installed: what a test that forgot its
    /// fake finds.
    pub fn none() -> Self {
        let lock = GH_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        *GH_OVERRIDE.lock().unwrap_or_else(PoisonError::into_inner) = None;
        Self {
            dir: None,
            _lock: lock,
        }
    }

    /// Make `gh` look uninstalled.
    pub fn missing() -> Self {
        let lock = GH_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        *GH_OVERRIDE.lock().unwrap_or_else(PoisonError::into_inner) = Some(GhOverride::Missing);
        Self {
            dir: None,
            _lock: lock,
        }
    }

    /// Every call so far, as the space-joined argument list.
    pub fn calls(&self) -> Vec<String> {
        self.read("calls.log").lines().map(str::to_owned).collect()
    }

    /// The repository each call was told to act on (`GH_REPO`), one per call, in
    /// the order of `calls`; empty where the call was not told.
    pub fn repos(&self) -> Vec<String> {
        self.read("repos.log").lines().map(str::to_owned).collect()
    }

    /// Everything the provider wrote to `gh`'s standard input.
    pub fn stdin(&self) -> String {
        self.read("stdin.log")
    }

    fn read(&self, name: &str) -> String {
        self.dir
            .as_ref()
            .and_then(|dir| fs::read_to_string(dir.path().join(name)).ok())
            .unwrap_or_default()
    }
}

impl Drop for InstalledGh {
    fn drop(&mut self) {
        *GH_OVERRIDE.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}
