//! Bitbucket's HTTP API, replaced for the tests: `MockHttp`.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

/// A canned response for one exact method and request target (path and query).
#[derive(Clone)]
pub struct Route {
    method: &'static str,
    target: String,
    status: u16,
    body: String,
    /// Where a redirect says to go.
    location: Option<String>,
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

    /// Answer with a `Location` header too, as a redirect does.
    pub fn redirecting_to(mut self, location: &str) -> Self {
        self.location = Some(location.to_owned());
        self
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
            location: None,
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

    let (status, reply, location) = route.map_or((404, "no route", None), |route| {
        (route.status, route.body.as_str(), route.location.as_deref())
    });
    let location = location.map_or_else(String::new, |to| format!("Location: {to}\r\n"));
    let response = format!(
        "HTTP/1.1 {status} Mock\r\nContent-Type: application/json\r\n{location}Content-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}
