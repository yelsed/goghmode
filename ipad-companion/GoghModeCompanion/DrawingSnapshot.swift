import CoreGraphics
import Foundation
import PencilKit
import UIKit

/// Mirrors `DrawingSnapshot` in `src/drawing.rs`, validated by `check_snapshot`
/// in `src/mobile_server.rs`. The two definitions must stay in step.
struct DrawingSnapshot: Codable, Equatable {
    let schemaVersion: Int
    let page: PageRef?
    let canvas: CanvasSize
    let strokes: [Stroke]
}

struct PageRef: Codable, Equatable {
    let id: String
    let title: String?
}

let currentSchemaVersion = 2
let pagelessSchemaVersion = 1
/// The version that can carry ruling. Only sent by a sheet that has some, so a
/// plain sheet never needs a host new enough to understand it.
let ruledSchemaVersion = 3

/// The sheet itself, in page units.
///
/// The drawing area used to be whatever the view bounds happened to be, so a sheet
/// changed shape with the way the iPad was held and there was nothing to zoom into.
/// A sheet is a sheet of paper: one size, portrait, the same on every device.
enum SheetPage {
    static let size = CGSize(width: 1024, height: 1366)
}

struct CanvasSize: Codable, Equatable {
    let width: Double
    let height: Double
    let background: String
    /// Absent on a plain sheet, which is what an unruled sheet has always sent.
    /// A `var` so the memberwise initialiser defaults it, and every call that
    /// builds a plain canvas reads as it always did.
    var ruling: SheetRuling?
}

/// What the sheet was written against. A writing aid the person chose per sheet,
/// drawn under the ink here and again at export, so the page the agent reads is
/// the page that was drawn on. See ADR-0007.
struct SheetRuling: Codable, Equatable, Hashable {
    var style: Style
    var spacing: Double

    enum Style: String, Codable, CaseIterable, Identifiable {
        case lines
        case grid
        case dots

        var id: String { rawValue }

        var label: String {
            switch self {
            case .lines: "Lines"
            case .grid: "Grid"
            case .dots: "Dots"
            }
        }

        var symbol: String {
            switch self {
            case .lines: "line.3.horizontal"
            case .grid: "grid"
            case .dots: "circle.grid.3x3"
            }
        }
    }

    /// One rhythm shared by all three styles, in page units, matching the range
    /// the host accepts.
    static let defaultSpacing: Double = 32

    init(style: Style, spacing: Double = SheetRuling.defaultSpacing) {
        self.style = style
        self.spacing = spacing
    }
}

struct Stroke: Codable, Equatable, Identifiable {
    let id: String
    let color: String
    let width: Double
    let points: [Point]
}

struct Point: Codable, Equatable {
    let x: Double
    let y: Double
    let pressure: Double
    let t: UInt64
}

extension DrawingSnapshot {
    static func empty(canvasSize: CGSize, page: PageRef? = nil) -> DrawingSnapshot {
        DrawingSnapshot(
            schemaVersion: currentSchemaVersion,
            page: page,
            canvas: CanvasSize(
                width: Double(max(1.0, canvasSize.width)),
                height: Double(max(1.0, canvasSize.height)),
                background: "#ffffff",
                ruling: nil
            ),
            strokes: []
        )
    }

    /// A host that predates pages rejects anything above version 1, so the app
    /// sends the same drawing without its page rather than not at all.
    func withoutPage() -> DrawingSnapshot {
        DrawingSnapshot(
            schemaVersion: pagelessSchemaVersion,
            page: nil,
            canvas: canvas,
            strokes: strokes
        )
    }

    /// The same drawing on a plain sheet, for a host that predates ruling. The
    /// strokes are what matter; the rules are the aid they were made against.
    func withoutRuling() -> DrawingSnapshot {
        guard canvas.ruling != nil else { return self }
        return DrawingSnapshot(
            schemaVersion: min(schemaVersion, currentSchemaVersion),
            page: page,
            canvas: CanvasSize(
                width: canvas.width,
                height: canvas.height,
                background: canvas.background,
                ruling: nil
            ),
            strokes: strokes
        )
    }

