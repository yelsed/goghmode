import PencilKit
import SwiftUI
import UIKit

/// The one key every view reads: the two that apply the flag and the one that
/// toggles it. A second, mistyped key would silently fork a second setting
/// that never meets the first.
let goghModeKeepAwakeKey = "goghModeKeepAwake"

/// The whole keep-awake rule in one place: the flag, and only while the scene
/// is active — the OS re-arms the idle timer the moment we background, so the
/// decision is re-made on every phase change rather than held.
func applyGoghModeKeepAwake(_ on: Bool, _ scenePhase: ScenePhase) {
    UIApplication.shared.isIdleTimerDisabled = on && scenePhase == .active
}

/// The register is home. A sheet is somewhere you go and come back from, which is
/// why the canvas is pushed rather than presented: the back button is the only
/// "done" this app needs, and new sheets are only made where sheets are kept.
struct ContentView: View {
    @Environment(\.scenePhase) private var scenePhase
    @AppStorage("goghModeEndpoint") private var endpointText = ""
    @AppStorage(goghModeKeepAwakeKey) private var keepAwake = true
    @StateObject private var uploader: UploadController
    @StateObject private var pageStore = PageStore()
    @StateObject private var hostStore: HostStore
    @State private var openPageID: String?
    @State private var showingSettings = false

    init() {
        // The uploader has to be able to write the address that actually
        // answered back into the store, so the two are born together: one
        // store, two owners, no copies that can fall out of step.
        let store = HostStore()
        _uploader = StateObject(wrappedValue: UploadController(hostStore: store))
        _hostStore = StateObject(wrappedValue: store)
    }

    /// The register's column widths are derived from one scaled unit, injected here
    /// so every screen in the stack measures its table the same way.
    @ScaledMetric(relativeTo: .body) private var columnUnit: CGFloat = 100

    /// Resolved once, so a host and a credential can never be paired up wrongly
    /// somewhere further down the view tree.
    private var destination: UploadController.Destination? {
        guard let host = hostStore.selectedHost else { return nil }
        return UploadController.Destination(
            host: host,
            secret: hostStore.secret(for: host.id),
            deviceID: hostStore.deviceID
        )
    }

    var body: some View {
        ZStack {
            Sheet.ground.ignoresSafeArea()

            if let destination {
                register(sending: destination)
            } else {
                HostListView(hostStore: hostStore)
            }
        }
        .environment(\.registerColumns, RegisterColumns(scale: columnUnit / 100))
        .onAppear {
            // An endpoint saved by an older build becomes the first entry in the
            // host list, so updating the app does not look like losing the
            // connection.
            hostStore.adoptLegacyEndpoint(endpointText)
            applyGoghModeKeepAwake(keepAwake, scenePhase)
        }
        .onChange(of: scenePhase) { _, newPhase in
            // Coming back to the app is the moment the host is most likely to
            // have been reopened — or updated — so both the pending upload and
            // what it claims to accept are worth asking about again.
            if newPhase == .active {
                uploader.forgetWhatTheHostAccepts()
                uploader.retryIfOffline()
            }
            applyGoghModeKeepAwake(keepAwake, scenePhase)
        }
        .onChange(of: keepAwake) { _, _ in
            // The toggle is reached from the host list, which sits right on top
            // of this view: flipping it while the app is on screen has to take
            // effect now, not at the next backgrounding.
            applyGoghModeKeepAwake(keepAwake, scenePhase)
        }
        .onChange(of: hostStore.selectedHostID) { _, _ in
            uploader.forgetWhatTheHostAccepts()
        }
    }

    private func register(sending destination: UploadController.Destination) -> some View {
        NavigationStack {
            RegisterView(
                store: pageStore,
                uploader: uploader,
                hostStore: hostStore,
                destination: destination,
                onOpen: { openPageID = $0 },
                onNew: { openPageID = pageStore.addPage().id },
                onSettings: { showingSettings = true }
            )
            .navigationDestination(item: $openPageID) { pageID in
                CanvasView(
                    store: pageStore,
                    uploader: uploader,
                    hostStore: hostStore,
                    pageID: pageID,
                    destination: destination
                )
            }
        }
        .sheet(isPresented: $showingSettings) {
            HostListView(hostStore: hostStore)
        }
    }

}

