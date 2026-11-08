import Foundation

/// A machine the hub can deliver to: allowed by the owner, and fetching.
struct HubMachine: Equatable, Identifiable {
    let name: String
    let lastFetchedAt: Date?

    var id: String { name }
}

/// What the hub said about one machine for one sheet or stamp.
struct MachineDelivery: Equatable, Identifiable {
    enum State: String, Decodable {
        case queued
        case refused
        case delivered
    }

    let machine: String
    let state: State
    /// Only on a refusal: the hub's own sentence, which names what to do.
    let reason: String?
    let confirmedAt: Date?

    var id: String { machine }
}

enum SketchHubStamp: String {
    case pin
    case promote
}

enum SketchHubError: LocalizedError, Equatable {
    /// The hub answered and said no, in a sentence meant for a person.
    case refused(String)
    case invalidAddress
    case invalidResponse

    var errorDescription: String? {
        switch self {
        case .refused(let reason):
            "The hub refused: \(reason)"
        case .invalidAddress:
            "The sketch hub address is not a usable https address."
        case .invalidResponse:
            "The sketch hub did not send a valid answer."
        }
    }
}

/// The four routes of the sketch hub in dev-portal (`docs/sketch-hub-api.md`).
/// Plain JSON over HTTPS, unsigned: the hub knows this iPad from its tailnet
/// address and the certificate says who the hub is (ADR-0011).
struct SketchHubClient {
    var session: URLSession = .shared

    func machines(at hubAddress: String) async throws -> [HubMachine] {
        struct Answer: Decodable {
            struct Machine: Decodable {
                let name: String
                let lastFetchedAt: String?
            }

            let machines: [Machine]
        }
        let answer: Answer = try await request("GET", "/hub/machines", at: hubAddress)
        return answer.machines.map { machine in
            HubMachine(name: machine.name, lastFetchedAt: machine.lastFetchedAt.flatMap(Self.date))
        }
    }

    func send(
        _ snapshot: DrawingSnapshot,
        to machines: [String],
        at hubAddress: String
    ) async throws -> [MachineDelivery] {
        struct Body: Encodable {
            let targets: [String]
            let snapshot: DrawingSnapshot
        }
        let answer: DeliveriesAnswer = try await request(
            "POST",
            "/hub/sheets",
            at: hubAddress,
            body: try JSONEncoder().encode(Body(targets: machines, snapshot: snapshot))
        )
        return answer.deliveries.map(\.delivery)
    }

    func stamp(
        _ kind: SketchHubStamp,
        pageID: String?,
        to machines: [String],
        at hubAddress: String
    ) async throws -> [MachineDelivery] {
        struct Body: Encodable {
            let targets: [String]
            let kind: String
            let pageId: String?

            // A nil page id clears the pin, so it has to be sent as null rather
            // than left out.
            func encode(to encoder: Encoder) throws {
                var container = encoder.container(keyedBy: CodingKeys.self)
                try container.encode(targets, forKey: .targets)
                try container.encode(kind, forKey: .kind)
                try container.encode(pageId, forKey: .pageId)
            }

            enum CodingKeys: String, CodingKey {
                case targets, kind, pageId
            }
        }
        let answer: DeliveriesAnswer = try await request(
            "POST",
            "/hub/stamps",
            at: hubAddress,
            body: try JSONEncoder().encode(Body(targets: machines, kind: kind.rawValue, pageId: pageID))
        )
        return answer.deliveries.map(\.delivery)
    }

    func deliveries(of pageID: String, at hubAddress: String) async throws -> [MachineDelivery] {
        let answer: DeliveriesAnswer = try await request(
            "GET",
            "/hub/deliveries",
            at: hubAddress,
            query: [URLQueryItem(name: "pageId", value: pageID)]
        )
        return answer.deliveries.map(\.delivery)
    }

    private struct DeliveriesAnswer: Decodable {
        struct Delivery: Decodable {
            let machine: String
            let state: MachineDelivery.State
            let reason: String?
            let confirmedAt: String?

            var delivery: MachineDelivery {
                MachineDelivery(
                    machine: machine,
                    state: state,
                    reason: reason,
                    confirmedAt: confirmedAt.flatMap(SketchHubClient.date)
                )
            }
        }

        let deliveries: [Delivery]
    }

    private func request<Answer: Decodable>(
        _ method: String,
        _ path: String,
        at hubAddress: String,
        query: [URLQueryItem] = [],
        body: Data? = nil
    ) async throws -> Answer {
        guard var components = URLComponents(string: hubAddress.trimmingCharacters(in: .whitespacesAndNewlines)),
              components.scheme == "https",
              components.host != nil
        else {
            throw SketchHubError.invalidAddress
        }
        components.path = path
        components.queryItems = query.isEmpty ? nil : query
        guard let url = components.url else {
            throw SketchHubError.invalidAddress
        }

        var request = URLRequest(url: url)
        request.httpMethod = method
        if let body {
            request.setValue("application/json; charset=utf-8", forHTTPHeaderField: "Content-Type")
            request.httpBody = body
        }

        let (data, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw SketchHubError.invalidResponse
        }
        guard (200..<300).contains(httpResponse.statusCode) else {
            struct Refusal: Decodable {
                let error: String
            }
            let reason = (try? JSONDecoder().decode(Refusal.self, from: data))?.error
            throw SketchHubError.refused(reason ?? "status \(httpResponse.statusCode)")
        }
        do {
            return try JSONDecoder().decode(Answer.self, from: data)
        } catch {
            throw SketchHubError.invalidResponse
        }
    }

    /// The hub writes times the way Go does: up to nine fractional digits and an
    /// offset. ISO8601DateFormatter takes neither nine digits nor none when told
    /// to expect fractions, so the fraction is cut to milliseconds first.
    static func date(_ text: String) -> Date? {
        var trimmed = text
        if let dot = text.firstIndex(of: "."),
           let zoneStart = text[dot...].firstIndex(where: { $0 == "Z" || $0 == "+" || $0 == "-" }) {
            let digits = text[text.index(after: dot)..<zoneStart].prefix(3)
            trimmed = String(text[..<dot]) + "." + digits.padding(toLength: 3, withPad: "0", startingAt: 0)
                + String(text[zoneStart...])
            let formatter = ISO8601DateFormatter()
            formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
            return formatter.date(from: trimmed)
        }
        return ISO8601DateFormatter().date(from: trimmed)
    }
}
