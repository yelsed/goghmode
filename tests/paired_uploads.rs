//! End-to-end proof for the paired-device protocol: that pairing hands nothing
//! useful to a watcher, and that an upload nobody signed does not land.

#[allow(dead_code)]
#[path = "../src/crypto.rs"]
mod crypto;
#[path = "../src/drawing.rs"]
mod drawing;
#[allow(dead_code)]
#[path = "../src/export.rs"]
mod export;
#[allow(dead_code)]
#[path = "../src/host.rs"]
mod host;
#[allow(dead_code)]
#[path = "../src/mobile_server.rs"]
mod mobile_server;
#[allow(dead_code)]
#[path = "../src/pages.rs"]
mod pages;
#[allow(dead_code)]
#[path = "../src/protocol.rs"]
mod protocol;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crypto::sha256_hex;
use host::{Host, PairingState, SharedHost};
use mobile_server::MobileServer;
use protocol::{
    derive_device_secret, pair_request_mac, response_mac, upload_mac, HEADER_DEVICE, HEADER_MAC,
    HEADER_NONCE, HEADER_PAIR_MAC, HEADER_TIMESTAMP,
};

const DEVICE_ID: &str = "ipad-of-desley";
const DEVICE_NAME: &str = "iPad of Desley";

struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl Response {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header_name, _)| header_name == name)
            .map(|(_, value)| value.as_str())
    }
}

fn request(
    url: &str,
    method: &str,
    path: &str,
    headers: &[(&str, String)],
    body: &str,
) -> Response {
    let (_, rest) = url.split_once("://").unwrap();
    let authority = rest.split('/').next().unwrap();
    let mut stream = TcpStream::connect(authority).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    write!(stream, "{head}{body}").unwrap();

    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    parse_response(&raw)
}

fn parse_response(raw: &str) -> Response {
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw, ""));
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();

    Response {
        status,
        headers,
        body: body.to_owned(),
    }
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn snapshot_json(page_id: &str) -> String {
    format!(
        r##"{{"schemaVersion":2,"page":{{"id":"{page_id}"}},"canvas":{{"width":40,"height":40,"background":"#ffffff"}},"strokes":[{{"id":"s1","color":"#111827","width":4,"points":[{{"x":1,"y":1,"pressure":0.5,"t":1}}]}}]}}"##
    )
}

/// Drives the whole handshake from the client side, approving on the host
/// thread the way a person would tap the sheet.
fn pair(server_url: &str, host: &Host, device_id: &str) -> String {
    let payload = host.arm_pairing(vec![server_url.to_owned()]).unwrap();
    let host_id = payload.host_id.clone();
    let pairing_secret = payload.pairing_secret.clone();

    let body = serde_json::json!({
        "hostId": host_id,
        "deviceId": device_id,
        "deviceName": DEVICE_NAME,
        "platform": "ipados",
    })
    .to_string();
    let mac = pair_request_mac(&pairing_secret, &host_id, device_id, DEVICE_NAME);

    let url = server_url.to_owned();
    let caller = thread::spawn(move || {
        request(
            &url,
            "POST",
            "/v2/pair",
            &[(HEADER_PAIR_MAC, mac)],
            &body,
        )
    });

    approve_when_asked(host, true);
    let response = caller.join().unwrap();
    assert_eq!(response.status, 200, "pairing should have been approved");

    // The device derives its own key. Nothing secret crossed the network.
    derive_device_secret(&pairing_secret, device_id)
}

