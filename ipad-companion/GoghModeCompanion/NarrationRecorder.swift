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

    /// Named on the wire so a sheet can say what read it back, and so a sheet
    /// transcribed by one model is never credited to a later one.
    var engineName: String? {
        variant.map { "whisperkit/\($0)" }
    }

    func whisperKit(progress: @escaping @Sendable (Double) -> Void) async throws -> WhisperKit {
        if let loaded { return loaded }
        if let loading { return try await loading.value }

        let task = Task { () throws -> WhisperKit in
            let recommended = WhisperKit.recommendedModels()
            let chosen = recommended.supported.contains(NarrationModel.preferredVariant)
                ? NarrationModel.preferredVariant
                : recommended.default

            let folder = try await WhisperKit.download(variant: chosen) { downloading in
                progress(downloading.fractionCompleted)
            }
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
        case preparingModel(Double)
        case recording(since: Date)
        case transcribing
        case failed(String)
    }

    @Published private(set) var state: State = .idle
    /// The last thing that went wrong, read by the sheet's notice line. Kept
    /// apart from `state` for the same reason the uploader keeps its complaint
    /// apart from its status: a sentence that blinks out is a sentence nobody reads.
    @Published private(set) var complaint: String?

    var isRecording: Bool {
        if case .recording = state { return true }
        return false
    }

    var engineName: String? {
        NarrationModel.shared.engineName
    }

    private var whisperKit: WhisperKit?
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

    /// Starts listening over the named sheet. The model comes down on first use,
    /// which is why preparing is a state the control can show rather than a wait
    /// with nothing on screen.
    func start(for pageID: String) async {
        switch state {
        case .idle, .failed: break
        case .preparingModel, .recording, .transcribing: return
        }

        self.pageID = pageID
        complaint = nil

        guard await AudioProcessor.requestRecordPermission() else {
            fail(
                "GoghMode may not use the microphone, so nothing was recorded. Allow it in Settings."
            )
            return
        }

        state = .preparingModel(0)
        do {
            let whisperKit = try await NarrationModel.shared.whisperKit { [weak self] fraction in
                Task { @MainActor in
                    guard let self, case .preparingModel = self.state else { return }
                    self.state = .preparingModel(fraction)
                }
            }
            self.whisperKit = whisperKit

            let startedAt = Date()
            recordingStartedAtMilliseconds = UInt64(max(0, startedAt.timeIntervalSince1970 * 1000))
            audioFile = try makeAudioFile(for: pageID, startedAt: recordingStartedAtMilliseconds)

            // Configures the audio session itself, so nothing here touches
            // `AVAudioSession` and the two cannot disagree about the category.
            // Empties the sample buffer as it starts, so the model outliving this
            // sheet does not mean the previous sheet's words do.
            try whisperKit.audioProcessor.startRecordingLive { [weak self] samples in
                Task { @MainActor in self?.append(samples) }
            }
            state = .recording(since: startedAt)
        } catch {
            fail("Recording could not start: \(error.localizedDescription)")
        }
    }

    /// Stops, transcribes, and hands back what was said.
    ///
    /// Held open by a background task, because the ordinary way to finish a
    /// recording is to walk away from the sheet — and a sentence lost to that is
    /// the sentence explaining the drawing.
    @discardableResult
    func stop() async -> [NarrationSegment] {
        guard case .recording = state, let whisperKit else { return [] }

        whisperKit.audioProcessor.stopRecording()
        let recorded = Array(whisperKit.audioProcessor.audioSamples)
        let startedAtMilliseconds = recordingStartedAtMilliseconds
        state = .transcribing

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
            state = .idle
            return spoken
        } catch {
            fail("What you said could not be turned into text: \(error.localizedDescription)")
            return []
        }
    }

    /// Picks up recordings whose transcription never finished — the app was
    /// killed, or the sheet was closed while the words were still being read.
    /// Called when a sheet is opened, which is the one moment the result is
    /// certainly wanted.
    func transcribePendingAudio(for pageID: String) async -> [NarrationSegment] {
        guard case .idle = state else { return [] }
        let pending = NarrationAudioStore.recordingsAwaitingTranscription(for: pageID)
        guard !pending.isEmpty else { return [] }

        self.pageID = pageID
        state = .transcribing
        beginBackgroundTask()
        defer { endBackgroundTask() }

        var recovered: [NarrationSegment] = []
        do {
            for audioURL in pending {
                guard let startedAtMilliseconds = UInt64(
                    audioURL.deletingPathExtension().lastPathComponent
                ) else { continue }

                let samples = try AudioProcessor.loadAudioAsFloatArray(fromPath: audioURL.path)
                recovered += try await transcribe(
                    samples,
                    startedAtMilliseconds: startedAtMilliseconds
                )
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
        guard !spoken.isEmpty else { return nil }

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
        let loaded = try await NarrationModel.shared.whisperKit { [weak self] fraction in
            Task { @MainActor in self?.state = .preparingModel(fraction) }
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
