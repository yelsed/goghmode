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

    /// Where one drawing is going. Carrying the resolved host and its key
    /// together means a request can never be assembled from one host and
    /// another host's credential.
    struct Destination: Equatable {
        let host: SavedHost
        let secret: String?
        let deviceID: String

        var isPaired: Bool {
            host.isPaired && secret != nil
        }
    }

    @Published private(set) var status: Status = .idle

    /// False once a host has told us it predates pages. The page switcher hides
    /// itself rather than pretending a page switch means anything there.
    @Published private(set) var pagesSupported = true

    /// False on a host that understands pages but not the stamp routes.
    @Published private(set) var pinningSupported = true

    /// Whether the two flags above are an answer or a guess. Until a host has
    /// actually replied they are optimism, and a control drawn on optimism that
    /// disappears the moment it is pressed reads as the app breaking.
    @Published private(set) var hostIsKnown = false

    private let client: GoghModeClient
    private var pendingUpload: Task<Void, Never>?
    private var lastSnapshot: DrawingSnapshot?
    private var lastDestination: Destination?
    private var capabilitiesByAddress: [String: GoghModeCapabilities] = [:]

    var pagesUnsupportedMessage: String? {
        pagesSupported
            ? nil
            : "GoghMode on the desktop is an older version, so pages are off. Update it there and reopen it."
    }

    /// Names the app, not the machine. The first version said "this Mac is too
    /// old", which reads as a verdict on the hardware for something a reopen
    /// fixes.
    static let hostAppOutOfDate =
        "GoghMode on the desktop is an older version that cannot stamp sheets yet. Update it there and reopen it."

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

        pendingUpload = Task {
            do {
                try await Task.sleep(for: .milliseconds(600))
                try Task.checkCancellation()
                try await upload(snapshot, to: destination)
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
        pendingUpload = Task {
            do {
                try await upload(snapshot, to: destination)
            } catch {
                record(error)
            }
        }
    }

    /// Sends a sheet and waits for the host to have it. Stamping cannot fire and
    /// forget: the host can only mirror a page it actually holds.
    @discardableResult
    func send(_ snapshot: DrawingSnapshot, to destination: Destination) async -> Bool {
        remember(snapshot, destination)
        pendingUpload?.cancel()
        do {
            try await upload(snapshot, to: destination)
            return true
        } catch {
            record(error)
            return false
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

    /// Asks the host what it accepts before anything needs the answer, so
    /// controls are drawn in the state they will actually behave in.
    func learnWhatTheHostAccepts(_ destination: Destination?) async {
        guard let destination else { return }
        _ = await resolvedCapabilities(for: destination)
    }

    /// Forgets what a host said it accepts. Called when the destination changes,
    /// and when the app comes back — the host may have been updated in between,
    /// and a cached "too old" answer would keep the stamp switched off forever.
    func forgetWhatTheHostAccepts() {
        capabilitiesByAddress.removeAll()
        hostIsKnown = false
        pagesSupported = true
        pinningSupported = true
    }

    /// Stamps a page as the one the agent reads, or clears the stamp with `nil`.
    /// Returns whether the host accepted it, so the caller records the pin only
    /// when it is actually true on disk.
    func pin(_ pageID: String?, to destination: Destination) async -> Bool {
        await stamp(destination) {
            if destination.isPaired {
                try await client.pin(
                    pageID,
                    on: destination.host,
                    secret: destination.secret ?? "",
                    deviceID: destination.deviceID
                )
            } else if let endpoint = GoghModeEndpoint(destination.host.address) {
                try await client.pin(pageID, on: endpoint)
            } else {
                throw UploadError.invalidEndpoint
            }
        }
    }

    /// Sends one page now without moving the stamp.
    func promote(_ pageID: String, to destination: Destination) async -> Bool {
        let accepted = await stamp(destination) {
            if destination.isPaired {
                try await client.promote(
                    pageID,
                    on: destination.host,
                    secret: destination.secret ?? "",
                    deviceID: destination.deviceID
                )
            } else if let endpoint = GoghModeEndpoint(destination.host.address) {
                try await client.promote(pageID, on: endpoint)
            } else {
                throw UploadError.invalidEndpoint
            }
        }
        if accepted {
            status = .saved(Date())
        }
        return accepted
    }

    private func stamp(
        _ destination: Destination,
        _ perform: () async throws -> Void
    ) async -> Bool {
        let capabilities = await resolvedCapabilities(for: destination)
        guard capabilities.supportsPinning else {
            // Deliberately not a `.failed` status. A capability verdict is not an
            // upload failure, and nothing clears a failure until an upload
            // succeeds — so setting one here left "the desktop app is an older
            // version" on screen long after it had been updated. The register
            // reads the capabilities directly and says so for exactly as long as
            // it is true.
            return false
        }

        do {
            try await perform()
            return true
        } catch {
            record(error)
            return false
        }
    }

    private func remember(_ snapshot: DrawingSnapshot, _ destination: Destination) {
        lastSnapshot = snapshot
        lastDestination = destination
    }

    private func record(_ error: Error) {
        if let uploadError = error as? UploadError, case .wrongHost(let name) = uploadError {
            status = .wrongHost(
                "The machine answering for \(name) could not prove it is that host. Nothing was sent."
            )
            return
        }
        status = .failed(guidance(for: error))
    }

    /// One probe per address, cached.
    ///
    /// A paired host needs no probe at all: pairing only exists in the build that
    /// also has pages and the stamp routes, so asking would be asking a question
    /// whose answer is already known — and a dropped probe would then be able to
    /// switch off a control that is certainly available.
    private func resolvedCapabilities(for destination: Destination) async -> GoghModeCapabilities {
        if destination.isPaired {
            pagesSupported = true
            pinningSupported = true
            hostIsKnown = true
            return .assumeCurrent
        }

        let address = destination.host.address
        if let known = capabilitiesByAddress[address] {
            return known
        }
        guard let endpoint = GoghModeEndpoint(address),
            let capabilities = await client.capabilities(of: endpoint)
        else {
            // Unreachable, not old. Nothing is cached and nothing is concluded, so
            // the next attempt asks again instead of inheriting a guess.
            return .assumeCurrent
        }
        capabilitiesByAddress[address] = capabilities
        pagesSupported = capabilities.supportsPages
        pinningSupported = capabilities.supportsPinning
        hostIsKnown = true
        return capabilities
    }

    /// One drawing, one host. A failure never reroutes to another saved host —
    /// silently sending someone's notes to the wrong machine is worse than not
    /// sending them at all.
    private func upload(_ snapshot: DrawingSnapshot, to destination: Destination) async throws {
        let capabilities = await resolvedCapabilities(for: destination)
        let outgoing = capabilities.supportsPages ? snapshot : snapshot.withoutPage()

        status = .saving
        do {
            try await deliver(outgoing, to: destination)
        } catch let error as URLError where error.isWorthRetrying {
            // URLSession can hand back a pooled socket the host already closed,
            // which surfaces as `networkConnectionLost` even though the host is
            // reachable. One retry separates a dead socket from a dead server.
            try await Task.sleep(for: .milliseconds(300))
            try await deliver(outgoing, to: destination)
        }

        // A save that lands is the strongest evidence there is: the host is
        // reachable, and if it took a page it understands pages. Any standing
        // complaint about it is now out of date, so it goes rather than waiting
        // to be re-probed.
        if outgoing.page != nil, !pagesSupported {
            pagesSupported = true
            capabilitiesByAddress.removeValue(forKey: destination.host.address)
            hostIsKnown = false
        }
        status = .saved(Date())
    }

    private func deliver(_ snapshot: DrawingSnapshot, to destination: Destination) async throws {
        if destination.isPaired {
            try await client.upload(
                snapshot,
                to: destination.host,
                secret: destination.secret ?? "",
                deviceID: destination.deviceID
            )
            return
        }
        guard let endpoint = GoghModeEndpoint(destination.host.address) else {
            throw UploadError.invalidEndpoint
        }
        try await client.upload(snapshot, to: endpoint)
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
            "Address not found. Pair again, or fix the address."
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
