use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::crypto::sha256_hex;
use crate::drawing::DrawingSnapshot;
use crate::host::{unix_millis, Host, PairOutcome, PLATFORM};
use crate::pages::{page_id_is_safe, write_page};
use crate::protocol::{
    device_id_is_safe, response_mac, upload_mac_matches, HEADER_DEVICE, HEADER_MAC, HEADER_NONCE,
    HEADER_PAIR_MAC, HEADER_TIMESTAMP, PROTOCOL_VERSION, TIMESTAMP_TOLERANCE_MILLIS,
};

const INDEX_HTML: &[u8] = include_bytes!("../mobile/index.html");
const MANIFEST: &[u8] = include_bytes!("../mobile/manifest.webmanifest");
const SERVICE_WORKER: &[u8] = include_bytes!("../mobile/service-worker.js");
const ICON: &[u8] = include_bytes!("../mobile/icon.svg");
const MAX_SAVE_BODY_BYTES: usize = 4 * 1024 * 1024;

/// Version 1 predates pages and keeps working. Bumping the accepted version
/// rather than widening it would brick every installed companion build.
const SUPPORTED_SCHEMA_VERSIONS: [u8; 2] = [1, 2];

/// Lets a companion ask what this host understands instead of inferring it
/// from a rejection. An older host has no such route and answers 404, which is
/// a usable answer.
const CAPABILITIES: &[u8] =
    br#"{"schemaVersions":[1,2],"features":["pages","pin","promote","pairing-v2"]}"#;

pub const DEFAULT_PORT: u16 = 8787;

/// Everything a request needs to be answered. The paired-device routes live
/// outside the token prefix — a device authenticates by signature, so the path
/// secret has no part to play there.
#[derive(Clone, Copy)]
enum LegacyWrite {
    Save,
    Pin,
    Promote,
}

struct ServerContext {
    route_prefix: String,
    drawings_dir: PathBuf,
    host: Host,
}

pub struct MobileServer {
    url: String,
    local_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl MobileServer {
    pub fn start(drawings_dir: impl AsRef<Path>, host: Host) -> anyhow::Result<Self> {
        let display_ip = preferred_lan_ip();
        let token = load_or_create_token(&default_token_path()).unwrap_or_else(|_| random_token());
        let drawings_dir = drawings_dir.as_ref().to_path_buf();
        match Self::start_with_token(
            Ipv4Addr::UNSPECIFIED,
            DEFAULT_PORT,
            display_ip,
            token.clone(),
            drawings_dir.clone(),
            Arc::clone(&host),
        ) {
            Ok(server) => Ok(server),
            Err(_) => Self::start_with_token(
                Ipv4Addr::UNSPECIFIED,
                0,
                display_ip,
                token,
                drawings_dir,
                host,
            ),
        }
    }

    #[allow(dead_code)]
    #[cfg(test)]
    pub fn start_loopback_for_test(host: Host) -> anyhow::Result<Self> {
        Self::start_loopback_with_drawings_dir_for_test(std::env::temp_dir(), host)
    }

    #[allow(dead_code)]
    #[cfg(test)]
    pub fn start_loopback_with_drawings_dir_for_test(
        drawings_dir: impl AsRef<Path>,
        host: Host,
    ) -> anyhow::Result<Self> {
        Self::start_with_token(
            Ipv4Addr::LOCALHOST,
            0,
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            random_token(),
            drawings_dir.as_ref().to_path_buf(),
            host,
        )
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// Where a paired companion sends its uploads. Carries no secret, because
    /// the signature is the credential.
    pub fn base_url(&self) -> String {
        self.url
            .rsplit_once('/')
            .and_then(|(head, _)| head.rsplit_once('/'))
            .map(|(head, _)| head.to_owned())
            .unwrap_or_else(|| self.url.clone())
    }

    /// The port actually bound. `start` falls back to a random port when 8787
    /// is taken, which leaves any previously copied URL silently pointing at
    /// nothing — the desktop app surfaces this rather than hiding it.
    pub fn port(&self) -> u16 {
        self.local_addr.port()
    }

    fn start_with_token(
        bind_ip: Ipv4Addr,
        port: u16,
        display_ip: IpAddr,
        token: String,
        drawings_dir: PathBuf,
        host: Host,
    ) -> anyhow::Result<Self> {
        let listener = TcpListener::bind((bind_ip, port))?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;
        let route_prefix = format!("/{token}/");
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);
        let context = ServerContext {
            route_prefix: route_prefix.clone(),
            drawings_dir,
            host,
        };
        let thread = thread::spawn(move || serve(listener, context, thread_shutdown));
        let url = format!(
            "http://{}:{}{}",
            display_ip,
            local_addr.port(),
            route_prefix
        );

        Ok(Self {
            url,
            local_addr,
            shutdown,
            thread: Some(thread),
        })
    }
}

impl Drop for MobileServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.local_addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(listener: TcpListener, context: ServerContext, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, peer)) => handle_connection(&mut stream, &context, peer),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(25));
            }
            Err(_) => break,
        }
    }
}

