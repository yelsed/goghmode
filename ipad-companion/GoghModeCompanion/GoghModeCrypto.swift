import CryptoKit
import Foundation

/// The companion's half of the signing contract. Every formula here has a twin
/// in `src/protocol.rs`, and the two must agree byte for byte — a drift between
/// them is invisible until nothing authenticates, so they are written to look
/// alike on purpose.
enum GoghModeCrypto {
    /// Frames the parts of a signed message so no two different field lists can
    /// produce the same bytes. A plain separator is not enough: a device name is
    /// arbitrary text and could contain whatever separator was chosen.
    ///
    /// The length is in UTF-8 bytes, matching Rust's `str::len()`. Swift's
    /// `count` is graphemes, which would disagree the moment a name contains an
    /// emoji — and then only for that user.
    static func signingString(_ parts: [String]) -> String {
        parts.map { "\($0.utf8.count):\($0)" }.joined()
    }

    static func hmacHex(key: String, message: String) -> String {
        let code = HMAC<SHA256>.authenticationCode(
            for: Data(message.utf8),
            using: SymmetricKey(data: Data(key.utf8))
        )
        return hex(code)
    }

    static func sha256Hex(_ data: Data) -> String {
        hex(SHA256.hash(data: data))
    }

    /// Compares in constant time. The host is the side an attacker can probe,
    /// but a verification that leaks nothing costs nothing either.
    static func matches(_ expected: String, _ candidate: String) -> Bool {
        let expectedBytes = Array(expected.utf8)
        let candidateBytes = Array(candidate.utf8)
        guard expectedBytes.count == candidateBytes.count else { return false }
        var difference: UInt8 = 0
        for index in expectedBytes.indices {
            difference |= expectedBytes[index] ^ candidateBytes[index]
        }
        return difference == 0
    }

    static func randomHex(byteCount: Int) -> String {
        var bytes = [UInt8](repeating: 0, count: byteCount)
        let status = SecRandomCopyBytes(kSecRandomDefault, byteCount, &bytes)
        guard status == errSecSuccess else {
            // Falling back to something guessable would be worse than failing,
            // so callers treat an empty string as "cannot proceed".
            return ""
        }
        return bytes.map { String(format: "%02x", $0) }.joined()
    }

    // MARK: - Protocol formulas

    static func deriveDeviceSecret(pairingSecret: String, deviceID: String) -> String {
        hmacHex(
            key: pairingSecret,
            message: signingString(["goghmode-device-v1", deviceID])
        )
    }

    static func pairRequestMac(
        pairingSecret: String,
        hostID: String,
        deviceID: String,
        deviceName: String
    ) -> String {
        hmacHex(
            key: pairingSecret,
            message: signingString(["pair", hostID, deviceID, deviceName])
        )
    }

    static func pairResponseMac(
        pairingSecret: String,
        hostID: String,
        deviceID: String
    ) -> String {
        hmacHex(
            key: pairingSecret,
            message: signingString(["paired", hostID, deviceID])
        )
    }

    static func uploadMac(
        deviceSecret: String,
        deviceID: String,
        timestampMillis: UInt64,
        nonce: String,
        hostID: String,
        bodyDigest: String
    ) -> String {
        hmacHex(
            key: deviceSecret,
            message: signingString([
                deviceID,
                String(timestampMillis),
                nonce,
                hostID,
                bodyDigest,
            ])
        )
    }

    static func responseMac(deviceSecret: String, nonce: String, status: Int) -> String {
        hmacHex(
            key: deviceSecret,
            message: signingString(["response", nonce, String(status)])
        )
    }

    private static func hex<Bytes: Sequence>(_ bytes: Bytes) -> String
    where Bytes.Element == UInt8 {
        bytes.map { String(format: "%02x", $0) }.joined()
    }
}

extension Date {
    var unixMillis: UInt64 {
        UInt64(timeIntervalSince1970 * 1000)
    }
}
