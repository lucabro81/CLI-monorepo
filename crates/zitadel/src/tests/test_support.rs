#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Helpers shared by unit tests across modules.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::thread;

/// Serves exactly one HTTP request with `status` + `body`, and hands back the
/// raw request the client sent: request line, header lines, blank line, body.
pub fn one_shot_server(status: &str, body: &str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        let mut headers = String::new();
        let mut content_length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            headers.push_str(&line);
            if line == "\r\n" {
                break;
            }
            if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = v.trim().parse().unwrap();
            }
        }
        let mut body = vec![0; content_length];
        reader.read_exact(&mut body).unwrap();
        stream.write_all(response.as_bytes()).unwrap();
        format!("{request_line}{headers}{}", String::from_utf8(body).unwrap())
    });
    (url, handle)
}
