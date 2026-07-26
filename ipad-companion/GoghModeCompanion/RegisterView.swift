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

    @State private var renaming: RenameTarget?
    @State private var openSeries: PageSeries?
    /// Sheets with a stamp request in flight. The control is disabled while it is
    /// out, so a second press cannot race the first.
    @State private var stamping: Set<String> = []

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
            .padding(.bottom, Sheet.margin)
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
        // Asked before anything needs the answer, and asked again whenever it has
        // been forgotten. Without this the stamp control is drawn on an optimistic
        // guess and then vanishes the first time it is pressed, which reads as the
        // app breaking.
        .task(id: uploader.macIsKnown) {
            await uploader.learnWhatTheMacAccepts(endpointText: endpointText)
        }
        .navigationDestination(item: $openSeries) { series in
            SeriesView(
                store: store,
                uploader: uploader,
                series: series,
                onOpen: onOpen,
                onNew: { onOpen(store.addPage(in: series.id).id) },
                onStamp: toggleStamp,
                onRename: { renaming = .sheet($0) },
                isStamping: { stamping.contains($0.id) }
            )
        }
        .sheet(item: $renaming) { target in
            RenameSheet(target: target, onSave: commitRename)
        }
    }

    /// The line that answers "what is Claude reading?" without opening anything,
    /// and the only place a failed upload or stamp can be seen from.
    private var registerHead: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
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

                Spacer(minLength: 8)

                StatusBadge(status: uploader.status, canRetry: uploader.canRetry) {
                    uploader.retry()
                }

                Text("\(store.pages.count) sheets")
                    .font(.caption.monospaced().weight(.medium))
                    .foregroundStyle(Sheet.onGroundSecondary)
                    .layoutPriority(1)
            }
            .padding(.horizontal, Sheet.margin)
            .padding(.vertical, 6)

            if let notice {
                Text(notice)
                    .font(.footnote)
                    .foregroundStyle(Sheet.onGround)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.horizontal, Sheet.margin)
                    .padding(.bottom, 8)
            }

            Rectangle().fill(Sheet.rule).frame(height: Sheet.hair)
        }
        .background(Sheet.ground)
    }

    /// One line of plain language for whatever is currently wrong, most urgent
    /// first. Silence here has to mean "nothing is wrong", or the register lies.
    private var notice: String? {
        if case .failed(let message) = uploader.status {
            return message
        }
        if let message = uploader.pagesUnsupportedMessage {
            return message
        }
        if uploader.macIsKnown && !uploader.pinningSupported {
            return "\(UploadController.stampNeedsANewerMac) Until then Claude reads whichever sheet you drew on last."
        }
        return nil
    }

    /// One ruled block of paper, hairline-separated, spanning the screen. A drawing
    /// set's register is a ruled index of sheets, so this is a table with aligned
    /// columns — not cards, and not a plain iOS list either.
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
                        stampState: stampState(for: page),
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
                            renaming = .series(series)
                        } label: {
                            Label("Rename series", systemImage: "pencil")
                        }
                    }
                }
            }
        }
        .background(Sheet.paper)
        .overlay(alignment: .top) {
            Rectangle().fill(Sheet.edge).frame(height: Sheet.hair)
        }
        .overlay(alignment: .bottom) {
            Rectangle().fill(Sheet.edge).frame(height: Sheet.hair)
        }
    }

    private func stampState(for page: NotebookPage) -> StampState {
        if !uploader.pinningSupported && uploader.macIsKnown {
            return .unavailable
        }
        if stamping.contains(page.id) {
            return .working
        }
        return page.id == store.pinnedPageID ? .issued : .available
    }

    @ViewBuilder
    private func menu(for page: NotebookPage) -> some View {
        Button {
            renaming = .sheet(page)
        } label: {
            Label("Rename", systemImage: "pencil")
        }

        if uploader.pinningSupported {
            Button {
                Task { _ = await uploader.promote(page.id, endpointText: endpointText) }
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

    private func commitRename(_ target: RenameTarget, _ name: String) {
        switch target {
        case .sheet(let page):
            store.rename(page.id, to: name)
            // The Mac keeps the title with the page, so a rename only reaches it on
            // the next save of that sheet. Send it now when it is the stamped one, so
            // the register and the Mac never disagree about the name Claude reads.
            if page.id == store.pinnedPageID {
                Task { _ = await uploader.promote(page.id, endpointText: endpointText) }
            }
        case .series(let series):
            store.renameSeries(series.id, to: name)
        }
    }

    private func toggleStamp(_ page: NotebookPage) {
        guard !stamping.contains(page.id) else { return }
        let target = page.id == store.pinnedPageID ? nil : page.id

        stamping.insert(page.id)
        Task {
            let accepted = await uploader.pin(target, endpointText: endpointText)
            stamping.remove(page.id)
            if accepted {
                store.recordPin(target)
            }
            // A refusal leaves the stamp exactly where the Mac says it is, and the
            // reason is already on the head line — the control must never show a
            // state the Mac has not agreed to.
        }
    }
}

/// Column widths, derived from one scaled unit so the header and every line cannot
/// drift apart, and so the whole table grows with the reading size instead of
/// clipping at large Dynamic Type.
struct RegisterColumns: Equatable {
    var scale: CGFloat = 1

    var issuedBar: CGFloat { 3 }
    var lead: CGFloat { 13 }
    var trail: CGFloat { 13 }
    var preview: CGFloat { 40 * scale }
    var previewHeight: CGFloat { 54 * scale }
    var previewGap: CGFloat { 13 }
    var number: CGFloat { 54 * scale }
    var date: CGFloat { 128 * scale }
    var strokes: CGFloat { 64 * scale }
    var stamp: CGFloat { 108 * scale }
    var chevron: CGFloat { 24 }

    /// Everything left of the `SHEET` column, so the header lines up with the rows.
    var beforeNumber: CGFloat { issuedBar + lead + preview + previewGap }
}

private struct RegisterColumnsKey: EnvironmentKey {
    static let defaultValue = RegisterColumns()
}

extension EnvironmentValues {
    var registerColumns: RegisterColumns {
        get { self[RegisterColumnsKey.self] }
        set { self[RegisterColumnsKey.self] = newValue }
    }
}

/// The ruled head of the register: the column names, in drafting lettering.
struct RegisterHeader: View {
    @Environment(\.registerColumns) private var columns
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        HStack(spacing: 0) {
            Spacer().frame(width: columns.beforeNumber)
            BlockLabel(text: "Sheet").frame(width: columns.number, alignment: .leading)
            BlockLabel(text: "Name").frame(maxWidth: .infinity, alignment: .leading)
            if sizeClass != .compact {
                BlockLabel(text: "Updated").frame(width: columns.date, alignment: .leading)
                BlockLabel(text: "Strokes").frame(width: columns.strokes, alignment: .leading)
            }
            BlockLabel(text: "Claude").frame(width: columns.stamp, alignment: .leading)
            Spacer().frame(width: columns.chevron)
        }
        .padding(.trailing, columns.trail)
        .padding(.vertical, 8)
        .accessibilityHidden(true)
    }
}

/// What the stamp control can do for one sheet right now.
enum StampState {
    case available
    case issued
    case working
    /// The Mac predates the stamp routes; the column stays, so its absence is
    /// explained rather than mysterious.
    case unavailable
}

/// One line of the register: preview, sheet number, name, facts, and the stamp. The
/// preview sets the row height, so thirty sheets stay scannable no matter how much
/// is drawn on them.
struct SheetRow: View {
    let page: NotebookPage
    let number: String
    let isIssued: Bool
    let stampState: StampState
    let onOpen: () -> Void
    let onStamp: () -> Void

    @Environment(\.registerColumns) private var columns
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        // One decode and one rasterise per sheet per edit, not per row rebuild —
        // `page.drawing` parses the stored data on every read.
        let rendered = SheetPreviewCache.rendered(for: page)

        return RegisterLine(isIssued: isIssued, onOpen: onOpen) {
            SheetPreview(image: rendered.image)
                .padding(.trailing, columns.previewGap)

            SheetNumber(text: number)
                .frame(width: columns.number, alignment: .leading)

            Text(page.title)
                .font(.body.weight(.semibold))
                .foregroundStyle(Sheet.ink)
                .lineLimit(1)
                .truncationMode(.tail)
                .frame(maxWidth: .infinity, alignment: .leading)

            if sizeClass != .compact {
                SheetNumber(text: SheetRow.stamped(page.updatedAt))
                    .frame(width: columns.date, alignment: .leading)
                SheetNumber(text: String(rendered.strokeCount))
                    .frame(width: columns.strokes, alignment: .leading)
            }
        } trailing: {
            StampControl(state: stampState, action: onStamp)
                .frame(width: columns.stamp, alignment: .leading)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(
            Text(
                isIssued
                    ? "Sheet \(number), \(page.title), \(rendered.strokeCount) strokes. Stamped — this is what Claude reads."
                    : "Sheet \(number), \(page.title), \(rendered.strokeCount) strokes"
            )
        )
    }

    /// Short enough for the column at any reading size: "26 Jul, 16:13". The full
    /// date is spoken by VoiceOver through the row label's own wording.
    static func stamped(_ date: Date) -> String {
        date.formatted(.dateTime.day().month(.abbreviated).hour().minute())
    }
}

/// A series as one line: paper behind paper, then the name and how many sheets are
/// filed into it.
struct SeriesRow: View {
    let series: PageSeries
    let sheets: [NotebookPage]
    let containsIssued: Bool
    let onOpen: () -> Void

    @Environment(\.registerColumns) private var columns
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
            .frame(width: columns.preview, height: columns.previewHeight, alignment: .bottomLeading)
            .padding(.trailing, columns.previewGap)

            SheetNumber(text: series.prefix)
                .frame(width: columns.number, alignment: .leading)

            Text(series.name)
                .font(.body.weight(.semibold))
                .foregroundStyle(Sheet.ink)
                .lineLimit(1)
                .truncationMode(.tail)
                .frame(maxWidth: .infinity, alignment: .leading)

            if sizeClass != .compact {
                SheetNumber(text: "\(sheets.count) sheets")
                    .frame(width: columns.date, alignment: .leading)
                Spacer().frame(width: columns.strokes)
            }
        } trailing: {
            Group {
                if containsIssued {
                    IssueStamp(scale: 0.68)
                } else {
                    Color.clear
                }
            }
            .frame(width: columns.stamp, alignment: .leading)
            .allowsHitTesting(false)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(
            Text(
                containsIssued
                    ? "Series \(series.name), \(sheets.count) sheets, one of them stamped"
                    : "Series \(series.name), \(sheets.count) sheets"
            )
        )
    }
}

