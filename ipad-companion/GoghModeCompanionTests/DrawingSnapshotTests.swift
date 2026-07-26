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
    /// rounded up past the canvas, which the Mac rejects with a 400.
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

    func testDowngradedSnapshotDropsThePageForAMacThatPredatesThem() throws {
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
        XCTAssertFalse(GoghModeCapabilities.pagelessMac.supportsPages)
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

    @MainActor
    private func temporaryStoreURL() -> URL {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("goghmode-pages-\(UUID().uuidString).json")
    }
}
