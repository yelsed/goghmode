import Foundation
import PencilKit

struct NotebookPage: Codable, Equatable, Identifiable {
    let id: String
    var title: String
    var createdAt: Date
    var updatedAt: Date
    var drawingData: Data
    /// `nil` means a loose sheet, not filed into a series.
    var seriesID: String?

    var drawing: PKDrawing {
        (try? PKDrawing(data: drawingData)) ?? PKDrawing()
    }

    var isEmpty: Bool {
        drawing.strokes.isEmpty
    }

    var pageRef: PageRef {
        PageRef(id: id, title: title)
    }

    /// The sheet as the wire format sees it, for sending a page the canvas does not
    /// currently have open.
    ///
    /// A page's canvas size is not stored, so it is derived the way the preview
    /// derives its source rect: a full page, grown to cover anything drawn past it.
    /// A fixed portrait canvas would clamp every stroke made in landscape onto the
    /// right-hand edge, because the Mac rejects points outside the canvas.
    var snapshot: DrawingSnapshot {
        let pencilDrawing = drawing
        let bounds = pencilDrawing.bounds
        let unusable = bounds.isNull || bounds.isInfinite || bounds.isEmpty
        let canvas = CGSize(
            width: unusable ? 1024 : max(1024, bounds.maxX),
            height: unusable ? 1366 : max(1366, bounds.maxY)
        )
        return DrawingSnapshot.fromPencilDrawing(pencilDrawing, canvasSize: canvas, page: pageRef)
    }
}

/// A stack, in drawing-set terms: a lettered series of sheets. Series live only
/// on the iPad — the host keeps one flat `pages/` directory and the wire format
/// does not know they exist.
struct PageSeries: Codable, Equatable, Hashable, Identifiable {
    let id: String
    var name: String
    /// A, B, C… Sheets inside read as A-01, A-02.
    var prefix: String
}

/// One entry in the register: either a loose sheet, or a series standing in for
/// the sheets filed into it.
enum RegisterEntry: Identifiable, Equatable {
    case sheet(NotebookPage)
    case series(PageSeries, [NotebookPage])

    var id: String {
        switch self {
        case .sheet(let page): page.id
        case .series(let series, _): series.id
        }
    }

    var updatedAt: Date {
        switch self {
        case .sheet(let page): page.updatedAt
        case .series(_, let sheets): sheets.map(\.updatedAt).max() ?? .distantPast
        }
    }
}

/// The iPad's own copy of every page. This is the write that makes work
/// survive — the host holds a mirror, so a page drawn while it is closed is
/// still here when it comes back.
@MainActor
final class PageStore: ObservableObject {
    @Published private(set) var pages: [NotebookPage] = []
    @Published private(set) var series: [PageSeries] = []
    @Published private(set) var selectedPageID: String = ""
    /// The sheet carrying the issue stamp — the one `/goghmode` reads. Mirrors
    /// state the Mac owns; the app records what the Mac confirmed rather than
    /// keeping a second opinion about it.
    @Published private(set) var pinnedPageID: String?

    private let storeURL: URL

    var selectedPage: NotebookPage? {
        pages.first { $0.id == selectedPageID }
    }

    var selectedDrawing: PKDrawing {
        selectedPage?.drawing ?? PKDrawing()
    }

    var pinnedPage: NotebookPage? {
        pages.first { $0.id == pinnedPageID }
    }

    /// Loose sheets and series together, most recently touched first.
    var register: [RegisterEntry] {
        let loose = pages.filter { $0.seriesID == nil }.map(RegisterEntry.sheet)
        let filed = series.map { RegisterEntry.series($0, sheets(in: $0.id)) }
        return (loose + filed).sorted { $0.updatedAt > $1.updatedAt }
    }

    func sheets(in seriesID: String) -> [NotebookPage] {
        pages
            .filter { $0.seriesID == seriesID }
            .sorted { $0.createdAt < $1.createdAt }
    }

    /// `A-03` inside a series, `03` for a loose sheet. Numbering follows creation
    /// order, so a sheet's number does not shift when it is edited.
    func sheetNumber(for page: NotebookPage) -> String {
        if let seriesID = page.seriesID,
           let series = series.first(where: { $0.id == seriesID }),
           let index = sheets(in: seriesID).firstIndex(where: { $0.id == page.id }) {
            return "\(series.prefix)-\(String(format: "%02d", index + 1))"
        }

        let loose = pages
            .filter { $0.seriesID == nil }
            .sorted { $0.createdAt < $1.createdAt }
        let index = loose.firstIndex { $0.id == page.id } ?? 0
        return String(format: "%02d", index + 1)
    }

    init(storeURL: URL? = nil) {
        self.storeURL = storeURL ?? PageStore.defaultStoreURL()
        load()
        if pages.isEmpty {
            appendPage()
        }
        selectedPageID = pages.first?.id ?? ""
    }

    static func defaultStoreURL() -> URL {
        let directory = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first ?? FileManager.default.temporaryDirectory
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory.appendingPathComponent("goghmode-pages.json")
    }

    @discardableResult
    func addPage(in seriesID: String? = nil) -> NotebookPage {
        let page = appendPage(in: seriesID)
        selectedPageID = page.id
        save()
        return page
    }

    func select(_ pageID: String) {
        guard pages.contains(where: { $0.id == pageID }) else { return }
        selectedPageID = pageID
    }

    func page(_ pageID: String) -> NotebookPage? {
        pages.first { $0.id == pageID }
    }

