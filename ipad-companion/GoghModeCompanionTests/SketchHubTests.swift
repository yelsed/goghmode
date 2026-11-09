import XCTest
@testable import GoghModeCompanion

/// Answers every request with whatever the running test set, and keeps what it was
/// asked, body included. Tests in this class run one at a time, so the shared state
/// is never touched from two places at once.
final class RecordingHubProtocol: URLProtocol {
    struct Answer {
        let status: Int
        let body: String
    }

    static var answer = Answer(status: 200, body: "{}")
    static var requests: [(request: URLRequest, body: Data)] = []

    static func reset(status: Int = 200, body: String = "{}") {
        answer = Answer(status: status, body: body)
        requests = []
    }

    static func session() -> URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [RecordingHubProtocol.self]
        return URLSession(configuration: configuration)
    }

    override class func canInit(with request: URLRequest) -> Bool { true }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        RecordingHubProtocol.requests.append((request, Self.readBody(of: request)))
        guard let url = request.url,
              let response = HTTPURLResponse(
                  url: url,
                  statusCode: Self.answer.status,
                  httpVersion: nil,
                  headerFields: ["Content-Type": "application/json"]
              ) else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(Self.answer.body.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}

    /// URLSession hands a protocol the body as a stream, never as `httpBody`.
    private static func readBody(of request: URLRequest) -> Data {
        if let body = request.httpBody {
            return body
        }
        guard let stream = request.httpBodyStream else { return Data() }
        stream.open()
        defer { stream.close() }
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while stream.hasBytesAvailable {
            let count = stream.read(&buffer, maxLength: buffer.count)
            if count <= 0 { break }
            data.append(buffer, count: count)
        }
        return data
    }
}

final class SketchHubTests: XCTestCase {
    private let hubAddress = "https://paperclip.procrast.nl:8441"

    override func setUp() {
        RecordingHubProtocol.reset()
    }

    private func client() -> SketchHubClient {
        SketchHubClient(session: RecordingHubProtocol.session())
    }

    private func sheet(_ pageID: String = "page-1") -> DrawingSnapshot {
        DrawingSnapshot(
            schemaVersion: currentSchemaVersion,
            page: PageRef(id: pageID, title: "Hub flow"),
            canvas: CanvasSize(width: 1024, height: 1366, background: "#ffffff"),
            strokes: []
        )
    }

    private func requestJSON(_ index: Int = 0) throws -> [String: Any] {
        let body = RecordingHubProtocol.requests[index].body
        return try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
    }

    // MARK: The hub client

    /// The hub writes times the way Go does, with nanoseconds and an offset. A
    /// formatter that only knows milliseconds would drop every machine.
    func testMachinesAreReadFromTheHubWithItsOwnTimes() async throws {
        RecordingHubProtocol.reset(
            body: #"{"machines":[{"name":"paperclip","lastFetchedAt":"2026-11-07T21:56:59.110770634+01:00"}]}"#
        )

        let machines = try await client().machines(at: hubAddress)

        XCTAssertEqual(RecordingHubProtocol.requests.first?.request.url?.path, "/hub/machines")
        XCTAssertEqual(machines.map(\.name), ["paperclip"])
        let expected = ISO8601DateFormatter().date(from: "2026-11-07T20:56:59Z")!
        XCTAssertEqual(
            try XCTUnwrap(machines.first?.lastFetchedAt).timeIntervalSince1970,
            expected.timeIntervalSince1970,
            accuracy: 1
        )
    }