fn handle_connection(stream: &mut TcpStream, context: &ServerContext, peer: SocketAddr) {
    // Accepted sockets inherit the listener's non-blocking flag on macOS, so
    // every read past the first returned WouldBlock and any upload spanning more
    // than one TCP segment was answered with 400.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let request = match read_http_request(stream) {
        Ok(request) => request,
        Err(reason) => {
            reject(stream, reason);
            return;
        }
    };

    let route_prefix = context.route_prefix.as_str();
    let drawings_dir = context.drawings_dir.as_path();
    let path = request
        .raw_path
        .split('?')
        .next()
        .unwrap_or(&request.raw_path)
        .to_owned();
    let path = path.as_str();

    if path.starts_with("/v2/") {
        handle_paired_route(stream, context, &request, path, peer);
        return;
    }

    if request.method == "POST" {
        let legacy_route = [
            (format!("{route_prefix}save"), LegacyWrite::Save),
            (format!("{route_prefix}pin"), LegacyWrite::Pin),
            (format!("{route_prefix}promote"), LegacyWrite::Promote),
        ]
        .into_iter()
        .find(|(candidate, _)| candidate == path)
        .map(|(_, route)| route);

        if let Some(route) = legacy_route {
            // Once a device has been paired, the anonymous door closes — for
            // every write, not only the drawing. `pin` names the sheet the agent
            // reads, so an unauthenticated pin is a way to choose what the agent
            // sees without ever sending a stroke.
            if !context.host.legacy_uploads_enabled() {
                write_response(
                    stream,
                    403,
                    "Forbidden",
                    "text/plain; charset=utf-8",
                    b"This host now requires a paired device. Re-enable the old mobile URL in GoghMode if you still need it.",
                );
                return;
            }
            match route {
                LegacyWrite::Save => handle_save_request(stream, drawings_dir, &request.body),
                LegacyWrite::Pin => handle_pin_request(stream, drawings_dir, &request.body),
                LegacyWrite::Promote => {
                    handle_promote_request(stream, drawings_dir, &request.body)
                }
            }
        } else {
            write_response(
                stream,
                405,
                "Method Not Allowed",
                "text/plain; charset=utf-8",
                b"Method Not Allowed",
            );
        }
        return;
    }

    if request.method != "GET" && request.method != "HEAD" {
        write_response(
            stream,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            b"Method Not Allowed",
        );
        return;
    }

    if path == route_prefix.trim_end_matches('/') {
        write_redirect_response(stream, route_prefix);
        return;
    }
    let Some((body, mime_type)) = asset_for_path(path, route_prefix) else {
        write_response(
            stream,
            404,
            "Not Found",
            "text/plain; charset=utf-8",
            b"Not Found",
        );
        return;
    };

    if request.method == "HEAD" {
        write_head_response(stream, 200, "OK", mime_type, body.len());
    } else {
        write_response(stream, 200, "OK", mime_type, body);
    }
}

#[derive(serde::Deserialize)]
struct PairRequestBody {
    #[serde(rename = "hostId")]
    host_id: String,
    #[serde(rename = "deviceId")]
    device_id: String,
    #[serde(rename = "deviceName")]
    device_name: String,
    platform: String,
}

struct AuthenticatedDevice {
    secret: String,
    nonce: String,
}

