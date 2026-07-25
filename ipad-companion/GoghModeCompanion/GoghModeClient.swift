import Foundation

struct GoghModeEndpoint: Equatable {
    let saveURL: URL

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
            components.percentEncodedPath = path
        } else {
            if !path.hasSuffix("/") {
                path += "/"
            }
            components.percentEncodedPath = path + "save"
        }

        guard let url = components.url else {
            return nil
        }

        saveURL = url
    }
}

struct GoghModeClient {
    var session: URLSession = .shared

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

    var errorDescription: String? {
        switch self {
        case .invalidEndpoint:
            "Paste the Mac mobile URL from GoghMode."
        case .invalidResponse:
            "The Mac did not send a valid response."
        case .serverStatus(let status):
            "The Mac rejected the drawing with status \(status)."
        }
    }
}