/// One sheet, open. Everything here is about the drawing: the register's facts stay
/// in the register, and the only chrome is the state of the sheet in front of you.
struct CanvasView: View {
    @ObservedObject var store: PageStore
    @ObservedObject var uploader: UploadController
    @ObservedObject var hostStore: HostStore

    let pageID: String
    let destination: UploadController.Destination

    @Environment(\.scenePhase) private var scenePhase
    @AppStorage(goghModeKeepAwakeKey) private var keepAwake = true
    @State private var drawing = PKDrawing()
    @State private var reloadSignal = 0
    /// The live drawing surface, which is the view. Reported by the canvas rather
    /// than assumed, so what is exported is what was drawn on.
    @State private var canvasSize = SheetPage.size
    /// Waits for the writing to pause before recording a state, the same way the
    /// upload does.
    @State private var pendingRevision: Task<Void, Never>?
    @State private var renaming: RenameTarget?
    @State private var confirmingClear = false
    @State private var stamping = false
    @State private var showingRePair = false
    /// One recorder per open sheet: a recording belongs to the sheet it was made
    /// over, and closing the sheet ends it.
    @StateObject private var recorder = NarrationRecorder()

    private var page: NotebookPage? {
        store.page(pageID)
    }

    var body: some View {
        PencilCanvasView(
            drawing: $drawing,
            reloadSignal: $reloadSignal,
            ruling: page?.ruling
        ) { newDrawing, surface in
            canvasSize = surface == .zero ? canvasSize : surface
            store.update(pageID, with: newDrawing)
            recordRevisionAfterAPause(newDrawing)
            uploader.schedule(snapshot: snapshot(of: newDrawing), to: destination)
        }
        .ignoresSafeArea(edges: .bottom)
        // Laid over the sheet, never stacked above it. A banner in the layout
        // resizes the canvas the moment it appears, which moves the paper under
        // the pen mid-line.
        .overlay(alignment: .top) {
            if let notice {
                Text(notice)
                    .font(.footnote)
                    .foregroundStyle(Sheet.onGround)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                    .background(Sheet.ground)
                    .allowsHitTesting(false)
            }
        }
        .background(Sheet.paper)
        .navigationTitle(page?.title ?? "Sheet")
        .navigationBarTitleDisplayMode(.inline)
        // The sheet is white to the edge of the screen, so a translucent bar over
        // it reads as more paper: people draw on it, get nothing, and conclude the
        // pen is broken. On ground, the bar is the desk the sheet lies on.
        .toolbarBackground(Sheet.ground, for: .navigationBar)
        .toolbarBackground(.visible, for: .navigationBar)
        .toolbar {
            ToolbarItemGroup(placement: .topBarTrailing) {
                StatusBadge(status: uploader.status, canRetry: uploader.canRetry) {
                    uploader.retry()
                } onRePair: {
                    showingRePair = true
                }

                if let page {
                    StampControl(state: stampState(for: page)) {
                        toggleStamp(page)
                    }
                }

                NarrationControl(
                    state: recorder.state,
                    recordedBefore: page?.narrationSeconds ?? 0,
                    action: toggleNarration
                )

                Button(action: stepBack) {
                    Label("Undo", systemImage: "arrow.uturn.backward")
                }
                .disabled(!store.canStepBack)

                Button(action: stepForward) {
                    Label("Redo", systemImage: "arrow.uturn.forward")
                }
                .disabled(!store.canStepForward)

                // Renaming and clearing are rare next to stepping back, and the
                // bar has no room for both as buttons.
                Menu {
                    Picker("Ruling", selection: rulingChoice) {
                        Label("Plain", systemImage: "rectangle").tag(SheetRuling.Style?.none)
                        ForEach(SheetRuling.Style.allCases) { style in
                            Label(style.label, systemImage: style.symbol)
                                .tag(SheetRuling.Style?.some(style))
                        }
                    }

                    Button {
                        if let page {
                            renaming = .sheet(page)
                        }
                    } label: {
                        Label("Rename", systemImage: "pencil")
                    }

                    Button(role: .destructive) {
                        confirmingClear = true
                    } label: {
                        Label("Clear", systemImage: "eraser")
                    }
                    .disabled(drawing.strokes.isEmpty)
                } label: {
                    Label("More", systemImage: "ellipsis.circle")
                }
            }
        }
        .onAppear {
            store.select(pageID)
            drawing = page?.drawing ?? PKDrawing()
            reloadSignal += 1
            // A recording whose words never arrived — the app was killed, or the
            // sheet was closed while they were still being read. Deliberately not
            // a `.task`, which would be cancelled by leaving the sheet again.
            keepWhenReady { [recorder, pageID] in
                await recorder.transcribePendingAudio(for: pageID)
            }
        }
        // Leaving the sheet — back to the register, or the app being put away — is
        // when work is most likely to be lost: the app can be killed in the
        // background before the 600ms debounce fires.
        .onDisappear {
            // The pause may never come: leaving is itself the pause.
            pendingRevision?.cancel()
            store.recordRevision(pageID, drawing)
            finishNarration()
            uploadCurrentSheet()
            store.flushRevisions()
        }
        .onChange(of: scenePhase) { _, newPhase in
            if newPhase == .background {
                pendingRevision?.cancel()
                store.recordRevision(pageID, drawing)
                finishNarration()
                uploadCurrentSheet()
                store.flushRevisions()
            }
            applyGoghModeKeepAwake(keepAwake, scenePhase)
        }
        .sheet(item: $renaming) { target in
            RenameSheet(target: target) { _, name in
                commitRename(to: name)
            }
        }
        // The repair for a host that has outlived its saved address: pair it
        // again, the same way it was added.
        .sheet(isPresented: $showingRePair) {
            PairingView(hostStore: hostStore)
        }
        // Clearing a sheet cannot be undone, so it asks. The eraser used to wipe
        // every stroke on the first press with no way back.
        .confirmationDialog(
            "Clear this sheet?",
            isPresented: $confirmingClear,
            titleVisibility: .visible
        ) {
            Button("Erase every stroke", role: .destructive, action: clearSheet)
            Button("Keep it", role: .cancel) {}
        } message: {
            Text(
                "\(drawing.strokes.count) strokes on \(page?.title ?? "this sheet") are erased on the iPad and on the Mac. This cannot be undone."
            )
        }
    }

