//! The three primitives the pairing protocol needs, and nothing else.
//!
//! Everything here is HMAC-SHA256 or SHA-256 from RustCrypto. No cryptography
//! is invented, and no key material is compared with `==`.

use std::fs::File;
use std::io::Read;

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Frames the parts of a signed message so no two different field lists can
/// produce the same bytes. A plain separator is not enough: a device name is
/// arbitrary user text and could contain whatever separator was chosen, which
/// would let one message be re-read as another. Length prefixes cannot be
/// forged that way.
pub fn signing_string(parts: &[&str]) -> String {
    let mut framed = String::new();
    for part in parts {
        framed.push_str(&part.len().to_string());
        framed.push(':');
        framed.push_str(part);
    }
    framed
}

pub fn hmac_hex(key: &str, message: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key.as_bytes())
        .expect("HMAC accepts a key of any length");
    mac.update(message.as_bytes());
    to_hex(&mac.finalize().into_bytes())
}

/// Constant-time by construction: the comparison happens inside `verify_slice`,
/// not on the strings. A malformed or wrong-length candidate is simply false.
pub fn hmac_matches(key: &str, message: &str, candidate_hex: &str) -> bool {
    let Some(candidate) = from_hex(candidate_hex) else {
        return false;
    };
    let mut mac = HmacSha256::new_from_slice(key.as_bytes())
        .expect("HMAC accepts a key of any length");
    mac.update(message.as_bytes());
    mac.verify_slice(&candidate).is_ok()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    to_hex(&hasher.finalize())
}

/// Fails rather than falling back. `mobile_server`'s legacy token can afford a
/// time-and-process-identifier fallback because it is only a path; a key
/// derived from a guessable value would be worse than no key at all.
pub fn random_hex(byte_count: usize) -> anyhow::Result<String> {
    let mut bytes = vec![0_u8; byte_count];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| {
            anyhow::anyhow!("no secure random source available, refusing to invent one: {error}")
        })?;
    Ok(to_hex(&bytes))
}

pub fn is_hex(text: &str, byte_count: usize) -> bool {
    text.len() == byte_count * 2 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    let bytes = text.as_bytes();
    (0..bytes.len() / 2)
        .map(|index| {
            let pair = std::str::from_utf8(&bytes[index * 2..index * 2 + 2]).ok()?;
            u8::from_str_radix(pair, 16).ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_keeps_two_different_field_lists_apart() {
        // Without length prefixes these collide: "ab" + "c" reads the same as
        // "a" + "bc". A device name is user-supplied, so this is reachable.
        assert_ne!(
            signing_string(&["ab", "c"]),
            signing_string(&["a", "bc"])
        );
    }

    #[test]
    fn a_signature_verifies_only_under_its_own_key_and_message() {
        let message = signing_string(&["device-1", "1785000000000"]);
        let signature = hmac_hex("secret", &message);

        assert!(hmac_matches("secret", &message, &signature));
        assert!(!hmac_matches("other-secret", &message, &signature));
        assert!(!hmac_matches("secret", "different message", &signature));
    }

    #[test]
    fn a_malformed_signature_is_refused_rather_than_panicking() {
        let message = signing_string(&["device-1"]);

        assert!(!hmac_matches("secret", &message, "not hex at all"));
        assert!(!hmac_matches("secret", &message, "abc"));
        assert!(!hmac_matches("secret", &message, ""));
    }

    #[test]
    fn random_hex_is_the_requested_length_and_does_not_repeat() {
        let first = random_hex(16).unwrap();
        let second = random_hex(16).unwrap();

        assert!(is_hex(&first, 16));
        assert_ne!(first, second);
    }
}