fn approve_when_asked(host: &Host, approved: bool) {
    for _ in 0..200 {
        if matches!(host.pairing_state(), PairingState::Pending { .. }) {
            host.decide_pending_pairing(approved);
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("no approval was ever requested");
}

fn signed_upload_headers(
    secret: &str,
    device_id: &str,
    host_id: &str,
    timestamp: u128,
    nonce: &str,
    body: &str,
) -> Vec<(&'static str, String)> {
    let mac = upload_mac(
        secret,
        device_id,
        timestamp,
        nonce,
        host_id,
        &sha256_hex(body.as_bytes()),
    );
    vec![
        (HEADER_DEVICE, device_id.to_owned()),
        (HEADER_TIMESTAMP, timestamp.to_string()),
        (HEADER_NONCE, nonce.to_owned()),
        (HEADER_MAC, mac),
    ]
}

fn upload(
    url: &str,
    secret: &str,
    device_id: &str,
    host_id: &str,
    timestamp: u128,
    nonce: &str,
    body: &str,
) -> Response {
    let headers = signed_upload_headers(secret, device_id, host_id, timestamp, nonce, body);
    request(url, "POST", "/v2/save", &headers, body)
}

struct Fixture {
    _host_dir: tempfile::TempDir,
    drawings: tempfile::TempDir,
    host: Host,
    server: MobileServer,
}

fn fixture() -> Fixture {
    let host_dir = tempfile::tempdir().unwrap();
    let drawings = tempfile::tempdir().unwrap();
    let host = SharedHost::load(host_dir.path()).unwrap();
    let server =
        MobileServer::start_loopback_with_drawings_dir_for_test(drawings.path(), host.clone())
            .unwrap();

    Fixture {
        _host_dir: host_dir,
        drawings,
        host,
        server,
    }
}

#[test]
fn pairing_never_puts_the_device_secret_on_the_network() {
    let harness = fixture();
    let base = harness.server.base_url();

    let payload = harness.host.arm_pairing(vec![base.clone()]).unwrap();
    let host_id = payload.host_id.clone();
    let pairing_secret = payload.pairing_secret.clone();
    let expected_secret = derive_device_secret(&pairing_secret, DEVICE_ID);

    let body = serde_json::json!({
        "hostId": host_id,
        "deviceId": DEVICE_ID,
        "deviceName": DEVICE_NAME,
        "platform": "ipados",
    })
    .to_string();
    let mac = pair_request_mac(&pairing_secret, &host_id, DEVICE_ID, DEVICE_NAME);
    let url = base.clone();
    let caller =
        thread::spawn(move || request(&url, "POST", "/v2/pair", &[(HEADER_PAIR_MAC, mac)], &body));
    approve_when_asked(&harness.host, true);
    let response = caller.join().unwrap();

    assert_eq!(response.status, 200);
    // The whole point: neither the derived key nor the pairing secret is in the
    // reply, so recording the exchange gains an attacker nothing.
    assert!(!response.body.contains(&expected_secret));
    assert!(!response.body.contains(&pairing_secret));
    for (_, value) in &response.headers {
        assert!(!value.contains(&expected_secret));
        assert!(!value.contains(&pairing_secret));
    }

    // Host and companion arrived at the same key independently.
    assert_eq!(harness.host.device_secret(DEVICE_ID), Some(expected_secret));
}

#[test]
fn an_unsigned_pair_request_never_reaches_the_approval_sheet() {
    let harness = fixture();
    let base = harness.server.base_url();
    let payload = harness.host.arm_pairing(vec![base.clone()]).unwrap();

    let body = serde_json::json!({
        "hostId": payload.host_id,
        "deviceId": DEVICE_ID,
        "deviceName": DEVICE_NAME,
        "platform": "ipados",
    })
    .to_string();

    let unsigned = request(&base, "POST", "/v2/pair", &[], &body);
    let wrongly_signed = request(
        &base,
        "POST",
        "/v2/pair",
        &[(HEADER_PAIR_MAC, "0".repeat(64))],
        &body,
    );

    assert_eq!(unsigned.status, 403);
    assert_eq!(wrongly_signed.status, 403);
    assert_eq!(unsigned.body, wrongly_signed.body);
    // Nothing on the network can raise a prompt on someone's screen.
    assert!(matches!(
        harness.host.pairing_state(),
        PairingState::Armed { .. }
    ));
    assert!(harness.host.devices().is_empty());
}

#[test]
fn a_pairing_secret_cannot_be_used_twice() {
    let harness = fixture();
    let base = harness.server.base_url();
    let payload = harness.host.arm_pairing(vec![base.clone()]).unwrap();
    let host_id = payload.host_id.clone();
    let pairing_secret = payload.pairing_secret.clone();

    let make_body = |device_id: &str| {
        serde_json::json!({
            "hostId": host_id,
            "deviceId": device_id,
            "deviceName": DEVICE_NAME,
            "platform": "ipados",
        })
        .to_string()
    };

    let first_body = make_body(DEVICE_ID);
    let first_mac = pair_request_mac(&pairing_secret, &host_id, DEVICE_ID, DEVICE_NAME);
    let url = base.clone();
    let caller = thread::spawn(move || {
        request(
            &url,
            "POST",
            "/v2/pair",
            &[(HEADER_PAIR_MAC, first_mac)],
            &first_body,
        )
    });
    approve_when_asked(&harness.host, true);
    assert_eq!(caller.join().unwrap().status, 200);

    let second_mac = pair_request_mac(&pairing_secret, &host_id, "second-device", DEVICE_NAME);
    let replayed = request(
        &base,
        "POST",
        "/v2/pair",
        &[(HEADER_PAIR_MAC, second_mac)],
        &make_body("second-device"),
    );

    assert_eq!(replayed.status, 403);
    assert_eq!(harness.host.devices().len(), 1);
}

#[test]
fn a_denied_request_and_an_expired_secret_are_indistinguishable() {
    let harness = fixture();
    let base = harness.server.base_url();
    let payload = harness.host.arm_pairing(vec![base.clone()]).unwrap();
    let host_id = payload.host_id.clone();

    let body = serde_json::json!({
        "hostId": host_id,
        "deviceId": DEVICE_ID,
        "deviceName": DEVICE_NAME,
        "platform": "ipados",
    })
    .to_string();
    let mac = pair_request_mac(&payload.pairing_secret, &host_id, DEVICE_ID, DEVICE_NAME);
    let url = base.clone();
    let denied_body = body.clone();
    let caller = thread::spawn(move || {
        request(
            &url,
            "POST",
            "/v2/pair",
            &[(HEADER_PAIR_MAC, mac)],
            &denied_body,
        )
    });
    approve_when_asked(&harness.host, false);
    let denied = caller.join().unwrap();

    // Nothing is armed now, which is the same position an expired secret leaves.
    let expired = request(
        &base,
        "POST",
        "/v2/pair",
        &[(HEADER_PAIR_MAC, "0".repeat(64))],
        &body,
    );

    assert_eq!(denied.status, 403);
    assert_eq!(expired.status, 403);
    assert_eq!(denied.body, expired.body);
    assert!(harness.host.devices().is_empty());
}

#[test]
fn a_signed_upload_lands_and_the_host_proves_who_answered() {
    let harness = fixture();
    let base = harness.server.base_url();
    let secret = pair(&base, &harness.host, DEVICE_ID);
    let host_id = harness.host.host_id();
    let body = snapshot_json("page-one");

    let response = upload(
        &base,
        &secret,
        DEVICE_ID,
        &host_id,
        unix_millis(),
        "nonce-one",
        &body,
    );

    assert_eq!(response.status, 200);
    assert_eq!(
        response.header("x-goghmode-host-mac"),
        Some(response_mac(&secret, "nonce-one", 200).as_str()),
        "the companion must be able to tell it reached the host it paired with"
    );
    assert!(harness
        .drawings
        .path()
        .join("pages")
        .join("page-one")
        .join("page.json")
        .exists());
    assert!(harness.drawings.path().join("latest.json").exists());
}

#[test]
fn every_way_of_failing_authentication_answers_the_same() {
    let harness = fixture();
    let base = harness.server.base_url();
    let secret = pair(&base, &harness.host, DEVICE_ID);
    let host_id = harness.host.host_id();
    let body = snapshot_json("page-one");
    let now = unix_millis();

    // Signed correctly, then the drawing swapped underneath the signature.
    let stale_signature = signed_upload_headers(&secret, DEVICE_ID, &host_id, now, "n6", &body);

    let attempts = [
        ("no headers at all", request(&base, "POST", "/v2/save", &[], &body)),
        (
            "wrong secret",
            upload(&base, "not-the-secret", DEVICE_ID, &host_id, now, "n1", &body),
        ),
        (
            "unknown device",
            upload(&base, &secret, "never-paired", &host_id, now, "n2", &body),
        ),
        (
            "timestamp far in the past",
            upload(&base, &secret, DEVICE_ID, &host_id, now - 600_000, "n3", &body),
        ),
        (
            "timestamp far in the future",
            upload(&base, &secret, DEVICE_ID, &host_id, now + 600_000, "n4", &body),
        ),
        (
            "signature made for another host",
            upload(&base, &secret, DEVICE_ID, "some-other-host", now, "n5", &body),
        ),
        (
            "body altered after signing",
            request(&base, "POST", "/v2/save", &stale_signature, &snapshot_json("other")),
        ),
    ];

    for (description, response) in &attempts {
        assert_eq!(response.status, 401, "{description} should be refused");
        assert_eq!(
            response.body, attempts[0].1.body,
            "{description} must not be distinguishable from any other failure"
        );
    }

    assert!(!harness.drawings.path().join("latest.json").exists());
}

#[test]
fn a_captured_upload_cannot_be_replayed() {
    let harness = fixture();
    let base = harness.server.base_url();
    let secret = pair(&base, &harness.host, DEVICE_ID);
    let host_id = harness.host.host_id();
    let body = snapshot_json("page-one");
    let timestamp = unix_millis();

    let first = upload(&base, &secret, DEVICE_ID, &host_id, timestamp, "n1", &body);
    let replayed = upload(&base, &secret, DEVICE_ID, &host_id, timestamp, "n1", &body);

    assert_eq!(first.status, 200);
    assert_eq!(replayed.status, 401);
}

/// The reason replay protection is a persisted number rather than a set of
/// seen nonces: a set lives in memory and a restart empties it.
#[test]
fn a_captured_upload_cannot_be_replayed_into_a_restarted_host() {
    let host_dir = tempfile::tempdir().unwrap();
    let drawings = tempfile::tempdir().unwrap();
    let host = SharedHost::load(host_dir.path()).unwrap();
    let server =
        MobileServer::start_loopback_with_drawings_dir_for_test(drawings.path(), host.clone())
            .unwrap();
    let base = server.base_url();
    let secret = pair(&base, &host, DEVICE_ID);
    let host_id = host.host_id();
    let body = snapshot_json("page-one");
    let timestamp = unix_millis();

    assert_eq!(
        upload(&base, &secret, DEVICE_ID, &host_id, timestamp, "n1", &body).status,
        200
    );

    drop(server);
    drop(host);
    let restarted = SharedHost::load(host_dir.path()).unwrap();
    let server =
        MobileServer::start_loopback_with_drawings_dir_for_test(drawings.path(), restarted.clone())
            .unwrap();
    let base = server.base_url();

    assert_eq!(restarted.host_id(), host_id, "identity must survive a restart");
    assert_eq!(
        upload(&base, &secret, DEVICE_ID, &host_id, timestamp, "n1", &body).status,
        401
    );
}

#[test]
fn a_revoked_device_stops_being_able_to_upload() {
    let harness = fixture();
    let base = harness.server.base_url();
    let secret = pair(&base, &harness.host, DEVICE_ID);
    let host_id = harness.host.host_id();
    let body = snapshot_json("page-one");

    assert_eq!(
        upload(&base, &secret, DEVICE_ID, &host_id, unix_millis(), "n1", &body).status,
        200
    );
    harness.host.revoke(DEVICE_ID).unwrap();

    assert_eq!(
        upload(&base, &secret, DEVICE_ID, &host_id, unix_millis(), "n2", &body).status,
        401
    );
}

/// Hashing is cheap and parsing is not. An unauthenticated caller must not be
/// able to make the host interpret four megabytes of adversarial JSON.
#[test]
fn an_unauthenticated_body_is_never_parsed() {
    let harness = fixture();
    let base = harness.server.base_url();
    pair(&base, &harness.host, DEVICE_ID);

    let response = request(
        &base,
        "POST",
        "/v2/save",
        &[
            (HEADER_DEVICE, DEVICE_ID.to_owned()),
            (HEADER_TIMESTAMP, unix_millis().to_string()),
            (HEADER_NONCE, "n1".to_owned()),
            (HEADER_MAC, "0".repeat(64)),
        ],
        "{ this is not valid json at all",
    );

    // A parse failure would answer 400 with a reason. 401 proves the body was
    // never looked at.
    assert_eq!(response.status, 401);
}

#[test]
fn pairing_a_device_closes_the_anonymous_route() {
    let harness = fixture();
    let base = harness.server.base_url();
    let legacy_path = {
        let url = harness.server.url();
        let (_, rest) = url.split_once("://").unwrap();
        let slash = rest.find('/').unwrap();
        format!("{}save", &rest[slash..])
    };
    let body = snapshot_json("browser-page");

    let before = request(&base, "POST", &legacy_path, &[], &body);
    assert_eq!(before.status, 200, "the old URL works until a device pairs");

    pair(&base, &harness.host, DEVICE_ID);

    let after = request(&base, "POST", &legacy_path, &[], &body);
    assert_eq!(
        after.status, 403,
        "an anonymous door beside an authenticated one is not an improvement"
    );

    // Reversible, because the browser companion has no pairing step.
    harness.host.set_legacy_uploads_enabled(true).unwrap();
    let reopened = request(&base, "POST", &legacy_path, &[], &body);
    assert_eq!(reopened.status, 200);
}

#[test]
fn hello_tells_a_stranger_nothing_that_identifies_the_machine() {
    let harness = fixture();
    let base = harness.server.base_url();
    let secret = pair(&base, &harness.host, DEVICE_ID);
    let host_id = harness.host.host_id();

    let anonymous = request(&base, "GET", "/v2/hello", &[], "");
    assert_eq!(anonymous.status, 200);
    assert!(
        !anonymous.body.contains(&host_id),
        "a stable identifier handed to any scanner is a tracking value"
    );
    assert!(anonymous.body.contains("pairing-v2"));
    assert!(anonymous.body.contains("time"));

    let headers = signed_upload_headers(&secret, DEVICE_ID, &host_id, unix_millis(), "n1", "");
    let signed = request(&base, "GET", "/v2/hello", &headers, "");
    assert!(signed.body.contains(&host_id));
}

/// The merge with the drawing-set work brought two new writes on the legacy
/// prefix. `pin` names the sheet the agent reads, so leaving it anonymous while
/// `save` is closed would guard the drawing and leave the pointer to it open.
#[test]
fn pairing_closes_the_anonymous_stamp_routes_too() {
    let harness = fixture();
    let base = harness.server.base_url();
    let prefix = {
        let url = harness.server.url();
        let (_, rest) = url.split_once("://").unwrap();
        let slash = rest.find('/').unwrap();
        rest[slash..].to_owned()
    };
    let body = snapshot_json("browser-page");
    let stamp_body = r#"{"pageId":"browser-page"}"#;

    assert_eq!(
        request(&base, "POST", &format!("{prefix}save"), &[], &body).status,
        200
    );
    assert_eq!(
        request(&base, "POST", &format!("{prefix}pin"), &[], stamp_body).status,
        200,
        "the old stamp routes work until a device pairs"
    );

    pair(&base, &harness.host, DEVICE_ID);

    for route in ["pin", "promote"] {
        let response = request(&base, "POST", &format!("{prefix}{route}"), &[], stamp_body);
        assert_eq!(
            response.status, 403,
            "{route} must close with the rest of the anonymous door"
        );
    }
}

#[test]
fn a_paired_device_can_stamp_but_an_unsigned_caller_cannot() {
    let harness = fixture();
    let base = harness.server.base_url();
    let secret = pair(&base, &harness.host, DEVICE_ID);
    let host_id = harness.host.host_id();
    let body = snapshot_json("page-one");

    assert_eq!(
        upload(&base, &secret, DEVICE_ID, &host_id, unix_millis(), "n1", &body).status,
        200
    );

    let stamp_body = r#"{"pageId":"page-one"}"#;
    let signed = signed_upload_headers(
        &secret,
        DEVICE_ID,
        &host_id,
        unix_millis(),
        "n2",
        stamp_body,
    );
    let pinned = request(&base, "POST", "/v2/pin", &signed, stamp_body);
    assert_eq!(pinned.status, 200);
    assert_eq!(
        pinned.header("x-goghmode-host-mac"),
        Some(response_mac(&secret, "n2", 200).as_str()),
        "a stamp is as consequential as a drawing, so it is proved the same way"
    );

    let unsigned = request(&base, "POST", "/v2/pin", &[], stamp_body);
    assert_eq!(unsigned.status, 401);

    let promoted = signed_upload_headers(
        &secret,
        DEVICE_ID,
        &host_id,
        unix_millis(),
        "n3",
        stamp_body,
    );
    assert_eq!(
        request(&base, "POST", "/v2/promote", &promoted, stamp_body).status,
        200
    );
}

/// The wire must stay uninformative while the person at the host learns why a
/// device stopped landing. Before this, a companion that had drifted, moved, or
/// been revoked produced the same silent 401 with nothing recorded anywhere.
#[test]
fn a_refusal_says_nothing_on_the_wire_and_names_itself_on_the_host() {
    let harness = fixture();
    let base = harness.server.base_url();
    let secret = pair(&base, &harness.host, DEVICE_ID);
    let host_id = harness.host.host_id();
    let body = snapshot_json("page-one");

    let refused = upload(
        &base,
        &secret,
        DEVICE_ID,
        &host_id,
        unix_millis() - 600_000,
        "n1",
        &body,
    );

    assert_eq!(refused.status, 401);
    let refusal = harness
        .host
        .last_refusal()
        .expect("the host should have recorded why it refused");
    assert!(
        refusal.reason.contains("clock"),
        "the recorded reason should name the failing check, got: {}",
        refusal.reason
    );
    assert!(
        !refused.body.contains("clock"),
        "the wire answer must not say which check failed"
    );
    assert!(
        !refusal.reason.contains(&secret),
        "a reason must never carry key material"
    );

    let accepted = upload(&base, &secret, DEVICE_ID, &host_id, unix_millis(), "n2", &body);

    assert_eq!(accepted.status, 200);
    assert_eq!(
        harness.host.last_refusal(),
        None,
        "a device that gets through should clear the complaint"
    );
}
