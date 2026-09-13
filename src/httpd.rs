//! A minimal HTTP/1.1 server: static files, JSON endpoints, and one
//! Server-Sent Events stream. No async runtime, no external HTTP crate; the
//! live loop is the Rust simulation, and a thread per connection is plenty for
//! a handful of viewers.

use anyhow::Result;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct Request {
    pub method: String,
    pub path: String,
    pub query: String,
}

impl Request {
    pub fn param(&self, key: &str) -> Option<String> {
        for pair in self.query.split('&') {
            let mut it = pair.splitn(2, '=');
            let k = it.next()?;
            if k == key {
                return Some(it.next().unwrap_or("").to_string());
            }
        }
        None
    }
}

pub fn parse_request(reader: &mut BufReader<TcpStream>) -> Result<Option<Request>> {
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let mut parts = line.trim_end().split(' ');
    let method = parts.next().unwrap_or("GET").to_string();
    let target = parts.next().unwrap_or("/").to_string();
    // Drain headers.
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 {
            break;
        }
        if h == "\r\n" || h == "\n" {
            break;
        }
    }
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target, String::new()),
    };
    Ok(Some(Request { method, path, query }))
}

pub fn write_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    ctype: &str,
    body: &[u8],
    extra: &[(&str, &str)],
) -> Result<()> {
    // Only default Cache-Control when the caller does not set one: emitting both
    // produced contradictory duplicate headers ("no-store" + "public, max-age=...").
    let caller_sets_cache = extra.iter().any(|(k, _)| k.eq_ignore_ascii_case("cache-control"));
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: keep-alive\r\n",
        body.len()
    );
    if !caller_sets_cache {
        head.push_str("Cache-Control: no-store\r\n");
    }
    for (k, v) in extra {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

pub fn json_response(stream: &mut TcpStream, body: &str) -> Result<()> {
    write_response(stream, 200, "OK", "application/json", body.as_bytes(), &[])
}

const TYPES: [(&str, &str); 9] = [
    (".html", "text/html; charset=utf-8"),
    (".js", "text/javascript; charset=utf-8"),
    (".mjs", "text/javascript; charset=utf-8"),
    (".css", "text/css; charset=utf-8"),
    (".json", "application/json"),
    (".png", "image/png"),
    (".stl", "model/stl"),
    (".f32", "application/octet-stream"),
    (".bin", "application/octet-stream"),
];

fn content_type(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    for (e, t) in TYPES {
        if e == ext {
            return t;
        }
    }
    "application/octet-stream"
}

/// Serve a file from `root`, refusing anything that escapes it.
pub fn serve_static(stream: &mut TcpStream, root: &Path, url_path: &str) -> Result<()> {
    let mut rel = url_path.trim_start_matches('/').to_string();
    if rel.is_empty() {
        rel = "index.html".to_string();
    }
    if rel.contains("..") {
        return write_response(stream, 403, "Forbidden", "text/plain", b"forbidden", &[]);
    }
    let full = root.join(&rel);
    let canon_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let canon = match full.canonicalize() {
        Ok(c) => c,
        Err(_) => {
            return write_response(stream, 404, "Not Found", "text/plain", b"not found", &[]);
        }
    };
    if !canon.starts_with(&canon_root) {
        return write_response(stream, 403, "Forbidden", "text/plain", b"forbidden", &[]);
    }
    match std::fs::read(&canon) {
        Ok(body) => {
            let ct = content_type(&canon);
            let cache = if rel.starts_with("assets/") {
                "public, max-age=3600"
            } else {
                "no-store"
            };
            write_response(stream, 200, "OK", ct, &body, &[("Cache-Control", cache)])
        }
        Err(_) => write_response(stream, 404, "Not Found", "text/plain", b"not found", &[]),
    }
}

/// Start an SSE response and return the stream ready for `event:`/`data:` writes.
pub fn open_sse(stream: &mut TcpStream) -> Result<()> {
    let head = "HTTP/1.1 200 OK\r\n\
                Content-Type: text/event-stream; charset=utf-8\r\n\
                Cache-Control: no-store\r\n\
                Connection: keep-alive\r\n\
                X-Accel-Buffering: no\r\n\
                \r\n";
    stream.write_all(head.as_bytes())?;
    stream.flush()?;
    Ok(())
}

pub fn sse_event(stream: &mut TcpStream, event: &str, data: &str) -> Result<()> {
    let mut buf = String::with_capacity(data.len() + 32);
    if !event.is_empty() {
        buf.push_str("event: ");
        buf.push_str(event);
        buf.push('\n');
    }
    for line in data.split('\n') {
        buf.push_str("data: ");
        buf.push_str(line);
        buf.push('\n');
    }
    buf.push('\n');
    stream.write_all(buf.as_bytes())?;
    stream.flush()?;
    Ok(())
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Run the accept loop. `handler` gets each parsed request and the stream.
pub fn serve<H>(listener: TcpListener, shutdown: Arc<AtomicBool>, handler: H) -> Result<()>
where
    H: Fn(&mut TcpStream, Request) -> Result<()> + Send + Sync + 'static,
{
    let handler = Arc::new(handler);
    for stream in listener.incoming() {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        let stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let handler = Arc::clone(&handler);
        std::thread::spawn(move || {
            let mut stream = stream;
            let _ = stream.set_nodelay(true);
            let mut reader = match stream.try_clone() {
                Ok(c) => BufReader::new(c),
                Err(_) => return,
            };
            loop {
                match parse_request(&mut reader) {
                    Ok(Some(req)) => {
                        if handler(&mut stream, req).is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        });
    }
    Ok(())
}

pub fn static_root() -> PathBuf {
    PathBuf::from("/opt/data/workspaces/skg/flybrain/flyverse/web")
}

pub fn not_found(stream: &mut TcpStream) -> Result<()> {
    write_response(stream, 404, "Not Found", "text/plain", b"not found", &[])
}
