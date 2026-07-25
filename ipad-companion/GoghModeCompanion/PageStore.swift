import Foundation
import PencilKit

struct NotebookPage: Codable, Equatable, Identifiable {
    let id: String
    var title: String
    var updatedAt: Date
    var drawingData: Data

    var drawing: PKDrawing {
        (try? PKDrawing(data: drawingData)) ?? PKDrawing()
    }

    var isEmpty: Bool {
        drawing.strokes.isEmpty
    }

    var pageRef: PageRef {
        PageRef(id: id, title: title)
    }
}

/// The iPad's own copy of every page. This is the write that makes work
/// survive — the Mac holds a mirror, so a page drawn while the Mac is closed is
/// still here when it comes back.
@MainActor
final class PageStore: ObservableObject {
    @Published private(set) var pages: [NotebookPage] = []
    @Published private(set) var selectedPageID: String = ""

    private let storeURL: URL

    var selectedPage: NotebookPage? {
        pages.first { $0.id == selectedPageID }
    }

    var selectedDrawing: PKDrawing {
        selectedPage?.drawing ?? PKDrawing()
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
    func addPage() -> NotebookPage {
        let page = appendPage()
        selectedPageID = page.id
        save()
        return page
    }

    func select(_ pageID: String) {
        guard pages.contains(where: { $0.id == pageID }) else { return }
        selectedPageID = pageID
    }

    func updateSelectedPage(with drawing: PKDrawing) {
        guard let index = pages.firstIndex(where: { $0.id == selectedPageID }) else { return }
        pages[index].drawingData = drawing.dataRepresentation()
        pages[index].updatedAt = Date()
        // Newest first, matching how the Mac lists them.
        let page = pages.remove(at: index)
        pages.insert(page, at: 0)
        save()
    }

    func rename(_ pageID: String, to title: String) {
        guard let index = pages.firstIndex(where: { $0.id == pageID }) else { return }
        pages[index].title = title
        save()
    }

    @discardableResult
    private func appendPage() -> NotebookPage {
        let page = NotebookPage(
            id: UUID().uuidString,
            title: PageStore.defaultTitle(for: Date()),
            updatedAt: Date(),
            drawingData: PKDrawing().dataRepresentation()
        )
        pages.insert(page, at: 0)
        return page
    }

    private static func defaultTitle(for date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "d MMM HH:mm"
        return formatter.string(from: date)
    }

    private func load() {
        guard let data = try? Data(contentsOf: storeURL),
              let stored = try? JSONDecoder().decode([NotebookPage].self, from: data) else {
            return
        }
        pages = stored
    }

    private func save() {
        guard let data = try? JSONEncoder().encode(pages) else { return }
        try? data.write(to: storeURL, options: .atomic)
    }
}
