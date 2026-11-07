//! The wire contract for paired devices: which bytes get signed, and by whom.
//!
//! Every formula lives here rather than at its call site, because the host and
//! the companion have to agree on them exactly and a drift between two copies
//! of a signing string is invisible until nothing authenticates.

use crate::crypto::{hmac_hex, hmac_matches, signing_string};

pub const PROTOCOL_VERSION: u32 = 1;

pub const PAIRING_SECRET_BYTES: usize = 16;
pub const DEVICE_ID_MAX_LENGTH: usize = 64;
pub const DEVICE_NAME_MAX_LENGTH: usize = 64;

/// How far a companion's clock may differ from the host's. Wide enough that
/// two machines with ordinary drift agree, narrow enough that a captured
/// request stops being useful quickly.
pub const TIMESTAMP_TOLERANCE_MILLIS: u128 = 120_000;

/// How long a shown pairing secret stays usable. It is single-use as well, so
/// this only bounds the window in which a photographed screen is a credential.
pub const PAIRING_LIFETIME_SECONDS: u64 = 120;

pub const HEADER_DEVICE: &str = "x-goghmode-device";
pub const HEADER_TIMESTAMP: &str = "x-goghmode-timestamp";
pub const HEADER_NONCE: &str = "x-goghmode-nonce";
pub const HEADER_MAC: &str = "x-goghmode-mac";
/// The companion's half of the protocol. The host never produces these, but
/// they live here beside the checks that consume them so the two descriptions
/// of one signing string cannot drift apart — a drift that is invisible until
/// nothing authenticates. The integration tests exercise them as a client would.
#[allow(dead_code)]
pub const HEADER_HOST_MAC: &str = "x-goghmode-host-mac";
pub const HEADER_PAIR_MAC: &str = "x-goghmode-pair-mac";

/// The long-lived per-device key. Derived on both sides from the value that
/// travelled screen-to-camera, so it is never transmitted — a full recording of
/// the pairing exchange yields nothing.
///
/// HMAC is a sound key-derivation function here because the input key is a
/// single uniformly random 128-bit value. The label pins the output to one
/// purpose and one version, so a future derivation cannot collide with this one.
pub fn derive_device_secret(pairing_secret: &str, device_id: &str) -> String {
    hmac_hex(
        pairing_secret,
        &signing_string(&["goghmode-device-v1", device_id]),
    )
}

#[allow(dead_code)]
pub fn pair_request_mac(
    pairing_secret: &str,
    host_id: &str,
    device_id: &str,
    device_name: &str,
) -> String {
    hmac_hex(
        pairing_secret,
        &signing_string(&["pair", host_id, device_id, device_name]),
    )
}

pub fn pair_request_mac_matches(
    pairing_secret: &str,
    host_id: &str,
    device_id: &str,
    device_name: &str,
    candidate: &str,
) -> bool {
    hmac_matches(
        pairing_secret,
        &signing_string(&["pair", host_id, device_id, device_name]),
        candidate,
    )
}

/// Proves to the companion that the machine which answered holds the value the
/// user was looking at. Without it, pairing would authenticate the device to
/// the host but not the host to the device.
pub fn pair_response_mac(pairing_secret: &str, host_id: &str, device_id: &str) -> String {
    hmac_hex(
        pairing_secret,
        &signing_string(&["paired", host_id, device_id]),
    )
}

/// `host_id` is signed so a request captured on one host cannot be replayed
/// against another. The body digest is signed so the drawing cannot be swapped
/// for a different one under a valid signature.
#[allow(dead_code)]
pub fn upload_mac(
    device_secret: &str,
    device_id: &str,
    timestamp_millis: u128,
    nonce: &str,
    host_id: &str,
    body_sha256_hex: &str,
) -> String {
    hmac_hex(device_secret, &upload_message(device_id, timestamp_millis, nonce, host_id, body_sha256_hex))
}

pub fn upload_mac_matches(
    device_secret: &str,
    device_id: &str,
    timestamp_millis: u128,
    nonce: &str,
    host_id: &str,
    body_sha256_hex: &str,
    candidate: &str,
) -> bool {
    hmac_matches(
        device_secret,
        &upload_message(device_id, timestamp_millis, nonce, host_id, body_sha256_hex),
        candidate,
    )
}

fn upload_message(
    device_id: &str,
    timestamp_millis: u128,
    nonce: &str,
    host_id: &str,
    body_sha256_hex: &str,
) -> String {
    signing_string(&[
        device_id,
        &timestamp_millis.to_string(),
        nonce,
        host_id,
        body_sha256_hex,
    ])
}

/// Binds the host's proof to the nonce the companion just chose, so it is a
/// live answer rather than a value replayable from an earlier exchange.
pub fn response_mac(device_secret: &str, nonce: &str, status: u16) -> String {
    hmac_hex(
        device_secret,
        &signing_string(&["response", nonce, &status.to_string()]),
    )
}

/// A device name is shown to a person in an approval sheet, so it is untrusted
/// display text: control characters could rewrite the line around it.
pub fn sanitise_device_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|character| !character.is_control())
        .take(DEVICE_NAME_MAX_LENGTH)
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "Unnamed device".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// A device identifier ends up as a registry key and appears in signed
/// messages. Keeping it to an unambiguous alphabet means it can never need
/// escaping anywhere it is used.
pub fn device_id_is_safe(device_id: &str) -> bool {
    !device_id.is_empty()
        && device_id.len() <= DEVICE_ID_MAX_LENGTH
        && device_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_devices_pairing_from_one_secret_get_different_keys() {
        let pairing_secret = "0123456789abcdef0123456789abcdef";

        let first = derive_device_secret(pairing_secret, "device-one");
        let second = derive_device_secret(pairing_secret, "device-two");

        assert_ne!(first, second);
    }

    #[test]
    fn an_upload_signature_does_not_transfer_to_another_host() {
        let secret = derive_device_secret("0123456789abcdef0123456789abcdef", "device-1");
        let signature = upload_mac(&secret, "device-1", 1_785_000_000_000, "nonce", "host-a", "digest");

        assert!(upload_mac_matches(
            &secret,
            "device-1",
            1_785_000_000_000,
            "nonce",
            "host-a",
            "digest",
            &signature
        ));
        assert!(!upload_mac_matches(
            &secret,
            "device-1",
            1_785_000_000_000,
            "nonce",
            "host-b",
            "digest",
            &signature
        ));
    }

    #[test]
    fn an_upload_signature_does_not_survive_the_body_changing() {
        let secret = "secret";
        let signature = upload_mac(secret, "device-1", 1, "nonce", "host", "digest-of-drawing");

        assert!(!upload_mac_matches(
            secret,
            "device-1",
            1,
            "nonce",
            "host",
            "digest-of-a-different-drawing",
            &signature
        ));
    }

    #[test]
    fn device_names_cannot_carry_control_characters_into_the_approval_sheet() {
        assert_eq!(sanitise_device_name("iPad\u{0007}\nof Desley"), "iPadof Desley");
        assert_eq!(sanitise_device_name("   "), "Unnamed device");
        assert_eq!(sanitise_device_name(&"x".repeat(200)).len(), DEVICE_NAME_MAX_LENGTH);
    }

    #[test]
    fn device_ids_that_could_need_escaping_are_refused() {
        assert!(device_id_is_safe("iPad-of-Desley_1"));
        assert!(!device_id_is_safe(""));
        assert!(!device_id_is_safe("../escape"));
        assert!(!device_id_is_safe("has space"));
        assert!(!device_id_is_safe(&"x".repeat(DEVICE_ID_MAX_LENGTH + 1)));
    }
}
