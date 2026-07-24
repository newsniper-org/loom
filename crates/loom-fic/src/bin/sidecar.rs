//! docs/05's Sidecar HTTP embedding path — "가벼운 러스트 HTTP 라이브러리,
//! 프레임워크 아님" taken literally: `std::net::TcpListener` only, no
//! external HTTP crate. This is deliberately the *lowest-effort, most
//! language-agnostic* path (docs/05: "통합 난이도 최저, 성능 최저") — any
//! backend that can issue an HTTP request can use it, at the cost of a
//! network hop `loom_fic_render()` itself doesn't have.
//!
//! Serves `GET /render` -> the same `render_html()` every other embedding
//! path calls, as `text/html`. No other routes, no request body parsing —
//! docs/05's `request_context` is empty for this component (see lib.rs's
//! doc comment), so there is nothing to parse yet.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

fn handle(stream: TcpStream) {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut stream = stream;

    if request_line.starts_with("GET /render") {
        let html = loom_fic::render_html();
        let body = html.into_bytes();
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(&body);
    } else {
        let body = b"not found";
        let header = format!(
            "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(body);
    }
}

fn main() {
    let addr = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:8790".to_string());
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| panic!("loom-fic-sidecar: failed to bind {addr}: {e}"));
    eprintln!("loom-fic-sidecar listening on http://{addr}/render");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => handle(stream),
            Err(e) => eprintln!("loom-fic-sidecar: connection error: {e}"),
        }
    }
}
