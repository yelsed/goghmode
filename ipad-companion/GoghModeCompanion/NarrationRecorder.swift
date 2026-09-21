import AVFoundation
import Foundation
import SwiftUI
import UIKit
import WhisperKit

/// The transcriber itself, loaded once per app run rather than once per sheet.
/// The model is 626 MB to fetch and several seconds to bring up, and the second
/// sheet of an afternoon must not pay for either again.
///
/// File-private on purpose: WhisperKit's types stop here, so nothing else in the
/// app — or in the test target that imports it — has to know the transcriber
/// exists.
/// What the model is doing before it can listen, so the control can say which.
private enum ModelPreparation: Equatable {
    case downloading(Double)
    /// Core ML compiling the model for the Neural Engine: minutes the first time,
    /// seconds after, and nothing to measure in between.
    case loading
}

@MainActor
private final class NarrationModel {
    static let shared = NarrationModel()

    /// The large model, which is the only size worth transcribing Dutch with —
    /// the small ones produce text that reads like a different language. Where an
    /// iPad cannot run it, WhisperKit's own recommendation stands in.
    private static let preferredVariant = "openai_whisper-large-v3-v20240930_626MB"

    private var loaded: WhisperKit?
    private var loading: Task<WhisperKit, Error>?
    private(set) var variant: String?

    /// Whether the next request is answered at once or has to bring the model up.
    var isLoaded: Bool {
        loaded != nil
    }

    /// Named on the wire so a sheet can say what read it back, and so a sheet
    /// transcribed by one model is never credited to a later one.
    var engineName: String? {
        variant.map { "whisperkit/\($0)" }
    }

    func whisperKit(
        reporting report: @escaping @Sendable (ModelPreparation) -> Void
    ) async throws -> WhisperKit {
        if let loaded { return loaded }
        if let loading { return try await loading.value }

        let task = Task { () throws -> WhisperKit in
            let recommended = WhisperKit.recommendedModels()
            let chosen = recommended.supported.contains(NarrationModel.preferredVariant)
                ? NarrationModel.preferredVariant
                : recommended.default

            let folder = try await WhisperKit.download(variant: chosen) { downloading in
                report(.downloading(downloading.fractionCompleted))
            }
            // The download is over but the wait is not: a percentage that sits on
            // 100 reads as stuck, so the control is told this is a different wait.
            report(.loading)
            // Loads the models itself when it is given a folder, so calling
            // `loadModels()` after this would bring Core ML up a second time.
            let whisperKit = try await WhisperKit(WhisperKitConfig(modelFolder: folder.path))
            self.variant = chosen
            return whisperKit
        }

        loading = task
        defer { loading = nil }
        let whisperKit = try await task.value
        loaded = whisperKit
        return whisperKit
    }
}

/// Recording what is said over one sheet, and turning it into text on the iPad.
///
/// Nothing here reaches the network: the audio stays in the app container and
/// only the words travel, which is the whole reason the transcription runs on
/// the device rather than somewhere cheaper.
@MainActor
final class NarrationRecorder: ObservableObject {
    enum State: Equatable {
        case idle
        /// A recording has stopped and its words wait for the model, which is
        /// still downloading, with how far along. Never before a recording: a tap
        /// means record now, and the model is fetched while the person talks.
        case preparingModel(Double)
        /// Downloaded, and being compiled for the Neural Engine before it can read
        /// what was said. Minutes the first time; there is no fraction to show, so
        /// the control shows motion instead.
        case loadingModel
        case recording(since: Date)
        case transcribing
        case failed(String)
    }

    /// What one recording produced, and how long the microphone was open for it.
    /// The duration is what lets the control show a sheet's total and count on
    /// from it, so a second recording visibly adds to the first.
    struct Recording: Equatable {
        var segments: [NarrationSegment]
        var duration: TimeInterval

        static let nothing = Recording(segments: [], duration: 0)
    }

    @Published private(set) var state: State = .idle
    /// The last thing that went wrong, read by the sheet's notice line. Kept
    /// apart from `state` for the same reason the uploader keeps its complaint
    /// apart from its status: a sentence that blinks out is a sentence nobody reads.
    @Published private(set) var complaint: String?
    /// A sentence about a wait that is expected, for the sheet's notice line, so
    /// a button that has stopped moving is not read as a button that is stuck.
    @Published private(set) var hint: String?

    var isRecording: Bool {
        if case .recording = state { return true }
        return false
    }

    var engineName: String? {
        NarrationModel.shared.engineName
    }

    private var whisperKit: WhisperKit?
    /// Listens on its own, without the model: a tap has to start recording at
    /// once, and the model can take minutes to arrive the first time.
    private let audioProcessor = AudioProcessor()
    private var audioFile: AVAudioFile?
    private var recordingStartedAtMilliseconds: UInt64 = 0
    private var pageID = ""
    private var backgroundTask: UIBackgroundTaskIdentifier = .invalid

    private static var decodingOptions: DecodingOptions {
        DecodingOptions(
            task: .transcribe,
            language: narrationLanguage,
            temperature: 0,
            // Off by default, and what it leaves in the text is the model's own
            // `<|…|>` markers rather than anything anyone said.
            skipSpecialTokens: true,
            chunkingStrategy: .vad
        )
    }

