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

    /// False on a Mac that understands pages but not the stamp routes.
    @Published private(set) var pinningSupported = true

    /// Whether the two flags above are an answer or a guess. Until a Mac has
    /// actually replied they are optimism, and a control drawn on optimism that
    /// disappears the moment it is pressed reads as the app breaking.
    @Published private(set) var macIsKnown = false

    private let client: GoghModeClient
    private var pendingUpload: Task<Void, Never>?
    private var lastSnapshot: DrawingSnapshot?
    private var lastEndpointText = ""
    private var capabilitiesByEndpoint: [String: GoghModeCapabilities] = [:]

    var pagesUnsupportedMessage: String? {
        pagesSupported
            ? nil
            : "GoghMode on the Mac is an older version, so pages are off. Update it there and reopen it."
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

    /// Sends a sheet and waits for the Mac to have it. Stamping cannot fire and
    /// forget: the Mac can only mirror a page it actually holds.
    @discardableResult
    func send(_ snapshot: DrawingSnapshot, endpointText: String) async -> Bool {
        remember(snapshot, endpointText)
        pendingUpload?.cancel()
        do {
            try await upload(snapshot, endpointText: endpointText, client: client)
            return true
        } catch {
            status = .failed(guidance(for: error))
            return false
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
        guard let capabilities = await client.capabilities(of: endpoint) else {
            // Unreachable, not old. Nothing is cached and nothing is concluded, so
            // the next attempt asks again instead of inheriting a guess.
            return .assumeCurrent
        }
        capabilitiesByEndpoint[endpointText] = capabilities
        pagesSupported = capabilities.supportsPages
        pinningSupported = capabilities.supportsPinning
        macIsKnown = true
        return capabilities
    }

    /// Asks the Mac what it accepts before anything needs the answer, so controls
    /// are drawn in the state they will actually behave in.
    func learnWhatTheMacAccepts(endpointText: String) async {
        guard let endpoint = GoghModeEndpoint(endpointText) else { return }
        _ = await resolvedCapabilities(for: endpoint, endpointText: endpointText, client: client)
    }

    /// Forgets what a Mac said it accepts. Called when the address changes, and when
    /// the app comes back — the Mac may have been updated in between, and a cached
    /// "too old" answer would keep the stamp switched off forever.
    func forgetWhatTheMacAccepts() {
        capabilitiesByEndpoint.removeAll()
        macIsKnown = false
        pagesSupported = true
        pinningSupported = true
    }

    /// Stamps a page as the one the agent reads, or clears the stamp with `nil`.
    /// Returns whether the Mac accepted it, so the caller records the pin only
    /// when it is actually true on disk.
    func pin(_ pageID: String?, endpointText: String) async -> Bool {
        guard let endpoint = GoghModeEndpoint(endpointText) else {
            status = .failed(UploadError.invalidEndpoint.errorDescription ?? "")
            return false
        }

        do {
            let capabilities = await resolvedCapabilities(
                for: endpoint,
                endpointText: endpointText,
                client: client
            )
            guard capabilities.supportsPinning else {
                // Deliberately not a `.failed` status. A capability verdict is not an
                // upload failure, and nothing clears a failure until an upload
                // succeeds — so setting one here left "the Mac app is an older
                // version" on screen long after the Mac had been updated. The
                // register reads the capabilities directly and says so for exactly as
                // long as it is true.
                return false
            }
            try await client.pin(pageID, on: endpoint)
            return true
        } catch {
            status = .failed(guidance(for: error))
            return false
        }
    }

    /// Names the app, not the machine. The first version said "this Mac is too old",
    /// which reads as a verdict on the hardware for something a reopen fixes.
    static let macAppOutOfDate =
        "GoghMode on the Mac is an older version that cannot stamp sheets yet. Update it there and reopen it."

    /// Sends one page now without moving the stamp.
    func promote(_ pageID: String, endpointText: String) async -> Bool {
        guard let endpoint = GoghModeEndpoint(endpointText) else {
            status = .failed(UploadError.invalidEndpoint.errorDescription ?? "")
            return false
        }

        do {
            let capabilities = await resolvedCapabilities(
                for: endpoint,
                endpointText: endpointText,
                client: client
            )
            guard capabilities.supportsPinning else {
                // Deliberately not a `.failed` status. A capability verdict is not an
                // upload failure, and nothing clears a failure until an upload
                // succeeds — so setting one here left "the Mac app is an older
                // version" on screen long after the Mac had been updated. The
                // register reads the capabilities directly and says so for exactly as
                // long as it is true.
                return false
            }
            try await client.promote(pageID, on: endpoint)
            status = .saved(Date())
            return true
        } catch {
            status = .failed(guidance(for: error))
            return false
        }
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

        // A save that lands is the strongest evidence there is: the Mac is reachable,
        // and if it took a page it understands pages. Any standing complaint about it
        // is now out of date, so it goes rather than waiting to be re-probed.
        if outgoing.page != nil, !pagesSupported {
            pagesSupported = true
            capabilitiesByEndpoint.removeValue(forKey: endpointText)
            macIsKnown = false
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
