import PencilKit
import SwiftUI
import UniformTypeIdentifiers

/// The register: the app's home screen. Every sheet in the set as one ruled line,
/// and exactly one wearing the issue stamp. Lines rather than cards because this
/// is a list you scan, not a gallery you browse — thirty sheets should fit on a
/// screen, and the drawing is the thing you open, not the thing you admire here.
struct RegisterView: View {
    @ObservedObject var store: PageStore
    @ObservedObject var uploader: UploadController

    let endpointText: String
    let onOpen: (String) -> Void
    let onNew: () -> Void
    let onSettings: () -> Void

    @State private var renaming: NotebookPage?
    @State private var renamingSeries: PageSeries?
    @State private var draftName = ""
    @State private var openSeries: PageSeries?

    /// Read through the preview cache rather than `page.isEmpty`, which would decode
    /// every stored drawing again on every rebuild.
    private var nothingDrawnYet: Bool {
        store.series.isEmpty
            && store.pages.allSatisfy { SheetPreviewCache.rendered(for: $0).strokeCount == 0 }
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 0) {
                if nothingDrawnYet {
                    EmptyRegister(onStart: { onOpen(store.pages.first?.id ?? store.selectedPageID) })
                        .padding(.top, 40)
                } else {
                    lines
                }
            }
            .padding(.horizontal, Sheet.margin)
            .padding(.bottom, Sheet.margin)
            .frame(maxWidth: 820)
            .frame(maxWidth: .infinity)
        }
        .background(Sheet.ground)
        .navigationTitle("Pages")
        .navigationBarTitleDisplayMode(.large)
        .safeAreaInset(edge: .top, spacing: 0) { registerHead }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button(action: onSettings) {
                    Label("Settings", systemImage: "gearshape")
                }
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button(action: onNew) {
                    Label("New sheet", systemImage: "plus")
                }
            }
        }
        .navigationDestination(item: $openSeries) { series in
            SeriesView(
                store: store,
                uploader: uploader,
                series: series,
                onOpen: onOpen,
                onNew: { onOpen(store.addPage(in: series.id).id) },
                onStamp: toggleStamp,
                onRename: beginRename
            )
        }
        .alert("Name this sheet", isPresented: renamingBinding) {
            TextField("Sheet name", text: $draftName)
            Button("Cancel", role: .cancel) { clearRename() }
            Button("Save") { commitRename() }
        } message: {
            Text("Names show in the register and travel to the Mac with the page.")
        }
    }

    /// The one line that answers "what is Claude reading?" without opening
    /// anything. Sits under the title as a rule, the way a sheet register is
    /// headed.
    private var registerHead: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                BlockLabel(text: "Claude reads")
                if let pinned = store.pinnedPage {
                    Text(pinned.title)
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(Sheet.onGround)
                        .lineLimit(1)
                    SheetNumber(text: store.sheetNumber(for: pinned))
                } else {
                    Text("whichever sheet you drew on last")
                        .font(.footnote)
                        .foregroundStyle(Sheet.onGroundSecondary)
                        .lineLimit(1)
                }
                Spacer(minLength: 0)
                Text("\(store.pages.count) sheets")
                    .font(.caption.monospaced().weight(.medium))
                    .foregroundStyle(Sheet.onGroundSecondary)
            }
            .padding(.horizontal, Sheet.margin)
            .padding(.vertical, 10)

            if let message = uploader.pagesUnsupportedMessage {
                Text(message)
                    .font(.footnote)
                    .foregroundStyle(Sheet.onGround)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, Sheet.margin)
                    .padding(.bottom, 10)
            }

            Rectangle().fill(Sheet.rule).frame(height: Sheet.hair)
        }
        .background(Sheet.ground)
    }

    /// One ruled block of paper, hairline-separated. A drawing set's register is a
    /// ruled index of sheets, so this is a table with aligned columns — not cards,
    /// and not a plain iOS list either.
    private var lines: some View {
        LazyVStack(spacing: 0) {
            RegisterHeader()
            Rectangle().fill(Sheet.rule).frame(height: Sheet.hair)

            ForEach(Array(store.register.enumerated()), id: \.element.id) { index, entry in
                if index > 0 {
                    Rectangle().fill(Sheet.ruleHair).frame(height: Sheet.hair)
                }

                switch entry {
                case .sheet(let page):
                    SheetRow(
                        page: page,
                        number: store.sheetNumber(for: page),
                        isIssued: page.id == store.pinnedPageID,
                        canStamp: uploader.pinningSupported,
                        onOpen: { onOpen(page.id) },
                        onStamp: { toggleStamp(page) }
                    )
                    .onDrag { NSItemProvider(object: page.id as NSString) }
                    .onDrop(
                        of: [UTType.plainText],
                        delegate: StackDrop(store: store, targetID: page.id)
                    )
                    .contextMenu { menu(for: page) }

                case .series(let series, let filed):
                    SeriesRow(
                        series: series,
                        sheets: filed,
                        containsIssued: filed.contains { $0.id == store.pinnedPageID },
                        onOpen: { openSeries = series }
                    )
                    .contextMenu {
                        Button {
                            beginRenameSeries(series)
                        } label: {
                            Label("Rename series", systemImage: "pencil")
                        }
                    }
                }
            }
        }
        .background(Sheet.paper)
        .overlay {
            Rectangle().strokeBorder(Sheet.edge, lineWidth: Sheet.hair)
        }
        .padding(.top, 4)
    }

    @ViewBuilder
    private func menu(for page: NotebookPage) -> some View {
        Button {
            beginRename(page)
        } label: {
            Label("Rename", systemImage: "pencil")
        }

        if uploader.pinningSupported {
            Button {
                send(page)
            } label: {
                Label("Send this one now", systemImage: "paperplane")
            }
        }

        if page.seriesID != nil {
            Button {
                store.removeFromSeries(page.id)
            } label: {
                Label("Take out of series", systemImage: "rectangle.stack.badge.minus")
            }
        }
    }

    private var renamingBinding: Binding<Bool> {
        Binding(
            get: { renaming != nil || renamingSeries != nil },
            set: { if !$0 { clearRename() } }
        )
    }

    private func beginRename(_ page: NotebookPage) {
        draftName = page.title
        renaming = page
    }

    private func beginRenameSeries(_ series: PageSeries) {
        draftName = series.name
        renamingSeries = series
    }

    private func commitRename() {
        if let page = renaming {
            store.rename(page.id, to: draftName)
            // The Mac stores the title with the page, so a rename only reaches it
            // on the next save of that sheet. Send it now when it is the stamped
            // one, so the register and the Mac never disagree about the name
            // Claude is reading.
            if page.id == store.pinnedPageID {
                send(page)
            }
        }
        if let series = renamingSeries {
            store.renameSeries(series.id, to: draftName)
        }
        clearRename()
    }

    private func clearRename() {
        renaming = nil
        renamingSeries = nil
        draftName = ""
    }

    private func toggleStamp(_ page: NotebookPage) {
        let target = page.id == store.pinnedPageID ? nil : page.id
        Task {
            if await uploader.pin(target, endpointText: endpointText) {
                store.recordPin(target)
            }
        }
    }

    private func send(_ page: NotebookPage) {
        Task { _ = await uploader.promote(page.id, endpointText: endpointText) }
    }
}