/// The shape every register line shares: an issue bar, tappable columns, then
/// controls that must stay outside the tap target.
struct RegisterLine<Columns: View, Trailing: View>: View {
    let isIssued: Bool
    let onOpen: () -> Void
    @ViewBuilder var columns: Columns
    @ViewBuilder var trailing: Trailing

    @Environment(\.registerColumns) private var metrics

    var body: some View {
        HStack(spacing: 0) {
            Rectangle()
                .fill(isIssued ? Sheet.stamp : Color.clear)
                .frame(width: metrics.issuedBar)

            Button(action: onOpen) {
                HStack(spacing: 0) {
                    columns
                }
                .padding(.leading, metrics.lead)
                .frame(minHeight: 44)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            trailing

            Image(systemName: "chevron.right")
                .font(.footnote.weight(.semibold))
                .foregroundStyle(Sheet.inkLabel)
                .frame(width: metrics.chevron, alignment: .trailing)
                .accessibilityHidden(true)
        }
        .padding(.trailing, metrics.trail)
        .padding(.vertical, 8)
    }
}

/// The one control that answers "which sheet does Claude read?". Unstamped it is a
/// quiet ruled button reading `STAMP`; on the stamped sheet the control *is* the
/// stamp, and pressing it lifts the stamp again.
struct StampControl: View {
    let state: StampState
    let action: () -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Button(action: action) {
            face
                .frame(minWidth: 44, minHeight: 44, alignment: .leading)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(state == .working || state == .unavailable)
        .animation(
            reduceMotion ? .easeInOut(duration: 0.15) : .spring(response: 0.28, dampingFraction: 0.62),
            value: state == .issued
        )
        .accessibilityLabel(Text(spokenLabel))
    }