fn handle_paired_route(
    stream: &mut TcpStream,
    context: &ServerContext,
    request: &HttpRequest,
    path: &str,
    peer: SocketAddr,
) {
    match (request.method.as_str(), path) {
        ("GET", "/v2/hello") | ("HEAD", "/v2/hello") => handle_hello(stream, context, request),
        ("POST", "/v2/pair") => handle_pair(stream, context, request, peer),
        ("POST", "/v2/save") => handle_authenticated_save(stream, context, request),
        ("POST", "/v2/pin") => handle_authenticated_stamp(stream, context, request, Stamp::Pin),
        ("POST", "/v2/promote") => {
            handle_authenticated_stamp(stream, context, request, Stamp::Promote)
        }
        ("GET", _) | ("HEAD", _) | ("POST", _) => write_response(
            stream,
            404,
            "Not Found",
            "text/plain; charset=utf-8",
            b"Not Found",
        ),
        _ => write_response(
            stream,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            b"Method Not Allowed",
        ),
    }
}

/// An unauthenticated caller gets protocol facts only. A stable host identifier
/// handed to anything that can reach the port is a tracking value for a machine
/// that joins many networks, and nothing needs it before pairing — the identity
/// arrives in the pairing payload, and after that it is already saved.
///
/// `time` is public on purpose: a host with a wrong clock rejects every upload,
/// and because authentication failures are opaque by design that would
/// otherwise be undiagnosable.
fn handle_hello(stream: &mut TcpStream, context: &ServerContext, request: &HttpRequest) {
    let identity = context.host.identity();
    let mut body = serde_json::json!({
        "v": PROTOCOL_VERSION,
        "schemaVersions": SUPPORTED_SCHEMA_VERSIONS,
        "features": ["pages", "pin", "promote", "pairing-v2"],
        "time": unix_millis().to_string(),
    });

    if let Some(device) = authenticate(context, request) {
        body["hostId"] = identity.host_id.clone().into();
        body["name"] = identity.display_name.clone().into();
        body["platform"] = PLATFORM.into();
        let text = body.to_string();
        write_response_with_headers(
            stream,
            200,
            "OK",
            "application/json; charset=utf-8",
            text.as_bytes(),
            &host_proof(&device, 200),
        );
        return;
    }

    write_response(
        stream,
        200,
        "OK",
        "application/json; charset=utf-8",
        body.to_string().as_bytes(),
    );
}

fn handle_pair(
    stream: &mut TcpStream,
    context: &ServerContext,
    request: &HttpRequest,
    peer: SocketAddr,
) {
    let refuse = |stream: &mut TcpStream| {
        // Denied, expired, reused, unsigned, and wrong all answer the same, so a
        // caller cannot tell them apart.
        write_response(
            stream,
            403,
            "Forbidden",
            "text/plain; charset=utf-8",
            b"Pairing refused",
        );
    };

    let Ok(body) = serde_json::from_slice::<PairRequestBody>(&request.body) else {
        refuse(stream);
        return;
    };
    let Some(pair_mac) = request.header(HEADER_PAIR_MAC) else {
        refuse(stream);
        return;
    };
    if !device_id_is_safe(&body.device_id) || body.host_id != context.host.host_id() {
        refuse(stream);
        return;
    }

    match context.host.complete_pairing(
        &body.device_id,
        &body.device_name,
        &body.platform,
        &peer.ip().to_string(),
        pair_mac,
    ) {
        PairOutcome::Approved {
            host_id,
            pair_response_mac,
        } => {
            let identity = context.host.identity();
            let response = serde_json::json!({
                "v": PROTOCOL_VERSION,
                "hostId": host_id,
                "name": identity.display_name,
                "platform": PLATFORM,
            })
            .to_string();
            // The secret itself is never sent. Both sides derive it from the
            // pairing secret that travelled screen-to-camera.
            write_response_with_headers(
                stream,
                200,
                "OK",
                "application/json; charset=utf-8",
                response.as_bytes(),
                &[("X-GoghMode-Pair-Mac".to_owned(), pair_response_mac)],
            );
        }
        PairOutcome::Refused => refuse(stream),
    }
}

