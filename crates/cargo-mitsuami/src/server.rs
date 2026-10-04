//! A small HTTP server on the loopback interface, for the review page: it
//! serves the page and the images, and accepts or rejects changes.
//!
//! Every request needs the token from the page's URL, so other pages open
//! in the browser can't act on the baselines.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use crate::changes::Change;
use crate::report::{self, Mode};

/// A token no other page can guess: 128 bits from the OS's secure RNG.
fn token() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(format!("no secure random numbers: {e}")))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Compares in constant time, so response times say nothing about how
/// much of a guess was right.
fn same_token(given: &str, token: &str) -> bool {
    given.len() == token.len() && given.bytes().zip(token.bytes()).fold(0, |diff, (a, b)| diff | (a ^ b)) == 0
}

/// Serves the review of `changes` on `port` (0 for any free port) until
/// the page says it's done. `ready` gets the page's URL.
pub fn serve(changes: &[Change], port: u16, ready: impl FnOnce(&str)) -> io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let token = token()?;
    let url = format!("http://127.0.0.1:{}/?token={token}", listener.local_addr()?.port());
    ready(&url);
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        match handle(stream, changes, &token) {
            Ok(Flow::Done) => break,
            Ok(Flow::Continue) => {}
            Err(e) => eprintln!("cargo-mitsuami: {e}"),
        }
    }
    Ok(())
}

/// Requests have no body, so their line and headers fit in this.
const MAX_REQUEST: u64 = 16 * 1024;

enum Flow {
    Continue,
    Done,
}

fn handle(mut stream: TcpStream, changes: &[Change], token: &str) -> io::Result<Flow> {
    // Requests are handled one at a time, so a connection that sends
    // nothing, or never ends a line, mustn't hold up the review.
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    // Nor one that asks for a capture and never reads it.
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?.take(MAX_REQUEST));
    let mut request = String::new();
    reader.read_line(&mut request)?;
    // Skip the headers; no request has a body.
    let mut line = String::new();
    while reader.read_line(&mut line)? > 2 {
        line.clear();
    }
    let mut parts = request.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if !query.split('&').any(|pair| pair.strip_prefix("token=").is_some_and(|given| same_token(given, token))) {
        return respond(&mut stream, 403, "text/plain", b"forbidden").map(|_| Flow::Continue);
    }
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    let change = |index: &str| index.parse::<usize>().ok().and_then(|i| changes.get(i));
    match (method, segments.as_slice()) {
        ("GET", [""]) => {
            let page = report::page(changes, &Mode::Live { token });
            respond(&mut stream, 200, "text/html; charset=utf-8", page.as_bytes())?;
        }
        ("GET", ["image", index, kind]) => {
            let file = change(index).map(|c| match *kind {
                "baseline" => c.baseline.clone(),
                "diff" => c.diff(),
                _ => c.capture(),
            });
            match file.and_then(|f| std::fs::read(f).ok()) {
                Some(bytes) => respond(&mut stream, 200, "image/png", &bytes)?,
                None => respond(&mut stream, 404, "text/plain", b"no such image")?,
            }
        }
        ("POST", [action @ ("accept" | "reject"), index]) => {
            let Some(change) = change(index) else {
                return respond(&mut stream, 404, "text/plain", b"no such change").map(|_| Flow::Continue);
            };
            let result = if *action == "accept" { change.accept() } else { change.reject() };
            match result {
                Ok(()) => {
                    println!("{}ed {}", action.trim_end_matches('e'), change.display);
                    respond(&mut stream, 200, "text/plain", b"ok")?;
                }
                Err(e) => respond(&mut stream, 500, "text/plain", e.to_string().as_bytes())?,
            }
        }
        ("POST", ["done"]) => {
            respond(&mut stream, 200, "text/plain", b"ok")?;
            return Ok(Flow::Done);
        }
        _ => respond(&mut stream, 404, "text/plain", b"not found")?,
    }
    Ok(Flow::Continue)
}

fn respond(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "Internal Server Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}