    /// Starts listening over the named sheet at once. A tap means record now;
    /// the model is fetched and warmed in the background while the person talks,
    /// and the words are read when the recording stops. The first version made
    /// the tap wait for the model, which took minutes the first time and then
    /// started recording on its own once nobody was watching.
    func start(for pageID: String) async {
        switch state {
        case .idle, .failed: break
        case .preparingModel, .loadingModel, .recording, .transcribing: return
        }

        self.pageID = pageID
        complaint = nil

        guard await AudioProcessor.requestRecordPermission() else {
            fail(
                "GoghMode may not use the microphone, so nothing was recorded. Allow it in Settings."
            )
            return
        }

        do {
            let startedAt = Date()
            recordingStartedAtMilliseconds = UInt64(max(0, startedAt.timeIntervalSince1970 * 1000))
            audioFile = try makeAudioFile(for: pageID, startedAt: recordingStartedAtMilliseconds)

            // Configures the audio session itself, so nothing here touches
            // `AVAudioSession` and the two cannot disagree about the category.
            // Empties the sample buffer as it starts, so a recorder outliving one
            // sheet does not carry the previous sheet's words into the next.
            try audioProcessor.startRecordingLive { [weak self] samples in
                Task { @MainActor in self?.append(samples) }
            }
            state = .recording(since: startedAt)
        } catch {
            fail("Recording could not start: \(error.localizedDescription)")
            return
        }

        if !NarrationModel.shared.isLoaded {
            hint = "Recording. The speech model is still being prepared in the background; what you say is kept and read when you stop."
            warmModel()
        }
    }

    /// Brings the model up while the recording runs, so stopping rarely has to
    /// wait. A failure is not reported from here: the audio is on disk, and
    /// stopping asks for the model again and reports then.
    private func warmModel() {
        Task { _ = try? await NarrationModel.shared.whisperKit { _ in } }
    }

    /// Stops, transcribes, and hands back what was said.
    ///
    /// Held open by a background task, because the ordinary way to finish a
    /// recording is to walk away from the sheet — and a sentence lost to that is
    /// the sentence explaining the drawing.
    @discardableResult
    func stop() async -> Recording {
        guard case .recording(let since) = state else { return .nothing }

        audioProcessor.stopRecording()
        let recorded = Array(audioProcessor.audioSamples)
        let startedAtMilliseconds = recordingStartedAtMilliseconds
        let duration = Date().timeIntervalSince(since)
        state = .transcribing
        hint = NarrationModel.shared.isLoaded
            ? nil
            : "Getting the speech model ready to read what you said. The first time this takes a few minutes; nothing is lost while it does."

        beginBackgroundTask()
        defer { endBackgroundTask() }

        // Buffers already queued for the file are written before this line runs,
        // so the last moment of a recording reaches the disk and not only the
        // transcription. Closing the file any earlier drops it.
        await Task.yield()
        audioFile = nil

        do {
            let spoken = try await transcribe(
                recorded,
                startedAtMilliseconds: startedAtMilliseconds
            )
            hint = nil
            state = .idle
            return Recording(segments: spoken, duration: duration)
        } catch {
            fail("What you said could not be turned into text: \(error.localizedDescription)")
            return .nothing
        }
    }

    /// Picks up recordings whose transcription never finished — the app was
    /// killed, or the sheet was closed while the words were still being read.
    /// Called when a sheet is opened, which is the one moment the result is
    /// certainly wanted.
    func transcribePendingAudio(for pageID: String) async -> Recording {
        guard case .idle = state else { return .nothing }
        let pending = NarrationAudioStore.recordingsAwaitingTranscription(for: pageID)
        guard !pending.isEmpty else { return .nothing }

        self.pageID = pageID
        state = .transcribing
        beginBackgroundTask()
        defer { endBackgroundTask() }

        var recovered = Recording.nothing
        do {
            for audioURL in pending {
                guard let startedAtMilliseconds = UInt64(
                    audioURL.deletingPathExtension().lastPathComponent
                ) else { continue }

                let samples = try AudioProcessor.loadAudioAsFloatArray(fromPath: audioURL.path)
                recovered.segments += try await transcribe(
                    samples,
                    startedAtMilliseconds: startedAtMilliseconds
                )
                recovered.duration += Double(samples.count) / Double(WhisperKit.sampleRate)
            }
            state = .idle
        } catch {
            fail(
                "What you said earlier could not be turned into text: \(error.localizedDescription)"
            )
        }
        return recovered
    }