fn handle_authenticated_save(
    stream: &mut TcpStream,
    context: &ServerContext,
    request: &HttpRequest,
) {
    let Some(device) = authenticate(context, request) else {
        // One generic answer for every authentication failure, so nothing is
        // learned from which check rejected the request.
        write_response(
            stream,
            401,
            "Unauthorized",
            "text/plain; charset=utf-8",
            b"Unauthorized",
        );
        return;
    };

    // Only now is the body interpreted. Hashing is cheap and parsing is not, so
    // an unauthenticated caller must never reach `serde_json`.
    let snapshot = match serde_json::from_slice::<DrawingSnapshot>(&request.body) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            respond_to_device(
                stream,
                &device,
                400,
                "Bad Request",
                format!("could not parse the drawing: {error}"),
            );
            return;
        }
    };
    if let Err(reason) = validate_snapshot(&snapshot) {
        respond_to_device(stream, &device, 400, "Bad Request", reason);
        return;
    }

    match write_page(&snapshot, &context.drawings_dir) {
        Ok(_) => write_response_with_headers(
            stream,
            200,
            "OK",
            "application/json; charset=utf-8",
            br#"{"ok":true}"#,
            &host_proof(&device, 200),
        ),
        Err(_) => respond_to_device(
            stream,
            &device,
            500,
            "Internal Server Error",
            "could not write the drawing".to_owned(),
        ),
    }
}

#[derive(Clone, Copy)]
enum Stamp {
    Pin,
    Promote,
}

/// Choosing which sheet the agent reads is as consequential as sending one, so
/// it goes through exactly the same door: signed, replay-protected, and
/// answered with a proof of who answered.
fn handle_authenticated_stamp(
    stream: &mut TcpStream,
    context: &ServerContext,
    request: &HttpRequest,
    stamp: Stamp,
) {
    let Some(device) = authenticate(context, request) else {
        write_response(
            stream,
            401,
            "Unauthorized",
            "text/plain; charset=utf-8",
            b"Unauthorized",
        );
        return;
    };

    let page_id = match page_id_request(&request.body) {
        Ok(page_id) => page_id,
        Err(reason) => {
            respond_to_device(stream, &device, 400, "Bad Request", reason);
            return;
        }
    };

    let outcome = match (stamp, page_id) {
        (Stamp::Pin, page_id) => crate::pages::set_pin(&context.drawings_dir, page_id.as_deref()),
        (Stamp::Promote, Some(page_id)) => {
            crate::pages::promote_page(&context.drawings_dir, &page_id).map(|_| ())
        }
        (Stamp::Promote, None) => {
            respond_to_device(
                stream,
                &device,
                400,
                "Bad Request",
                "promote needs a pageId".to_owned(),
            );
            return;
        }
    };

    match outcome {
        Ok(()) => write_response_with_headers(
            stream,
            200,
            "OK",
            "application/json; charset=utf-8",
            br#"{"ok":true}"#,
            &host_proof(&device, 200),
        ),
        Err(error) => respond_to_device(
            stream,
            &device,
            400,
            "Bad Request",
            format!("could not stamp that sheet: {error}"),
        ),
    }
}

fn respond_to_device(
    stream: &mut TcpStream,
    device: &AuthenticatedDevice,
    status: u16,
    reason: &str,
    message: String,
) {
    eprintln!("goghmode: rejected upload: {message}");
    write_response_with_headers(
        stream,
        status,
        reason,
        "text/plain; charset=utf-8",
        message.as_bytes(),
        &host_proof(device, status),
    );
}

/// Proves to the companion that this answer came from the host it paired with,
/// bound to the nonce it just chose so it cannot be replayed from an earlier
/// exchange. Every answer to an authenticated request carries it, including the
/// failures — otherwise a rejection would be the one reply an impostor could
/// forge.
fn host_proof(device: &AuthenticatedDevice, status: u16) -> [(String, String); 1] {
    [(
        "X-GoghMode-Host-Mac".to_owned(),
        response_mac(&device.secret, &device.nonce, status),
    )]
}

