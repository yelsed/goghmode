import PencilKit
import SwiftUI

struct PencilCanvasView: UIViewRepresentable {
    @Binding var drawing: PKDrawing
    /// Bumped whenever the canvas should adopt `drawing` wholesale — clearing
    /// it, switching to another page, or stepping back through its history.
    @Binding var reloadSignal: Int

    var ruling: SheetRuling?
    var onDrawingChanged: (PKDrawing) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeUIView(context: Context) -> SheetView {
        let sheet = SheetView()
        let canvasView = sheet.canvas
        canvasView.delegate = context.coordinator
        // Clear rather than white, so the ruling behind it shows through.
        canvasView.backgroundColor = .clear
        context.coordinator.load(drawing, into: canvasView)
        // `.default` respects the system pencil-only preference while the tool
        // picker is visible, so palm and finger taps stop leaving stray dots.
        // The picker exposes a toggle for people drawing without a Pencil.
        canvasView.drawingPolicy = .default
        canvasView.alwaysBounceHorizontal = false
        canvasView.alwaysBounceVertical = false
        canvasView.contentInsetAdjustmentBehavior = .never
        canvasView.showsVerticalScrollIndicator = false
        canvasView.showsHorizontalScrollIndicator = false

        // PKCanvasView conforms to PKToolPickerObserver, so observing the picker
        // is all it takes for pen, eraser, lasso, colors and widths to work.
        let toolPicker = context.coordinator.toolPicker
        toolPicker.addObserver(canvasView)
        toolPicker.setVisible(true, forFirstResponder: canvasView)

        sheet.ruling.ruling = ruling
        context.coordinator.lastReloadSignal = reloadSignal
        return sheet
    }

    func updateUIView(_ sheet: SheetView, context: Context) {
        context.coordinator.parent = self
        sheet.ruling.ruling = ruling

        // The picker only appears for the first responder, and a view cannot
        // become one until it is in a window.
        if sheet.canvas.window != nil && !sheet.canvas.isFirstResponder {
            sheet.canvas.becomeFirstResponder()
        }

        if context.coordinator.lastReloadSignal != reloadSignal {
            context.coordinator.lastReloadSignal = reloadSignal
            context.coordinator.load(drawing, into: sheet.canvas)
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
            parent.onDrawingChanged(canvasView.drawing)
        }

        /// `PKCanvasViewDelegate` inherits from `UIScrollViewDelegate`, so the pan
        /// and zoom PencilKit drives are reported here. The ruling belongs to the
        /// page, not to the screen, so it has to follow both.
        func scrollViewDidZoom(_ scrollView: UIScrollView) {
            guard let canvas = scrollView as? SheetCanvasView else { return }
            canvas.sizeContentToPage()
            (canvas.superview as? SheetView)?.alignRulingToPage()
        }

        func scrollViewDidScroll(_ scrollView: UIScrollView) {
            guard let canvas = scrollView as? SheetCanvasView else { return }
            (canvas.superview as? SheetView)?.alignRulingToPage()
        }
    }
}

/// One sheet: the ruling it was written against, and the ink on top of it.
final class SheetView: UIView {
    let ruling = SheetRulingView()
    let canvas = SheetCanvasView()

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .white
        ruling.backgroundColor = .clear
        addSubview(ruling)
        addSubview(canvas)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("SheetView is created in code only")
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        ruling.frame = bounds
        canvas.frame = bounds
        alignRulingToPage()
    }

    /// Where the page currently sits on screen, so the rules land under the ink
    /// rather than sliding against it.
    func alignRulingToPage() {
        ruling.pageRect = CGRect(
            x: -canvas.contentOffset.x,
            y: -canvas.contentOffset.y,
            width: SheetPage.size.width * canvas.zoomScale,
            height: SheetPage.size.height * canvas.zoomScale
        )
    }
}

/// The rules themselves. Drawn rather than tiled, because seventy-odd lines cost
/// nothing and a tiled pattern would not stay pinned to the page under zoom.
final class SheetRulingView: UIView {
    /// In page units, matching `RULING_INK`'s companions in `src/export.rs`: the
    /// exporter strokes a one-unit rule and fills a one-unit dot radius.
    static let ruleWidth: CGFloat = 1
    static let dotRadius: CGFloat = 1

