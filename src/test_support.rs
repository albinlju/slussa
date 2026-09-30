//! Test doubles for the two transports tuipr talks through: the `gh` CLI
//! (`FakeGh`) and Bitbucket's HTTP API (`MockHttp`). Tests built on them drive
//! the real provider, fetcher and store code without a network.
//!
//! `FakeGh` replaces the `gh` program process-wide, so installing one takes a
//! lock and tests that use it run one at a time until the guard is dropped.
//! The fake runs as a script under `/bin/sh`, never as a freshly written
//! executable, which avoids "text file busy" races with tests that spawn
//! processes at the same time.

use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

/// A unique temporary directory, removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "tuipr-{prefix}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// gh
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum GhOverride {
    Script(PathBuf),
    Missing,
}

static GH_OVERRIDE: Mutex<Option<GhOverride>> = Mutex::new(None);
static GH_LOCK: Mutex<()> = Mutex::new(());

/// The command the provider should run instead of `gh`, while a fake is
/// installed.
pub fn gh_command() -> Option<Command> {
    let installed = GH_OVERRIDE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()?;
    Some(match installed {
        GhOverride::Script(path) => {
            let mut command = Command::new("/bin/sh");
            command.arg(path);
            command
        }
        GhOverride::Missing => Command::new("/nonexistent/tuipr-test-gh"),
    })
}

const GH_SCRIPT: &str = r#"dir=$(dirname "$0")
printf '%s\n' "$*" >> "$dir/calls.log"
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

/// While alive, provider calls to `gh` go to the fake. Dropping it restores
/// the real program and lets the next gh-using test run.
pub struct InstalledGh {
    dir: Option<TempDir>,
    _lock: MutexGuard<'static, ()>,
}

impl InstalledGh {
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

// ---------------------------------------------------------------------------
// GitHub payloads
// ---------------------------------------------------------------------------

pub fn gh_pr(number: u64, created: &str) -> Value {
    json!({
        "id": format!("PR_{number}"),
        "url": format!("https://github.com/o/r/pull/{number}"),
        "title": format!("PR number {number}"),
        "number": number,
        "author": {"login": "alice"},
        "state": "OPEN",
        "isDraft": false,
        "headRefName": "feature",
        "baseRefName": "main",
        "createdAt": created,
        "updatedAt": created,
        "additions": 3,
        "deletions": 1,
        "changedFiles": 2,
        "comments": {"totalCount": 4},
        "reviewThreads": {"totalCount": 2},
        "latestReviews": {
            "nodes": [{"state": "APPROVED", "author": {"login": "bob"}}],
            "pageInfo": {"hasNextPage": false}
        },
        "commits": {"nodes": [{"commit": {"statusCheckRollup": {"state": "SUCCESS"}}}]}
    })
}

/// A PR as `gh_pr` builds it, in the given GitHub state (`MERGED` or `CLOSED`).
pub fn gh_closed_pr(number: u64, created: &str, state: &str) -> Value {
    let mut pr = gh_pr(number, created);
    pr["state"] = json!(state);
    pr
}

/// One page of `repository { connection { nodes pageInfo } }`.
pub fn gh_list_page(nodes: &[Value], next: Option<&str>) -> String {
    json!({"data": {"repository": {"connection": {
        "nodes": nodes,
        "pageInfo": {"hasNextPage": next.is_some(), "endCursor": next}
    }}}})
    .to_string()
}

// ---------------------------------------------------------------------------
// HTTP
// ---------------------------------------------------------------------------

/// A canned response for one exact method and request target (path and query).
#[derive(Clone)]
pub struct Route {
    method: &'static str,
    target: String,
    status: u16,
    body: String,
    /// Answer at most this many requests, then fall through to later routes.
    limit: Option<usize>,
    used: usize,
}

impl Route {
    pub fn get(target: &str, status: u16, body: &str) -> Self {
        Self::new("GET", target, status, body)
    }

    pub fn post(target: &str, status: u16, body: &str) -> Self {
        Self::new("POST", target, status, body)
    }

    pub fn put(target: &str, status: u16, body: &str) -> Self {
        Self::new("PUT", target, status, body)
    }

    /// Answer only the first `count` matching requests.
    pub fn times(mut self, count: usize) -> Self {
        self.limit = Some(count);
        self
    }

    fn new(method: &'static str, target: &str, status: u16, body: &str) -> Self {
        Self {
            method,
            target: target.to_owned(),
            status,
            body: body.to_owned(),
            limit: None,
            used: 0,
        }
    }
}

/// One request the mock received. Header names are lower-cased.
#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

/// A loopback HTTP server that answers from a route table and records what it
/// was asked. Unmatched requests get a 404. One request per connection.
pub struct MockHttp {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl MockHttp {
    pub fn start(routes: Vec<Route>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let handle = {
            let requests = Arc::clone(&requests);
            let stop = Arc::clone(&stop);
            let mut routes = routes;
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Ok(stream) = stream {
                        let _ = respond(stream, &mut routes, &requests);
                    }
                }
            })
        };
        Self {
            addr,
            requests,
            stop,
            handle: Some(handle),
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// `host:port`, as the provider reports it in an authentication error.
    pub fn host(&self) -> String {
        self.addr.to_string()
    }

    pub fn requests(&self) -> Vec<Request> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for MockHttp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop so it can see the flag.
        let _ = TcpStream::connect(self.addr);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn respond(
    mut stream: TcpStream,
    routes: &mut [Route],
    requests: &Mutex<Vec<Request>>,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();
    let mut headers = HashMap::new();
    loop {
        line.clear();
        reader.read_line(&mut line)?;
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
        }
    }
    let length: usize = headers
        .get("content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let body = String::from_utf8_lossy(&body).into_owned();

    let matched = routes.iter().position(|route| {
        route.method == method
            && route.target == target
            && route.limit.is_none_or(|limit| route.used < limit)
    });
    if let Some(index) = matched {
        routes[index].used += 1;
    }
    let route = matched.map(|index| &routes[index]);
    requests
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(Request {
            method,
            target,
            headers,
            body,
        });

    let (status, reply) = route.map_or((404, "no route"), |route| {
        (route.status, route.body.as_str())
    });
    let response = format!(
        "HTTP/1.1 {status} Mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}
