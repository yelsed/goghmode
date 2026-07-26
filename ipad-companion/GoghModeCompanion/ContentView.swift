import PencilKit
import SwiftUI

struct ContentView: View {
    @Environment(\.scenePhase) private var scenePhase
    @AppStorage("goghModeEndpoint") private var endpointText = ""
    @StateObject private var uploader = UploadController()
    @StateObject private var pageStore = PageStore()
    @State private var drawing = PKDrawing()
    @State private var canvasSize = CGSize(width: 1024, height: 1366)
    @State private var reloadSignal = 0
    @State private var showingSettings = false
    @State private var showingRegister = false

    private var endpoint: GoghModeEndpoint? {
        GoghModeEndpoint(endpointText)
    }

    var body: some View {
        ZStack {
            Sheet.ground.ignoresSafeArea()

            if endpoint == nil || showingSettings {
                SetupView(
                    endpointText: $endpointText,
                    isValid: endpoint != nil,
                    onDone: { showingSettings = false }
                )
            } else {
                drawingView
            }
        }
        .onChange(of: scenePhase) { _, newPhase in
            // Coming back to the app is the moment the Mac is most likely to
            // have been reopened, so it is the natural time to re-check.
            if newPhase == .active {
                uploader.retryIfOffline()
            }
            // Leaving is when work is most likely to be lost: the app can be
            // killed in the background before the debounce fires.
            if newPhase == .background {
                uploadCurrentPage()
            }
        }
        .onAppear {
            drawing = pageStore.selectedDrawing
        }
    }

    private func snapshot(of pencilDrawing: PKDrawing) -> DrawingSnapshot {
        DrawingSnapshot.fromPencilDrawing(
            pencilDrawing,
            canvasSize: canvasSize,
            page: pageStore.selectedPage?.pageRef
        )
    }

    private func uploadCurrentPage() {
        uploader.uploadNow(snapshot: snapshot(of: drawing), endpointText: endpointText)
    }

    private func open(pageID: String) {
        guard pageID != pageStore.selectedPageID else { return }

        // The outgoing sheet goes up before the canvas swaps, so switching away
        // never leaves an edit behind.
        pageStore.updateSelectedPage(with: drawing)
        uploadCurrentPage()

        pageStore.select(pageID)
        drawing = pageStore.selectedDrawing
        reloadSignal += 1
    }

    private func addPage() {
        pageStore.updateSelectedPage(with: drawing)
        uploadCurrentPage()

        pageStore.addPage()
        drawing = PKDrawing()
        reloadSignal += 1
    }

    private func clearSheet() {
        drawing = PKDrawing()
        reloadSignal += 1
        pageStore.updateSelectedPage(with: drawing)
        uploader.uploadNow(
            snapshot: DrawingSnapshot.empty(
                canvasSize: canvasSize,
                page: pageStore.selectedPage?.pageRef
            ),
            endpointText: endpointText
        )
    }

    private var drawingView: some View {
        VStack(spacing: 0) {
            toolbar

            if let message = uploader.pagesUnsupportedMessage {
                Text(message)
                    .font(.footnote)
                    .foregroundStyle(Sheet.onGround)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 16)
                    .padding(.bottom, 10)
                    .background(Sheet.ground)
            }

            GeometryReader { geometry in
                PencilCanvasView(drawing: $drawing, reloadSignal: $reloadSignal) { newDrawing, newCanvasSize in
                    canvasSize = newCanvasSize == .zero ? geometry.size : newCanvasSize
                    pageStore.updateSelectedPage(with: newDrawing)
                    uploader.schedule(snapshot: snapshot(of: newDrawing), endpointText: endpointText)
                }
                .ignoresSafeArea(edges: .bottom)
                .onAppear { canvasSize = geometry.size }
            }
        }
        .sheet(isPresented: $showingRegister) {
            RegisterView(
                store: pageStore,
                uploader: uploader,
                endpointText: endpointText,
                onOpen: { open(pageID: $0) }
            )
        }
    }

    /// The canvas header reads like the top rule of a sheet: which sheet this is,
    /// and whether it is the stamped one.
    private var toolbar: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                StatusBadge(status: uploader.status, canRetry: uploader.canRetry) {
                    uploader.retry()
                }

                if uploader.pagesSupported {
                    Button {
                        pageStore.updateSelectedPage(with: drawing)
                        showingRegister = true
                    } label: {
                        HStack(spacing: 8) {
                            if let page = pageStore.selectedPage {
                                SheetNumber(text: pageStore.sheetNumber(for: page))
                                Text(page.title)
                                    .font(.subheadline.weight(.semibold))
                                    .foregroundStyle(Sheet.onGround)
                                    .lineLimit(1)
                            }
                            Image(systemName: "chevron.down")
                                .font(.caption2.weight(.semibold))
                                .foregroundStyle(Sheet.onGroundSecondary)
                        }
                        .frame(minHeight: 44)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(Text("Open the register"))

                    if pageStore.selectedPageID == pageStore.pinnedPageID {
                        IssueStamp(scale: 0.72)
                    }
                }

                Spacer(minLength: 0)

                if uploader.pagesSupported {
                    toolbarButton("New sheet", systemImage: "plus", action: addPage)
                }
                toolbarButton("Clear", systemImage: "eraser", action: clearSheet)
                toolbarButton("Settings", systemImage: "gearshape") { showingSettings = true }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 4)

            Rectangle().fill(Sheet.rule).frame(height: Sheet.hair)
        }
        .background(Sheet.ground)
    }

    private func toolbarButton(
        _ title: String,
        systemImage: String,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.callout.weight(.semibold))
                .frame(width: 44, height: 44)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundStyle(Sheet.onGround)
        .accessibilityLabel(Text(title))
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
            .padding(.horizontal, 10)
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
                    Text("Write here. The Mac keeps every sheet, and Claude reads the one you stamp.")
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
                    Text(isValid ? "Open the notebook" : "Waiting for an address")
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
