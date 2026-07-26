#[allow(dead_code)]
#[path = "../src/drawing.rs"]
mod drawing;
#[path = "../src/export.rs"]
mod export;

#[allow(dead_code)]
#[path = "../src/mobile_server.rs"]
mod mobile_server;
#[allow(dead_code)]
#[path = "../src/pages.rs"]
mod pages;

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

fn snapshot_body(schema_version: u8, page: &str) -> String {
    format!(
        r##"{{
        "schemaVersion": {schema_version},
        {page}
        "canvas": {{ "width": 32, "height": 24, "background": "#ffffff" }},
        "strokes": [
            {{
                "id": "mobile-1",
                "color": "#111827",
                "width": 4,
                "points": [
                    {{ "x": 2, "y": 3, "pressure": 0.5, "t": 10 }},
                    {{ "x": 20, "y": 12, "pressure": 0.5, "t": 11 }}
                ]
            }}
        ]
    }}"##
    )
}

fn save_snapshot(server: &MobileServer, body: &str) -> String {
    let save_path = format!("{}save", path_from_url(server.url()));
    http_post(server.url(), &save_path, body)
}

#[test]
fn schema_version_one_snapshots_still_save_and_gain_a_legacy_page() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(directory.path()).unwrap();

    let response = save_snapshot(&server, &snapshot_body(1, ""));

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(directory.path().join("latest.json").exists());
    assert!(directory
        .path()
        .join("pages")
        .join("legacy")
        .join("page.json")
        .exists());
}

#[test]
fn schema_version_two_snapshots_are_stored_under_their_page_and_mirrored_to_latest() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(directory.path()).unwrap();

    let response = save_snapshot(
        &server,
        &snapshot_body(2, r#""page": { "id": "note-1", "title": "Server sketch" },"#),
    );

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    let page_dir = directory.path().join("pages").join("note-1");
    assert!(page_dir.join("page.json").exists());
    assert!(page_dir.join("page.svg").exists());
    assert!(page_dir.join("page.png").exists());

    let page_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(page_dir.join("page.json")).unwrap()).unwrap();
    let latest_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(directory.path().join("latest.json")).unwrap(),
    )
    .unwrap();

    assert_eq!(page_json["page"]["id"], "note-1");
    assert_eq!(page_json["page"]["title"], "Server sketch");
    assert_eq!(latest_json["strokes"], page_json["strokes"]);
    assert_eq!(latest_json["updatedAt"], page_json["updatedAt"]);
    assert_eq!(latest_json["files"]["svg"], "drawings/latest.svg");
    assert_eq!(
        page_json["files"]["svg"],
        "drawings/pages/note-1/page.svg"
    );
}