    func testASheetIsPostedOnceWithTheTickedMachinesAndTheSnapshotAHostGets() async throws {
        RecordingHubProtocol.reset(
            body: #"{"deliveries":[{"machine":"paperclip","state":"queued"},{"machine":"omarchy","state":"refused","reason":"omarchy is not an allowed machine"}]}"#
        )
        let snapshot = sheet()

        let deliveries = try await client().send(snapshot, to: ["paperclip", "omarchy"], at: hubAddress)

        XCTAssertEqual(RecordingHubProtocol.requests.count, 1)
        let request = RecordingHubProtocol.requests[0].request
        XCTAssertEqual(request.httpMethod, "POST")
        XCTAssertEqual(request.url?.path, "/hub/sheets")
        let body = try requestJSON()
        XCTAssertEqual(body["targets"] as? [String], ["paperclip", "omarchy"])
        let sent = try JSONSerialization.data(withJSONObject: try XCTUnwrap(body["snapshot"]))
        XCTAssertEqual(try JSONDecoder().decode(DrawingSnapshot.self, from: sent), snapshot)
        XCTAssertEqual(deliveries.map(\.state), [.queued, .refused])
        XCTAssertEqual(deliveries[1].reason, "omarchy is not an allowed machine")
    }

    func testAStampIsPostedWithItsKindPageAndMachines() async throws {
        RecordingHubProtocol.reset(body: #"{"deliveries":[{"machine":"paperclip","state":"queued"}]}"#)

        _ = try await client().stamp(.pin, pageID: nil, to: ["paperclip"], at: hubAddress)

        XCTAssertEqual(RecordingHubProtocol.requests[0].request.url?.path, "/hub/stamps")
        let body = try requestJSON()
        XCTAssertEqual(body["kind"] as? String, "pin")
        XCTAssertTrue(body["pageId"] is NSNull, "clearing the pin sends pageId null")
        XCTAssertEqual(body["targets"] as? [String], ["paperclip"])
    }

    func testARefusalCarriesTheHubsOwnSentence() async {
        RecordingHubProtocol.reset(
            status: 403,
            body: #"{"error":"ipad148 is not allowed on the sketch hub; allow it on the dev-portal page"}"#
        )

        do {
            _ = try await client().send(sheet(), to: ["paperclip"], at: hubAddress)
            XCTFail("a 403 must throw")
        } catch let SketchHubError.refused(reason) {
            XCTAssertEqual(reason, "ipad148 is not allowed on the sketch hub; allow it on the dev-portal page")
        } catch {
            XCTFail("unexpected \(error)")
        }
    }

    func testDeliveriesAreReadPerMachineForOnePage() async throws {
        RecordingHubProtocol.reset(
            body: #"{"deliveries":[{"machine":"macbook","state":"queued"},{"machine":"paperclip","state":"delivered","confirmedAt":"2026-11-07T21:56:59.110770634+01:00"}]}"#
        )

        let deliveries = try await client().deliveries(of: "page-1", at: hubAddress)

        let url = try XCTUnwrap(RecordingHubProtocol.requests.first?.request.url)
        XCTAssertEqual(url.path, "/hub/deliveries")
        XCTAssertEqual(URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first?.value, "page-1")
        XCTAssertEqual(deliveries.map(\.state), [.queued, .delivered])
        XCTAssertNotNil(deliveries[1].confirmedAt)
    }

    // MARK: The machine selection

    private func freshDefaults() -> UserDefaults {
        let name = "goghmode-hub-tests-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defaults.removePersistentDomain(forName: name)
        return defaults
    }

    @MainActor
    func testTheTickedMachinesAreRememberedAcrossLaunches() {
        let defaults = freshDefaults()
        let selection = MachineSelection(defaults: defaults)
        selection.toggle("paperclip")
        selection.toggle("omarchy")
        selection.toggle("omarchy")

        XCTAssertEqual(MachineSelection(defaults: defaults).ticked, ["paperclip"])
    }

    @MainActor
    func testATickedMachineTheHubNoLongerListsStaysVisibleAsUnavailable() {
        let selection = MachineSelection(defaults: freshDefaults())
        selection.toggle("omarchy")
        let offered = [HubMachine(name: "paperclip", lastFetchedAt: Date())]

        let rows = selection.rows(offered: offered)

        XCTAssertEqual(rows.map(\.name), ["omarchy", "paperclip"])
        XCTAssertEqual(rows.map(\.isTicked), [true, false])
        XCTAssertEqual(rows.map(\.isOffered), [false, true])
    }

    // MARK: The hub as a destination

    @MainActor
    func testTheHubIsSavedListedAndChosenLikeAHost() {
        let defaults = freshDefaults()
        let store = HostStore(defaults: defaults)

        store.addSketchHub(address: hubAddress)

        let reloaded = HostStore(defaults: defaults)
        XCTAssertEqual(reloaded.selectedHost?.address, hubAddress)
        XCTAssertTrue(reloaded.selectedHost?.isSketchHub ?? false)
        XCTAssertFalse(reloaded.selectedHost?.isPaired ?? true)
    }

    @MainActor
    private func hubDestination() -> UploadController.Destination {
        UploadController.Destination(
            host: SavedHost.sketchHub(address: hubAddress),
            secret: nil,
            deviceID: "ipad-test"
        )
    }

    @MainActor
    func testWithNothingTickedNothingIsSentAndTheAppSaysToTickAMachine() async {
        let controller = UploadController(
            hubClient: SketchHubClient(session: RecordingHubProtocol.session()),
            machineSelection: MachineSelection(defaults: freshDefaults())
        )

        let sent = await controller.send(sheet(), to: hubDestination())

        XCTAssertFalse(sent)
        XCTAssertTrue(RecordingHubProtocol.requests.isEmpty)
        XCTAssertEqual(controller.complaint, MachineSelection.nothingTickedMessage)
        if case .failed = controller.status {
            XCTFail("nothing ticked is not a failed upload")
        }
    }

    @MainActor
    func testAHubSheetRecordsWhatTheHubSaidPerMachine() async {
        RecordingHubProtocol.reset(body: #"{"deliveries":[{"machine":"paperclip","state":"queued"}]}"#)
        let selection = MachineSelection(defaults: freshDefaults())
        selection.toggle("paperclip")
        let controller = UploadController(
            hubClient: SketchHubClient(session: RecordingHubProtocol.session()),
            machineSelection: selection
        )

        let sent = await controller.send(sheet("page-7"), to: hubDestination())

        XCTAssertTrue(sent)
        XCTAssertEqual(controller.hubDeliveries["page-7"]?.map(\.machine), ["paperclip"])
        XCTAssertEqual(controller.hubDeliveries["page-7"]?.first?.state, .queued)
        if case .saved = controller.status {} else {
            XCTFail("a sheet the hub took is saved, got \(controller.status)")
        }
    }

    @MainActor
    func testARefusedIPadReadsRefusedWithTheHubsSentence() async {
        RecordingHubProtocol.reset(
            status: 403,
            body: #"{"error":"ipad148 is not allowed on the sketch hub; allow it on the dev-portal page"}"#
        )
        let selection = MachineSelection(defaults: freshDefaults())
        selection.toggle("paperclip")
        let controller = UploadController(
            hubClient: SketchHubClient(session: RecordingHubProtocol.session()),
            machineSelection: selection
        )

        _ = await controller.send(sheet(), to: hubDestination())

        XCTAssertEqual(controller.status.label, "Refused")
        XCTAssertEqual(
            controller.complaint,
            "The hub refused: ipad148 is not allowed on the sketch hub; allow it on the dev-portal page"
        )
    }

    @MainActor
    func testAPinThroughTheHubGoesToTheTickedMachines() async throws {
        RecordingHubProtocol.reset(body: #"{"deliveries":[{"machine":"paperclip","state":"queued"}]}"#)
        let selection = MachineSelection(defaults: freshDefaults())
        selection.toggle("paperclip")
        let controller = UploadController(
            hubClient: SketchHubClient(session: RecordingHubProtocol.session()),
            machineSelection: selection
        )

        let accepted = await controller.pin("page-7", to: hubDestination())

        XCTAssertTrue(accepted)
        XCTAssertEqual(RecordingHubProtocol.requests.map { $0.request.url?.path }, ["/hub/stamps"])
        XCTAssertEqual(try requestJSON()["pageId"] as? String, "page-7")
    }

    @MainActor
    private func controllerTicking(_ machines: [String]) -> UploadController {
        let selection = MachineSelection(defaults: freshDefaults())
        machines.forEach(selection.toggle)
        return UploadController(
            hubClient: SketchHubClient(session: RecordingHubProtocol.session()),
            machineSelection: selection
        )
    }

    /// The hub answers 200 even when it queued for nobody; that is not a save.
    @MainActor
    func testASheetEveryMachineRefusedIsNotReportedAsSaved() async {
        RecordingHubProtocol.reset(
            body: #"{"deliveries":[{"machine":"omarchy","state":"refused","reason":"the queue for omarchy is full"}]}"#
        )
        let controller = controllerTicking(["omarchy"])

        let sent = await controller.send(sheet("page-7"), to: hubDestination())

        XCTAssertFalse(sent)
        XCTAssertEqual(controller.status.label, "Refused")
        XCTAssertEqual(controller.complaint, "The hub refused: the queue for omarchy is full")
        XCTAssertEqual(controller.hubDeliveries["page-7"]?.first?.state, .refused)
    }

    @MainActor
    func testAPinEveryMachineRefusedIsNotAccepted() async {
        RecordingHubProtocol.reset(
            body: #"{"deliveries":[{"machine":"omarchy","state":"refused","reason":"omarchy is not an allowed machine"}]}"#
        )
        let controller = controllerTicking(["omarchy"])

        let accepted = await controller.pin("page-7", to: hubDestination())

        XCTAssertFalse(accepted)
    }

    /// The deliveries route lists only machines the page was queued for, so a
    /// refusal from the send answer has to survive a refresh.
    @MainActor
    func testARefreshKeepsTheRefusalsFromTheSendAnswer() async {
        RecordingHubProtocol.reset(
            body: #"{"deliveries":[{"machine":"paperclip","state":"queued"},{"machine":"omarchy","state":"refused","reason":"the queue for omarchy is full"}]}"#
        )
        let controller = controllerTicking(["omarchy", "paperclip"])
        _ = await controller.send(sheet("page-7"), to: hubDestination())

        RecordingHubProtocol.reset(
            body: #"{"deliveries":[{"machine":"paperclip","state":"delivered","confirmedAt":"2026-11-08T10:00:00Z"}]}"#
        )
        await controller.refreshDeliveries(of: "page-7", from: hubDestination())

        let deliveries = controller.hubDeliveries["page-7"] ?? []
        XCTAssertEqual(deliveries.first { $0.machine == "paperclip" }?.state, .delivered)
        XCTAssertEqual(deliveries.first { $0.machine == "omarchy" }?.reason, "the queue for omarchy is full")
    }

    /// Drawing with nothing ticked keeps the sheet; ticking a machine sends it.
    @MainActor
    func testTickingAMachineSendsTheSheetThatWasWaiting() async throws {
        let selection = MachineSelection(defaults: freshDefaults())
        let controller = UploadController(
            hubClient: SketchHubClient(session: RecordingHubProtocol.session()),
            machineSelection: selection
        )
        _ = await controller.send(sheet("page-7"), to: hubDestination())
        XCTAssertTrue(RecordingHubProtocol.requests.isEmpty)

        RecordingHubProtocol.reset(body: #"{"deliveries":[{"machine":"paperclip","state":"queued"}]}"#)
        selection.toggle("paperclip")
        await controller.sendWhatWasWaitingForAMachine()

        XCTAssertEqual(RecordingHubProtocol.requests.map { $0.request.url?.path }, ["/hub/sheets"])
        XCTAssertNil(controller.complaint)
    }
}