/// Column widths, shared by the header and every line so numbers, dates and stamps
/// align down their columns. The register is unreadable the moment they drift.
enum RegisterColumn {
    static let issuedBar: CGFloat = 3
    static let lead: CGFloat = 13
    static let preview: CGFloat = 40
    static let previewGap: CGFloat = 13
    static let number: CGFloat = 56
    static let date: CGFloat = 124
    static let strokes: CGFloat = 72
    static let stamp: CGFloat = 104
    static let chevron: CGFloat = 26

    /// Everything left of the `SHEET` column, so the header lines up with the rows.
    static let beforeNumber = issuedBar + lead + preview + previewGap
}

/// The ruled head of the register: the column names, in drafting lettering.
struct RegisterHeader: View {
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        HStack(spacing: 0) {
            Spacer().frame(width: RegisterColumn.beforeNumber)
            BlockLabel(text: "Sheet").frame(width: RegisterColumn.number, alignment: .leading)
            BlockLabel(text: "Name").frame(maxWidth: .infinity, alignment: .leading)
            if sizeClass != .compact {
                BlockLabel(text: "Updated").frame(width: RegisterColumn.date, alignment: .leading)
                BlockLabel(text: "Strokes").frame(width: RegisterColumn.strokes, alignment: .leading)
            }
            BlockLabel(text: "Claude").frame(width: RegisterColumn.stamp, alignment: .leading)
            Spacer().frame(width: RegisterColumn.chevron)
        }
        .padding(.trailing, 13)
        .padding(.vertical, 8)
        .accessibilityHidden(true)
    }
}