#[test]
fn page_ids_that_could_escape_the_drawings_directory_are_refused() {
    let parent = tempfile::tempdir().unwrap();
    let drawings_dir = parent.path().join("drawings");
    std::fs::create_dir_all(&drawings_dir).unwrap();
    let before: Vec<_> = std::fs::read_dir(parent.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .collect();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(&drawings_dir).unwrap();

    for page_id in ["../escape", "a/b", "", "/etc/passwd", &"x".repeat(65)] {
        let body = snapshot_body(2, &format!(r#""page": {{ "id": "{page_id}" }},"#));

        let response = save_snapshot(&server, &body);

        assert!(
            response.starts_with("HTTP/1.1 400 Bad Request"),
            "page id {page_id:?} was not refused"
        );
        assert!(
            response.contains("not usable as a folder name"),
            "page id {page_id:?} was refused without saying why"
        );
    }

    let after: Vec<_> = std::fs::read_dir(parent.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .collect();
    assert_eq!(before, after, "a refused page id created something");
    assert!(!drawings_dir.join("latest.json").exists());
}

#[test]
fn schema_version_two_without_a_page_is_refused_rather_than_filed_as_legacy() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(directory.path()).unwrap();

    let response = save_snapshot(&server, &snapshot_body(2, ""));

    assert!(response.starts_with("HTTP/1.1 400 Bad Request"));
    assert!(response.contains("schemaVersion 2 must carry a page"));
    assert!(!directory.path().join("latest.json").exists());
}

#[test]
fn unknown_schema_versions_are_refused_with_a_reason_the_companion_can_read() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(directory.path()).unwrap();

    let response = save_snapshot(&server, &snapshot_body(3, ""));

    assert!(response.starts_with("HTTP/1.1 400 Bad Request"));
    assert!(response.contains("unsupported schemaVersion 3"));
    assert!(response.contains("understands 1 and 2"));
    assert!(!directory.path().join("latest.json").exists());
}

#[test]
fn capabilities_endpoint_tells_a_companion_which_schema_versions_this_mac_takes() {
    let server = MobileServer::start_loopback_for_test().unwrap();
    let base_path = path_from_url(server.url());

    let response = http_request(server.url(), "GET", &format!("{base_path}capabilities"));

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("application/json"));
    assert!(response.contains("\"schemaVersions\":[1,2]"));
    assert!(response.contains("pages"));

    let unknown = http_request(server.url(), "GET", &format!("{base_path}capability"));
    assert!(unknown.starts_with("HTTP/1.1 404 Not Found"));
}

#[test]
fn saving_two_pages_indexes_both_and_points_latest_at_the_newer_one() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(directory.path()).unwrap();

    save_snapshot(
        &server,
        &snapshot_body(2, r#""page": { "id": "page-a", "title": "A" },"#),
    );
    save_snapshot(
        &server,
        &snapshot_body(2, r#""page": { "id": "page-b", "title": "B" },"#),
    );

    let index: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(directory.path().join("pages").join("index.json")).unwrap(),
    )
    .unwrap();
    let pages = index["pages"].as_array().unwrap();
    let ids: Vec<&str> = pages
        .iter()
        .map(|page| page["pageId"].as_str().unwrap())
        .collect();

    assert_eq!(pages.len(), 2);
    assert!(ids.contains(&"page-a"));
    assert!(ids.contains(&"page-b"));
    assert_eq!(pages[0]["strokeCount"], 1);

    let latest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(directory.path().join("latest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(latest["page"]["id"], "page-b");
}

fn post_json(server: &MobileServer, route: &str, body: &str) -> String {
    let path = format!("{}{route}", path_from_url(server.url()));
    http_post(server.url(), &path, body)
}

fn latest_page_id(directory: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(directory.join("latest.json")).ok()?;
    let latest: serde_json::Value = serde_json::from_str(&text).ok()?;
    latest["page"]["id"].as_str().map(str::to_owned)
}

#[test]
fn a_pinned_page_keeps_latest_even_when_another_page_is_drawn_on() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(directory.path()).unwrap();
    save_snapshot(
        &server,
        &snapshot_body(2, r#""page": { "id": "keeper", "title": "Keeper" },"#),
    );

    let pinned = post_json(&server, "pin", r#"{"pageId":"keeper"}"#);
    save_snapshot(
        &server,
        &snapshot_body(2, r#""page": { "id": "scribble", "title": "Scribble" },"#),
    );

    assert!(pinned.starts_with("HTTP/1.1 200 OK"));
    // The other page is still stored in full — pinning protects the agent's
    // view, it does not discard work.
    assert!(directory
        .path()
        .join("pages")
        .join("scribble")
        .join("page.json")
        .exists());
    assert_eq!(latest_page_id(directory.path()).as_deref(), Some("keeper"));
}

#[test]
fn clearing_the_pin_returns_latest_to_following_the_newest_page() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(directory.path()).unwrap();
    save_snapshot(&server, &snapshot_body(2, r#""page": { "id": "keeper" },"#));
    post_json(&server, "pin", r#"{"pageId":"keeper"}"#);

    post_json(&server, "pin", r#"{"pageId":null}"#);
    save_snapshot(&server, &snapshot_body(2, r#""page": { "id": "scribble" },"#));

    assert_eq!(latest_page_id(directory.path()).as_deref(), Some("scribble"));
}

#[test]
fn pinning_points_latest_at_that_page_immediately() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(directory.path()).unwrap();
    save_snapshot(&server, &snapshot_body(2, r#""page": { "id": "first" },"#));
    save_snapshot(&server, &snapshot_body(2, r#""page": { "id": "second" },"#));

    post_json(&server, "pin", r#"{"pageId":"first"}"#);

    assert_eq!(latest_page_id(directory.path()).as_deref(), Some("first"));
    let index: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(directory.path().join("pages").join("index.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(index["pinnedPageId"], "first");
}

#[test]
fn promote_sends_one_page_without_moving_the_pin() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(directory.path()).unwrap();
    save_snapshot(&server, &snapshot_body(2, r#""page": { "id": "pinned-one" },"#));
    save_snapshot(&server, &snapshot_body(2, r#""page": { "id": "other" },"#));
    post_json(&server, "pin", r#"{"pageId":"pinned-one"}"#);

    let response = post_json(&server, "promote", r#"{"pageId":"other"}"#);

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert_eq!(latest_page_id(directory.path()).as_deref(), Some("other"));

    // The pin is untouched, so an unpinned page still cannot take latest.* — the
    // sent page stays until something allowed to write it does.
    save_snapshot(&server, &snapshot_body(2, r#""page": { "id": "third" },"#));
    assert_eq!(latest_page_id(directory.path()).as_deref(), Some("other"));

    // Drawing on the pinned page is allowed, and that is what ends the override.
    save_snapshot(&server, &snapshot_body(2, r#""page": { "id": "pinned-one" },"#));
    assert_eq!(
        latest_page_id(directory.path()).as_deref(),
        Some("pinned-one")
    );
}

#[test]
fn pin_and_promote_refuse_page_ids_that_could_escape_the_directory() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(directory.path()).unwrap();

    for route in ["pin", "promote"] {
        let response = post_json(&server, route, r#"{"pageId":"../escape"}"#);

        assert!(
            response.starts_with("HTTP/1.1 400 Bad Request"),
            "{route} accepted a traversal id"
        );
        assert!(response.contains("not usable as a folder name"));
    }

    let missing = post_json(&server, "promote", r#"{"pageId":"never-drawn"}"#);
    assert!(missing.starts_with("HTTP/1.1 400 Bad Request"));
    assert!(!directory.path().join("latest.json").exists());
}

#[test]
fn capabilities_advertise_pin_and_promote_so_an_older_mac_is_distinguishable() {
    let server = MobileServer::start_loopback_for_test().unwrap();
    let base_path = path_from_url(server.url());

    let response = http_request(server.url(), "GET", &format!("{base_path}capabilities"));

    assert!(response.contains("\"pin\""));
    assert!(response.contains("\"promote\""));
}

/// Stamping a sheet the Mac has never received used to fail with a 400, because
/// pinning tried to promote a page that was not on disk. A pin is a declaration
/// about which page `latest.*` follows, so it has to survive naming a page that has
/// not arrived yet — and take effect the moment it does.
#[test]
fn a_sheet_can_be_pinned_before_the_mac_has_ever_received_it() {
    let directory = tempfile::tempdir().unwrap();
    let server = MobileServer::start_loopback_with_drawings_dir_for_test(directory.path()).unwrap();
    save_snapshot(
        &server,
        &snapshot_body(2, r#""page": { "id": "already-here", "title": "Here" },"#),
    );

    let pinned = post_json(&server, "pin", r#"{"pageId":"not-yet-drawn"}"#);

    assert!(
        pinned.starts_with("HTTP/1.1 200 OK"),
        "pinning an unsent sheet was refused: {pinned}"
    );
    // Nothing to mirror yet, so the agent keeps reading what it was reading.
    assert_eq!(
        latest_page_id(directory.path()).as_deref(),
        Some("already-here")
    );

    // The pin also has to hold: another sheet arriving must not steal latest.*.
    save_snapshot(
        &server,
        &snapshot_body(2, r#""page": { "id": "already-here", "title": "Here" },"#),
    );
    assert_eq!(
        latest_page_id(directory.path()).as_deref(),
        Some("already-here"),
        "an unpinned sheet took latest.* while another sheet was pinned"
    );

    // And the moment the pinned sheet arrives, it becomes what the agent reads.
    save_snapshot(
        &server,
        &snapshot_body(2, r#""page": { "id": "not-yet-drawn", "title": "Arrived" },"#),
    );
    assert_eq!(
        latest_page_id(directory.path()).as_deref(),
        Some("not-yet-drawn")
    );
}
