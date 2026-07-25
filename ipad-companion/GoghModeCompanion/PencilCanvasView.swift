import PencilKit
import SwiftUI

struct PencilCanvasView: UIViewRepresentable {
    @Binding var drawing: PKDrawing
    @Binding var clearSignal: Int

    var onDrawingChanged: (PKDrawing, CGSize) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeUIView(context: Context) -> PKCanvasView {
        let canvasView = PKCanvasView()
        canvasView.delegate = context.coordinator
        canvasView.backgroundColor = .white
        canvasView.drawing = drawing
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

        context.coordinator.lastClearSignal = clearSignal
        return canvasView
    }

    func updateUIView(_ canvasView: PKCanvasView, context: Context) {
        context.coordinator.parent = self

        // The picker only appears for the first responder, and a view cannot
        // become one until it is in a window.
        if canvasView.window != nil && !canvasView.isFirstResponder {
            canvasView.becomeFirstResponder()
        }

        if context.coordinator.lastClearSignal != clearSignal {
            context.coordinator.lastClearSignal = clearSignal
            canvasView.drawing = PKDrawing()
            drawing = canvasView.drawing
            onDrawingChanged(canvasView.drawing, canvasView.bounds.size)
            return
        }
    }

    final class Coordinator: NSObject, PKCanvasViewDelegate {
        var parent: PencilCanvasView
        var lastClearSignal = 0

        // Held here on purpose: a released PKToolPicker takes the palette with it.
        let toolPicker: PKToolPicker = {
            let picker = PKToolPicker()
            picker.stateAutosaveName = "goghModeToolPicker"
            return picker
        }()

        init(parent: PencilCanvasView) {
            self.parent = parent
        }

        func canvasViewDrawingDidChange(_ canvasView: PKCanvasView) {
            parent.drawing = canvasView.drawing
            parent.onDrawingChanged(canvasView.drawing, canvasView.bounds.size)
        }
    }
}
