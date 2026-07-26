import PencilKit
import SwiftUI

struct ContentView: View {
    @Environment(\.scenePhase) private var scenePhase
    @AppStorage("goghModeEndpoint") private var endpointText = ""
    @StateObject private var uploader = UploadController()
    @StateObject private var pageStore = PageStore()
    @StateObject private var hostStore = HostStore()
    @State private var drawing = PKDrawing()
    @State private var canvasSize = CGSize(width: 1024, height: 1366)
    @State private var clearSignal = 0
    @State private var showingSettings = false
    @State private var showingPages = false
    @State private var showingHosts = false

    var body: some View {
        ZStack {
            Color.white.ignoresSafeArea()

            if hostStore.selectedHost == nil || showingSettings {
                setupView
            } else {
                drawingView
            }
        }
        .onChange(of: scenePhase) { _, newPhase in
            // Coming back to the app is the moment the host is most likely to
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
            // An endpoint saved by an older build becomes the first entry in the
            // host list, so updating the app does not look like losing the
            // connection.
            hostStore.adoptLegacyEndpoint(endpointText)
        }
    }

    /// Resolves the destination once, so a host and a credential can never be
    /// mixed up between two saved hosts.
    private func destination() -> UploadController.Destination? {
        guard let host = hostStore.selectedHost else { return nil }
        return UploadController.Destination(
            host: host,
            secret: hostStore.secret(for: host.id),
            deviceID: hostStore.deviceID
        )
    }

    private func snapshot(of pencilDrawing: PKDrawing) -> DrawingSnapshot {
        DrawingSnapshot.fromPencilDrawing(
            pencilDrawing,
            canvasSize: canvasSize,
            page: pageStore.selectedPage?.pageRef
        )
    }

    private func uploadCurrentPage() {
        guard let destination = destination() else { return }
        uploader.uploadNow(snapshot: snapshot(of: drawing), to: destination)
    }

    private func switchTo(pageID: String) {
        showingPages = false
        guard pageID != pageStore.selectedPageID else { return }

        // The outgoing page goes up before the canvas swaps, so switching away
        // never leaves an edit behind.
        pageStore.updateSelectedPage(with: drawing)
        uploadCurrentPage()

        pageStore.select(pageID)
        drawing = pageStore.selectedDrawing
        clearSignal += 1
    }

    private func addPage() {
        showingPages = false
        pageStore.updateSelectedPage(with: drawing)
        uploadCurrentPage()

        pageStore.addPage()
        drawing = PKDrawing()
        clearSignal += 1
    }

    private var setupView: some View {
        VStack(alignment: .leading, spacing: 24) {
            Spacer()

            VStack(alignment: .leading, spacing: 8) {
                Text("GoghMode Companion")
                    .font(.largeTitle.bold())
                Text("Open Devices in GoghMode on your desktop, tap Pair a device, and scan the code it shows. Your notes go to the host you choose — never to more than one.")
                    .font(.body)
                    .foregroundStyle(.secondary)
            }

            HostListView(hostStore: hostStore)
                .frame(maxHeight: 320)

            Button {
                showingSettings = false
            } label: {
                Text(hostStore.selectedHost == nil ? "Pair a host to begin" : "Open notebook")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .disabled(hostStore.selectedHost == nil)

            Spacer()
        }
        .padding(32)
        .frame(maxWidth: 640)
    }

    private var drawingView: some View {
        VStack(spacing: 0) {
            toolbar

            if let message = uploader.pagesUnsupportedMessage {
                Text(message)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 16)
                    .padding(.bottom, 8)
                    .background(.regularMaterial)
            }

            GeometryReader { geometry in
                PencilCanvasView(drawing: $drawing, reloadSignal: $clearSignal) { newDrawing, newCanvasSize in
                    canvasSize = newCanvasSize == .zero ? geometry.size : newCanvasSize
                    pageStore.updateSelectedPage(with: newDrawing)
                    if let destination = destination() {
                        uploader.schedule(snapshot: snapshot(of: newDrawing), to: destination)
                    }
                }
                .ignoresSafeArea(edges: .bottom)
                .onAppear {
                    canvasSize = geometry.size
                }
            }
        }
        .sheet(isPresented: $showingPages) {
            pageOverview
        }
        .sheet(isPresented: $showingHosts) {
            HostListView(hostStore: hostStore)
        }
    }

