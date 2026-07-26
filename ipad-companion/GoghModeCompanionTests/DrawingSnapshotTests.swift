import PencilKit
import XCTest
@testable import GoghModeCompanion

final class DrawingSnapshotTests: XCTestCase {
    private func drawing(at locations: [CGPoint]) -> PKDrawing {
        let points = locations.map { location in
            PKStrokePoint(
                location: location,
                timeOffset: 0,
                size: CGSize(width: 4, height: 4),
                opacity: 1,
                force: 0.5,
                azimuth: 0,
                altitude: 0
            )
        }
        let path = PKStrokePath(controlPoints: points, creationDate: Date(timeIntervalSince1970: 0))
        return PKDrawing(strokes: [PKStroke(ink: PKInk(.pen, color: .black), path: path)])
    }

    func testPencilPointsAreRoundedToKeepUploadsSmall() throws {
        let canvas = CGSize(width: 320, height: 240)
        let snapshot = DrawingSnapshot.fromPencilDrawing(
            drawing(at: [CGPoint(x: 10.123456789, y: 20.987654321)]),
            canvasSize: canvas
        )

        let point = try XCTUnwrap(snapshot.strokes.first?.points.first)
        XCTAssertEqual(point.x, 10.12, accuracy: 0.0001)
        XCTAssertEqual(point.y, 20.99, accuracy: 0.0001)
    }

    /// Rounding runs before clamping precisely so a value at the edge cannot be
    /// rounded up past the canvas, which the host rejects with a 400.
    func testRoundingNeverPushesPointsOutsideTheCanvas() throws {
        let canvas = CGSize(width: 320, height: 240)
        let snapshot = DrawingSnapshot.fromPencilDrawing(
            drawing(at: [CGPoint(x: 319.999, y: 239.999), CGPoint(x: -5, y: -5)]),
            canvasSize: canvas
        )

        let points = try XCTUnwrap(snapshot.strokes.first?.points)
        for point in points {
            XCTAssertGreaterThanOrEqual(point.x, 0)
            XCTAssertGreaterThanOrEqual(point.y, 0)
            XCTAssertLessThanOrEqual(point.x, snapshot.canvas.width)
            XCTAssertLessThanOrEqual(point.y, snapshot.canvas.height)
        }
    }

    func testSnapshotEncodingMatchesRustSchema() throws {
        let snapshot = DrawingSnapshot(
            schemaVersion: 1,
            page: nil,
            canvas: CanvasSize(width: 320, height: 240, background: "#ffffff"),
            strokes: [
                Stroke(
                    id: "stroke-1",
                    color: "#111827",
                    width: 4,
                    points: [Point(x: 10, y: 20, pressure: 0.5, t: 12)]
                )
            ]
        )

        let data = try JSONEncoder().encode(snapshot)
        let object = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let canvas = try XCTUnwrap(object["canvas"] as? [String: Any])
        let strokes = try XCTUnwrap(object["strokes"] as? [[String: Any]])
        let firstStroke = try XCTUnwrap(strokes.first)
        let points = try XCTUnwrap(firstStroke["points"] as? [[String: Any]])
        let firstPoint = try XCTUnwrap(points.first)

        XCTAssertEqual(object["schemaVersion"] as? Int, 1)
        XCTAssertEqual(canvas["width"] as? Double, 320)
        XCTAssertEqual(canvas["height"] as? Double, 240)
        XCTAssertEqual(canvas["background"] as? String, "#ffffff")
        XCTAssertEqual(firstStroke["id"] as? String, "stroke-1")
        XCTAssertEqual(firstStroke["color"] as? String, "#111827")
        XCTAssertEqual(firstStroke["width"] as? Double, 4)
        XCTAssertEqual(firstPoint["x"] as? Double, 10)
        XCTAssertEqual(firstPoint["y"] as? Double, 20)
        XCTAssertEqual(firstPoint["pressure"] as? Double, 0.5)
        XCTAssertEqual(firstPoint["t"] as? Int, 12)
    }

