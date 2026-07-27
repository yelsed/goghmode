import PencilKit
import SwiftUI

struct PencilCanvasView: UIViewRepresentable {
    @Binding var drawing: PKDrawing
    /// Bumped whenever the canvas should adopt `drawing` wholesale — clearing
    /// it, or switching to another page.
    @Binding var reloadSignal: Int

    var onDrawingChanged: (PKDrawing, CGSize) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeUIView(context: Context) -> PKCanvasView {
        let canvasView = PKCanvasView()
        canvasView.delegate = context.coordinator
        canvasView.backgroundColor = .white
        context.coordinator.load(drawing, into: canvasView)
        // `.default` respects the system pencil-only preference while the tool
        // picker is visible, so palm and finger taps stop leaving stray dots.
        // The picker exposes a toggle for people drawing without a Pencil.
        canvasView.drawingPolicy = .default
        canvasView.alwaysBounceHorizontal = false
        canvasView.alwaysBounceVertical = false
        canvasView.minimumZoomScale = 1
        canvasView.maximumZoomScale = 1
        canvasView.contentInsetAdjustmentBehavior = .never

        // PKCanvasView conforms to PKToolPickerObserver, so observing the picker
        // is all it takes for pen, eraser, lasso, colors and widths to work.
        let toolPicker = context.coordinator.toolPicker
        toolPicker.addObserver(canvasView)
        toolPicker.setVisible(true, forFirstResponder: canvasView)

        context.coordinator.lastReloadSignal = reloadSignal
        return canvasView
    }

    func updateUIView(_ canvasView: PKCanvasView, context: Context) {
        context.coordinator.parent = self

        // The picker only appears for the first responder, and a view cannot
        // become one until it is in a window.
        if canvasView.window != nil && !canvasView.isFirstResponder {
            canvasView.becomeFirstResponder()
        }

        if context.coordinator.lastReloadSignal != reloadSignal {
            context.coordinator.lastReloadSignal = reloadSignal
            context.coordinator.load(drawing, into: canvasView)
            return
        }
    }

    final class Coordinator: NSObject, PKCanvasViewDelegate {
        var parent: PencilCanvasView
        var lastReloadSignal = 0

        // Held here on purpose: a released PKToolPicker takes the palette with it.
        let toolPicker: PKToolPicker = {
            let picker = PKToolPicker()
            picker.stateAutosaveName = "goghModeToolPicker"
            return picker
        }()

        /// PencilKit reports a drawing the app assigns through the same delegate
        /// call it uses for one the pencil made. Taking that echo for an edit is
        /// what blanked a sheet the moment it was opened: the canvas is built
        /// empty, and the echo wrote that emptiness back over the page and sent
        /// it to the host.
        private var isLoading = false

        init(parent: PencilCanvasView) {
            self.parent = parent
        }

        /// Puts a drawing on the canvas without it counting as an edit.
        func load(_ newDrawing: PKDrawing, into canvasView: PKCanvasView) {
            isLoading = true
            canvasView.drawing = newDrawing
            // The callback can arrive after the assignment returns, so the flag
            // is lowered a runloop later rather than on the next line.
            DispatchQueue.main.async { [weak self] in
                self?.isLoading = false
            }
        }

        func canvasViewDrawingDidChange(_ canvasView: PKCanvasView) {
            guard !isLoading else { return }
            parent.drawing = canvasView.drawing
            parent.onDrawingChanged(canvasView.drawing, canvasView.bounds.size)
        }
    }
}
