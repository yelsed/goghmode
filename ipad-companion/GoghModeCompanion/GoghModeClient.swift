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

/// What a Mac says it accepts. A Mac from before pages has no such route and
/// answers 404, which the app reads as "schema version 1 only".
struct GoghModeCapabilities: Codable, Equatable {
    let schemaVersions: [Int]
    let features: [String]

    static let pagelessMac = GoghModeCapabilities(
        schemaVersions: [pagelessSchemaVersion],
        features: []
    )

    var supportsPages: Bool {
        schemaVersions.contains(currentSchemaVersion)
    }

    /// A Mac that knows about pages but not pinning still works; the stamp is
    /// simply not offered rather than silently doing nothing.
    var supportsPinning: Bool {
        features.contains("pin") && features.contains("promote")
    }
}

struct GoghModeClient {
    var session: URLSession = .shared

    /// Asks the Mac what it accepts. Probing beats inferring from a rejection:
    /// a 404 here is an old Mac, and anything else unreadable is treated the
    /// same way, so the drawing still gets through as version 1.
    func capabilities(of endpoint: GoghModeEndpoint) async -> GoghModeCapabilities {
        guard let (data, response) = try? await session.data(from: endpoint.capabilitiesURL),
              let httpResponse = response as? HTTPURLResponse,
              (200..<300).contains(httpResponse.statusCode),
              let capabilities = try? JSONDecoder().decode(GoghModeCapabilities.self, from: data) else {
            return .pagelessMac
        }
        return capabilities
    }

    /// Stamps a page as the one `latest.*` follows, or clears the stamp with
    /// `nil`. The Mac owns this state; the app asks and then records the answer.
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
            // The Mac names what was wrong in the body; passing it through beats
            // inventing a generic message on top of a specific one.
            let reason = String(data: data, encoding: .utf8) ?? ""
            throw UploadError.rejected(reason.isEmpty ? "The Mac rejected it." : reason)
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
}

enum UploadError: Error, Equatable, LocalizedError {
    case invalidEndpoint
    case invalidResponse
    case serverStatus(Int)
    case rejected(String)

    var errorDescription: String? {
        switch self {
        case .invalidEndpoint:
            "Paste the Mac mobile URL from GoghMode."
        case .invalidResponse:
            "The Mac did not send a valid response."
        case .serverStatus(let status):
            "The Mac rejected the drawing with status \(status)."
        case .rejected(let reason):
            reason
        }
    }
}