    func testEndpointNormalizesMobileRootURLToSaveURL() throws {
        let endpoint = try XCTUnwrap(GoghModeEndpoint("http://192.168.1.10:8787/abc123/"))

        XCTAssertEqual(endpoint.saveURL.absoluteString, "http://192.168.1.10:8787/abc123/save")
    }

    func testEndpointKeepsExistingSaveURL() throws {
        let endpoint = try XCTUnwrap(GoghModeEndpoint("http://192.168.1.10:8787/abc123/save"))

        XCTAssertEqual(endpoint.saveURL.absoluteString, "http://192.168.1.10:8787/abc123/save")
    }

    func testEndpointRejectsNonHttpURL() {
        XCTAssertNil(GoghModeEndpoint("file:///tmp/latest.json"))
    }

    func testEndpointExposesCapabilitiesBesideSave() throws {
        let fromRoot = try XCTUnwrap(GoghModeEndpoint("http://192.168.1.10:8787/abc123/"))
        let fromSaveURL = try XCTUnwrap(GoghModeEndpoint("http://192.168.1.10:8787/abc123/save"))

        XCTAssertEqual(
            fromRoot.capabilitiesURL.absoluteString,
            "http://192.168.1.10:8787/abc123/capabilities"
        )
        XCTAssertEqual(fromSaveURL.capabilitiesURL, fromRoot.capabilitiesURL)
        XCTAssertEqual(fromSaveURL.saveURL, fromRoot.saveURL)
    }

    func testPageSnapshotEncodesPageAtSchemaVersionTwo() throws {
        let snapshot = DrawingSnapshot.empty(
            canvasSize: CGSize(width: 320, height: 240),
            page: PageRef(id: "note-1", title: "Server sketch")
        )

        let data = try JSONEncoder().encode(snapshot)
        let object = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let page = try XCTUnwrap(object["page"] as? [String: Any])

        XCTAssertEqual(object["schemaVersion"] as? Int, 2)
        XCTAssertEqual(page["id"] as? String, "note-1")
        XCTAssertEqual(page["title"] as? String, "Server sketch")
    }

