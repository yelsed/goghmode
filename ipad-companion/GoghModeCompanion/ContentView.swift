import PencilKit
import SwiftUI

/// The register is home. A sheet is somewhere you go and come back from, which is
/// why the canvas is pushed rather than presented: the back button is the only
/// "done" this app needs, and new sheets are only made where sheets are kept.
struct ContentView: View {
    @Environment(\.scenePhase) private var scenePhase
    @AppStorage("goghModeEndpoint") private var endpointText = ""
    @StateObject private var uploader = UploadController()
    @StateObject private var pageStore = PageStore()
    @StateObject private var hostStore = HostStore()
    @State private var openPageID: String?
    @State private var showingSettings = false

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
        }
        .onChange(of: scenePhase) { _, newPhase in
            // Coming back to the app is the moment the host is most likely to
            // have been reopened — or updated — so both the pending upload and
            // what it claims to accept are worth asking about again.
            if newPhase == .active {
                uploader.forgetWhatTheHostAccepts()
                uploader.retryIfOffline()
            }
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
                destination: destination,
                onOpen: { openPageID = $0 },
                onNew: { openPageID = pageStore.addPage().id },
                onSettings: { showingSettings = true }
            )
            .navigationDestination(item: $openPageID) { pageID in
                CanvasView(
                    store: pageStore,
                    uploader: uploader,
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

    let pageID: String
    let destination: UploadController.Destination

    @Environment(\.scenePhase) private var scenePhase
    @State private var drawing = PKDrawing()
    @State private var reloadSignal = 0
    /// Waits for the writing to pause before recording a state, the same way the
    /// upload does.
    @State private var pendingRevision: Task<Void, Never>?
    @State private var renaming: RenameTarget?
    @State private var confirmingClear = false
    @State private var stamping = false

    private var page: NotebookPage? {
        store.page(pageID)
    }

    var body: some View {
        VStack(spacing: 0) {
            if let notice {
                Text(notice)
                    .font(.footnote)
                    .foregroundStyle(Sheet.onGround)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                    .background(Sheet.ground)
            }

            PencilCanvasView(
                drawing: $drawing,
                reloadSignal: $reloadSignal,
                ruling: page?.ruling
            ) { newDrawing in
                store.update(pageID, with: newDrawing)
                recordRevisionAfterAPause(newDrawing)
                uploader.schedule(snapshot: snapshot(of: newDrawing), to: destination)
            }
            .ignoresSafeArea(edges: .bottom)
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
                }

                if let page {
                    StampControl(state: stampState(for: page)) {
                        toggleStamp(page)
                    }
                }

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
        }
        // Leaving the sheet — back to the register, or the app being put away — is
        // when work is most likely to be lost: the app can be killed in the
        // background before the 600ms debounce fires.
        .onDisappear {
            // The pause may never come: leaving is itself the pause.
            pendingRevision?.cancel()
            store.recordRevision(pageID, drawing)
            uploadCurrentSheet()
            store.flushRevisions()
        }
        .onChange(of: scenePhase) { _, newPhase in
            if newPhase == .background {
                pendingRevision?.cancel()
                store.recordRevision(pageID, drawing)
                uploadCurrentSheet()
                store.flushRevisions()
            }
        }
        .sheet(item: $renaming) { target in
            RenameSheet(target: target) { _, name in
                commitRename(to: name)
            }
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
        if case .wrongHost(let message) = uploader.status {
            return message
        }
        if case .failed(let message) = uploader.status {
            return message
        }
        return uploader.pagesUnsupportedMessage ?? uploader.rulingUnsupportedMessage
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
            canvasSize: SheetPage.size,
            page: page?.pageRef,
            ruling: page?.ruling
        )
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
            snapshot: DrawingSnapshot.empty(canvasSize: SheetPage.size, page: page?.pageRef),
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
/// jumped on every transition. Both slots are reserved at their widest now, and
/// the sentence belongs to the notice line, which has room for it.
struct StatusBadge: View {
    let status: UploadController.Status
    let canRetry: Bool
    let onRetry: () -> Void

    /// The longest label any state can produce. Held here so the reservation and
    /// `Status.label` cannot drift apart unnoticed.
    private static let widestLabel = "Wrong host"

    var body: some View {
        Button(action: onRetry) {
            HStack(spacing: 7) {
                Circle()
                    .fill(tint)
                    .frame(width: 8, height: 8)

                label
                savedTime
            }
            .padding(.horizontal, 8)
            .frame(height: 44)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!canRetry)
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

    private var tint: Color {
        switch status {
        case .idle, .saved: Sheet.review
        case .waiting, .saving: Sheet.inkLabel
        case .failed, .wrongHost: Sheet.stamp
        }
    }
}

/// Pairing. Rebuilt because the old screen put `.secondary` grey on a white
/// ground, which is unreadable at a desk: every string here is full-weight ink,
/// and the address sits on paper so it reads as a field to fill in.
#Preview {
    ContentView()
}