/// The order is the security property. A device must be known, its clock close
/// enough, and its signature right, all before anything mutates state or the
/// body is interpreted. Recording the timestamp comes last because it is a
/// write, and an unauthenticated caller must never cause one.
fn authenticate(context: &ServerContext, request: &HttpRequest) -> Option<AuthenticatedDevice> {
    let device_id = request.header(HEADER_DEVICE)?;
    if !device_id_is_safe(device_id) {
        return None;
    }
    let timestamp: u128 = request.header(HEADER_TIMESTAMP)?.parse().ok()?;
    let nonce = request.header(HEADER_NONCE)?;
    let candidate = request.header(HEADER_MAC)?;
    let secret = context.host.device_secret(device_id)?;

    if unix_millis().abs_diff(timestamp) > TIMESTAMP_TOLERANCE_MILLIS {
        return None;
    }
    if !upload_mac_matches(
        &secret,
        device_id,
        timestamp,
        nonce,
        &context.host.host_id(),
        &sha256_hex(&request.body),
        candidate,
    ) {
        return None;
    }
    // Strictly increasing per device, persisted, so a captured request cannot be
    // replayed — not even into a freshly restarted host.
    if !context.host.accept_timestamp(device_id, timestamp) {
        return None;
    }

    Some(AuthenticatedDevice {
        secret,
        nonce: nonce.to_owned(),
    })
}

struct HttpRequest {
    method: String,
    raw_path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl HttpRequest {
    /// Header names are matched lowercased because a client may send any casing
    /// and two clients here are written in different languages.
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header_name, _)| header_name == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Reads until `want` more bytes are buffered, tolerating the interruptions a
/// real socket produces. Returns the reason a request could not be read so the
/// client is told what went wrong instead of a bare 400.
fn read_more(stream: &mut TcpStream, bytes: &mut Vec<u8>) -> Result<usize, &'static str> {
    let mut chunk = [0_u8; 8192];
    loop {
        return match stream.read(&mut chunk) {
            Ok(0) => Err("connection closed before the request finished"),
            Ok(bytes_read) => {
                bytes.extend_from_slice(&chunk[..bytes_read]);
                Ok(bytes_read)
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                Err("timed out waiting for the rest of the request")
            }
            Err(_) => Err("could not read the request"),
        };
    }
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, &'static str> {
    let mut bytes = Vec::with_capacity(8192);
    let header_end = loop {
        if let Some(header_end) = find_header_end(&bytes) {
            break header_end;
        }
        if bytes.len() > MAX_SAVE_BODY_BYTES {
            return Err("request headers too large");
        }
        read_more(stream, &mut bytes)?;
    };

    let header_text = String::from_utf8_lossy(&bytes[..header_end]);
    let mut lines = header_text.lines();
    let request_line = lines.next().ok_or("empty request")?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().ok_or("malformed request line")?.to_owned();
    let raw_path = request_parts.next().ok_or("malformed request line")?.to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let content_length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > MAX_SAVE_BODY_BYTES {
        return Err("drawing is too large to upload");
    }

    let body_start = header_end + 4;
    let expected_len = body_start
        .checked_add(content_length)
        .ok_or("declared body length is not usable")?;
    while bytes.len() < expected_len {
        read_more(stream, &mut bytes)?;
    }

    Ok(HttpRequest {
        method,
        raw_path,
        headers,
        // A client may send more than Content-Length; take only what was declared
        // rather than treating the extra as a malformed request.
        body: bytes[body_start..expected_len].to_vec(),
    })
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn handle_save_request(stream: &mut TcpStream, drawings_dir: &Path, body: &[u8]) {
    let snapshot = match serde_json::from_slice::<DrawingSnapshot>(body) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            reject_owned(stream, format!("could not parse the drawing: {error}"));
            return;
        }
    };
    if let Err(reason) = validate_snapshot(&snapshot) {
        reject_owned(stream, reason);
        return;
    }

    match write_page(&snapshot, drawings_dir) {
        Ok(_) => write_response(
            stream,
            200,
            "OK",
            "application/json; charset=utf-8",
            br#"{"ok":true}"#,
        ),
        Err(_) => write_response(
            stream,
            500,
            "Internal Server Error",
            "text/plain; charset=utf-8",
            b"Internal Server Error",
        ),
    }
}

#[derive(serde::Deserialize)]
struct PageIdRequest {
    #[serde(rename = "pageId")]
    page_id: Option<String>,
}

