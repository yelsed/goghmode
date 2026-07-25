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

    private let client: GoghModeClient
    private var pendingUpload: Task<Void, Never>?

    init(client: GoghModeClient = GoghModeClient()) {
        self.client = client
    }

    func schedule(snapshot: DrawingSnapshot, endpointText: String) {
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
                status = .failed(error.localizedDescription)
            }
        }
    }

    func uploadNow(snapshot: DrawingSnapshot, endpointText: String) {
        pendingUpload?.cancel()
        pendingUpload = Task { [client] in
            do {
                try await upload(snapshot, endpointText: endpointText, client: client)
            } catch {
                status = .failed(error.localizedDescription)
            }
        }
    }

    private func upload(_ snapshot: DrawingSnapshot, endpointText: String, client: GoghModeClient) async throws {
        guard let endpoint = GoghModeEndpoint(endpointText) else {
            throw UploadError.invalidEndpoint
        }

        status = .saving
        try await client.upload(snapshot, to: endpoint)
        status = .saved(Date())
    }
}
