// DIRECTION CONTRACT — Drawing Set
//
// THESIS: pages are sheets in a drawing set, and one sheet is stamped ISSUED —
// the one the agent reads. Refuses the neutral dark thumbnail grid every gallery
// app ships, and refuses cream-paper notebook skeuomorphism.
// OWN-WORLD: white sheets on a warm grey register ground, hairline rules, ruled
// title blocks with labelled cells, SF Mono only for sheet numbers. One
// saturated colour exists: rubber-stamp red, spent solely on the issue stamp.
// STORY: every page I drew is here and named, and I can see from across the desk
// which one Claude is reading — because I stamped it.
// FIRST VIEWPORT: large title "Pages", a mono status line naming the issued
// sheet, then a grid of white sheets each carrying its title block. Exactly one
// wears a rotated stamp.
// FORM: drawing set / sheet register. Ranked #2 of seven; taken after the roll
// was re-rolled off Contact Sheet. Seed unavailable — catalog unreachable.

import SwiftUI
import UIKit

/// The Drawing Set world, as tokens. Values are normative here and in DESIGN.md;
/// nothing in the app should invent its own paper or ink.
///
/// Every colour is a dynamic pair. Light is the primary appearance — this is used
/// at a desk in daylight and the drawings are black on white, so dark chrome
/// would fight its own content. Dark is the same set on a light table at night.
enum Sheet {
    private static func dynamic(
        light: (CGFloat, CGFloat, CGFloat),
        dark: (CGFloat, CGFloat, CGFloat)
    ) -> Color {
        Color(UIColor { traits in
            let (red, green, blue) = traits.userInterfaceStyle == .dark ? dark : light
            return UIColor(red: red, green: green, blue: blue, alpha: 1)
        })
    }

    /// Carries the drawing. Stays near-white in dark mode too: a drawing is a
    /// white sheet, and inverting it would misrepresent what was drawn.
    static let paper = dynamic(light: (1, 1, 1), dark: (0.925, 0.914, 0.890))

    /// The surface sheets sit on. Never white, so sheets read as objects on top.
    static let ground = dynamic(light: (0.929, 0.918, 0.894), dark: (0.075, 0.071, 0.067))

    static let edge = dynamic(light: (0.847, 0.831, 0.800), dark: (0.267, 0.255, 0.239))
    static let rule = dynamic(light: (0.659, 0.635, 0.604), dark: (0.612, 0.592, 0.561))
    static let ruleHair = dynamic(light: (0.788, 0.769, 0.733), dark: (0.729, 0.710, 0.678))

    /// Rubber-stamp ink. The only saturated colour in the app, and it belongs to
    /// the issue stamp alone — a second red thing destroys the one signal the
    /// design exists to carry.
    static let stamp = dynamic(light: (0.706, 0.200, 0.122), dark: (0.769, 0.243, 0.161))
    /// The lesser stamp: sent for review, without moving the pin.
    static let review = dynamic(light: (0.169, 0.361, 0.541), dark: (0.216, 0.435, 0.635))

    /// Ink sits on paper, which stays light in both appearances, so these do not
    /// invert with the ground.
    static let ink = dynamic(light: (0.102, 0.098, 0.090), dark: (0.102, 0.098, 0.090))
    /// 8.9:1 on paper.
    static let inkSecondary = dynamic(light: (0.290, 0.275, 0.251), dark: (0.290, 0.275, 0.251))
    /// 4.6:1 on paper — the floor for title-block field labels. Deliberately not
    /// `.secondary`, which is what made the old settings screen unreadable.
    static let inkLabel = dynamic(light: (0.420, 0.400, 0.369), dark: (0.420, 0.400, 0.369))

    /// Text that sits directly on `ground` rather than on a sheet.
    static let onGround = dynamic(light: (0.102, 0.098, 0.090), dark: (0.949, 0.937, 0.914))
    /// 5.3:1 on ground in both appearances.
    static let onGroundSecondary =
        dynamic(light: (0.361, 0.345, 0.318), dark: (0.678, 0.659, 0.627))

    static let sheetRadius: CGFloat = 2
    static let controlRadius: CGFloat = 8
    static let hair: CGFloat = 1
    static let block: CGFloat = 8
    static let gutter: CGFloat = 20
    static let margin: CGFloat = 28
}

/// Drafting lettering: small, uppercase, widely tracked. Labels fields, never
/// runs as prose.
struct BlockLabel: View {
    let text: String

    var body: some View {
        Text(text.uppercased())
            .font(.system(size: 10, weight: .semibold))
            .tracking(0.8)
            .foregroundStyle(Sheet.inkLabel)
            .lineLimit(1)
    }
}

/// Sheet numbers are monospaced because they are an index that must align down a
/// column, not because monospace looks technical.
struct SheetNumber: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 12, weight: .medium, design: .monospaced))
            .foregroundStyle(Sheet.inkSecondary)
            .lineLimit(1)
    }
}

/// One labelled cell of a title block.
struct BlockField<Content: View>: View {
    let label: String
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            BlockLabel(text: label)
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// The ruled form along the bottom edge of every sheet. It is part of the sheet,
/// not a caption underneath a card, which is why it carries its own top rule and
/// no padding gap above it.
struct TitleBlock: View {
    let number: String
    let name: String
    let date: String
    let strokes: Int

    var body: some View {
        VStack(spacing: 0) {
            Rectangle()
                .fill(Sheet.rule)
                .frame(height: Sheet.hair)

            HStack(spacing: 0) {
                BlockField(label: "Sheet") {
                    SheetNumber(text: number)
                }
                .frame(width: 62, alignment: .leading)

                cellRule

                BlockField(label: "Name") {
                    Text(name)
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(Sheet.ink)
                        .lineLimit(1)
                        .truncationMode(.tail)
                }
                .padding(.horizontal, Sheet.block)

                cellRule

                BlockField(label: "Strokes") {
                    SheetNumber(text: String(strokes))
                }
                .frame(width: 54, alignment: .leading)
                .padding(.leading, Sheet.block)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
        }
        .background(Sheet.paper)
    }

    private var cellRule: some View {
        Rectangle()
            .fill(Sheet.ruleHair)
            .frame(width: Sheet.hair)
            .frame(maxHeight: .infinity)
    }
}

/// The stamp that says which sheet is built from. Rotated off-axis with uneven
/// ink, because a rubber stamp is pressed by hand — a straight, evenly filled
/// red pill would be a button wearing the world's clothes.
struct IssueStamp: View {
    var text = "Issued"
    var tint: Color = Sheet.stamp
    var scale: CGFloat = 1

    var body: some View {
        Text(text.uppercased())
            .font(.system(size: 13 * scale, weight: .heavy))
            .tracking(1.3 * scale)
            .foregroundStyle(tint)
            .padding(.horizontal, 10 * scale)
            .padding(.vertical, 6 * scale)
            .overlay {
                Rectangle()
                    .strokeBorder(tint, lineWidth: 2 * scale)
            }
            .opacity(0.88)
            .rotationEffect(.degrees(-3.5))
            .accessibilityLabel(Text("\(text). This is the page Claude reads."))
    }
}