fn page_id_request(body: &[u8]) -> Result<Option<String>, String> {
    let request: PageIdRequest = serde_json::from_slice(body)
        .map_err(|error| format!("could not parse the request: {error}"))?;
    match request.page_id {
        Some(page_id) if !page_id_is_safe(&page_id) => Err(format!(
            "page id {page_id:?} is not usable as a folder name (letters, digits, - and _ only, up to 64)"
        )),
        page_id => Ok(page_id),
    }
}

/// Pins the page `latest.*` follows, or clears the pin with a null id. Without a
/// pin, `latest.*` keeps following whatever was written last.
fn handle_pin_request(stream: &mut TcpStream, drawings_dir: &Path, body: &[u8]) {
    let page_id = match page_id_request(body) {
        Ok(page_id) => page_id,
        Err(reason) => {
            reject_owned(stream, reason);
            return;
        }
    };

    match crate::pages::set_pin(drawings_dir, page_id.as_deref()) {
        Ok(()) => write_response(
            stream,
            200,
            "OK",
            "application/json; charset=utf-8",
            br#"{"ok":true}"#,
        ),
        Err(error) => reject_owned(stream, format!("could not pin that page: {error}")),
    }
}

/// Points `latest.*` at one stored page without moving the pin.
fn handle_promote_request(stream: &mut TcpStream, drawings_dir: &Path, body: &[u8]) {
    let page_id = match page_id_request(body) {
        Ok(Some(page_id)) => page_id,
        Ok(None) => {
            reject_owned(stream, "promote needs a pageId".to_owned());
            return;
        }
        Err(reason) => {
            reject_owned(stream, reason);
            return;
        }
    };

    match crate::pages::promote_page(drawings_dir, &page_id) {
        Ok(_) => write_response(
            stream,
            200,
            "OK",
            "application/json; charset=utf-8",
            br#"{"ok":true}"#,
        ),
        Err(error) => reject_owned(stream, format!("could not send that page: {error}")),
    }
}

/// Names the first thing wrong with a snapshot. A bare "invalid" is impossible to
/// act on when the drawing has thousands of points.
fn validate_snapshot(snapshot: &DrawingSnapshot) -> Result<(), String> {
    if !SUPPORTED_SCHEMA_VERSIONS.contains(&snapshot.schema_version) {
        return Err(format!(
            "unsupported schemaVersion {} (this host understands 1 and 2)",
            snapshot.schema_version
        ));
    }

    match snapshot.page.as_ref() {
        // The page id becomes a directory name. Reject before anything joins it
        // to a path, so a traversal attempt never reaches the filesystem.
        Some(page) if !page_id_is_safe(&page.id) => {
            return Err(format!(
                "page id {:?} is not usable as a folder name (letters, digits, - and _ only, up to 64)",
                page.id
            ))
        }
        // Version 2 is the version that promises a page; without one the write
        // would silently land in the legacy page instead.
        None if snapshot.schema_version >= 2 => {
            return Err("schemaVersion 2 must carry a page".to_owned())
        }
        _ => {}
    }

    if let Some(title) = snapshot.page.as_ref().and_then(|page| page.title.as_ref()) {
        if title.len() > 200 {
            return Err(format!("page title is too long: {} chars", title.len()));
        }
    }

    if !valid_canvas_extent(snapshot.canvas.width) || !valid_canvas_extent(snapshot.canvas.height) {
        return Err(format!(
            "canvas {}x{} is outside the supported 1-4096 range",
            snapshot.canvas.width, snapshot.canvas.height
        ));
    }
    if snapshot.canvas.background.len() > 64 {
        return Err("canvas background colour is too long".to_owned());
    }
    if snapshot.strokes.len() > 4096 {
        return Err(format!(
            "{} strokes exceeds the 4096 limit",
            snapshot.strokes.len()
        ));
    }

    let mut point_count = 0_usize;
    for stroke in &snapshot.strokes {
        if stroke.id.len() > 128 {
            return Err(format!("stroke id is too long: {} chars", stroke.id.len()));
        }
        if stroke.color.len() > 64 {
            return Err(format!("stroke {} has an over-long colour", stroke.id));
        }
        if !stroke.width.is_finite() || !(0.5..=80.0).contains(&stroke.width) {
            return Err(format!(
                "stroke {} width {} is outside 0.5-80",
                stroke.id, stroke.width
            ));
        }

        point_count = point_count.saturating_add(stroke.points.len());
        if point_count > 200_000 {
            return Err("drawing has more than 200000 points".to_owned());
        }

        for point in &stroke.points {
            if !point.x.is_finite() || !point.y.is_finite() || !point.pressure.is_finite() {
                return Err(format!("stroke {} contains a non-finite value", stroke.id));
            }
            if point.x < 0.0
                || point.y < 0.0
                || point.x > snapshot.canvas.width
                || point.y > snapshot.canvas.height
            {
                return Err(format!(
                    "stroke {} has a point at ({}, {}) outside the {}x{} canvas",
                    stroke.id, point.x, point.y, snapshot.canvas.width, snapshot.canvas.height
                ));
            }
        }
    }

    Ok(())
}