    /// The destination, always on screen. With more than one host saved, the
    /// question "where did that drawing go?" must never need asking.
    private var hostChip: some View {
        Button {
            showingHosts = true
        } label: {
            HStack(spacing: 6) {
                Image(systemName: hostStore.selectedHost?.isPaired == true ? "lock.fill" : "link")
                    .font(.caption)
                Text(hostStore.selectedHost?.name ?? "No host")
                    .font(.subheadline.weight(.semibold))
                    .lineLimit(1)
                if hostStore.hosts.count > 1 {
                    Image(systemName: "chevron.up.chevron.down").font(.caption2)
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(.thinMaterial, in: Capsule())
        }
        .buttonStyle(.plain)
    }

    private var toolbar: some View {
        HStack(spacing: 12) {
            statusBadge
            hostChip

            if uploader.pagesSupported {
                Button {
                    showingPages = true
                } label: {
                    Label(
                        pageStore.selectedPage?.title ?? "Pages",
                        systemImage: "square.stack"
                    )
                    .lineLimit(1)
                }
                .buttonStyle(.bordered)

                Button {
                    addPage()
                } label: {
                    Label("New page", systemImage: "plus")
                }
                .buttonStyle(.bordered)
            }

            Spacer()

            Button("Save Now") {
                uploadCurrentPage()
            }
            .buttonStyle(.borderedProminent)

            Button("Clear") {
                drawing = PKDrawing()
                clearSignal += 1
                pageStore.updateSelectedPage(with: drawing)
                if let destination = destination() {
                    uploader.uploadNow(
                        snapshot: DrawingSnapshot.empty(
                            canvasSize: canvasSize,
                            page: pageStore.selectedPage?.pageRef
                        ),
                        to: destination
                    )
                }
            }
            .buttonStyle(.bordered)

            Button("Settings") {
                showingSettings = true
            }
            .buttonStyle(.bordered)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(.regularMaterial)
    }

    private var pageOverview: some View {
        NavigationStack {
            List(pageStore.pages) { page in
                Button {
                    switchTo(pageID: page.id)
                } label: {
                    HStack(spacing: 12) {
                        thumbnail(for: page)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(page.title)
                                .font(.body.weight(.medium))
                            Text("\(page.drawing.strokes.count) strokes")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        if page.id == pageStore.selectedPageID {
                            Image(systemName: "checkmark.circle.fill")
                                .foregroundStyle(.tint)
                        }
                    }
                }
                .buttonStyle(.plain)
            }
            .navigationTitle("Pages")
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button("New page") {
                        addPage()
                    }
                }
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") {
                        showingPages = false
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func thumbnail(for page: NotebookPage) -> some View {
        let bounds = CGRect(origin: .zero, size: CGSize(width: 160, height: 110))
        Image(uiImage: page.drawing.image(from: bounds, scale: 1))
            .resizable()
            .aspectRatio(contentMode: .fit)
            .frame(width: 80, height: 55)
            .background(Color.white)
            .clipShape(RoundedRectangle(cornerRadius: 6))
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .stroke(Color.secondary.opacity(0.3))
            )
    }

    private var statusBadge: some View {
        Button {
            uploader.retry()
        } label: {
            HStack(spacing: 8) {
                Circle()
                    .fill(statusColor)
                    .frame(width: 10, height: 10)
                Text(uploader.status.label)
                    .font(.subheadline.weight(.semibold))
                if case .failed(let message) = uploader.status {
                    Text(message)
                        .font(.caption)
                        .lineLimit(1)
                        .foregroundStyle(.secondary)
                }
                if case .wrongHost(let message) = uploader.status {
                    Text(message)
                        .font(.caption)
                        .lineLimit(2)
                        .foregroundStyle(.red)
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(.thinMaterial, in: Capsule())
        }
        .buttonStyle(.plain)
        .disabled(!uploader.canRetry)
    }

    private var statusColor: Color {
        switch uploader.status {
        case .idle, .saved:
            .green
        case .waiting, .saving:
            .orange
        case .failed:
            .red
        case .wrongHost:
            .red
        }
    }
}

#Preview {
    ContentView()
}
