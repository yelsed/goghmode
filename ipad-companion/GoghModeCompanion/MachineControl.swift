import SwiftUI

/// The chip that says how many machines a sheet goes to through the hub, and
/// opens the list to change that. Shaped like `StatusBadge`, beside which it
/// sits: a dot, a label in the chip's lettering, the slot reserved at its widest.
struct MachineControl: View {
    @ObservedObject var uploader: UploadController
    @ObservedObject var selection: MachineSelection
    let destination: UploadController.Destination
    /// The open sheet, whose deliveries the list shows; `nil` on the register.
    let pageID: String?

    @State private var showingList = false

    private static let widestLabel = "12 machines"

    var body: some View {
        Button {
            showingList = true
        } label: {
            HStack(spacing: 7) {
                Circle()
                    .fill(selection.ticked.isEmpty ? Sheet.inkLabel : Sheet.review)
                    .frame(width: 8, height: 8)
                chipText(Self.widestLabel.uppercased())
                    .hidden()
                    .overlay(alignment: .leading) {
                        chipText(label.uppercased())
                            .foregroundStyle(Sheet.onGround)
                            .fixedSize()
                    }
                Image(systemName: "chevron.down")
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(Sheet.inkLabel)
            }
            .padding(.horizontal, 8)
            .frame(height: 44)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(Text("Sends to \(label)"))
        .popover(isPresented: $showingList) {
            MachineListView(
                uploader: uploader,
                selection: selection,
                destination: destination,
                pageID: pageID
            )
            .frame(minWidth: 340, minHeight: 200)
        }
    }

    private var label: String {
        switch selection.ticked.count {
        case 0: "No machine"
        case 1: "1 machine"
        default: "\(selection.ticked.count) machines"
        }
    }

    private func chipText(_ text: String) -> some View {
        Text(text)
            .font(.caption2.weight(.semibold))
            .tracking(0.8)
            .lineLimit(1)
    }
}

/// The machines the hub offers, ticked or not, and per machine what the hub said
/// about the open sheet. Asks the hub again every few seconds while it is open,
/// so a queued sheet turns delivered in front of the owner.
struct MachineListView: View {
    @ObservedObject var uploader: UploadController
    @ObservedObject var selection: MachineSelection
    let destination: UploadController.Destination
    let pageID: String?

    private static let refreshInterval: Duration = .seconds(3)

    var body: some View {
        List {
            Section {
                if let complaint = uploader.machineListComplaint, uploader.offeredMachines == nil {
                    Text(complaint)
                        .font(.footnote)
                        .foregroundStyle(Sheet.inkLabel)
                } else if rows.isEmpty, uploader.offeredMachines != nil {
                    Text("No machine fetches from the hub yet. Run goghmode receive on a machine and allow it on dev.procrast.nl.")
                        .font(.footnote)
                        .foregroundStyle(Sheet.inkLabel)
                }
                ForEach(rows) { row in
                    Button {
                        selection.toggle(row.name)
                    } label: {
                        MachineRowView(row: row, delivery: delivery(for: row.name))
                    }
                    .buttonStyle(.plain)
                }
            } header: {
                BlockLabel(text: pageID == nil ? "Send sheets to" : "Send this sheet to")
            }
        }
        .listStyle(.insetGrouped)
        .task {
            while !Task.isCancelled {
                await uploader.loadOfferedMachines(from: destination)
                if let pageID {
                    await uploader.refreshDeliveries(of: pageID, from: destination)
                }
                try? await Task.sleep(for: Self.refreshInterval)
            }
        }
    }

    private var rows: [MachineSelection.Row] {
        selection.rows(offered: uploader.offeredMachines ?? [])
    }

    private func delivery(for machine: String) -> MachineDelivery? {
        guard let pageID else { return nil }
        return uploader.hubDeliveries[pageID]?.first { $0.machine == machine }
    }
}

struct MachineRowView: View {
    let row: MachineSelection.Row
    let delivery: MachineDelivery?

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: row.isTicked ? "checkmark.circle.fill" : "circle")
                .foregroundStyle(row.isTicked ? Color.accentColor : .secondary)
                .font(.title3)
            VStack(alignment: .leading, spacing: 2) {
                Text(row.name)
                    .font(.body.weight(.medium))
                    .foregroundStyle(row.isOffered ? Sheet.onGround : Sheet.inkLabel)
                    .lineLimit(1)
                    .truncationMode(.middle)
                if let detail {
                    Text(detail)
                        .font(.caption)
                        .foregroundStyle(Sheet.inkLabel)
                }
            }
            Spacer(minLength: 8)
            if let delivery {
                deliveryText(delivery)
            }
        }
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(row.isTicked ? .isSelected : [])
    }

    /// A refusal names its reason under the machine; a ticked machine the hub no
    /// longer lists says since when it has not fetched.
    private var detail: String? {
        if let delivery, delivery.state == .refused, let reason = delivery.reason {
            return reason
        }
        if !row.isOffered {
            return "not fetching from the hub now"
        }
        return nil
    }

    @ViewBuilder
    private func deliveryText(_ delivery: MachineDelivery) -> some View {
        switch delivery.state {
        case .queued:
            Text("queued")
                .font(.caption)
                .foregroundStyle(Sheet.inkLabel)
        case .refused:
            Text("refused")
                .font(.caption)
                .foregroundStyle(Sheet.onGround)
        case .delivered:
            HStack(spacing: 4) {
                Text("delivered")
                    .font(.caption)
                if let confirmedAt = delivery.confirmedAt {
                    Text(confirmedAt.formatted(.dateTime.hour().minute()))
                        .font(.caption.monospaced())
                }
            }
            .foregroundStyle(Sheet.onGroundSecondary)
        }
    }
}
