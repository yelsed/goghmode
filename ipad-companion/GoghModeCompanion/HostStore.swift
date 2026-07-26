import Foundation
#if canImport(UIKit)
    import UIKit
#endif

/// How the companion proves itself to a host.
///
/// Two kinds coexist on purpose. A host paired the new way holds a key derived
/// during pairing; one saved before that exists still works through its secret
/// URL, because stranding those users to make the list uniform would be a poor
/// trade. The kind is visible in the interface, so nobody has to guess which
/// destination is authenticated.
enum HostCredential: Codable, Equatable {
    case paired
    case legacyURL(String)
}

struct SavedHost: Codable, Equatable, Identifiable {
    /// The host's own identity for a paired host, a locally minted one for a
    /// legacy host, which has no identity to offer.
    let id: String
    var name: String
    var platform: String
    /// Scheme, address and port — no path, no secret. For a legacy host this is
    /// the full secret URL, because there the path *is* the credential.
    var address: String
    var credential: HostCredential

    var isPaired: Bool {
        credential == .paired
    }
}

/// The saved hosts and the one that is currently selected.
///
/// Secrets never live here. They are in the Keychain, keyed by host identity,
/// non-syncing so a device backup does not carry them to another device.
@MainActor
final class HostStore: ObservableObject {
    @Published private(set) var hosts: [SavedHost] = []
    @Published private(set) var selectedHostID: String?

    private let defaults: UserDefaults
    private let hostsKey = "goghmode-hosts"
    private let selectionKey = "goghmode-selected-host"

    var selectedHost: SavedHost? {
        hosts.first { $0.id == selectedHostID }
    }

    /// The identity this device presents when pairing. Minted once and kept, so
    /// re-pairing with the same host replaces one entry rather than growing the
    /// host's device list every time.
    private(set) var deviceID: String

    var deviceName: String {
        #if canImport(UIKit)
            return UIDevice.current.name
        #else
            return "iPad"
        #endif
    }

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        deviceID = HostStore.loadOrCreateDeviceID(defaults)
        load()
    }

    func add(_ host: SavedHost, secret: String?) {
        if let secret {
            Keychain.store(secret: secret, for: host.id)
        }
        hosts.removeAll { $0.id == host.id }
        hosts.append(host)
        selectedHostID = host.id
        save()
    }

    func remove(_ hostID: String) {
        Keychain.removeSecret(for: hostID)
        hosts.removeAll { $0.id == hostID }
        if selectedHostID == hostID {
            selectedHostID = hosts.first?.id
        }
        save()
    }

    func select(_ hostID: String) {
        guard hosts.contains(where: { $0.id == hostID }) else { return }
        selectedHostID = hostID
        save()
    }

    /// A host that moved keeps its identity, so this is an update rather than a
    /// new host — which is the whole reason identity is not the address.
    func updateAddress(_ address: String, for hostID: String) {
        guard let index = hosts.firstIndex(where: { $0.id == hostID }) else { return }
        hosts[index].address = address
        save()
    }

    func secret(for hostID: String) -> String? {
        Keychain.secret(for: hostID)
    }

    /// Carries a single endpoint saved by an older build into the host list, so
    /// updating the app does not look like losing the connection.
    func adoptLegacyEndpoint(_ endpointText: String) {
        let trimmed = endpointText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, hosts.isEmpty else { return }
        add(
            SavedHost(
                id: "legacy-\(UUID().uuidString)",
                name: "Desktop",
                platform: "unknown",
                address: trimmed,
                credential: .legacyURL(trimmed)
            ),
            secret: nil
        )
    }

    private func load() {
        if let data = defaults.data(forKey: hostsKey),
            let stored = try? JSONDecoder().decode([SavedHost].self, from: data)
        {
            hosts = stored
        }
        selectedHostID = defaults.string(forKey: selectionKey) ?? hosts.first?.id
    }

    private func save() {
        if let data = try? JSONEncoder().encode(hosts) {
            defaults.set(data, forKey: hostsKey)
        }
        defaults.set(selectedHostID, forKey: selectionKey)
    }

    private static func loadOrCreateDeviceID(_ defaults: UserDefaults) -> String {
        let key = "goghmode-device-id"
        if let existing = defaults.string(forKey: key), !existing.isEmpty {
            return existing
        }
        // The host only accepts letters, digits, hyphen and underscore, so the
        // identifier is built from an alphabet that needs no escaping anywhere.
        let minted = "ipad-" + GoghModeCrypto.randomHex(byteCount: 8)
        defaults.set(minted, forKey: key)
        return minted
    }
}

/// Device secrets, kept out of `UserDefaults` and out of backups.
enum Keychain {
    private static let service = "dev.goghmode.companion"

    static func store(secret: String, for hostID: String) {
        removeSecret(for: hostID)
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: hostID,
            kSecValueData as String: Data(secret.utf8),
            // This device only, and not while locked. A secret that syncs is a
            // secret on hardware the user did not pair.
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
        ]
        SecItemAdd(query as CFDictionary, nil)
    }

    static func secret(for hostID: String) -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: hostID,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
            let data = result as? Data
        else {
            return nil
        }
        return String(data: data, encoding: .utf8)
    }

    static func removeSecret(for hostID: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: hostID,
        ]
        SecItemDelete(query as CFDictionary)
    }
}