    /// One line of plain language for whatever is currently wrong, most urgent
    /// first, in the same order the register uses. The status chip no longer
    /// carries these sentences, so this is where they are read.
    private var notice: String? {
        uploader.complaint
            ?? recorder.complaint
            ?? recorder.hint
            ?? uploader.pagesUnsupportedMessage
            ?? uploader.rulingUnsupportedMessage
            ?? uploader.narrationUnsupportedMessage
    }

    /// Changing the ruling changes what the exported page looks like, so the host
    /// is told at once rather than at the next stroke.
    private var rulingChoice: Binding<SheetRuling.Style?> {
        Binding(
            get: { page?.ruling?.style },
            set: { style in
                store.setRuling(style.map { SheetRuling(style: $0) }, on: pageID)
                uploadCurrentSheet()
            }
        )
    }

    private func stampState(for page: NotebookPage) -> StampState {
        if !uploader.pinningSupported && uploader.hostIsKnown {
            return .unavailable
        }
        if stamping {
            return .working
        }
        return page.id == store.pinnedPageID ? .issued : .available
    }

    private func snapshot(of pencilDrawing: PKDrawing) -> DrawingSnapshot {
        DrawingSnapshot.fromPencilDrawing(
            pencilDrawing,
            canvasSize: canvasSize,
            page: page?.pageRef,
            ruling: page?.ruling,
            narration: page?.spokenNarration
        )
    }

    /// One button, two meanings: start listening, or stop and keep what was said.
    private func toggleNarration() {
        switch recorder.state {
        case .recording:
            keepWhenReady { [recorder] in await recorder.stop() }
        case .idle, .failed:
            Task { [recorder, pageID] in await recorder.start(for: pageID) }
        case .preparingModel, .loadingModel, .transcribing:
            break
        }
    }

    /// Leaving the sheet ends the recording that belongs to it. The transcription
    /// runs on under a background task and the words arrive when they are ready,
    /// which is why nothing here waits for them.
    private func finishNarration() {
        guard recorder.isRecording else { return }
        keepWhenReady { [recorder] in await recorder.stop() }
    }

