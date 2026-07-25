import XCTest
@testable import GoghModeCompanion

final class DrawingSnapshotTests: XCTestCase {
    func testSnapshotEncodingMatchesRustSchema() throws {
        let snapshot = DrawingSnapshot(
            schemaVersion: 1,
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
}
