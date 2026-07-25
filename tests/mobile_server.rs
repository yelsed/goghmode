#[allow(dead_code)]
#[path = "../src/drawing.rs"]
mod drawing;
#[path = "../src/export.rs"]
mod export;

#[path = "../src/mobile_server.rs"]
mod mobile_server;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use mobile_server::MobileServer;

fn http_request(url: &str, method: &str, path: &str) -> String {
    let (_, rest) = url.split_once("://").unwrap();
    let authority = rest.split('/').next().unwrap();
    let mut stream = TcpStream::connect(authority).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}
fn http_post(url: &str, path: &str, body: &str) -> String {
    let (_, rest) = url.split_once("://").unwrap();
    let authority = rest.split('/').next().unwrap();
    let mut stream = TcpStream::connect(authority).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn path_from_url(url: &str) -> &str {
    let (_, rest) = url.split_once("://").unwrap();
    let slash = rest.find('/').unwrap();
    &rest[slash..]
}
#[test]
fn production_start_api_is_available_for_desktop_app() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start(directory.path()).unwrap();
    assert!(server.url().starts_with("http://"));
}

#[test]
fn local_mobile_server_serves_embedded_app_shell_under_secret_path() {
    let server = MobileServer::start_loopback_for_test().unwrap();
    let base_path = path_from_url(server.url());

    assert!(server.url().starts_with("http://127.0.0.1:"));
    assert!(base_path.ends_with('/'));
    assert!(base_path.len() > "/".len() + 16);

    let index = http_request(server.url(), "GET", base_path);
    assert!(index.starts_with("HTTP/1.1 200 OK"));
    assert!(index.contains("GoghMode Mobile"));
    assert!(index.contains("service-worker.js"));

    let manifest = http_request(
        server.url(),
        "GET",
        &format!("{base_path}manifest.webmanifest"),
    );
    assert!(manifest.starts_with("HTTP/1.1 200 OK"));
    assert!(manifest.contains("\"display\": \"standalone\""));
}

#[test]
fn local_mobile_server_redirects_secret_path_to_trailing_slash() {
    let server = MobileServer::start_loopback_for_test().unwrap();
    let base_path = path_from_url(server.url());
    let no_slash = base_path.trim_end_matches('/');

    let response = http_request(server.url(), "GET", no_slash);

    assert!(response.starts_with("HTTP/1.1 308 Permanent Redirect"));
    assert!(response.contains(&format!("Location: {base_path}")));
}

#[test]
fn local_mobile_server_rejects_unknown_paths_and_write_methods() {
    let server = MobileServer::start_loopback_for_test().unwrap();
    let base_path = path_from_url(server.url());

    let wrong_path = http_request(server.url(), "GET", "/");
    assert!(wrong_path.starts_with("HTTP/1.1 404 Not Found"));

    let missing_asset = http_request(server.url(), "GET", &format!("{base_path}missing.png"));
    assert!(missing_asset.starts_with("HTTP/1.1 404 Not Found"));

    let post = http_request(server.url(), "POST", base_path);
    assert!(post.starts_with("HTTP/1.1 405 Method Not Allowed"));
}

#[test]
fn local_mobile_server_accepts_snapshot_and_writes_latest_files() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(directory.path()).unwrap();
    let save_path = format!("{}save", path_from_url(server.url()));
    let body = r##"{
        "schemaVersion": 1,
        "canvas": { "width": 32, "height": 24, "background": "#ffffff" },
        "strokes": [
            {
                "id": "mobile-1",
                "color": "#111827",
                "width": 4,
                "points": [
                    { "x": 2, "y": 3, "pressure": 0.5, "t": 10 },
                    { "x": 20, "y": 12, "pressure": 0.5, "t": 11 }
                ]
            }
        ]
    }"##;

    let response = http_post(server.url(), &save_path, body);

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("\"ok\":true"));
    assert!(directory.path().join("latest.json").exists());
    assert!(directory.path().join("latest.svg").exists());
    assert!(directory.path().join("latest.png").exists());
    let saved_json = std::fs::read_to_string(directory.path().join("latest.json")).unwrap();
    assert!(saved_json.contains("\"id\": \"mobile-1\""));
}

fn large_snapshot_body(point_count: usize) -> String {
    let points = (0..point_count)
        .map(|index| {
            let x = (index % 1000) as f32 / 1000.0 * 31.0;
            let y = (index % 700) as f32 / 700.0 * 23.0;
            format!("{{\"x\":{x},\"y\":{y},\"pressure\":0.5,\"t\":{index}}}")
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{{\"schemaVersion\":1,\"canvas\":{{\"width\":32,\"height\":24,\"background\":\"#ffffff\"}},\
         \"strokes\":[{{\"id\":\"big-1\",\"color\":\"#111827\",\"width\":4,\"points\":[{points}]}}]}}"
    )
}

/// A real upload from the iPad arrives over several TCP segments, unlike every
/// other test here which writes headers and body in one burst.
fn http_post_in_chunks(url: &str, path: &str, body: &str) -> String {
    let (_, rest) = url.split_once("://").unwrap();
    let authority = rest.split('/').next().unwrap();
    let mut stream = TcpStream::connect(authority).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();

    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.flush().unwrap();

    for chunk in body.as_bytes().chunks(16 * 1024) {
        std::thread::sleep(Duration::from_millis(5));
        stream.write_all(chunk).unwrap();
        stream.flush().unwrap();
    }

    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn local_mobile_server_accepts_snapshot_split_across_packets() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(directory.path()).unwrap();
    let save_path = format!("{}save", path_from_url(server.url()));
    let body = large_snapshot_body(4000);
    assert!(body.len() > 64 * 1024, "body should span several reads");

    let response = http_post_in_chunks(server.url(), &save_path, &body);

    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "large multi-packet upload was rejected: {}",
        response.lines().next().unwrap_or("<no response>")
    );
    assert!(directory.path().join("latest.json").exists());
}

#[test]
fn local_mobile_server_rejects_invalid_snapshot_payloads() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(directory.path()).unwrap();
    let save_path = format!("{}save", path_from_url(server.url()));

    let response = http_post(server.url(), &save_path, "{\"schemaVersion\":1}");

    assert!(response.starts_with("HTTP/1.1 400 Bad Request"));
    assert!(!directory.path().join("latest.json").exists());
}