    /// Words land on the sheet and go to the Mac at once rather than waiting for
    /// the next stroke: someone who has finished talking about a sheet is often
    /// finished drawing on it too.
    ///
    /// They may arrive after the sheet has been left, so everything they need is
    /// taken before the wait. A `@StateObject` or `@State` read once its view is
    /// gone is a fresh value, not the recorder that was listening or the drawing
    /// that was made — which is also why the upload is built from the store's
    /// copy of the sheet rather than the canvas.
    private func keepWhenReady(
        _ words: @escaping @MainActor () async -> NarrationRecorder.Recording
    ) {
        let recorder = self.recorder
        let store = self.store
        let uploader = self.uploader
        let destination = self.destination
        let pageID = self.pageID
        Task { @MainActor in
            let recording = await words()
            guard !recording.segments.isEmpty else { return }
            store.appendNarration(
                recording.segments,
                to: pageID,
                from: recorder.engineName,
                recorded: recording.duration
            )
            if let page = store.page(pageID) {
                uploader.uploadNow(snapshot: page.snapshot, to: destination)
            }
        }
    }

    /// One state per pause in the writing.
    ///
    /// This used to fire on the stroke count changing, which is cheap but blind to
    /// every edit that leaves the count alone: dragging a lasso selection, or an
    /// eraser shortening a stroke without splitting it. Those are exactly the
    /// edits someone wants back. `PageStore.recordRevision` compares against the
    /// state it already holds, so a pause that changed nothing still records
    /// nothing.
    private func recordRevisionAfterAPause(_ newDrawing: PKDrawing) {
        pendingRevision?.cancel()
        pendingRevision = Task {
            try? await Task.sleep(for: .milliseconds(600))
            guard !Task.isCancelled else { return }
            store.recordRevision(pageID, newDrawing)
        }
    }

    private func stepBack() {
        guard let restored = store.stepBack(pageID) else { return }
        apply(restored)
    }

    private func stepForward() {
        guard let restored = store.stepForward(pageID) else { return }
        apply(restored)
    }

    /// Sent to the host straight away rather than on the next pause: stepping back
    /// is a deliberate change to what the sheet says, and the agent reads the host.
    private func apply(_ restored: PKDrawing) {
        // A pause recorded after a step back would land on the state just stepped
        // to, which is already where the cursor sits.
        pendingRevision?.cancel()
        drawing = restored
        reloadSignal += 1
        store.restore(pageID, to: restored)
        uploader.uploadNow(snapshot: snapshot(of: restored), to: destination)
    }

    private func uploadCurrentSheet() {
        uploader.uploadNow(snapshot: snapshot(of: drawing), to: destination)
    }

    private func clearSheet() {
        // Recorded on both sides of the erase, so an accidental clear is one step
        // back rather than the thing the confirmation exists to prevent.
        store.recordRevision(pageID, drawing)

        pendingRevision?.cancel()
        let emptied = PKDrawing()
        drawing = emptied
        reloadSignal += 1
        store.clear(pageID)
        store.recordRevision(pageID, emptied)
        uploader.uploadNow(
            snapshot: DrawingSnapshot.empty(canvasSize: canvasSize, page: page?.pageRef),
            to: destination
        )
    }

    private func commitRename(to name: String) {
        store.rename(pageID, to: name)
        // The Mac keeps the title with the page, so a rename only reaches it on the
        // next save. Send it now, so the register and the Mac never disagree about
        // the name the agent is reading.
        uploadCurrentSheet()
    }

    private func toggleStamp(_ page: NotebookPage) {
        guard !stamping else { return }
        let target = page.id == store.pinnedPageID ? nil : page.id

        stamping = true
        Task {
            // Sent before pinned, so the Mac is holding this sheet by the time it is
            // told to follow it.
            if target != nil {
                await uploader.send(snapshot(of: drawing), to: destination)
            }

            let accepted = await uploader.pin(target, to: destination)
            stamping = false
            if accepted {
                store.recordPin(target)
            }
        }
    }
}