/// One line of the register: preview, sheet number, name, facts, and the stamp. The
/// preview sets the row height, so thirty sheets stay scannable no matter how much
/// is drawn on them.
struct SheetRow: View {
    let page: NotebookPage
    let number: String
    let isIssued: Bool
    let canStamp: Bool
    let onOpen: () -> Void
    let onStamp: () -> Void

    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        // One decode and one rasterise per sheet per edit, not per row rebuild —
        // `page.drawing` parses the stored data on every read.
        let rendered = SheetPreviewCache.rendered(for: page)

        return RegisterLine(isIssued: isIssued, onOpen: onOpen) {
            SheetPreview(image: rendered.image)
                .padding(.trailing, RegisterColumn.previewGap)

            SheetNumber(text: number)
                .frame(width: RegisterColumn.number, alignment: .leading)

            Text(page.title)
                .font(.body.weight(.semibold))
                .foregroundStyle(Sheet.ink)
                .lineLimit(1)
                .truncationMode(.tail)
                .frame(maxWidth: .infinity, alignment: .leading)

            if sizeClass != .compact {
                SheetNumber(text: page.updatedAt.formatted(date: .abbreviated, time: .shortened))
                    .frame(width: RegisterColumn.date, alignment: .leading)
                SheetNumber(text: String(rendered.strokeCount))
                    .frame(width: RegisterColumn.strokes, alignment: .leading)
            }
        } trailing: {
            if canStamp {
                StampControl(isIssued: isIssued, scale: 0.68, action: onStamp)
                    .frame(width: RegisterColumn.stamp, alignment: .leading)
            } else {
                Spacer().frame(width: RegisterColumn.stamp)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(
            Text(
                isIssued
                    ? "Sheet \(number), \(page.title). Stamped — this is what Claude reads."
                    : "Sheet \(number), \(page.title)"
            )
        )
    }
}

/// A series as one line: paper behind paper, then the name and how many sheets are
/// filed into it.
struct SeriesRow: View {
    let series: PageSeries
    let sheets: [NotebookPage]
    let containsIssued: Bool
    let onOpen: () -> Void

    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        RegisterLine(isIssued: containsIssued, onOpen: onOpen) {
            ZStack {
                ForEach(
                    Array(sheets.prefix(3).enumerated().reversed()),
                    id: \.element.id
                ) { offset, sheet in
                    SheetPreview(page: sheet)
                        .offset(x: CGFloat(offset) * 3, y: CGFloat(offset) * -3)
                }
            }
            .frame(width: RegisterColumn.preview, height: 54, alignment: .bottomLeading)
            .padding(.trailing, RegisterColumn.previewGap)

            SheetNumber(text: series.prefix)
                .frame(width: RegisterColumn.number, alignment: .leading)

            Text(series.name)
                .font(.body.weight(.semibold))
                .foregroundStyle(Sheet.ink)
                .lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)

            if sizeClass != .compact {
                SheetNumber(text: "\(sheets.count) sheets")
                    .frame(width: RegisterColumn.date, alignment: .leading)
                Spacer().frame(width: RegisterColumn.strokes)
            }
        } trailing: {
            if containsIssued {
                IssueStamp(scale: 0.68)
                    .frame(width: RegisterColumn.stamp, alignment: .leading)
                    .allowsHitTesting(false)
            } else {
                Spacer().frame(width: RegisterColumn.stamp)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(Text("Series \(series.name), \(sheets.count) sheets"))
    }
}

/// The shape every register line shares: an issue bar, tappable columns, then
/// controls that must stay outside the tap target.
struct RegisterLine<Columns: View, Trailing: View>: View {
    let isIssued: Bool
    let onOpen: () -> Void
    @ViewBuilder var columns: Columns
    @ViewBuilder var trailing: Trailing