    @ViewBuilder
    private var face: some View {
        switch state {
        case .issued:
            IssueStamp(scale: 0.68)
                .transition(
                    reduceMotion ? .opacity : .scale(scale: 1.25).combined(with: .opacity)
                )
        case .working:
            ProgressView()
                .controlSize(.small)
                .padding(.leading, 4)
        case .unavailable:
            Text("—")
                .font(.caption.monospaced())
                .foregroundStyle(Sheet.inkLabel)
                .padding(.leading, 4)
        case .available:
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

    private var spokenLabel: String {
        switch state {
        case .issued: "Stamped for Claude. Press to lift the stamp."
        case .working: "Asking the Mac to move the stamp"
        case .unavailable: "Stamping needs a newer Mac app"
        case .available: "Stamp this sheet so Claude reads it"
        }
    }
}

/// Renaming lives in its own sheet rather than an alert with a text field: the draft
/// name is state *inside* this view, so typing rebuilds only this form. Held in the
/// parent it rebuilt the whole register on every keystroke, which dismissed the
/// alert mid-word.
struct RenameSheet: View {
    let target: RenameTarget
    let onSave: (RenameTarget, String) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var draft: String
    @FocusState private var editing: Bool

    init(target: RenameTarget, onSave: @escaping (RenameTarget, String) -> Void) {
        self.target = target
        self.onSave = onSave
        _draft = State(initialValue: target.currentName)
    }

    private var trimmed: String {
        draft.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 0) {
                VStack(spacing: 0) {
                    Rectangle().fill(Sheet.rule).frame(height: Sheet.hair)

                    HStack(spacing: 0) {
                        BlockField(label: target.numberLabel) {
                            SheetNumber(text: target.number)
                        }
                        .frame(width: 74, alignment: .leading)

                        Rectangle()
                            .fill(Sheet.ruleHair)
                            .frame(width: Sheet.hair)
                            .frame(maxHeight: .infinity)

                        BlockField(label: "Name") {
                            TextField("Sheet name", text: $draft)
                                .font(.body.weight(.semibold))
                                .foregroundStyle(Sheet.ink)
                                .focused($editing)
                                .submitLabel(.done)
                                .onSubmit(save)
                                .frame(minHeight: 32)
                        }
                        .padding(.horizontal, Sheet.block)
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 8)

                    Rectangle().fill(Sheet.ruleHair).frame(height: Sheet.hair)
                }
                .background(Sheet.paper)
                .overlay {
                    Rectangle().strokeBorder(Sheet.edge, lineWidth: Sheet.hair)
                }

                Text(target.explanation)
                    .font(.footnote)
                    .foregroundStyle(Sheet.onGroundSecondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 12)

                Spacer(minLength: 0)
            }
            .padding(Sheet.margin)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Sheet.ground)
            .navigationTitle(target.formTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save", action: save)
                        .disabled(trimmed.isEmpty)
                }
            }
        }
        .presentationDetents([.height(260), .medium])
        .onAppear { editing = true }
    }

    private func save() {
        guard !trimmed.isEmpty else { return }
        onSave(target, trimmed)
        dismiss()
    }
}