/// Connection state as a chip that keeps one shape in every state.
///
/// It used to size itself to whatever it was saying, and two states appended a
/// whole sentence on top of that, so the toolbar and the register head line
/// jumped on every transition. Every slot is reserved now — the label and the
/// time at their widest, the machine name at a fixed width — so the chip is one
/// size in every state, and the sentence belongs to the notice line, which has
/// room for it.
struct StatusBadge: View {
    let status: UploadController.Status
    let canRetry: Bool
    let onRetry: () -> Void
    /// The repair for `.needsRepair`: present the pairing screen, because a
    /// dead address is past retry.
    let onRePair: (() -> Void)?

    init(
        status: UploadController.Status,
        canRetry: Bool,
        onRetry: @escaping () -> Void,
        onRePair: (() -> Void)? = nil
    ) {
        self.status = status
        self.canRetry = canRetry
        self.onRetry = onRetry
        self.onRePair = onRePair
    }

    /// The longest label any state can produce. Held here so the reservation and
    /// `Status.label` cannot drift apart unnoticed.
    private static let widestLabel = "Wrong host"

    var body: some View {
        Button(action: {
            if case .needsRepair = status {
                onRePair?()
            } else if canRetry {
                onRetry()
            }
        }) {
            HStack(spacing: 7) {
                Circle()
                    .fill(tint)
                    .frame(width: 8, height: 8)

                label
                savedTime
                repairTarget
            }
            .padding(.horizontal, 8)
            .frame(height: 44)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        // Disabled in every state that is not a retry — in particular
        // `.wrongHost`: a tap there would re-upload into a machine that cannot
        // prove itself, which is the thing the state exists to prevent.
        .disabled(!canRetry && !isRepair)
        // The stamp control beside this one animates on a spring. Without this the
        // badge's own relayout gets dragged along by it.
        .animation(nil, value: status)
        .accessibilityLabel(Text(spokenLabel))
    }

    /// Laid over an invisible copy of the widest label, so the slot is reserved at
    /// whatever the reading size makes that width.
    private var label: some View {
        badgeText(StatusBadge.widestLabel.uppercased())
            .hidden()
            .overlay(alignment: .leading) {
                badgeText(status.label.uppercased())
                    .foregroundStyle(Sheet.onGround)
                    .fixedSize()
            }
    }

    /// Reserved in every state, filled only when there is a save to time. Mono
    /// because it is a measurement, and because mono digits do not change width
    /// between 11:11 and 20:48.
    private var savedTime: some View {
        Text("00:00")
            .font(.caption2.monospaced().weight(.medium))
            .hidden()
            .overlay(alignment: .leading) {
                if case .saved(let at) = status {
                    Text(at.formatted(.dateTime.hour().minute()))
                        .font(.caption2.monospaced().weight(.medium))
                        .foregroundStyle(Sheet.onGroundSecondary)
                        .fixedSize()
                }
            }
    }

    /// The machine whose whole address set went silent. The slot is present at
    /// a fixed width in every state — the chip keeps its one shape, so a long
    /// name truncates rather than stretching it — and only the repair fills it,
    /// because naming the machine tells you which one to re-pair.
    private var repairTarget: some View {
        Text("GoghMode host")
            .font(.caption)
            .hidden()
            .frame(width: 76, alignment: .leading)
            .overlay(alignment: .leading) {
                if case .needsRepair(let name) = status {
                    Text(name)
                        .font(.caption)
                        .foregroundStyle(Sheet.onGroundSecondary)
                        .lineLimit(1)
                        .frame(width: 76, alignment: .leading)
                }
            }
    }

    private func badgeText(_ text: String) -> some View {
        Text(text)
            .font(.caption2.weight(.semibold))
            .tracking(0.8)
            .lineLimit(1)
    }

    private var spokenLabel: String {
        if case .saved(let at) = status {
            return "Saved at \(at.formatted(.dateTime.hour().minute()))"
        }
        return status.label
    }

    /// The one state the chip is tappable in that `canRetry` deliberately
    /// excludes: a re-pair starts a fresh identity check, which is allowed,
    /// whereas retrying the current host would be retrying into the wall.
    private var isRepair: Bool {
        if case .needsRepair = status {
            return true
        }
        return false
    }

    private var tint: Color {
        switch status {
        case .idle, .saved: Sheet.review
        case .waiting, .saving: Sheet.inkLabel
        case .failed, .wrongHost, .needsRepair: Sheet.stamp
        }
    }
}

/// Recording what is being said over the sheet, as one button that reports what
/// it is doing.
///
/// Sized like `StatusBadge`: the widest thing it can ever show is reserved in
/// every state and the live content laid over it, so starting a recording or
/// bringing a model down does not shift the buttons beside it.
struct NarrationControl: View {
    let state: NarrationRecorder.State
    /// How much has already been recorded over this sheet. Shown at rest, so the
    /// button says there is something here and that a tap adds to it, and counted
    /// on from while recording, so the clock continues where it left off.
    let recordedBefore: TimeInterval
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 5) {
                Image(systemName: symbol)
                    .imageScale(.medium)
                reading
            }
            .foregroundStyle(tint)
            .padding(.horizontal, 6)
            .frame(height: 44)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!isTappable)
        // The stamp control beside this one animates on a spring. Without this
        // its relayout drags this one along with it.
        .animation(nil, value: state)
        .accessibilityLabel(Text(spokenLabel))
    }

    /// Mono, and reserved at the width of the longest thing any state puts here,
    /// so 09% and 12:34 occupy exactly the same slot.
    private var reading: some View {
        readingText("00:00")
            .hidden()
            .overlay(alignment: .leading) {
                switch state {
                case .idle, .failed:
                    if recordedBefore > 0 {
                        readingText(NarrationControl.clock(recordedBefore))
                            .fixedSize()
                    }
                case .preparingModel(let fraction):
                    readingText("\(Int((fraction * 100).rounded()))%")
                        .fixedSize()
                case .loadingModel, .transcribing:
                    ProgressView()
                        .controlSize(.mini)
                case .recording(let since):
                    elapsed(since: since)
                }
            }
    }

    private func readingText(_ text: String) -> some View {
        Text(text)
            .font(.caption2.monospaced().weight(.medium))
            .lineLimit(1)
    }

    private func elapsed(since start: Date) -> some View {
        TimelineView(.periodic(from: start, by: 1)) { context in
            readingText(
                NarrationControl.clock(recordedBefore + context.date.timeIntervalSince(start))
            )
            .fixedSize()
        }
    }

    /// mm:ss, and it keeps counting past the hour rather than wrapping — a long
    /// explanation is the case this feature exists for.
    static func clock(_ seconds: TimeInterval) -> String {
        let whole = Int(max(0, seconds))
        return String(format: "%02d:%02d", whole / 60, whole % 60)
    }

    /// A filled microphone says the sheet already has words; the tap adds more.
    private var symbol: String {
        switch state {
        case .idle, .preparingModel, .loadingModel: recordedBefore > 0 ? "mic.fill" : "mic"
        case .recording: "waveform"
        case .transcribing: "text.bubble"
        case .failed: "mic.slash"
        }
    }

    /// Review blue while recording, because something is live and the sheet
    /// should say so. Stamp red stays reserved for the issue stamp, so a failure
    /// goes quiet here and the sentence goes to the notice line.
    private var tint: Color {
        switch state {
        case .recording: Sheet.review
        case .failed: Sheet.inkLabel
        case .idle, .preparingModel, .loadingModel, .transcribing: Sheet.onGround
        }
    }

    /// A model coming down and a transcription under way both end on their own,
    /// and there is nothing a tap could do but interrupt them.
    private var isTappable: Bool {
        switch state {
        case .idle, .recording, .failed: true
        case .preparingModel, .loadingModel, .transcribing: false
        }
    }

    private var spokenLabel: String {
        switch state {
        case .idle where recordedBefore > 0:
            "This sheet has \(NarrationControl.clock(recordedBefore)) of narration. Press to add to it."
        case .idle: "Record what you say while you draw"
        case .preparingModel(let fraction):
            "Downloading the transcriber, \(Int((fraction * 100).rounded())) per cent"
        case .loadingModel: "Getting the transcriber ready"
        case .recording(let since):
            "Recording, \(NarrationControl.clock(recordedBefore + Date().timeIntervalSince(since))). Press to stop."
        case .transcribing: "Turning what you said into text"
        case .failed: "Recording failed. Press to try again."
        }
    }
}

/// Pairing. Rebuilt because the old screen put `.secondary` grey on a white
/// ground, which is unreadable at a desk: every string here is full-weight ink,
/// and the address sits on paper so it reads as a field to fill in.
#Preview {
    ContentView()
}