    func testDowngradedSnapshotDropsThePageForAHostThatPredatesThem() throws {
        let snapshot = DrawingSnapshot.empty(
            canvasSize: CGSize(width: 320, height: 240),
            page: PageRef(id: "note-1", title: "Server sketch")
        )

        let data = try JSONEncoder().encode(snapshot.withoutPage())
        let object = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])

        XCTAssertEqual(object["schemaVersion"] as? Int, 1)
        XCTAssertNil(object["page"])
        XCTAssertNotNil(object["canvas"])
    }

    func testCapabilitiesDecodeAndReportPageSupport() throws {
        let json = Data(#"{"schemaVersions":[1,2],"features":["pages"]}"#.utf8)

        let capabilities = try JSONDecoder().decode(GoghModeCapabilities.self, from: json)

        XCTAssertTrue(capabilities.supportsPages)
        XCTAssertFalse(GoghModeCapabilities.pagelessHost.supportsPages)
    }

    @MainActor
    func testPageStoreKeepsPagesAndSeriesAcrossReloads() throws {
        let storeURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("goghmode-pages-\(UUID().uuidString).json")
        defer { try? FileManager.default.removeItem(at: storeURL) }

        let store = PageStore(storeURL: storeURL)
        let first = store.selectedPageID
        let second = store.addPage()
        store.rename(second.id, to: "Server sketch")
        store.stack(first, onto: second.id)

        let reloaded = PageStore(storeURL: storeURL)

        XCTAssertEqual(reloaded.pages.count, 2)
        XCTAssertEqual(reloaded.series.count, 1)
        XCTAssertEqual(reloaded.pages.first { $0.id == second.id }?.title, "Server sketch")
        XCTAssertTrue(reloaded.pages.allSatisfy { $0.seriesID != nil })
    }

    @MainActor
    func testStackedSheetsNumberWithinTheirSeries() throws {
        let store = PageStore(storeURL: temporaryStoreURL())
        let first = store.selectedPageID
        let second = store.addPage()
        store.stack(first, onto: second.id)

        guard let series = store.series.first else { return XCTFail("no series created") }
        let filed = store.sheets(in: series.id)

        XCTAssertEqual(filed.count, 2)
        XCTAssertEqual(store.sheetNumber(for: filed[0]), "\(series.prefix)-01")
        XCTAssertEqual(store.sheetNumber(for: filed[1]), "\(series.prefix)-02")
    }

    @MainActor
    func testLooseSheetsAreNumberedInCreationOrder() throws {
        let store = PageStore(storeURL: temporaryStoreURL())
        let first = store.pages[0]
        let second = store.addPage()

        XCTAssertEqual(store.sheetNumber(for: first), "01")
        XCTAssertEqual(store.sheetNumber(for: second), "02")
    }

    @MainActor
    func testTakingASheetOutOfASeriesDiscardsTheEmptySeries() throws {
        let store = PageStore(storeURL: temporaryStoreURL())
        let first = store.selectedPageID
        let second = store.addPage()
        store.stack(first, onto: second.id)

        store.removeFromSeries(first)
        store.removeFromSeries(second.id)

        XCTAssertTrue(store.series.isEmpty)
        XCTAssertTrue(store.pages.allSatisfy { $0.seriesID == nil })
    }

    @MainActor
    func testTheStampIsRecordedOnlyFromWhatTheMacConfirmed() throws {
        let storeURL = temporaryStoreURL()
        let store = PageStore(storeURL: storeURL)
        let page = store.pages[0]

        XCTAssertNil(store.pinnedPageID)
        store.recordPin(page.id)

        XCTAssertEqual(PageStore(storeURL: storeURL).pinnedPageID, page.id)
    }

    @MainActor
    func testCapabilitiesDistinguishAMacWithoutTheStampRoutes() throws {
        let pagesOnly = Data(#"{"schemaVersions":[1,2],"features":["pages"]}"#.utf8)
        let full = Data(#"{"schemaVersions":[1,2],"features":["pages","pin","promote"]}"#.utf8)

        let older = try JSONDecoder().decode(GoghModeCapabilities.self, from: pagesOnly)
        let current = try JSONDecoder().decode(GoghModeCapabilities.self, from: full)

        XCTAssertTrue(older.supportsPages)
        XCTAssertFalse(older.supportsPinning)
        XCTAssertTrue(current.supportsPinning)
    }

    func testEndpointExposesPinAndPromoteBesideSave() throws {
        let endpoint = try XCTUnwrap(GoghModeEndpoint("http://192.168.1.10:8787/abc123/"))

        XCTAssertEqual(endpoint.pinURL.absoluteString, "http://192.168.1.10:8787/abc123/pin")
        XCTAssertEqual(endpoint.promoteURL.absoluteString, "http://192.168.1.10:8787/abc123/promote")
    }

    /// Why register previews came out blank: the source rect was pinned to a portrait
    /// canvas, so anything drawn on an iPad held in landscape fell outside it and was
    /// cropped away. Rendered under a dark trait too, since previews sit on paper
    /// that stays light in both appearances.
    func testPreviewShowsInkDrawnPastThePortraitCanvas() throws {
        let landscape = drawing(at: [
            CGPoint(x: 1180, y: 400),
            CGPoint(x: 1220, y: 430),
            CGPoint(x: 1260, y: 460),
            CGPoint(x: 1300, y: 490),
        ])

        var rendered: UIImage?
        UITraitCollection(userInterfaceStyle: .dark).performAsCurrent {
            rendered = SheetPreview.render(landscape)
        }

        XCTAssertTrue(carriesInk(try XCTUnwrap(rendered)))
    }

    /// The defect itself, kept as a witness: the same drawing rendered the old way —
    /// against a portrait-only source rect — produces a blank image. Without this the
    /// test above could pass for the wrong reason and the fix could be removed
    /// unnoticed.
    func testTheOldPortraitOnlySourceRectProducedABlankImage() throws {
        let landscape = drawing(at: [CGPoint(x: 1180, y: 400), CGPoint(x: 1300, y: 490)])
        let portraitOnly = landscape.image(
            from: CGRect(x: 0, y: 0, width: 1024, height: 1366),
            scale: 0.2
        )

        XCTAssertFalse(carriesInk(portraitOnly))
    }

    func testPreviewOfAnUntouchedSheetRendersEmptyPaper() throws {
        let image = SheetPreview.render(PKDrawing())

        XCTAssertGreaterThan(image.size.width, 0)
        XCTAssertFalse(carriesInk(image))
    }

    /// Dark, opaque pixels mean ink landed. Transparent or pale ones mean the
    /// preview came out blank, which is the defect this guards.
    private func carriesInk(_ image: UIImage) -> Bool {
        guard let cgImage = image.cgImage else { return false }
        let width = cgImage.width
        let height = cgImage.height
        var pixels = [UInt8](repeating: 0, count: width * height * 4)

        guard let context = CGContext(
            data: &pixels,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            return false
        }
        context.draw(cgImage, in: CGRect(x: 0, y: 0, width: width, height: height))

        return stride(from: 0, to: pixels.count, by: 4).contains { index in
            pixels[index + 3] > 40 && pixels[index] < 140
        }
    }

    /// A Mac that cannot stamp is a fact about the Mac, not a failed upload. Recording
    /// it as `.failed` made the message stick: nothing clears a failure until an
    /// upload succeeds, so "the Mac app is an older version" stayed on screen long
    /// after the Mac had been updated.
    @MainActor
    func testAnOutOfDateMacDoesNotLeaveAStickyUploadFailure() async {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [PagelessMacProtocol.self]
        let controller = UploadController(
            client: GoghModeClient(session: URLSession(configuration: configuration))
        )

        let accepted = await controller.pin("note-1", to: legacyDestination())

        XCTAssertFalse(accepted)
        XCTAssertFalse(controller.pinningSupported)
        XCTAssertTrue(controller.hostIsKnown)
        if case .failed(let message) = controller.status {
            XCTFail("a capability verdict must not read as an upload failure: \(message)")
        }
    }

    /// A probe that could not complete says nothing about the Mac. Reading it as "old
    /// Mac" cached that verdict, so one dropped request switched pages and stamping
    /// off and left the complaint on screen until the app was restarted.
    @MainActor
    func testAnUnreachableMacIsNotRecordedAsAnOldOne() async {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [UnreachableMacProtocol.self]
        let controller = UploadController(
            client: GoghModeClient(session: URLSession(configuration: configuration))
        )

        _ = await controller.pin("note-1", to: legacyDestination())

        XCTAssertTrue(controller.pagesSupported)
        XCTAssertTrue(controller.pinningSupported)
        XCTAssertFalse(controller.hostIsKnown, "a failed probe must not count as an answer")
    }

    /// A host saved the old way: the probe still decides what it can do, which is
    /// what these two tests are about. A paired host answers that question by
    /// existing, so it would not exercise the probe at all.
    private func legacyDestination(
        _ address: String = "http://10.0.0.1:8787/abc123/"
    ) -> UploadController.Destination {
        UploadController.Destination(
            host: SavedHost(
                id: "legacy-test",
                name: "Desktop",
                platform: "unknown",
                address: address,
                credential: .legacyURL(address)
            ),
            secret: nil,
            deviceID: "ipad-test"
        )
    }

    @MainActor
    private func temporaryStoreURL() -> URL {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("goghmode-pages-\(UUID().uuidString).json")
    }
}

/// A Mac that knows about pages but has no stamp routes. No mutable state, so it is
/// safe to hand to a URLSession from any test.
final class PagelessMacProtocol: URLProtocol {
    override class func canInit(with request: URLRequest) -> Bool { true }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let url = request.url,
              let response = HTTPURLResponse(
                  url: url,
                  statusCode: 200,
                  httpVersion: nil,
                  headerFields: ["Content-Type": "application/json"]
              ) else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }

        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(#"{"schemaVersions":[1,2],"features":["pages"]}"#.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

/// A Mac that cannot be reached at all — the probe fails at the network level rather
/// than answering 404.
final class UnreachableMacProtocol: URLProtocol {
    override class func canInit(with request: URLRequest) -> Bool { true }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        client?.urlProtocol(self, didFailWithError: URLError(.cannotConnectToHost))
    }

    override func stopLoading() {}
}
