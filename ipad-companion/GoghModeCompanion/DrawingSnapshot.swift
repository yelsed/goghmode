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

struct CanvasSize: Codable, Equatable {
    let width: Double
    let height: Double
    let background: String
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
                background: "#ffffff"
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

    static func fromPencilDrawing(
        _ drawing: PKDrawing,
        canvasSize: CGSize,
        page: PageRef? = nil
    ) -> DrawingSnapshot {
        let width = max(1.0, canvasSize.width)
        let height = max(1.0, canvasSize.height)
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
            schemaVersion: currentSchemaVersion,
            page: page,
            canvas: CanvasSize(width: Double(width), height: Double(height), background: "#ffffff"),
            strokes: strokes
        )
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
