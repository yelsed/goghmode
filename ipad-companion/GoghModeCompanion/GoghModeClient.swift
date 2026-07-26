import Foundation

struct GoghModeEndpoint: Equatable {
    let saveURL: URL
    let capabilitiesURL: URL

    init?(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, var components = URLComponents(string: trimmed) else {
            return nil
        }

        guard components.scheme == "http" || components.scheme == "https", components.host != nil else {
            return nil
        }

        var path = components.percentEncodedPath
        if path.isEmpty {
            return nil
        }

        if path.hasSuffix("/save") {
            path.removeLast("save".count)
        }
        if !path.hasSuffix("/") {
            path += "/"
        }

        let root = path
        components.percentEncodedPath = root + "save"
        guard let save = components.url else {
            return nil
        }
        components.percentEncodedPath = root + "capabilities"
        guard let capabilities = components.url else {
            return nil
        }

        saveURL = save
        capabilitiesURL = capabilities
    }
}

/// What a host says it accepts. A host from before pages has no such route
/// and answers 404, which the app reads as "schema version 1 only".
struct GoghModeCapabilities: Codable, Equatable {
    let schemaVersions: [Int]
    let features: [String]

    static let pagelessHost = GoghModeCapabilities(
        schemaVersions: [pagelessSchemaVersion],
        features: []
    )

    var supportsPages: Bool {
        schemaVersions.contains(currentSchemaVersion)
    }
}

struct GoghModeClient {
    var session: URLSession = .shared

    /// Asks the host what it accepts. Probing beats inferring from a rejection:
    /// a 404 here is an old host, and anything else unreadable is treated the
    /// same way, so the drawing still gets through as version 1.
    func capabilities(of endpoint: GoghModeEndpoint) async -> GoghModeCapabilities {
        guard let (data, response) = try? await session.data(from: endpoint.capabilitiesURL),
              let httpResponse = response as? HTTPURLResponse,
              (200..<300).contains(httpResponse.statusCode),
              let capabilities = try? JSONDecoder().decode(GoghModeCapabilities.self, from: data) else {
            return .pagelessHost
        }
        return capabilities
    }

    func upload(_ snapshot: DrawingSnapshot, to endpoint: GoghModeEndpoint) async throws {
        var request = URLRequest(url: endpoint.saveURL)
        request.httpMethod = "POST"
        request.setValue("application/json; charset=utf-8", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(snapshot)

        let (_, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw UploadError.invalidResponse
        }

        guard (200..<300).contains(httpResponse.statusCode) else {
            throw UploadError.serverStatus(httpResponse.statusCode)
        }
    }

    /// Sends a drawing to a paired host, signed, and refuses to call it a
    /// success until the host has signed its answer back.
    ///
    /// Without that last check a machine that merely answers at the saved
    /// address — because the address was reassigned, or because someone took
    /// it — would look exactly like the host the user paired with.
    func upload(
        _ snapshot: DrawingSnapshot,
        to host: SavedHost,
        secret: String,
        deviceID: String
    ) async throws {
        guard let url = URL(string: host.address + "/v2/save") else {
            throw UploadError.invalidEndpoint
        }
        let body = try JSONEncoder().encode(snapshot)
        let timestamp = Date().unixMillis
        let nonce = GoghModeCrypto.randomHex(byteCount: 16)
        guard !nonce.isEmpty else {
            throw UploadError.noSecureRandom
        }

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json; charset=utf-8", forHTTPHeaderField: "Content-Type")
        request.setValue(deviceID, forHTTPHeaderField: "X-GoghMode-Device")
        request.setValue(String(timestamp), forHTTPHeaderField: "X-GoghMode-Timestamp")
        request.setValue(nonce, forHTTPHeaderField: "X-GoghMode-Nonce")
        request.setValue(
            GoghModeCrypto.uploadMac(
                deviceSecret: secret,
                deviceID: deviceID,
                timestampMillis: timestamp,
                nonce: nonce,
                hostID: host.id,
                bodyDigest: GoghModeCrypto.sha256Hex(body)
            ),
            forHTTPHeaderField: "X-GoghMode-Mac"
        )
        request.httpBody = body

        let (_, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw UploadError.invalidResponse
        }

        let expectedProof = GoghModeCrypto.responseMac(
            deviceSecret: secret,
            nonce: nonce,
            status: httpResponse.statusCode
        )
        let offeredProof = httpResponse.value(forHTTPHeaderField: "X-GoghMode-Host-Mac") ?? ""
        guard GoghModeCrypto.matches(expectedProof, offeredProof) else {
            throw UploadError.wrongHost(host.name)
        }

        guard (200..<300).contains(httpResponse.statusCode) else {
            throw UploadError.serverStatus(httpResponse.statusCode)
        }
    }
}

enum UploadError: Error, Equatable, LocalizedError {
    case invalidEndpoint
    case invalidResponse
    case serverStatus(Int)
    case wrongHost(String)
    case noSecureRandom

    var errorDescription: String? {
        switch self {
        case .invalidEndpoint:
            "Paste the mobile URL from GoghMode on your desktop."
        case .invalidResponse:
            "The desktop did not send a valid response."
        case .serverStatus(let status):
            "The desktop rejected the drawing with status \(status)."
        case .wrongHost(let name):
            "The machine at that address is not \(name). Nothing was sent."
        case .noSecureRandom:
            "This device could not generate a secure value, so nothing was sent."
        }
    }
}
