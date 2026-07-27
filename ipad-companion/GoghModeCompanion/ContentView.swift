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
    @State private var canvasSize = CGSize(width: 1024, height: 1366)
    @State private var reloadSignal = 0
    @State private var renaming: RenameTarget?
    @State private var confirmingClear = false
    @State private var stamping = false

    private var page: NotebookPage? {
        store.page(pageID)
    }

    var body: some View {
        VStack(spacing: 0) {
            if let message = uploader.pagesUnsupportedMessage {
                Text(message)
                    .font(.footnote)
                    .foregroundStyle(Sheet.onGround)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                    .background(Sheet.ground)
            }

            GeometryReader { geometry in
                PencilCanvasView(drawing: $drawing, reloadSignal: $reloadSignal) { newDrawing, newCanvasSize in
                    canvasSize = newCanvasSize == .zero ? geometry.size : newCanvasSize
                    store.update(pageID, with: newDrawing)
                    uploader.schedule(snapshot: snapshot(of: newDrawing), to: destination)
                }
                .ignoresSafeArea(edges: .bottom)
                .onAppear { canvasSize = geometry.size }
            }
        }
        .background(Sheet.paper)
        .navigationTitle(page?.title ?? "Sheet")
        .navigationBarTitleDisplayMode(.inline)
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
        .onDisappear { uploadCurrentSheet() }
        .onChange(of: scenePhase) { _, newPhase in
            if newPhase == .background {
                uploadCurrentSheet()
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
            page: page?.pageRef
        )
    }

    private func uploadCurrentSheet() {
        uploader.uploadNow(snapshot: snapshot(of: drawing), to: destination)
    }

    private func clearSheet() {
        drawing = PKDrawing()
        reloadSignal += 1
        store.clear(pageID)
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

/// Connection state as a stamped chip. The old version paired a coloured dot with
/// grey caption text; the label carries the state here so colour is not the only
/// thing saying it.
struct StatusBadge: View {
    let status: UploadController.Status
    let canRetry: Bool
    let onRetry: () -> Void

    var body: some View {
        Button(action: onRetry) {
            HStack(spacing: 7) {
                Circle()
                    .fill(tint)
                    .frame(width: 8, height: 8)
                Text(status.label.uppercased())
                    .font(.caption2.weight(.semibold))
                    .tracking(0.8)
                    .foregroundStyle(Sheet.onGround)
                if case .failed(let message) = status {
                    Text(message)
                        .font(.caption)
                        .foregroundStyle(Sheet.onGroundSecondary)
                        .lineLimit(1)
                }
                if case .wrongHost(let message) = status {
                    Text(message)
                        .font(.caption)
                        .foregroundStyle(Sheet.stamp)
                        .lineLimit(2)
                }
            }
            .padding(.horizontal, 8)
            .frame(height: 44)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!canRetry)
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