    var body: some View {
        HStack(spacing: 0) {
            Rectangle()
                .fill(isIssued ? Sheet.stamp : Color.clear)
                .frame(width: RegisterColumn.issuedBar)

            Button(action: onOpen) {
                HStack(spacing: 0) {
                    columns
                }
                .padding(.leading, RegisterColumn.lead)
                .frame(minHeight: 44)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            trailing

            Image(systemName: "chevron.right")
                .font(.footnote.weight(.semibold))
                .foregroundStyle(Sheet.inkLabel)
                .frame(width: RegisterColumn.chevron, alignment: .trailing)
        }
        .padding(.trailing, 13)
        .padding(.vertical, 8)
    }
}

/// The one control that answers "which sheet does Claude read?". Unstamped it is a
/// quiet ruled button; on the stamped sheet the control *is* the stamp, and
/// pressing it lifts the stamp again.
struct StampControl: View {
    let isIssued: Bool
    var scale: CGFloat = 1
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Group {
                if isIssued {
                    IssueStamp(scale: scale)
                } else {
                    HStack(spacing: 5) {
                        Image(systemName: "seal")
                        Text("STAMP").tracking(0.8)
                    }
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(Sheet.inkLabel)
                    .padding(.horizontal, 9)
                    .frame(height: 30)
                    .overlay {
                        Rectangle().strokeBorder(Sheet.rule, lineWidth: Sheet.hair)
                    }
                }
            }
            .frame(minWidth: 44, minHeight: 44)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(
            Text(
                isIssued
                    ? "Stamped for Claude. Press to lift the stamp."
                    : "Stamp this sheet so Claude reads it"
            )
        )
    }
}

/// The sheets filed into one series, as the same ruled lines.
struct SeriesView: View {
    @ObservedObject var store: PageStore
    @ObservedObject var uploader: UploadController
    let series: PageSeries
    let onOpen: (String) -> Void
    let onNew: () -> Void
    let onStamp: (NotebookPage) -> Void
    let onRename: (NotebookPage) -> Void

    var body: some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                RegisterHeader()
                Rectangle().fill(Sheet.rule).frame(height: Sheet.hair)

                ForEach(Array(store.sheets(in: series.id).enumerated()), id: \.element.id) { index, page in
                    if index > 0 {
                        Rectangle().fill(Sheet.ruleHair).frame(height: Sheet.hair)
                    }
                    SheetRow(
                        page: page,
                        number: store.sheetNumber(for: page),
                        isIssued: page.id == store.pinnedPageID,
                        canStamp: uploader.pinningSupported,
                        onOpen: { onOpen(page.id) },
                        onStamp: { onStamp(page) }
                    )
                    .contextMenu {
                        Button { onRename(page) } label: {
                            Label("Rename", systemImage: "pencil")
                        }
                        Button { store.removeFromSeries(page.id) } label: {
                            Label("Take out of series", systemImage: "rectangle.stack.badge.minus")
                        }
                    }
                }
            }
            .background(Sheet.paper)
            .overlay {
                Rectangle().strokeBorder(Sheet.edge, lineWidth: Sheet.hair)
            }
            .padding(.horizontal, Sheet.margin)
            .padding(.vertical, 4)
            .frame(maxWidth: 820)
            .frame(maxWidth: .infinity)
        }
        .background(Sheet.ground)
        .navigationTitle(series.name)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button(action: onNew) {
                    Label("New sheet in series", systemImage: "plus")
                }
            }
        }
    }
}

/// Rendered previews, keyed by page and last write.
///
/// Without this the register re-rasterises every visible sheet on every stroke: it
/// stays alive underneath the canvas in the navigation stack, so each saved stroke
/// republishes the store and rebuilds its rows. Keying on `updatedAt` means an edit
/// invalidates its own entry and nothing has to be told to clear it.
enum SheetPreviewCache {
    struct Rendered {
        let image: UIImage
        let strokeCount: Int
    }

    private final class Entry: NSObject {
        let value: Rendered
        init(_ value: Rendered) { self.value = value }
    }

    // ponytail: NSCache handles the eviction; 200 sheets of 40×54 is trivial memory
    // and the limit only exists so a very long register cannot grow unbounded.
    private static let cache: NSCache<NSString, Entry> = {
        let cache = NSCache<NSString, Entry>()
        cache.countLimit = 200
        return cache
    }()

