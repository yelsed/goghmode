use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::drawing::DrawingSnapshot;
use crate::export::write_snapshot;

const INDEX_HTML: &[u8] = include_bytes!("../mobile/index.html");
const MANIFEST: &[u8] = include_bytes!("../mobile/manifest.webmanifest");
const SERVICE_WORKER: &[u8] = include_bytes!("../mobile/service-worker.js");
const ICON: &[u8] = include_bytes!("../mobile/icon.svg");
const MAX_SAVE_BODY_BYTES: usize = 4 * 1024 * 1024;

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
            8787,
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
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let Some(request) = read_http_request(stream) else {
        write_response(
            stream,
            400,
            "Bad Request",
            "text/plain; charset=utf-8",
            b"Bad Request",
        );
        return;
    };

    let path = request
        .raw_path
        .split('?')
        .next()
        .unwrap_or(&request.raw_path);
    if request.method == "POST" {
        if path == format!("{route_prefix}save") {
            handle_save_request(stream, drawings_dir, &request.body);
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

fn read_http_request(stream: &mut TcpStream) -> Option<HttpRequest> {
    let mut bytes = Vec::with_capacity(8192);
    let mut chunk = [0_u8; 8192];
    let header_end = loop {
        let bytes_read = stream.read(&mut chunk).ok()?;
        if bytes_read == 0 {
            return None;
        }
        bytes.extend_from_slice(&chunk[..bytes_read]);
        if bytes.len() > MAX_SAVE_BODY_BYTES {
            return None;
        }
        if let Some(header_end) = find_header_end(&bytes) {
            break header_end;
        }
    };

    let header_text = String::from_utf8_lossy(&bytes[..header_end]);
    let mut lines = header_text.lines();
    let request_line = lines.next()?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next()?.to_owned();
    let raw_path = request_parts.next()?.to_owned();
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > MAX_SAVE_BODY_BYTES {
        return None;
    }

    let body_start = header_end + 4;
    let expected_len = body_start.checked_add(content_length)?;
    while bytes.len() < expected_len {
        let bytes_read = stream.read(&mut chunk).ok()?;
        if bytes_read == 0 {
            return None;
        }
        bytes.extend_from_slice(&chunk[..bytes_read]);
        if bytes.len() > expected_len || bytes.len() > MAX_SAVE_BODY_BYTES + body_start {
            return None;
        }
    }

    Some(HttpRequest {
        method,
        raw_path,
        body: bytes[body_start..expected_len].to_vec(),
    })
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn handle_save_request(stream: &mut TcpStream, drawings_dir: &Path, body: &[u8]) {
    let Ok(snapshot) = serde_json::from_slice::<DrawingSnapshot>(body) else {
        write_response(
            stream,
            400,
            "Bad Request",
            "text/plain; charset=utf-8",
            b"Bad Request",
        );
        return;
    };
    if !is_valid_snapshot(&snapshot) {
        write_response(
            stream,
            400,
            "Bad Request",
            "text/plain; charset=utf-8",
            b"Bad Request",
        );
        return;
    }

    match write_snapshot(&snapshot, drawings_dir) {
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

fn is_valid_snapshot(snapshot: &DrawingSnapshot) -> bool {
    if snapshot.schema_version != 1
        || !valid_canvas_extent(snapshot.canvas.width)
        || !valid_canvas_extent(snapshot.canvas.height)
        || snapshot.canvas.background.len() > 64
        || snapshot.strokes.len() > 4096
    {
        return false;
    }

    let mut point_count = 0_usize;
    for stroke in &snapshot.strokes {
        if stroke.id.len() > 128
            || stroke.color.len() > 64
            || !stroke.width.is_finite()
            || !(0.5..=80.0).contains(&stroke.width)
        {
            return false;
        }

        point_count = point_count.saturating_add(stroke.points.len());
        if point_count > 200_000 {
            return false;
        }

        for point in &stroke.points {
            if !point.x.is_finite()
                || !point.y.is_finite()
                || !point.pressure.is_finite()
                || point.x < 0.0
                || point.y < 0.0
                || point.x > snapshot.canvas.width
                || point.y > snapshot.canvas.height
            {
                return false;
            }
        }
    }

    true
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