/// What is being renamed. Carries its own labels so the form does not have to ask.
enum RenameTarget: Identifiable {
    case sheet(NotebookPage)
    case series(PageSeries)

    var id: String {
        switch self {
        case .sheet(let page): "sheet-\(page.id)"
        case .series(let series): "series-\(series.id)"
        }
    }

    var currentName: String {
        switch self {
        case .sheet(let page): page.title
        case .series(let series): series.name
        }
    }

    var formTitle: String {
        switch self {
        case .sheet: "Name this sheet"
        case .series: "Name this series"
        }
    }

    var numberLabel: String {
        switch self {
        case .sheet: "Sheet"
        case .series: "Series"
        }
    }

    var number: String {
        switch self {
        case .sheet: "—"
        case .series(let series): series.prefix
        }
    }

    var explanation: String {
        switch self {
        case .sheet:
            "Names show in the register and travel to the Mac with the page."
        case .series:
            "Sheets filed into this series are numbered within it."
        }
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
    let isStamping: (NotebookPage) -> Bool

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
                        stampState: stampState(for: page),
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
            .overlay(alignment: .top) {
                Rectangle().fill(Sheet.edge).frame(height: Sheet.hair)
            }
            .overlay(alignment: .bottom) {
                Rectangle().fill(Sheet.edge).frame(height: Sheet.hair)
            }
            .padding(.vertical, 4)
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

    private func stampState(for page: NotebookPage) -> StampState {
        if !uploader.pinningSupported && uploader.macIsKnown {
            return .unavailable
        }
        if isStamping(page) {
            return .working
        }
        return page.id == store.pinnedPageID ? .issued : .available
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

    @Environment(\.registerColumns) private var columns

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
            .frame(width: columns.preview, height: columns.previewHeight)
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
