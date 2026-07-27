import SwiftUI

/// The saved hosts and the way to add one. Selecting a destination is a
/// deliberate act here, never something that happens as a side effect of a
/// failure elsewhere.
struct HostListView: View {
    @ObservedObject var hostStore: HostStore
    @Environment(\.dismiss) private var dismiss
    @State private var showingPairing = false

    var body: some View {
        NavigationStack {
            List {
                if hostStore.hosts.isEmpty {
                    Text("No hosts yet. Open Devices in GoghMode on your desktop and pair.")
                        .foregroundStyle(.secondary)
                }

                ForEach(hostStore.hosts) { host in
                    Button {
                        hostStore.select(host.id)
                        dismiss()
                    } label: {
                        HStack(spacing: 12) {
                            Image(systemName: host.isPaired ? "lock.fill" : "link")
                                .foregroundStyle(host.isPaired ? Color.accentColor : .secondary)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(host.name).font(.body.weight(.medium))
                                Text(subtitle(for: host))
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            if host.id == hostStore.selectedHostID {
                                Image(systemName: "checkmark.circle.fill")
                                    .foregroundStyle(.tint)
                            }
                        }
                    }
                    .buttonStyle(.plain)
                }
                .onDelete { offsets in
                    for index in offsets {
                        hostStore.remove(hostStore.hosts[index].id)
                    }
                }
            }
            .navigationTitle("Hosts")
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button("Pair") { showingPairing = true }
                }
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }
                }
            }
            .sheet(isPresented: $showingPairing) {
                PairingView(hostStore: hostStore)
            }
        }
    }

    private func subtitle(for host: SavedHost) -> String {
        host.isPaired
            ? "\(host.platform) · paired"
            // Named plainly rather than dressed up: this one is only as private
            // as the URL it was set up with.
            : "unauthenticated link"
    }
}

struct PairingView: View {
    @ObservedObject var hostStore: HostStore
    @Environment(\.dismiss) private var dismiss

    @State private var pastedCode = ""
    @State private var isPairing = false
    @State private var failure: String?

    var body: some View {
        NavigationStack {
            VStack(spacing: 16) {
                if isPairing {
                    ProgressView("Waiting for the desktop to approve…")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    PairingScanner { scanned in
                        pair(with: scanned)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                }

                if let failure {
                    Text(failure)
                        .font(.footnote)
                        .foregroundStyle(.red)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }

                VStack(alignment: .leading, spacing: 6) {
                    Text("Or paste the pairing code")
                        .font(.headline)
                    HStack {
                        TextField("{\"v\":1,…}", text: $pastedCode)
                            .textFieldStyle(.roundedBorder)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                        Button("Pair") { pair(with: pastedCode) }
                            .buttonStyle(.borderedProminent)
                            .disabled(pastedCode.isEmpty || isPairing)
                    }
                }
            }
            .padding(20)
            .navigationTitle("Pair a host")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
    }

    private func pair(with text: String) {
        guard !isPairing else { return }
        guard let payload = PairingService.parse(text) else {
            failure = PairingError.unreadableCode.errorDescription
            return
        }

        isPairing = true
        failure = nil
        Task {
            do {
                let result = try await PairingService.pair(
                    with: payload,
                    deviceID: hostStore.deviceID,
                    deviceName: hostStore.deviceName
                )
                hostStore.add(result.host, secret: result.secret)
                dismiss()
            } catch {
                failure = (error as? PairingError)?.errorDescription
                    ?? error.localizedDescription
                isPairing = false
            }
        }
    }
}
