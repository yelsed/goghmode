import PencilKit
import SwiftUI

/// The register is home. A sheet is somewhere you go and come back from, which is
/// why the canvas is pushed rather than presented: the back button is the only
/// "done" this app needs, and new sheets are only made where sheets are kept.
struct ContentView: View {
    @Environment(\.scenePhase) private var scenePhase
    @AppStorage("goghModeEndpoint") private var endpointText = ""
    @AppStorage("goghModePaired") private var paired = false
    @StateObject private var uploader = UploadController()
    @StateObject private var pageStore = PageStore()
    @State private var openPageID: String?
    @State private var showingSettings = false

    /// The register's column widths are derived from one scaled unit, injected here
    /// so every screen in the stack measures its table the same way.
    @ScaledMetric(relativeTo: .body) private var columnUnit: CGFloat = 100

    private var endpoint: GoghModeEndpoint? {
        GoghModeEndpoint(endpointText)
    }

    var body: some View {
        ZStack {
            Sheet.ground.ignoresSafeArea()

            if endpoint == nil || !paired {
                SetupView(
                    endpointText: $endpointText,
                    isValid: endpoint != nil,
                    onDone: { paired = true }
                )
            } else {
                register
            }
        }
        .environment(\.registerColumns, RegisterColumns(scale: columnUnit / 100))
        .onChange(of: scenePhase) { _, newPhase in
            // Coming back to the app is the moment the Mac is most likely to have
            // been reopened — or updated — so both the pending upload and what the
            // Mac claims to accept are worth asking about again.
            if newPhase == .active {
                uploader.forgetWhatTheMacAccepts()
                uploader.retryIfOffline()
            }
        }
        .onChange(of: endpointText) { _, _ in
            uploader.forgetWhatTheMacAccepts()
        }
    }

    private var register: some View {
        NavigationStack {
            RegisterView(
                store: pageStore,
                uploader: uploader,
                endpointText: endpointText,
                onOpen: { openPageID = $0 },
                onNew: { openPageID = pageStore.addPage().id },
                onSettings: { showingSettings = true }
            )
            .navigationDestination(item: $openPageID) { pageID in
                CanvasView(
                    store: pageStore,
                    uploader: uploader,
                    pageID: pageID,
                    endpointText: endpointText
                )
            }
        }
        .sheet(isPresented: $showingSettings) {
            SetupView(
                endpointText: $endpointText,
                isValid: endpoint != nil,
                onDone: { showingSettings = false }
            )
        }
    }
}

/// One sheet, open. Everything here is about the drawing: the register's facts stay
/// in the register, and the only chrome is the state of the sheet in front of you.
struct CanvasView: View {
    @ObservedObject var store: PageStore
    @ObservedObject var uploader: UploadController

    let pageID: String
    let endpointText: String

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
                    uploader.schedule(snapshot: snapshot(of: newDrawing), endpointText: endpointText)
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
        if !uploader.pinningSupported && uploader.macIsKnown {
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
        uploader.uploadNow(snapshot: snapshot(of: drawing), endpointText: endpointText)
    }

    private func clearSheet() {
        drawing = PKDrawing()
        reloadSignal += 1
        store.update(pageID, with: drawing)
        uploader.uploadNow(
            snapshot: DrawingSnapshot.empty(canvasSize: canvasSize, page: page?.pageRef),
            endpointText: endpointText
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
            let accepted = await uploader.pin(target, endpointText: endpointText)
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
        case .failed: Sheet.stamp
        }
    }
}

/// Pairing. Rebuilt because the old screen put `.secondary` grey on a white
/// ground, which is unreadable at a desk: every string here is full-weight ink,
/// and the address sits on paper so it reads as a field to fill in.
struct SetupView: View {
    @Binding var endpointText: String
    let isValid: Bool
    let onDone: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("GoghMode")
                        .font(.largeTitle.weight(.bold))
                        .foregroundStyle(Sheet.onGround)
                    Text("Write here. The Mac keeps every sheet, and your agent reads the one you stamp.")
                        .font(.callout)
                        .foregroundStyle(Sheet.onGroundSecondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(.bottom, 28)

                VStack(spacing: 0) {
                    Rectangle().fill(Sheet.rule).frame(height: Sheet.hair)

                    VStack(alignment: .leading, spacing: 8) {
                        BlockLabel(text: "Mac address")
                        TextField("http://192.168.1.10:8787/token/", text: $endpointText)
                            .font(.callout.monospaced())
                            .foregroundStyle(Sheet.ink)
                            .keyboardType(.URL)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .frame(minHeight: 44)

                        if !endpointText.isEmpty && !isValid {
                            Text("That is not a full address. Copy the mobile URL from the Mac — it starts with http:// and ends in a token.")
                                .font(.footnote)
                                .foregroundStyle(Sheet.stamp)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                    .padding(14)
                }
                .background(Sheet.paper)
                .overlay {
                    Rectangle().strokeBorder(Sheet.edge, lineWidth: Sheet.hair)
                }

                Text("Open GoghMode on the Mac and press Copy mobile URL.")
                    .font(.subheadline)
                    .foregroundStyle(Sheet.onGroundSecondary)
                    .padding(.top, 12)

                Button(action: onDone) {
                    Text(isValid ? "Open the register" : "Waiting for an address")
                        .font(.callout.weight(.semibold))
                        .frame(maxWidth: .infinity)
                        .frame(height: 50)
                        .background(isValid ? Sheet.ink : Sheet.edge)
                        .foregroundStyle(isValid ? Sheet.paper : Sheet.onGround)
                        .clipShape(RoundedRectangle(cornerRadius: Sheet.controlRadius))
                }
                .disabled(!isValid)
                .padding(.top, 28)
            }
            .padding(Sheet.margin)
            .frame(maxWidth: 640, alignment: .leading)
            .frame(maxWidth: .infinity)
        }
        .background(Sheet.ground)
    }
}

#Preview {
    ContentView()
}
