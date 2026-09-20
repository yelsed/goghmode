import XCTest
@testable import GoghModeCompanion

/// Only the mapping from what the transcriber says to what goes on the wire.
/// Everything around it needs a microphone; this is the part that can be wrong
/// in a way nobody notices until the host refuses the sheet.
final class NarrationRecorderTests: XCTestCase {
    private let recordingStartedAt: UInt64 = 1_758_290_000_000

    private func segment(
        _ start: Float,
        _ end: Float,
        _ text: String
    ) -> NarrationSegment? {
        NarrationRecorder.segment(
            startingAt: start,
            endingAt: end,
            text: text,
            recordingStartedAtMilliseconds: recordingStartedAt
        )
    }

    func testSecondsIntoTheRecordingBecomeAPlaceOnTheClock() throws {
        let spoken = try XCTUnwrap(segment(1.5, 3.4, "Dit is de database."))

        XCTAssertEqual(spoken.start, recordingStartedAt + 1_500)
        XCTAssertEqual(spoken.end, recordingStartedAt + 3_400)
        XCTAssertEqual(spoken.text, "Dit is de database.")
    }

    func testSurroundingWhitespaceIsNotPartOfWhatWasSaid() throws {
        let spoken = try XCTUnwrap(segment(0, 1, "  En hier de API.\n"))

        XCTAssertEqual(spoken.text, "En hier de API.")
    }

    func testASegmentWithNoWordsInItIsDropped() {
        XCTAssertNil(segment(0, 1, "   "))
        XCTAssertNil(segment(0, 1, ""))
    }

    /// The model's own markers are not something anyone said, and a segment that
    /// holds nothing else is silence.
    func testTheModelsOwnMarkersAreNotWords() throws {
        let spoken = try XCTUnwrap(segment(0, 1, "<|nl|><|transcribe|> Dit is de database."))

        XCTAssertEqual(spoken.text, "Dit is de database.")
        XCTAssertNil(segment(0, 1, "<|startoftranscript|>"))
    }

    /// A word said at the very start of a recording is worth keeping, so a
    /// negative offset is the start rather than a reason to drop it.
    func testANegativeOffsetIsTheStartOfTheRecording() throws {
        let spoken = try XCTUnwrap(segment(-0.4, 1, "Dit is de database."))

        XCTAssertEqual(spoken.start, recordingStartedAt)
    }

    /// The host refuses a segment that ends before it starts, and one stray pair
    /// from the model must not cost the whole sheet.
    func testASegmentCanNeverEndBeforeItStarts() throws {
        let spoken = try XCTUnwrap(segment(4, 2, "Dit is de database."))

        XCTAssertEqual(spoken.start, recordingStartedAt + 4_000)
        XCTAssertEqual(spoken.end, spoken.start)
    }

    func testTheElapsedClockCountsPastTheHourRatherThanWrapping() {
        XCTAssertEqual(NarrationControl.clock(9), "00:09")
        XCTAssertEqual(NarrationControl.clock(754), "12:34")
        XCTAssertEqual(NarrationControl.clock(3_725), "62:05")
    }
}
