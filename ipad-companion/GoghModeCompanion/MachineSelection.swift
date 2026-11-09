import Foundation

/// The machines this iPad sends to through the hub: a standing selection, kept
/// until the owner changes it, so every sheet goes where the last one went.
@MainActor
final class MachineSelection: ObservableObject {
    /// One line in the machine list.
    struct Row: Equatable, Identifiable {
        let name: String
        let isTicked: Bool
        /// False for a ticked machine the hub no longer lists: it stays visible,
        /// so a selection never shrinks without the owner seeing it.
        let isOffered: Bool
        let lastFetchedAt: Date?

        var id: String { name }
    }

    static let nothingTickedMessage =
        "Tick a machine to send this sheet to. Your drawing stays on the iPad until you do."

    @Published private(set) var ticked: [String]

    private let defaults: UserDefaults
    private let tickedKey = "goghmode-hub-machines"

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        ticked = (defaults.stringArray(forKey: tickedKey) ?? []).sorted()
    }

    func toggle(_ machine: String) {
        if ticked.contains(machine) {
            ticked.removeAll { $0 == machine }
        } else {
            ticked = (ticked + [machine]).sorted()
        }
        defaults.set(ticked, forKey: tickedKey)
    }

    func rows(offered: [HubMachine]) -> [Row] {
        let offeredRows = offered.map { machine in
            Row(
                name: machine.name,
                isTicked: ticked.contains(machine.name),
                isOffered: true,
                lastFetchedAt: machine.lastFetchedAt
            )
        }
        let offeredNames = Set(offered.map(\.name))
        let missingRows = ticked.filter { !offeredNames.contains($0) }.map { name in
            Row(name: name, isTicked: true, isOffered: false, lastFetchedAt: nil)
        }
        return (offeredRows + missingRows).sorted { $0.name < $1.name }
    }
}