    /// Seconds from the start of a recording become a place on the iPad's own
    /// clock, which is the one clock the host sorts ink and words by.
    ///
    /// Pure on purpose: everything above it needs a microphone, and this is the
    /// part that can be wrong in a way nobody notices. Not tied to the main actor,
    /// so a test can call it from wherever it runs.
    nonisolated static func segment(
        startingAt start: Float,
        endingAt end: Float,
        text: String,
        recordingStartedAtMilliseconds: UInt64
    ) -> NarrationSegment? {
        // WhisperKit hands back the decoded text, which can still carry the
        // model's own `<|…|>` markers. Nobody said those.
        let spoken = text
            .replacingOccurrences(of: "<\\|[^|]*\\|>", with: "", options: .regularExpression)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        // Whisper writes a run of asterisks or dots for a stretch it heard nothing
        // in. Nobody said that, and it must not leave the device as if someone had.
        guard spoken.rangeOfCharacter(from: .alphanumerics) != nil else { return nil }

        let from = recordingStartedAtMilliseconds + milliseconds(from: start)
        let until = recordingStartedAtMilliseconds + milliseconds(from: end)
        // The host refuses a segment that ends before it starts, and a model that
        // hands back a stray pair like that should cost one word's timing, not
        // the whole sheet.
        return NarrationSegment(start: from, end: max(from, until), text: spoken)
    }

    /// A negative or unusable offset is the start of the recording rather than a
    /// reason to drop what was said there.
    private nonisolated static func milliseconds(from seconds: Float) -> UInt64 {
        guard seconds.isFinite, seconds > 0 else { return 0 }
        return UInt64(Double(seconds) * 1000)
    }

    private func transcribe(
        _ samples: [Float],
        startedAtMilliseconds: UInt64
    ) async throws -> [NarrationSegment] {
        let results = try await whisperKitForTranscription().transcribe(
            audioArray: samples,
            decodeOptions: NarrationRecorder.decodingOptions
        )
        let spoken = results.flatMap(\.segments).compactMap {
            NarrationRecorder.segment(
                startingAt: $0.start,
                endingAt: $0.end,
                text: $0.text,
                recordingStartedAtMilliseconds: startedAtMilliseconds
            )
        }
        writeSidecar(spoken, startedAtMilliseconds: startedAtMilliseconds)
        return spoken
    }

    private func whisperKitForTranscription() async throws -> WhisperKit {
        if let whisperKit { return whisperKit }
        let loaded = try await NarrationModel.shared.whisperKit { [weak self] preparation in
            Task { @MainActor in
                switch preparation {
                case .downloading(let fraction): self?.state = .preparingModel(fraction)
                case .loading: self?.state = .loadingModel
                }
            }
        }
        whisperKit = loaded
        state = .transcribing
        return loaded
    }

    /// Marks a recording as read. Written even when nothing was said, or the same
    /// silence would be transcribed again every time the sheet is opened.
    private func writeSidecar(_ segments: [NarrationSegment], startedAtMilliseconds: UInt64) {
        let audioURL = NarrationAudioStore.audioURL(
            for: pageID,
            startedAtMilliseconds: startedAtMilliseconds
        )
        guard let data = try? JSONEncoder().encode(segments) else { return }
        try? data.write(to: NarrationAudioStore.sidecarURL(beside: audioURL), options: .atomic)
    }

    private func makeAudioFile(for pageID: String, startedAt: UInt64) throws -> AVAudioFile {
        let audioURL = NarrationAudioStore.audioURL(for: pageID, startedAtMilliseconds: startedAt)
        try FileManager.default.createDirectory(
            at: audioURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        return try AVAudioFile(
            forWriting: audioURL,
            settings: [
                AVFormatIDKey: kAudioFormatLinearPCM,
                AVLinearPCMIsFloatKey: true,
                AVLinearPCMBitDepthKey: 32,
                AVSampleRateKey: Double(WhisperKit.sampleRate),
                AVNumberOfChannelsKey: 1
            ],
            commonFormat: .pcmFormatFloat32,
            interleaved: false
        )
    }

    /// Written while the recording runs rather than at the end, so an app killed
    /// mid-sentence keeps everything said up to that point.
    ///
    /// ponytail: the file is written on the main actor, one small buffer at a
    /// time. That is nothing at 16 kHz mono; give it its own serial queue if it
    /// ever shows up as a stutter under the pen.
    private func append(_ samples: [Float]) {
        guard let audioFile, !samples.isEmpty else { return }
        guard let buffer = AVAudioPCMBuffer(
            pcmFormat: audioFile.processingFormat,
            frameCapacity: AVAudioFrameCount(samples.count)
        ), let channel = buffer.floatChannelData?[0] else { return }

        buffer.frameLength = AVAudioFrameCount(samples.count)
        samples.withUnsafeBufferPointer { source in
            guard let base = source.baseAddress else { return }
            channel.update(from: base, count: samples.count)
        }
        try? audioFile.write(from: buffer)
    }

    private func fail(_ message: String) {
        audioFile = nil
        hint = nil
        complaint = message
        state = .failed(message)
    }

    private func beginBackgroundTask() {
        guard backgroundTask == .invalid else { return }
        backgroundTask = UIApplication.shared
            .beginBackgroundTask(withName: "goghmode-narration") { [weak self] in
                Task { @MainActor in self?.endBackgroundTask() }
            }
    }

    private func endBackgroundTask() {
        guard backgroundTask != .invalid else { return }
        UIApplication.shared.endBackgroundTask(backgroundTask)
        backgroundTask = .invalid
    }
}