    var ruling: SheetRuling? {
        didSet {
            guard ruling != oldValue else { return }
            setNeedsDisplay()
        }
    }

    var pageRect: CGRect = .zero {
        didSet {
            guard pageRect != oldValue, ruling != nil else { return }
            setNeedsDisplay()
        }
    }

    override func draw(_ rect: CGRect) {
        guard let ruling, let context = UIGraphicsGetCurrentContext() else { return }
        guard pageRect.width > 0, pageRect.height > 0 else { return }

        let scale = pageRect.width / SheetPage.size.width
        let spacing = ruling.spacing * scale
        guard spacing > 1 else { return }

        context.setFillColor(Sheet.rulingInk.cgColor)
        context.setStrokeColor(Sheet.rulingInk.cgColor)
        // Everything here is a page measurement scaled to the screen, never a
        // screen measurement. The exporter draws a one-unit rule on the page, so
        // a fixed one-point rule here would be heavier than its own export at
        // anything but 1:1, and the promise is that the two match.
        context.setLineWidth(SheetRulingView.ruleWidth * scale)

        let down = stops(upTo: pageRect.height, every: spacing).map { pageRect.minY + $0 }
        let across = stops(upTo: pageRect.width, every: spacing).map { pageRect.minX + $0 }

        switch ruling.style {
        case .lines:
            for y in down {
                context.stroke(CGRect(x: pageRect.minX, y: y, width: pageRect.width, height: 0))
            }
        case .grid:
            for y in down {
                context.stroke(CGRect(x: pageRect.minX, y: y, width: pageRect.width, height: 0))
            }
            for x in across {
                context.stroke(CGRect(x: x, y: pageRect.minY, width: 0, height: pageRect.height))
            }
        case .dots:
            let radius = SheetRulingView.dotRadius * scale
            for y in down {
                for x in across {
                    context.fillEllipse(
                        in: CGRect(x: x - radius, y: y - radius, width: radius * 2, height: radius * 2)
                    )
                }
            }
        }
    }

    /// The first rule is one space in, matching the exporter, so the page does not
    /// start on a line sitting against its own edge.
    private func stops(upTo extent: CGFloat, every spacing: CGFloat) -> [CGFloat] {
        var stops: [CGFloat] = []
        var at = spacing
        while at < extent {
            stops.append(at)
            at += spacing
        }
        return stops
    }
}

/// A canvas whose content is a sheet of paper rather than the view it happens to
/// be shown in.
///
/// Before this the drawing area was the view bounds, so a sheet changed shape with
/// the way the iPad was held, the exported page did too, and there was nothing to
/// zoom into. The page is now one fixed size and the view is a window onto it.
final class SheetCanvasView: PKCanvasView {
    /// How far past fitting the page the canvas will go. Four is enough to write
    /// a word inside a diagram without it turning into a microscope.
    private static let deepestZoom: CGFloat = 4

    private var lastFittedBounds: CGRect = .zero

    override func layoutSubviews() {
        super.layoutSubviews()
        fitPageToBounds()
        sizeContentToPage()
    }

    func sizeContentToPage() {
        contentSize = CGSize(
            width: SheetPage.size.width * zoomScale,
            height: SheetPage.size.height * zoomScale
        )
    }

    /// Recomputed only when the bounds actually change, because assigning
    /// `zoomScale` lays out again and would otherwise never settle.
    private func fitPageToBounds() {
        guard bounds.width > 0, bounds.height > 0, bounds != lastFittedBounds else { return }
        lastFittedBounds = bounds

        // Someone who has zoomed in keeps their scale across a rotation. Someone
        // looking at the whole page keeps seeing the whole page.
        let wasShowingWholePage = zoomScale <= minimumZoomScale + 0.001

        let fit = min(
            bounds.width / SheetPage.size.width,
            bounds.height / SheetPage.size.height
        )
        minimumZoomScale = fit
        maximumZoomScale = fit * SheetCanvasView.deepestZoom
        if wasShowingWholePage {
            zoomScale = fit
        }
    }
}