fn reject(stream: &mut TcpStream, reason: &str) {
    reject_owned(stream, reason.to_owned());
}

fn reject_owned(stream: &mut TcpStream, reason: String) {
    eprintln!("goghmode: rejected upload: {reason}");
    write_response(
        stream,
        400,
        "Bad Request",
        "text/plain; charset=utf-8",
        reason.as_bytes(),
    );
}

fn valid_canvas_extent(value: f32) -> bool {
    value.is_finite() && (1.0..=4096.0).contains(&value)
}

fn asset_for_path<'a>(path: &str, route_prefix: &str) -> Option<(&'a [u8], &'static str)> {
    if path == route_prefix {
        return Some((INDEX_HTML, "text/html; charset=utf-8"));
    }

    let asset = path.strip_prefix(route_prefix)?;
    match asset {
        "" | "index.html" => Some((INDEX_HTML, "text/html; charset=utf-8")),
        "capabilities" => Some((CAPABILITIES, "application/json; charset=utf-8")),
        "manifest.webmanifest" => Some((MANIFEST, "application/manifest+json; charset=utf-8")),
        "service-worker.js" => Some((SERVICE_WORKER, "text/javascript; charset=utf-8")),
        "icon.svg" => Some((ICON, "image/svg+xml; charset=utf-8")),
        _ => None,
    }
}

fn write_response(stream: &mut TcpStream, status: u16, reason: &str, mime_type: &str, body: &[u8]) {
    write_response_with_headers(stream, status, reason, mime_type, body, &[]);
}

fn write_response_with_headers(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    mime_type: &str,
    body: &[u8],
    extra_headers: &[(String, String)],
) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {mime_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in extra_headers {
        let _ = write!(stream, "{name}: {value}\r\n");
    }
    let _ = write!(stream, "\r\n");
    let _ = stream.write_all(body);
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

fn write_redirect_response(stream: &mut TcpStream, location: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 308 Permanent Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
    );
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

fn write_head_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    mime_type: &str,
    content_length: usize,
) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {mime_type}\r\nContent-Length: {content_length}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
    );
}

fn default_token_path() -> PathBuf {
    home::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".goghmode")
        .join("mobile-token")
}

fn load_or_create_token(path: &Path) -> anyhow::Result<String> {
    if let Ok(existing) = fs::read_to_string(path) {
        let token = existing.trim();
        if is_valid_token(token) {
            return Ok(token.to_owned());
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let token = random_token();
    fs::write(path, &token)?;
    Ok(token)
}

fn is_valid_token(token: &str) -> bool {
    token.len() >= 32 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn preferred_lan_ip() -> IpAddr {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|socket| {
            socket.connect((Ipv4Addr::new(8, 8, 8, 8), 80))?;
            socket.local_addr()
        })
        .map(|addr| addr.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

fn random_token() -> String {
    let mut bytes = [0_u8; 16];
    if File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .is_err()
    {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        bytes[..8].copy_from_slice(&(nanos as u64).to_le_bytes());
        bytes[8..].copy_from_slice(&u64::from(std::process::id()).to_le_bytes());
    }

    let mut token = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut token, "{byte:02x}");
    }
    token
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_token_is_reused_for_stable_phone_shortcuts() {
        let directory = tempfile::tempdir().unwrap();
        let token_path = directory.path().join("mobile-token");

        let first = load_or_create_token(&token_path).unwrap();
        let second = load_or_create_token(&token_path).unwrap();

        assert_eq!(first, second);
        assert!(first.len() >= 32);
    }
}