    static func rendered(for page: NotebookPage) -> Rendered {
        let key = "\(page.id)@\(page.updatedAt.timeIntervalSince1970)" as NSString
        if let hit = cache.object(forKey: key) {
            return hit.value
        }

        let drawing = page.drawing
        let rendered = Rendered(
            image: SheetPreview.render(drawing),
            strokeCount: drawing.strokes.count
        )
        cache.setObject(Entry(rendered), forKey: key)
        return rendered
    }
}

/// A drawing at register scale: small enough for a list line, and rendered so what
/// was drawn actually shows up.
struct SheetPreview: View {
    let image: UIImage
    var width: CGFloat = 40
    var height: CGFloat = 54

    init(page: NotebookPage) {
        self.image = SheetPreviewCache.rendered(for: page).image
    }

    init(image: UIImage) {
        self.image = image
    }

    var body: some View {
        Image(uiImage: image)
            .resizable()
            .aspectRatio(contentMode: .fit)
            .frame(width: width, height: height)
            .background(Sheet.paper)
            .overlay {
                Rectangle().strokeBorder(Sheet.ruleHair, lineWidth: Sheet.hair)
            }
            .accessibilityHidden(true)
    }

    /// Previews came out blank because the source rect was pinned to a portrait
    /// canvas: anything drawn on an iPad held in landscape fell outside it and was
    /// cropped away entirely. The rect grows to cover the drawing instead. It still
    /// starts at a full page so one small stroke in a corner is not blown up to fill
    /// the whole preview.
    ///
    /// The light trait is not a bug fix — `PKInk` resolves its colour when the stroke
    /// is made, so a stored drawing carries fixed colours. It is here so the render
    /// is deterministic: previews sit on `Sheet.paper`, which stays light in both
    /// appearances, so anything trait-dependent should resolve against light.
    static func render(_ drawing: PKDrawing) -> UIImage {
        let content = drawing.bounds
        let usable = content.isNull || content.isInfinite || content.isEmpty
        let source = CGRect(
            x: 0,
            y: 0,
            width: usable ? 1024 : max(1024, content.maxX),
            height: usable ? 1366 : max(1366, content.maxY)
        )

        var image = UIImage()
        UITraitCollection(userInterfaceStyle: .light).performAsCurrent {
            image = drawing.image(from: source, scale: 0.2)
        }
        return image
    }
}

/// A blank sheet with a ruled but unfilled title block: the form waiting to be
/// completed, rather than an apology in grey text.
struct EmptyRegister: View {
    let onStart: () -> Void

    var body: some View {
        VStack(spacing: 18) {
            VStack(spacing: 0) {
                Rectangle()
                    .fill(Sheet.paper)
                    .frame(height: 168)
                TitleBlock(number: "—", name: "Not started", date: "—", strokes: 0)
            }
            .frame(maxWidth: 280)
            .overlay {
                Rectangle().strokeBorder(Sheet.edge, lineWidth: Sheet.hair)
            }

            Text("No sheets yet")
                .font(.headline)
                .foregroundStyle(Sheet.onGround)
            Text("Open the first sheet and start writing. Every sheet you draw on files itself here.")
                .font(.subheadline)
                .foregroundStyle(Sheet.onGroundSecondary)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: 340)

            Button(action: onStart) {
                Text("Start writing")
                    .font(.callout.weight(.semibold))
                    .padding(.horizontal, 24)
                    .frame(height: 48)
                    .background(Sheet.ink)
                    .foregroundStyle(Sheet.paper)
                    .clipShape(RoundedRectangle(cornerRadius: Sheet.controlRadius))
            }
            .buttonStyle(.plain)
        }
        .padding(Sheet.margin)
    }
}

/// Dropping a sheet onto another files both into a series.
private struct StackDrop: DropDelegate {
    let store: PageStore
    let targetID: String

    func performDrop(info: DropInfo) -> Bool {
        guard let provider = info.itemProviders(for: [UTType.plainText]).first else { return false }
        provider.loadObject(ofClass: NSString.self) { value, _ in
            guard let draggedID = value as? String else { return }
            Task { @MainActor in
                store.stack(draggedID, onto: targetID)
            }
        }
        return true
    }
}
