import PencilKit
import XCTest
@testable import GoghModeCompanion

@MainActor
final class PageStoreTests: XCTestCase {
    private var storeURL: URL!

    override func setUpWithError() throws {
        storeURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("goghmode-pages-\(UUID().uuidString).json")
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: storeURL)
    }

    private func store() -> PageStore {
        PageStore(storeURL: storeURL)
    }

    private func strokedDrawing() -> PKDrawing {
        let points = [CGPoint(x: 10, y: 10), CGPoint(x: 40, y: 40)].map { location in
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
        let path = PKStrokePath(controlPoints: points, creationDate: Date())
        return PKDrawing(strokes: [PKStroke(ink: PKInk(.pen, color: .black), path: path)])
    }

    /// The bug this exists to prevent: opening a sheet made the canvas report its
    /// own loading as an edit, and the empty canvas was written over the page.
    func testAnEmptyDrawingCannotWipeASheetThatHasStrokes() throws {
        let store = self.store()
        let pageID = store.addPage().id
        store.update(pageID, with: strokedDrawing())
        XCTAssertEqual(store.page(pageID)?.drawing.strokes.count, 1)

        store.update(pageID, with: PKDrawing())

        XCTAssertEqual(
            store.page(pageID)?.drawing.strokes.count,
            1,
            "an empty drawing must not be able to erase a sheet through the ordinary save path"
        )
    }

    func testClearErasesOnPurpose() throws {
        let store = self.store()
        let pageID = store.addPage().id
        store.update(pageID, with: strokedDrawing())

        store.clear(pageID)

        XCTAssertEqual(store.page(pageID)?.drawing.strokes.count, 0)
    }

    func testAnEmptySheetStillAcceptsItsFirstStrokes() throws {
        let store = self.store()
        let pageID = store.addPage().id

        store.update(pageID, with: strokedDrawing())

        XCTAssertEqual(store.page(pageID)?.drawing.strokes.count, 1)
    }

    func testDeleteRemovesOnlyTheNamedSheet() throws {
        let store = self.store()
        let kept = store.addPage().id
        let doomed = store.addPage().id

        store.delete(doomed)

        XCTAssertNil(store.page(doomed))
        XCTAssertNotNil(store.page(kept))
    }

    /// The register must always have something to open, the way a new install
    /// does.
    func testDeletingEverySheetLeavesAFreshOne() throws {
        let store = self.store()
        for page in store.pages {
            store.delete(page.id)
        }

        XCTAssertEqual(store.pages.count, 1)
        XCTAssertTrue(store.pages[0].isEmpty)
        XCTAssertEqual(store.selectedPageID, store.pages[0].id)
    }

    func testDeletingTheOpenSheetSelectsAnother() throws {
        let store = self.store()
        let other = store.addPage().id
        let open = store.addPage().id
        store.select(open)

        store.delete(open)

        XCTAssertNotEqual(store.selectedPageID, open)
        XCTAssertNotNil(store.page(store.selectedPageID))
        XCTAssertNotNil(store.page(other))
    }

    func testDeletionSurvivesReopeningTheStore() throws {
        let first = store()
        let doomed = first.addPage().id
        first.delete(doomed)

        let reopened = store()

        XCTAssertNil(reopened.page(doomed))
    }
}
