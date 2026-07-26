import PencilKit
import SwiftUI

struct ContentView: View {
    @Environment(\.scenePhase) private var scenePhase
    @AppStorage("goghModeEndpoint") private var endpointText = ""
    @StateObject private var uploader = UploadController()
    @StateObject private var pageStore = PageStore()
    @State private var drawing = PKDrawing()
    @State private var canvasSize = CGSize(width: 1024, height: 1366)
    @State private var clearSignal = 0
    @State private var showingSettings = false
    @State private var showingPages = false

    private var endpoint: GoghModeEndpoint? {
        GoghModeEndpoint(endpointText)
    }

    var body: some View {
        ZStack {
            Color.white.ignoresSafeArea()

            if endpoint == nil || showingSettings {
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
                Text("Paste the mobile URL from GoghMode on your desktop. The app will send your PencilKit drawing there as `drawings/latest.*`.")
                    .font(.body)
                    .foregroundStyle(.secondary)
            }

            VStack(alignment: .leading, spacing: 8) {
                Text("Desktop URL")
                    .font(.headline)
                TextField("http://192.168.1.10:8787/token/", text: $endpointText)
                    .keyboardType(.URL)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .textFieldStyle(.roundedBorder)
                if !endpointText.isEmpty && endpoint == nil {
                    Text("Use the full mobile URL from the desktop app. It must start with http:// or https://.")
                        .font(.footnote)
                        .foregroundStyle(.red)
                }
            }

            Button {
                showingSettings = false
            } label: {
                Text(endpoint == nil ? "Waiting for valid URL" : "Open notebook")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .disabled(endpoint == nil)

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
                    uploader.schedule(snapshot: snapshot(of: newDrawing), endpointText: endpointText)
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
    }

    private var toolbar: some View {
        HStack(spacing: 12) {
            statusBadge

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
                uploader.uploadNow(
                    snapshot: DrawingSnapshot.empty(
                        canvasSize: canvasSize,
                        page: pageStore.selectedPage?.pageRef
                    ),
                    endpointText: endpointText
                )
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
        }
    }
}

#Preview {
    ContentView()
}