    /// Named rather than implied: the open sheet is addressed by id, so an in-flight
    /// stroke can never land on whichever page the register happens to have
    /// selected.
    ///
    /// An empty drawing arriving for a sheet that has strokes is refused. Nobody
    /// erases seventeen strokes by drawing, so in practice that only ever means
    /// the canvas reported its own loading as an edit — which emptied several
    /// sheets, on the iPad and on the Mac, before the canvas stopped doing it.
    /// Erasing on purpose goes through `clear`.
    func update(_ pageID: String, with drawing: PKDrawing) {
        guard let index = pages.firstIndex(where: { $0.id == pageID }) else { return }
        if drawing.strokes.isEmpty && !pages[index].isEmpty { return }
        pages[index].drawingData = drawing.dataRepresentation()
        pages[index].updatedAt = Date()
        save()
    }

    /// Erasing a sheet on purpose — the one path allowed to empty one that has
    /// strokes on it.
    func clear(_ pageID: String) {
        guard let index = pages.firstIndex(where: { $0.id == pageID }) else { return }
        pages[index].drawingData = PKDrawing().dataRepresentation()
        pages[index].updatedAt = Date()
        save()
    }

    /// Takes a sheet off this iPad. The host keeps its own copy: deleting here
    /// says nothing about `pages/` on the Mac, and the register would be lying
    /// if it implied otherwise.
    ///
    /// The register is never left with nothing to open, so emptying it hands
    /// back a fresh sheet the way a new install does.
    func delete(_ pageID: String) {
        guard pages.contains(where: { $0.id == pageID }) else { return }
        pages.removeAll { $0.id == pageID }
        discardEmptySeries()
        if pages.isEmpty {
            appendPage()
        }
        if selectedPageID == pageID {
            selectedPageID = pages.first?.id ?? ""
        }
        save()
    }

    func updateSelectedPage(with drawing: PKDrawing) {
        update(selectedPageID, with: drawing)
    }

    func rename(_ pageID: String, to title: String) {
        let trimmed = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let index = pages.firstIndex(where: { $0.id == pageID }) else {
            return
        }
        pages[index].title = trimmed
        save()
    }

    func renameSeries(_ seriesID: String, to name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let index = series.firstIndex(where: { $0.id == seriesID }) else {
            return
        }
        series[index].name = trimmed
        save()
    }

    /// Dropping one sheet onto another files both into a series, the way stacking
    /// works in a gallery. Dropping onto a sheet already filed joins that series.
    @discardableResult
    func stack(_ draggedID: String, onto targetID: String) -> String? {
        guard draggedID != targetID,
              let dragged = pages.firstIndex(where: { $0.id == draggedID }),
              let target = pages.firstIndex(where: { $0.id == targetID }) else {
            return nil
        }

        let seriesID: String
        if let existing = pages[target].seriesID {
            seriesID = existing
        } else {
            let prefix = nextPrefix()
            let created = PageSeries(id: UUID().uuidString, name: "Series \(prefix)", prefix: prefix)
            series.append(created)
            seriesID = created.id
            pages[target].seriesID = seriesID
        }

        pages[dragged].seriesID = seriesID
        save()
        return seriesID
    }

    func removeFromSeries(_ pageID: String) {
        guard let index = pages.firstIndex(where: { $0.id == pageID }) else { return }
        pages[index].seriesID = nil
        discardEmptySeries()
        save()
    }

    /// Records the pin the Mac confirmed. The Mac owns which page `latest.*`
    /// follows; a local guess would be a second source of truth for the one fact
    /// this app exists to make unambiguous.
    func recordPin(_ pageID: String?) {
        pinnedPageID = pageID
        save()
    }

    @discardableResult
    private func appendPage(in seriesID: String? = nil) -> NotebookPage {
        let now = Date()
        let page = NotebookPage(
            id: UUID().uuidString,
            title: PageStore.defaultTitle(for: now),
            createdAt: now,
            updatedAt: now,
            drawingData: PKDrawing().dataRepresentation(),
            seriesID: seriesID
        )
        pages.insert(page, at: 0)
        return page
    }

    private func nextPrefix() -> String {
        let used = Set(series.map(\.prefix))
        for scalar in UnicodeScalar("A").value...UnicodeScalar("Z").value {
            guard let letter = UnicodeScalar(scalar) else { continue }
            let candidate = String(letter)
            if !used.contains(candidate) {
                return candidate
            }
        }
        return "Z"
    }

    private func discardEmptySeries() {
        let occupied = Set(pages.compactMap(\.seriesID))
        series.removeAll { !occupied.contains($0.id) }
    }

    private static func defaultTitle(for date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "d MMM HH:mm"
        return formatter.string(from: date)
    }

    private struct Stored: Codable {
        var pages: [NotebookPage]
        var series: [PageSeries]
        var pinnedPageID: String?
    }

    private func load() {
        guard let data = try? Data(contentsOf: storeURL) else { return }
        if let stored = try? JSONDecoder().decode(Stored.self, from: data) {
            pages = stored.pages
            series = stored.series
            pinnedPageID = stored.pinnedPageID
            return
        }
        // Builds before series and pinning stored a bare array of pages.
        if let legacy = try? JSONDecoder().decode([LegacyPage].self, from: data) {
            pages = legacy.map(\.migrated)
        }
    }

    /// The shape shipped before sheets could be filed or stamped.
    private struct LegacyPage: Codable {
        let id: String
        var title: String
        var updatedAt: Date
        var drawingData: Data

        var migrated: NotebookPage {
            NotebookPage(
                id: id,
                title: title,
                createdAt: updatedAt,
                updatedAt: updatedAt,
                drawingData: drawingData,
                seriesID: nil
            )
        }
    }

    private func save() {
        let stored = Stored(pages: pages, series: series, pinnedPageID: pinnedPageID)
        guard let data = try? JSONEncoder().encode(stored) else { return }
        try? data.write(to: storeURL, options: .atomic)
    }
}
