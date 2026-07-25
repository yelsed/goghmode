import Foundation
import SwiftUI

@MainActor
final class UploadController: ObservableObject {
    enum Status: Equatable {
        case idle
        case waiting
        case saving
        case saved(Date)
        case failed(String)

        var label: String {
            switch self {
            case .idle:
                "Ready"
            case .waiting:
                "Waiting"
            case .saving:
                "Saving"
            case .saved:
                "Saved"
            case .failed:
                "Offline"
            }
        }
    }

    @Published private(set) var status: Status = .idle

    /// False once a Mac has told us it predates pages. The page switcher hides
    /// itself rather than pretending a page switch means anything there.
    @Published private(set) var pagesSupported = true

    private let client: GoghModeClient
    private var pendingUpload: Task<Void, Never>?
    private var lastSnapshot: DrawingSnapshot?
    private var lastEndpointText = ""
    private var capabilitiesByEndpoint: [String: GoghModeCapabilities] = [:]

    var pagesUnsupportedMessage: String? {
        pagesSupported
            ? nil
            : "This Mac runs an older GoghMode, so pages are off. Update the Mac app."
    }

    var canRetry: Bool {
        if case .failed = status {
            return lastSnapshot != nil
        }
        return false
    }

    init(client: GoghModeClient = GoghModeClient()) {
        self.client = client
    }

    func schedule(snapshot: DrawingSnapshot, endpointText: String) {
        remember(snapshot, endpointText)
        pendingUpload?.cancel()
        status = .waiting

        pendingUpload = Task { [client] in
            do {
                try await Task.sleep(for: .milliseconds(600))
                try Task.checkCancellation()
                try await upload(snapshot, endpointText: endpointText, client: client)
            } catch is CancellationError {
                return
            } catch {
                status = .failed(guidance(for: error))
            }
        }
    }

    func uploadNow(snapshot: DrawingSnapshot, endpointText: String) {
        remember(snapshot, endpointText)
        pendingUpload?.cancel()
        pendingUpload = Task { [client] in
            do {
                try await upload(snapshot, endpointText: endpointText, client: client)
            } catch {
                status = .failed(guidance(for: error))
            }
        }
    }

    /// Re-sends the last drawing. Without this the status stays `Offline` forever
    /// once an upload fails, because nothing retries until the drawing changes —
    /// so quitting and reopening the Mac app looked like a permanent failure.
    func retry() {
        guard let snapshot = lastSnapshot else { return }
        uploadNow(snapshot: snapshot, endpointText: lastEndpointText)
    }

    func retryIfOffline() {
        guard canRetry else { return }
        retry()
    }

    private func remember(_ snapshot: DrawingSnapshot, _ endpointText: String) {
        lastSnapshot = snapshot
        lastEndpointText = endpointText
    }

    /// One probe per endpoint, cached. Asking the Mac what it takes is cheaper
    /// and clearer than sending version 2 and reading the rejection.
    private func resolvedCapabilities(
        for endpoint: GoghModeEndpoint,
        endpointText: String,
        client: GoghModeClient
    ) async -> GoghModeCapabilities {
        if let known = capabilitiesByEndpoint[endpointText] {
            return known
        }
        let capabilities = await client.capabilities(of: endpoint)
        capabilitiesByEndpoint[endpointText] = capabilities
        pagesSupported = capabilities.supportsPages
        return capabilities
    }

    private func upload(_ snapshot: DrawingSnapshot, endpointText: String, client: GoghModeClient) async throws {
        guard let endpoint = GoghModeEndpoint(endpointText) else {
            throw UploadError.invalidEndpoint
        }

        let capabilities = await resolvedCapabilities(
            for: endpoint,
            endpointText: endpointText,
            client: client
        )
        let outgoing = capabilities.supportsPages ? snapshot : snapshot.withoutPage()

        status = .saving
        do {
            try await client.upload(outgoing, to: endpoint)
        } catch let error as URLError where error.isWorthRetrying {
            // URLSession can hand back a pooled socket the Mac already closed,
            // which surfaces as `networkConnectionLost` even though the Mac is
            // reachable. One retry separates a dead socket from a dead server.
            try await Task.sleep(for: .milliseconds(300))
            try await client.upload(outgoing, to: endpoint)
        }
        status = .saved(Date())
    }

    private func guidance(for error: Error) -> String {
        if let uploadError = error as? UploadError {
            return uploadError.errorDescription ?? "Upload failed."
        }

        guard let urlError = error as? URLError else {
            return error.localizedDescription
        }

        return switch urlError.code {
        case .networkConnectionLost, .cannotConnectToHost, .timedOut:
            "Mac not answering. Open GoghMode on the Mac, then tap to retry."
        case .notConnectedToInternet:
            "No network. Join the same Wi-Fi as the Mac."
        case .cannotFindHost, .dnsLookupFailed:
            "Address not found. Copy the mobile URL from the Mac again."
        default:
            urlError.localizedDescription
        }
    }
}

private extension URLError {
    /// Failures that a second attempt can plausibly clear, as opposed to a wrong
    /// address or a Mac that is genuinely not running the app.
    var isWorthRetrying: Bool {
        switch code {
        case .networkConnectionLost, .timedOut, .cannotConnectToHost:
            true
        default:
            false
        }
    }
}
