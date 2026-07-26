use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::drawing::DrawingSnapshot;
use crate::pages::{page_id_is_safe, write_page};

const INDEX_HTML: &[u8] = include_bytes!("../mobile/index.html");
const MANIFEST: &[u8] = include_bytes!("../mobile/manifest.webmanifest");
const SERVICE_WORKER: &[u8] = include_bytes!("../mobile/service-worker.js");
const ICON: &[u8] = include_bytes!("../mobile/icon.svg");
const MAX_SAVE_BODY_BYTES: usize = 4 * 1024 * 1024;

/// Version 1 predates pages and keeps working. Bumping the accepted version
/// rather than widening it would brick every installed companion build.
const SUPPORTED_SCHEMA_VERSIONS: [u8; 2] = [1, 2];

/// Lets a companion ask what this Mac understands instead of inferring it from
/// a rejection. An older Mac has no such route and answers 404, which is itself
/// a usable answer.
const CAPABILITIES: &[u8] = br#"{"schemaVersions":[1,2],"features":["pages","pin","promote"]}"#;

pub const DEFAULT_PORT: u16 = 8787;

pub struct MobileServer {
    url: String,
    local_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl MobileServer {
    pub fn start(drawings_dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let display_ip = preferred_lan_ip();
        let token = load_or_create_token(&default_token_path()).unwrap_or_else(|_| random_token());
        let drawings_dir = drawings_dir.as_ref().to_path_buf();
        match Self::start_with_token(
            Ipv4Addr::UNSPECIFIED,
            DEFAULT_PORT,
            display_ip,
            token.clone(),
            drawings_dir.clone(),
        ) {
            Ok(server) => Ok(server),
            Err(_) => {
                Self::start_with_token(Ipv4Addr::UNSPECIFIED, 0, display_ip, token, drawings_dir)
            }
        }
    }

    #[allow(dead_code)]
    #[cfg(test)]
    pub fn start_loopback_for_test() -> anyhow::Result<Self> {
        Self::start_loopback_with_drawings_dir_for_test(std::env::temp_dir())
    }

    #[allow(dead_code)]
    #[cfg(test)]
    pub fn start_loopback_with_drawings_dir_for_test(
        drawings_dir: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        Self::start_with_token(
            Ipv4Addr::LOCALHOST,
            0,
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            random_token(),
            drawings_dir.as_ref().to_path_buf(),
        )
    }

    pub fn url(&self) -> &str {
        &self.url
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
    ) -> anyhow::Result<Self> {
        let listener = TcpListener::bind((bind_ip, port))?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;
        let route_prefix = format!("/{token}/");
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);
        let thread_prefix = route_prefix.clone();
        let thread_drawings_dir = drawings_dir.clone();
        let thread = thread::spawn(move || {
            serve(
                listener,
                thread_prefix,
                thread_drawings_dir,
                thread_shutdown,
            )
        });
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

fn serve(
    listener: TcpListener,
    route_prefix: String,
    drawings_dir: PathBuf,
    shutdown: Arc<AtomicBool>,
) {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => handle_connection(&mut stream, &route_prefix, &drawings_dir),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(25));
            }
            Err(_) => break,
        }
    }
}

fn handle_connection(stream: &mut TcpStream, route_prefix: &str, drawings_dir: &Path) {
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

    let path = request
        .raw_path
        .split('?')
        .next()
        .unwrap_or(&request.raw_path);
    if request.method == "POST" {
        if path == format!("{route_prefix}save") {
            handle_save_request(stream, drawings_dir, &request.body);
        } else if path == format!("{route_prefix}pin") {
            handle_pin_request(stream, drawings_dir, &request.body);
        } else if path == format!("{route_prefix}promote") {
            handle_promote_request(stream, drawings_dir, &request.body);
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

struct HttpRequest {
    method: String,
    raw_path: String,
    body: Vec<u8>,
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
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
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
            "unsupported schemaVersion {} (this Mac understands 1 and 2)",
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
    write_head_response(stream, status, reason, mime_type, body.len());
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
