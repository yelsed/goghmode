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

@MainActor
final class SheetHistoryTests: XCTestCase {
    private var storeURL: URL!

    override func setUpWithError() throws {
        storeURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("goghmode-pages-\(UUID().uuidString).json")
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: storeURL)
        try? FileManager.default.removeItem(
            at: storeURL.deletingPathExtension().appendingPathExtension("revisions")
        )
    }

    private func store() -> PageStore {
        PageStore(storeURL: storeURL)
    }

    private func drawing(strokes count: Int) -> PKDrawing {
        let strokes = (0..<count).map { index -> PKStroke in
            let points = [CGPoint(x: index, y: 0), CGPoint(x: index, y: 10)].map { location in
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
            return PKStroke(
                ink: PKInk(.pen, color: .black),
                path: PKStrokePath(controlPoints: points, creationDate: Date())
            )
        }
        return PKDrawing(strokes: strokes)
    }

    /// The whole point of the sidecar: the canvas's own undo stack dies when the
    /// sheet is closed, because reopening builds a fresh canvas.
    func testASheetCanBeSteppedBackThroughAfterItHasBeenReopened() throws {
        let pageID: String
        do {
            let first = store()
            pageID = first.addPage().id
            first.select(pageID)
            for count in 1...3 {
                first.update(pageID, with: drawing(strokes: count))
                first.recordRevision(pageID, drawing(strokes: count))
            }
            // History is written when a sheet is put down, not on every stroke.
            first.flushRevisions()
        }

        let reopened = store()
        reopened.select(pageID)
        XCTAssertTrue(reopened.canStepBack, "a reopened sheet lost its history")

        XCTAssertEqual(reopened.stepBack(pageID)?.strokes.count, 2)
        XCTAssertEqual(reopened.stepBack(pageID)?.strokes.count, 1)
        // The blank sheet the page started out as is a state like any other, so a
        // first stroke can be taken back too.
        XCTAssertEqual(reopened.stepBack(pageID)?.strokes.count, 0)
        XCTAssertFalse(reopened.canStepBack, "stepping past the first state")
    }

    func testSteppingForwardReturnsWhatWasSteppedBackFrom() throws {
        let store = self.store()
        let pageID = store.addPage().id
        store.select(pageID)
        store.recordRevision(pageID, drawing(strokes: 1))
        store.recordRevision(pageID, drawing(strokes: 2))

        XCTAssertEqual(store.stepBack(pageID)?.strokes.count, 1)
        XCTAssertEqual(store.stepForward(pageID)?.strokes.count, 2)
        XCTAssertFalse(store.canStepForward)
    }

    /// Undo has to be able to take a sheet back to empty, which is the one thing
    /// the ordinary save path refuses.
    func testRestoreCanEmptyASheetWhereAnOrdinarySaveCannot() throws {
        let store = self.store()
        let pageID = store.addPage().id
        store.update(pageID, with: drawing(strokes: 2))

        store.update(pageID, with: PKDrawing())
        XCTAssertEqual(store.page(pageID)?.drawing.strokes.count, 2)

        store.restore(pageID, to: PKDrawing())
        XCTAssertEqual(store.page(pageID)?.drawing.strokes.count, 0)
    }

    func testHistoryKeepsOnlyTheMostRecentStates() throws {
        let store = self.store()
        let pageID = store.addPage().id
        store.select(pageID)
        for count in 1...30 {
            store.recordRevision(pageID, drawing(strokes: count))
        }

        var stepsTaken = 0
        while store.stepBack(pageID) != nil {
            stepsTaken += 1
        }
        XCTAssertEqual(stepsTaken, 19, "twenty states means nineteen steps back")
    }

    func testDrawingAfterSteppingBackAbandonsWhatWasAhead() throws {
        let store = self.store()
        let pageID = store.addPage().id
        store.select(pageID)
        store.recordRevision(pageID, drawing(strokes: 1))
        store.recordRevision(pageID, drawing(strokes: 2))
        _ = store.stepBack(pageID)

        store.recordRevision(pageID, drawing(strokes: 7))

        XCTAssertFalse(store.canStepForward, "the abandoned state is still reachable")
        XCTAssertEqual(store.stepBack(pageID)?.strokes.count, 1)
    }

    func testDeletingASheetTakesItsHistoryWithIt() throws {
        let store = self.store()
        let pageID = store.addPage().id
        store.select(pageID)
        store.recordRevision(pageID, drawing(strokes: 1))
        store.flushRevisions()

        let sidecar = storeURL
            .deletingPathExtension()
            .appendingPathExtension("revisions")
            .appendingPathComponent("\(pageID).json")
        XCTAssertTrue(FileManager.default.fileExists(atPath: sidecar.path))

        store.delete(pageID)

        XCTAssertFalse(FileManager.default.fileExists(atPath: sidecar.path))
    }

    /// The page is saved on every stroke; its history is not, because a trail is
    /// twenty drawings and writing it that often costs far more than the page does.
    func testHistoryIsNotWrittenUntilTheSheetIsPutDown() throws {
        let store = self.store()
        let pageID = store.addPage().id
        store.select(pageID)
        store.recordRevision(pageID, drawing(strokes: 1))

        let sidecar = storeURL
            .deletingPathExtension()
            .appendingPathExtension("revisions")
            .appendingPathComponent("\(pageID).json")
        XCTAssertFalse(FileManager.default.fileExists(atPath: sidecar.path))
        XCTAssertTrue(store.canStepBack, "stepping back must work before anything is written")

        store.flushRevisions()

        XCTAssertTrue(FileManager.default.fileExists(atPath: sidecar.path))
    }

    func testASheetIsPlainUntilARulingIsChosenAndThenRemembersIt() throws {
        let pageID: String
        do {
            let first = store()
            pageID = first.addPage().id
            XCTAssertNil(first.page(pageID)?.ruling, "a new sheet should be plain")
            first.setRuling(SheetRuling(style: .grid), on: pageID)
        }

        let reopened = store()
        XCTAssertEqual(reopened.page(pageID)?.ruling?.style, .grid)
        XCTAssertEqual(reopened.page(pageID)?.ruling?.spacing, SheetRuling.defaultSpacing)
    }
}
