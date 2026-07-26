import PencilKit
import SwiftUI
import UniformTypeIdentifiers

/// The drawing register: every sheet in the set, each carrying its title block,
/// and exactly one wearing the issue stamp.
struct RegisterView: View {
    @ObservedObject var store: PageStore
    @ObservedObject var uploader: UploadController

    let endpointText: String
    let onOpen: (String) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var renaming: NotebookPage?
    @State private var renamingSeries: PageSeries?
    @State private var draftName = ""
    @State private var openSeries: PageSeries?

    var body: some View {
        NavigationStack {
            ScrollView {
                // The store always holds at least one sheet, so "no sheets" is
                // never literally true — the real empty state is a set nothing
                // has been drawn on yet.
                if store.pages.allSatisfy(\.isEmpty) {
                    EmptyRegister()
                        .padding(.top, 80)
                } else {
                    sheets
                }
            }
            .background(Sheet.ground)
            .navigationTitle("Pages")
            .navigationBarTitleDisplayMode(.large)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }
                }
                ToolbarItem(placement: .primaryAction) {
                    Button {
                        store.addPage()
                        onOpen(store.selectedPageID)
                        dismiss()
                    } label: {
                        Label("New sheet", systemImage: "plus")
                    }
                }
            }
            .safeAreaInset(edge: .top, spacing: 0) { statusRule }
            .navigationDestination(item: $openSeries) { series in
                SeriesView(
                    store: store,
                    series: series,
                    onOpen: { pageID in
                        onOpen(pageID)
                        dismiss()
                    },
                    onPin: pin,
                    onSend: send,
                    onRename: beginRename
                )
            }
        }
        .alert("Name this sheet", isPresented: renamingBinding) {
            TextField("Sheet name", text: $draftName)
            Button("Cancel", role: .cancel) { clearRename() }
            Button("Save") { commitRename() }
        } message: {
            Text("Names show in the title block and travel to the Mac with the page.")
        }
    }

    /// The one line that answers "what is Claude reading?" without opening
    /// anything. Sits under the title as a rule, the way a sheet register is
    /// headed.
    private var statusRule: some View {
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

            Rectangle().fill(Sheet.rule).frame(height: Sheet.hair)
        }
        .background(Sheet.ground)
    }

    private var sheets: some View {
        LazyVGrid(
            columns: [GridItem(.adaptive(minimum: 260), spacing: Sheet.gutter)],
            spacing: Sheet.gutter
        ) {
            ForEach(store.register) { entry in
                switch entry {
                case .sheet(let page):
                    SheetCard(
                        page: page,
                        number: store.sheetNumber(for: page),
                        isIssued: page.id == store.pinnedPageID
                    )
                    .onTapGesture {
                        onOpen(page.id)
                        dismiss()
                    }
                    .onDrag { NSItemProvider(object: page.id as NSString) }
                    .onDrop(
                        of: [UTType.plainText],
                        delegate: StackDrop(store: store, targetID: page.id)
                    )
                    .contextMenu { menu(for: page) }

                case .series(let series, let filed):
                    SeriesCard(
                        series: series,
                        sheets: filed,
                        issuedID: store.pinnedPageID
                    )
                    .onTapGesture { openSeries = series }
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
        .padding(Sheet.margin)
    }

    @ViewBuilder
    private func menu(for page: NotebookPage) -> some View {
        Button {
            beginRename(page)
        } label: {
            Label("Rename", systemImage: "pencil")
        }

        if uploader.pinningSupported {
            if page.id == store.pinnedPageID {
                Button {
                    pin(nil)
                } label: {
                    Label("Remove stamp", systemImage: "seal.slash")
                }
            } else {
                Button {
                    pin(page.id)
                } label: {
                    Label("Stamp for Claude", systemImage: "seal")
                }
            }

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

    private func pin(_ pageID: String?) {
        Task {
            if await uploader.pin(pageID, endpointText: endpointText) {
                store.recordPin(pageID)
            }
        }
    }

    private func send(_ page: NotebookPage) {
        Task { _ = await uploader.promote(page.id, endpointText: endpointText) }
    }
}

/// A sheet in the register: the drawing, then its title block. The block is part
/// of the sheet, not a caption below a card.
struct SheetCard: View {
    let page: NotebookPage
    let number: String
    let isIssued: Bool

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: 0) {
            SheetThumbnail(drawing: page.drawing)
                .frame(height: 168)
                .overlay(alignment: .topTrailing) {
                    if isIssued {
                        IssueStamp()
                            .padding(10)
                            // The stamp lands with a short impact settle. Under
                            // Reduce Motion it simply appears.
                            .transition(
                                reduceMotion
                                    ? .opacity
                                    : .scale(scale: 1.25).combined(with: .opacity)
                            )
                    }
                }

            TitleBlock(
                number: number,
                name: page.title,
                date: page.updatedAt.formatted(date: .abbreviated, time: .omitted),
                strokes: page.drawing.strokes.count
            )
        }
        .background(Sheet.paper)
        .clipShape(RoundedRectangle(cornerRadius: Sheet.sheetRadius))
        .overlay {
            RoundedRectangle(cornerRadius: Sheet.sheetRadius)
                .strokeBorder(isIssued ? Sheet.stamp.opacity(0.55) : Sheet.edge, lineWidth: Sheet.hair)
        }
        .animation(
            reduceMotion ? .easeInOut(duration: 0.15) : .spring(response: 0.28, dampingFraction: 0.62),
            value: isIssued
        )
        .accessibilityElement(children: .combine)
        .accessibilityLabel(
            Text(
                isIssued
                    ? "Sheet \(number), \(page.title). Stamped — this is what Claude reads."
                    : "Sheet \(number), \(page.title)"
            )
        )
    }
}

/// A stack: sheets filed into a lettered series, drawn as paper behind paper.
struct SeriesCard: View {
    let series: PageSeries
    let sheets: [NotebookPage]
    let issuedID: String?

    private var containsIssued: Bool {
        sheets.contains { $0.id == issuedID }
    }

    var body: some View {
        VStack(spacing: 0) {
            ZStack {
                ForEach(Array(sheets.prefix(3).enumerated().reversed()), id: \.element.id) { offset, sheet in
                    SheetThumbnail(drawing: sheet.drawing)
                        .frame(height: 168)
                        .background(Sheet.paper)
                        .overlay {
                            Rectangle().strokeBorder(Sheet.edge, lineWidth: Sheet.hair)
                        }
                        .offset(x: CGFloat(offset) * 7, y: CGFloat(offset) * -7)
                }
            }
            .frame(height: 168)
            .overlay(alignment: .topTrailing) {
                if containsIssued {
                    IssueStamp(scale: 0.85).padding(10)
                }
            }

            VStack(spacing: 0) {
                Rectangle().fill(Sheet.rule).frame(height: Sheet.hair)
                HStack(spacing: 0) {
                    BlockField(label: "Series") {
                        SheetNumber(text: series.prefix)
                    }
                    .frame(width: 62, alignment: .leading)

                    Rectangle().fill(Sheet.ruleHair).frame(width: Sheet.hair).frame(maxHeight: .infinity)

                    BlockField(label: "Name") {
                        Text(series.name)
                            .font(.subheadline.weight(.semibold))
                            .foregroundStyle(Sheet.ink)
                            .lineLimit(1)
                    }
                    .padding(.horizontal, Sheet.block)

                    Rectangle().fill(Sheet.ruleHair).frame(width: Sheet.hair).frame(maxHeight: .infinity)

                    BlockField(label: "Sheets") {
                        SheetNumber(text: String(sheets.count))
                    }
                    .frame(width: 54, alignment: .leading)
                    .padding(.leading, Sheet.block)
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 7)
            }
            .background(Sheet.paper)
        }
        .background(Sheet.paper)
        .clipShape(RoundedRectangle(cornerRadius: Sheet.sheetRadius))
        .overlay {
            RoundedRectangle(cornerRadius: Sheet.sheetRadius)
                .strokeBorder(Sheet.edge, lineWidth: Sheet.hair)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(Text("Series \(series.name), \(sheets.count) sheets"))
    }
}

/// The sheets filed into one series.
struct SeriesView: View {
    @ObservedObject var store: PageStore
    let series: PageSeries
    let onOpen: (String) -> Void
    let onPin: (String?) -> Void
    let onSend: (NotebookPage) -> Void
    let onRename: (NotebookPage) -> Void

    var body: some View {
        ScrollView {
            LazyVGrid(
                columns: [GridItem(.adaptive(minimum: 260), spacing: Sheet.gutter)],
                spacing: Sheet.gutter
            ) {
                ForEach(store.sheets(in: series.id)) { page in
                    SheetCard(
                        page: page,
                        number: store.sheetNumber(for: page),
                        isIssued: page.id == store.pinnedPageID
                    )
                    .onTapGesture { onOpen(page.id) }
                    .contextMenu {
                        Button { onRename(page) } label: {
                            Label("Rename", systemImage: "pencil")
                        }
                        Button { onPin(page.id) } label: {
                            Label("Stamp for Claude", systemImage: "seal")
                        }
                        Button { onSend(page) } label: {
                            Label("Send this one now", systemImage: "paperplane")
                        }
                        Button { store.removeFromSeries(page.id) } label: {
                            Label("Take out of series", systemImage: "rectangle.stack.badge.minus")
                        }
                    }
                }
            }
            .padding(Sheet.margin)
        }
        .background(Sheet.ground)
        .navigationTitle(series.name)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    store.addPage(in: series.id)
                    onOpen(store.selectedPageID)
                } label: {
                    Label("New sheet in series", systemImage: "plus")
                }
            }
        }
    }
}

/// A drawing, rendered at the size it is shown. `PKDrawing.image` renders the
/// drawing's own bounds, so a sheet with one small stroke in a corner would blow
/// that stroke up to fill the card without a fixed source rect.
struct SheetThumbnail: View {
    let drawing: PKDrawing

    var body: some View {
        GeometryReader { geometry in
            let source = CGRect(origin: .zero, size: CGSize(width: 1024, height: 1366))
            Image(uiImage: drawing.image(from: source, scale: 0.25))
                .resizable()
                .aspectRatio(contentMode: .fill)
                .frame(width: geometry.size.width, height: geometry.size.height)
                .clipped()
        }
        .background(Sheet.paper)
    }
}

/// A blank sheet with a ruled but unfilled title block: the form waiting to be
/// completed, rather than an apology in grey text.
struct EmptyRegister: View {
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
            Text("Draw, and the sheet files itself here.")
                .font(.subheadline)
                .foregroundStyle(Sheet.onGroundSecondary)
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
