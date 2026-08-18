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
    /// `nil` is a plain sheet, which is what every sheet was before ruling and
    /// what every new one still is. Defaulted on decode so a store written by an
    /// older build reads without migration.
    var ruling: SheetRuling?

    init(
        id: String,
        title: String,
        createdAt: Date,
        updatedAt: Date,
        drawingData: Data,
        seriesID: String? = nil,
        ruling: SheetRuling? = nil
    ) {
        self.id = id
        self.title = title
        self.createdAt = createdAt
        self.updatedAt = updatedAt
        self.drawingData = drawingData
        self.seriesID = seriesID
        self.ruling = ruling
    }

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
    /// currently have open. `fromPencilDrawing` grows the page to cover anything
    /// drawn past it, so a sheet written in landscape before the page had a fixed
    /// size is still sent whole.
    var snapshot: DrawingSnapshot {
        DrawingSnapshot.fromPencilDrawing(
            drawing,
            canvasSize: SheetPage.size,
            page: pageRef,
            ruling: ruling
        )
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

    /// Whether the open sheet has anywhere to step back to, or forward into.
    @Published private(set) var canStepBack = false
    @Published private(set) var canStepForward = false

    private let storeURL: URL
    private let revisionsURL: URL
    /// Loaded per sheet on demand. A trail is only read when a sheet is opened, so
    /// the register never pays for history it does not show.
    private var trails: [String: RevisionTrail] = [:]
    private var trailsToWrite: Set<String> = []

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
        let resolved = storeURL ?? PageStore.defaultStoreURL()
        self.storeURL = resolved
        // Named after the store it belongs to rather than sharing one folder, so
        // two stores can never read each other's history.
        self.revisionsURL = resolved.deletingPathExtension().appendingPathExtension("revisions")
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
        publishStepAvailability(for: pageID)
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
        discardTrail(for: pageID)
        discardEmptySeries()
        if pages.isEmpty {
            appendPage()
        }
        if selectedPageID == pageID {
            selectedPageID = pages.first?.id ?? ""
        }
        save()
    }

    /// Puts a sheet back to a state it held before.
    ///
    /// The one write besides `clear` allowed to empty a sheet that has strokes.
    /// `update` refuses that, because a canvas reporting its own loading as an edit
    /// is what emptied several sheets, but a sheet stepped back past its first
    /// stroke is genuinely empty and has to be allowed to say so.
    func restore(_ pageID: String, to drawing: PKDrawing) {
        guard let index = pages.firstIndex(where: { $0.id == pageID }) else { return }
        pages[index].drawingData = drawing.dataRepresentation()
        pages[index].updatedAt = Date()
        save()
        // Stepping back is deliberate and rare, so where it left the sheet is
        // worth putting on disk at once rather than waiting for the sheet to close.
        flushRevisions()
    }

    /// Records where a sheet is now, so it can be come back to after the sheet has
    /// been closed and opened again, which is where the canvas's own undo stack
    /// goes, since reopening builds a fresh canvas.
    ///
    /// Called once per finished stroke. PencilKit reports a drawing many times
    /// while one is being made, so the caller uses the stroke count changing as the
    /// signal rather than every report.
    func recordRevision(_ pageID: String, _ drawing: PKDrawing) {
        var trail = trail(for: pageID)
        let state = drawing.dataRepresentation()
        if trail.states.indices.contains(trail.cursor), trail.states[trail.cursor] == state {
            return
        }

        // Stepping back and then drawing abandons what was ahead, the way undo
        // behaves everywhere else.
        if trail.cursor >= 0 && trail.cursor + 1 < trail.states.count {
            trail.states.removeSubrange((trail.cursor + 1)...)
        }
        trail.states.append(state)
        if trail.states.count > PageStore.revisionDepth {
            trail.states.removeFirst(trail.states.count - PageStore.revisionDepth)
        }
        trail.cursor = trail.states.count - 1
        commit(trail, for: pageID)
    }

    func stepBack(_ pageID: String) -> PKDrawing? {
        var trail = trail(for: pageID)
        guard trail.cursor > 0 else { return nil }
        trail.cursor -= 1
        commit(trail, for: pageID)
        return PageStore.drawing(from: trail.states[trail.cursor])
    }

    func stepForward(_ pageID: String) -> PKDrawing? {
        var trail = trail(for: pageID)
        guard trail.cursor + 1 < trail.states.count else { return nil }
        trail.cursor += 1
        commit(trail, for: pageID)
        return PageStore.drawing(from: trail.states[trail.cursor])
    }

    func updateSelectedPage(with drawing: PKDrawing) {
        update(selectedPageID, with: drawing)
    }

    /// The ruling a sheet is written against. Per sheet rather than per app: a
    /// lined note and a squared diagram are the ordinary case, not an edge one.
    func setRuling(_ ruling: SheetRuling?, on pageID: String) {
        guard let index = pages.firstIndex(where: { $0.id == pageID }) else { return }
        pages[index].ruling = ruling
        save()
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

    /// A sheet's recent states, oldest first, with the cursor on the one the canvas
    /// is showing.
    private struct RevisionTrail: Codable {
        var states: [Data] = []
        var cursor: Int = -1
    }

    /// ponytail: twenty states per sheet, kept whole rather than as differences.
    /// A long page's states are tens of kilobytes each, so if the sidecars ever get
    /// heavy the upgrade is to store stroke differences, not to keep fewer.
    private static let revisionDepth = 20

    /// Loaded from disk once per sheet, then held. Seeded from the sheet's current
    /// state so there is always somewhere to come back to.
    private func trail(for pageID: String) -> RevisionTrail {
        if let held = trails[pageID] {
            return held
        }

        var trail = RevisionTrail()
        if let data = try? Data(contentsOf: trailURL(for: pageID)),
           let stored = try? JSONDecoder().decode(RevisionTrail.self, from: data) {
            trail = stored
        } else if let page = page(pageID) {
            trail.states = [page.drawingData]
            trail.cursor = 0
        }

        trails[pageID] = trail
        return trail
    }

    /// Held in memory and written out later.
    ///
    /// Writing here would put the whole trail on disk after every stroke, and a
    /// trail is twenty drawings: far more per stroke than the page itself costs.
    /// The page is still saved immediately, which is the part nobody may lose;
    /// history is a convenience, so it is written when the sheet is put down.
    private func commit(_ trail: RevisionTrail, for pageID: String) {
        trails[pageID] = trail
        trailsToWrite.insert(pageID)
        publishStepAvailability(for: pageID)
    }

    /// Called when a sheet is closed and when the app goes to the background,
    /// which are the two moments its history could otherwise be lost.
    func flushRevisions() {
        guard !trailsToWrite.isEmpty else { return }
        try? FileManager.default.createDirectory(
            at: revisionsURL,
            withIntermediateDirectories: true
        )

        for pageID in trailsToWrite {
            guard let trail = trails[pageID],
                  let data = try? JSONEncoder().encode(trail) else { continue }
            try? data.write(to: trailURL(for: pageID), options: .atomic)
        }
        trailsToWrite.removeAll()
    }

    private func discardTrail(for pageID: String) {
        trails[pageID] = nil
        trailsToWrite.remove(pageID)
        try? FileManager.default.removeItem(at: trailURL(for: pageID))
    }

    private func publishStepAvailability(for pageID: String) {
        guard pageID == selectedPageID else { return }
        let held = trail(for: pageID)
        canStepBack = held.cursor > 0
        canStepForward = held.cursor + 1 < held.states.count
    }

    /// The page id is minted by this app as a UUID string, so it is already safe as
    /// a file name. Percent-encoded anyway, because a file name built from stored
    /// data is a path either way.
    private func trailURL(for pageID: String) -> URL {
        let safe = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-_"))
        let name = pageID.addingPercentEncoding(withAllowedCharacters: safe) ?? "unnamed"
        return revisionsURL.appendingPathComponent("\(name).json")
    }

    private static func drawing(from data: Data) -> PKDrawing {
        (try? PKDrawing(data: data)) ?? PKDrawing()
    }

    private func save() {
        let stored = Stored(pages: pages, series: series, pinnedPageID: pinnedPageID)
        guard let data = try? JSONEncoder().encode(stored) else { return }
        try? data.write(to: storeURL, options: .atomic)
    }
}