    static func fromPencilDrawing(
        _ drawing: PKDrawing,
        canvasSize: CGSize,
        page: PageRef? = nil,
        ruling: SheetRuling? = nil
    ) -> DrawingSnapshot {
        // Grown to cover anything drawn past the page rather than clamped to it: a
        // sheet written on before the page had a fixed size, on an iPad held in
        // landscape, would otherwise have every stroke past the edge flattened onto
        // it, and the host rejects points outside the canvas anyway.
        let covering = canvasSize.covering(drawing.bounds)
        let width = max(1.0, covering.width)
        let height = max(1.0, covering.height)
        let strokes = drawing.strokes.enumerated().compactMap { strokeIndex, pencilStroke -> Stroke? in
            let points = pencilStroke.path.enumerated().map { pointIndex, strokePoint in
                // Full Double precision costs ~250 bytes per point on the wire and
                // the host stores f32 regardless, so the extra digits are discarded
                // after inflating every upload. Rounding happens before clamping so
                // a rounded-up value can never land outside the canvas.
                Point(
                    x: roundedToHundredths(strokePoint.location.x).clamped(to: 0...Double(width)),
                    y: roundedToHundredths(strokePoint.location.y).clamped(to: 0...Double(height)),
                    pressure: roundedToThousandths(strokePoint.force).clamped(to: 0...1),
                    t: UInt64(max(0, strokePoint.timeOffset * 1000)) + UInt64(pointIndex)
                )
            }

            guard !points.isEmpty else { return nil }

            return Stroke(
                id: "stroke-\(strokeIndex + 1)",
                color: pencilStroke.ink.color.hexRGB,
                width: Double(averagePointWidth(in: pencilStroke.path).clamped(to: 1...80)),
                points: points
            )
        }

        return DrawingSnapshot(
            // Only a ruled sheet asks for the newer version, so nothing changes for
            // anyone drawing on plain paper against an older host.
            schemaVersion: ruling == nil ? currentSchemaVersion : ruledSchemaVersion,
            page: page,
            canvas: CanvasSize(
                width: Double(width),
                height: Double(height),
                background: "#ffffff",
                ruling: ruling
            ),
            strokes: strokes
        )
    }
}

extension CGSize {
    /// This size, grown so the given rect fits inside it. An unusable rect (a
    /// drawing with no strokes reports a null one) leaves the size alone.
    func covering(_ rect: CGRect) -> CGSize {
        guard !rect.isNull, !rect.isInfinite, !rect.isEmpty else { return self }
        return CGSize(width: max(width, rect.maxX), height: max(height, rect.maxY))
    }
}

/// Sub-pixel on a canvas about a thousand points wide, so nothing visible is lost.
private func roundedToHundredths(_ value: CGFloat) -> Double {
    guard value.isFinite else { return 0 }
    return (Double(value) * 100).rounded() / 100
}

private func roundedToThousandths(_ value: CGFloat) -> Double {
    guard value.isFinite else { return 0 }
    return (Double(value) * 1000).rounded() / 1000
}

private func averagePointWidth(in path: PKStrokePath) -> CGFloat {
    var total: CGFloat = 0
    var count: CGFloat = 0

    for point in path {
        total += max(point.size.width, point.size.height)
        count += 1
    }

    if count == 0 {
        return 4
    }

    return total / count
}

private extension Comparable {
    func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}

private extension UIColor {
    var hexRGB: String {
        var red: CGFloat = 0
        var green: CGFloat = 0
        var blue: CGFloat = 0
        var alpha: CGFloat = 0

        guard getRed(&red, green: &green, blue: &blue, alpha: &alpha) else {
            return "#111827"
        }

        return String(
            format: "#%02X%02X%02X",
            Int((red * 255).rounded()),
            Int((green * 255).rounded()),
            Int((blue * 255).rounded())
        )
    }
}
