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
        /// A machine answered but could not prove it is the paired host. Kept
        /// apart from `failed` because "offline" invites a retry and this must
        /// not be retried into.
        case wrongHost(String)

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
            case .wrongHost:
                "Wrong host"
            }
        }
    }

    @Published private(set) var status: Status = .idle

    /// False once a host has told us it predates pages. The page switcher hides
    /// itself rather than pretending a page switch means anything there.
    @Published private(set) var pagesSupported = true

    private let client: GoghModeClient
    private var pendingUpload: Task<Void, Never>?
    private var lastSnapshot: DrawingSnapshot?
    private var lastEndpointText = ""
    private var lastDestination: Destination?
    private var capabilitiesByEndpoint: [String: GoghModeCapabilities] = [:]

    /// Where one drawing is going. Carrying the resolved host and its key
    /// together means an upload can never be assembled from a host and somebody
    /// else's credential.
    struct Destination {
        let host: SavedHost
        let secret: String?
        let deviceID: String
    }

    var pagesUnsupportedMessage: String? {
        pagesSupported
            ? nil
            : "This desktop runs an older GoghMode, so pages are off. Update the desktop app."
    }

    var canRetry: Bool {
        // Deliberately not offered for `wrongHost`: retrying into a machine that
        // could not prove itself is exactly what must not happen automatically.
        if case .failed = status {
            return lastSnapshot != nil
        }
        return false
    }

    init(client: GoghModeClient = GoghModeClient()) {
        self.client = client
    }

    func schedule(snapshot: DrawingSnapshot, to destination: Destination) {
        remember(snapshot, destination)
        pendingUpload?.cancel()
        status = .waiting

        pendingUpload = Task { [client] in
            do {
                try await Task.sleep(for: .milliseconds(600))
                try Task.checkCancellation()
                try await send(snapshot, to: destination, client: client)
            } catch is CancellationError {
                return
            } catch {
                record(error)
            }
        }
    }

    func uploadNow(snapshot: DrawingSnapshot, to destination: Destination) {
        remember(snapshot, destination)
        pendingUpload?.cancel()
        pendingUpload = Task { [client] in
            do {
                try await send(snapshot, to: destination, client: client)
            } catch {
                record(error)
            }
        }
    }

    /// Re-sends the last drawing. Without this the status stays `Offline` forever
    /// once an upload fails, because nothing retries until the drawing changes —
    /// so quitting and reopening the desktop app looked like a permanent failure.
    func retry() {
        guard let snapshot = lastSnapshot, let destination = lastDestination else { return }
        uploadNow(snapshot: snapshot, to: destination)
    }

    func retryIfOffline() {
        guard canRetry else { return }
        retry()
    }

    private func remember(_ snapshot: DrawingSnapshot, _ destination: Destination) {
        lastSnapshot = snapshot
        lastDestination = destination
        lastEndpointText = destination.host.address
    }

    private func record(_ error: Error) {
        if let uploadError = error as? UploadError, case .wrongHost(let name) = uploadError {
            status = .wrongHost(
                "The machine at \(lastEndpointText) is not \(name). Pair again or fix the address."
            )
            return
        }
        status = .failed(guidance(for: error))
    }

    /// One drawing, one host. A failure never reroutes to another saved host —
    /// silently sending someone's notes to the wrong machine is worse than not
    /// sending them at all.
    private func send(
        _ snapshot: DrawingSnapshot,
        to destination: Destination,
        client: GoghModeClient
    ) async throws {
        guard destination.host.isPaired else {
            try await uploadOverLegacyURL(snapshot, to: destination.host, client: client)
            return
        }
        guard let secret = destination.secret else {
            throw UploadError.invalidEndpoint
        }

        status = .saving
        do {
            try await client.upload(
                snapshot,
                to: destination.host,
                secret: secret,
                deviceID: destination.deviceID
            )
        } catch let error as URLError where error.isWorthRetrying {
            try await Task.sleep(for: .milliseconds(300))
            try await client.upload(
                snapshot,
                to: destination.host,
                secret: secret,
                deviceID: destination.deviceID
            )
        }
        status = .saved(Date())
    }

    /// One probe per endpoint, cached. Asking the host what it takes is cheaper
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

    private func uploadOverLegacyURL(
        _ snapshot: DrawingSnapshot,
        to host: SavedHost,
        client: GoghModeClient
    ) async throws {
        let endpointText = host.address
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
            // URLSession can hand back a pooled socket the host already closed,
            // which surfaces as `networkConnectionLost` even though the host is
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
            "Desktop not answering. Open GoghMode there, then tap to retry."
        case .notConnectedToInternet:
            "No network. Join the same Wi-Fi as the desktop."
        case .cannotFindHost, .dnsLookupFailed:
            "Address not found. Copy the mobile URL from the desktop again."
        default:
            urlError.localizedDescription
        }
    }
}

private extension URLError {
    /// Failures that a second attempt can plausibly clear, as opposed to a wrong
    /// address or a host that is genuinely not running the app.
    var isWorthRetrying: Bool {
        switch code {
        case .networkConnectionLost, .timedOut, .cannotConnectToHost:
            true
        default:
            false
        }
    }
}
