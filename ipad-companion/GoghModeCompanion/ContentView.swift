import PencilKit
import SwiftUI

struct ContentView: View {
    @AppStorage("goghModeEndpoint") private var endpointText = ""
    @StateObject private var uploader = UploadController()
    @State private var drawing = PKDrawing()
    @State private var canvasSize = CGSize(width: 1024, height: 1366)
    @State private var clearSignal = 0
    @State private var showingSettings = false

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
    }

    private var setupView: some View {
        VStack(alignment: .leading, spacing: 24) {
            Spacer()

            VStack(alignment: .leading, spacing: 8) {
                Text("GoghMode Companion")
                    .font(.largeTitle.bold())
                Text("Paste the Mac mobile URL from GoghMode. The app will send your PencilKit drawing to the Mac as `drawings/latest.*`.")
                    .font(.body)
                    .foregroundStyle(.secondary)
            }

            VStack(alignment: .leading, spacing: 8) {
                Text("Mac URL")
                    .font(.headline)
                TextField("http://192.168.1.10:8787/token/", text: $endpointText)
                    .keyboardType(.URL)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .textFieldStyle(.roundedBorder)
                if !endpointText.isEmpty && endpoint == nil {
                    Text("Use the full mobile URL from the Mac app. It must start with http:// or https://.")
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

            GeometryReader { geometry in
                PencilCanvasView(drawing: $drawing, clearSignal: $clearSignal) { newDrawing, newCanvasSize in
                    canvasSize = newCanvasSize == .zero ? geometry.size : newCanvasSize
                    let snapshot = DrawingSnapshot.fromPencilDrawing(newDrawing, canvasSize: canvasSize)
                    uploader.schedule(snapshot: snapshot, endpointText: endpointText)
                }
                .ignoresSafeArea(edges: .bottom)
                .onAppear {
                    canvasSize = geometry.size
                }
            }
        }
    }

    private var toolbar: some View {
        HStack(spacing: 12) {
            statusBadge

            Spacer()

            Button("Save Now") {
                let snapshot = DrawingSnapshot.fromPencilDrawing(drawing, canvasSize: canvasSize)
                uploader.uploadNow(snapshot: snapshot, endpointText: endpointText)
            }
            .buttonStyle(.borderedProminent)

            Button("Clear") {
                drawing = PKDrawing()
                clearSignal += 1
                let snapshot = DrawingSnapshot.empty(canvasSize: canvasSize)
                uploader.uploadNow(snapshot: snapshot, endpointText: endpointText)
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

    private var statusBadge: some View {
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
