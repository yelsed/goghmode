import Foundation

struct GoghModeEndpoint: Equatable {
    let saveURL: URL
    let capabilitiesURL: URL
    let pinURL: URL
    let promoteURL: URL

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
        func route(_ name: String) -> URL? {
            components.percentEncodedPath = root + name
            return components.url
        }

        guard let save = route("save"),
              let capabilities = route("capabilities"),
              let pin = route("pin"),
              let promote = route("promote") else {
            return nil
        }

        saveURL = save
        capabilitiesURL = capabilities
        pinURL = pin
        promoteURL = promote
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

    /// What to assume when the host could not be asked. Optimism is right here: the
    /// save that follows is the real test, and it reports its own failure. Assuming
    /// the worst instead turns one dropped probe into a standing complaint.
    static let assumeCurrent = GoghModeCapabilities(
        schemaVersions: [pagelessSchemaVersion, currentSchemaVersion],
        features: ["pages", "pin", "promote"]
    )

    var supportsPages: Bool {
        schemaVersions.contains(currentSchemaVersion)
    }

    /// A host that knows about pages but not pinning still works; the stamp is
    /// simply not offered rather than silently doing nothing.
    var supportsPinning: Bool {
        features.contains("pin") && features.contains("promote")
    }
}

struct GoghModeClient {
    var session: URLSession = .shared

    /// Asks the host what it accepts. Probing beats inferring from a rejection: a
    /// 404 here is a host from before pages, so the drawing still gets through as
    /// version 1.
    ///
    /// Returns `nil` when the host could not be asked at all. A timeout or a
    /// dropped socket is not evidence of an old host, and treating it as one
    /// switched pages and stamping off until the app was restarted — a complaint
    /// that outlived every successful save after it.
    func capabilities(of endpoint: GoghModeEndpoint) async -> GoghModeCapabilities? {
        guard let (data, response) = try? await session.data(from: endpoint.capabilitiesURL),
              let httpResponse = response as? HTTPURLResponse else {
            return nil
        }
        guard (200..<300).contains(httpResponse.statusCode) else {
            return .pagelessHost
        }
        return (try? JSONDecoder().decode(GoghModeCapabilities.self, from: data)) ?? .pagelessHost
    }

    /// Stamps a page as the one `latest.*` follows, or clears the stamp with
    /// `nil`. The host owns this state; the app asks and then records the answer.
    func pin(_ pageID: String?, on endpoint: GoghModeEndpoint) async throws {
        try await postPageID(pageID, to: endpoint.pinURL)
    }

    /// Sends one page now without moving the stamp.
    func promote(_ pageID: String, on endpoint: GoghModeEndpoint) async throws {
        try await postPageID(pageID, to: endpoint.promoteURL)
    }

    private func postPageID(_ pageID: String?, to url: URL) async throws {
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json; charset=utf-8", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(["pageId": pageID])

        let (data, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw UploadError.invalidResponse
        }
        guard (200..<300).contains(httpResponse.statusCode) else {
            // The host names what was wrong in the body; passing it through beats
            // inventing a generic message on top of a specific one.
            let reason = String(data: data, encoding: .utf8) ?? ""
            throw UploadError.rejected(reason.isEmpty ? "The desktop rejected it." : reason)
        }
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
        _ = try await signedPost(
            try JSONEncoder().encode(snapshot),
            to: "/v2/save",
            on: host,
            secret: secret,
            deviceID: deviceID
        )
    }

    /// Stamps a sheet on a paired host, or clears the stamp with `nil`.
    ///
    /// Choosing which sheet the agent reads is as consequential as sending one,
    /// so it goes through the same signed door rather than a quieter one.
    func pin(
        _ pageID: String?,
        on host: SavedHost,
        secret: String,
        deviceID: String
    ) async throws {
        _ = try await signedPost(
            try JSONEncoder().encode(["pageId": pageID]),
            to: "/v2/pin",
            on: host,
            secret: secret,
            deviceID: deviceID
        )
    }

    func promote(
        _ pageID: String,
        on host: SavedHost,
        secret: String,
        deviceID: String
    ) async throws {
        _ = try await signedPost(
            try JSONEncoder().encode(["pageId": pageID]),
            to: "/v2/promote",
            on: host,
            secret: secret,
            deviceID: deviceID
        )
    }

    @discardableResult
    private func signedPost(
        _ body: Data,
        to routePath: String,
        on host: SavedHost,
        secret: String,
        deviceID: String
    ) async throws -> Data {
        guard let url = URL(string: host.address + routePath) else {
            throw UploadError.invalidEndpoint
        }
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

        let (data, response) = try await session.data(for: request)
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
            // The host names what was wrong in the body, and it has just proved
            // it is the host, so the reason can be trusted enough to show.
            let reason = String(data: data, encoding: .utf8) ?? ""
            throw reason.isEmpty
                ? UploadError.serverStatus(httpResponse.statusCode)
                : UploadError.rejected(reason)
        }
        return data
    }
}

enum UploadError: Error, Equatable, LocalizedError {
    case invalidEndpoint
    case invalidResponse
    case serverStatus(Int)
    case wrongHost(String)
    case noSecureRandom
    case rejected(String)

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
        case .rejected(let reason):
            reason
        }
    }
}
