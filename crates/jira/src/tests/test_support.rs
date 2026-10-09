#![allow(clippy::unwrap_used, clippy::expect_used)]
//! A local HTTP server for client tests (copied from crates/zitadel).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

/// Serves `responses` in order, one per incoming request, and hands back the
/// raw requests received: request line, header lines, blank line, body.
pub fn mock_server(responses: &[(&str, &str)]) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let responses: Vec<String> = responses
        .iter()
        .map(|(status, body)| {
            format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
        })
        .collect();
    let handle = thread::spawn(move || {
        responses
            .into_iter()
            .map(|response| {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_request(&mut BufReader::new(stream.try_clone().unwrap()));
                stream.write_all(response.as_bytes()).unwrap();
                request
            })
            .collect()
    });
    (url, handle)
}

fn read_request(reader: &mut impl BufRead) -> String {
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
    format!("{request_line}{headers}{}", String::from_utf8(body).unwrap())
}
