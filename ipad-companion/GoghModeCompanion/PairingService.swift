import AVFoundation
import Foundation
import SwiftUI
import UIKit

/// What the host shows in its QR code.
struct PairingPayload: Codable, Equatable {
    let v: Int
    let hostId: String
    let name: String
    let platform: String
    let addresses: [String]
    let pairingSecret: String
}

enum PairingError: LocalizedError, Equatable {
    case unreadableCode
    case unreachable
    case refused
    case hostCouldNotProveItself

    var errorDescription: String? {
        switch self {
        case .unreadableCode:
            "That is not a GoghMode pairing code. Open Devices on the desktop and pair again."
        case .unreachable:
            "No answer at that address. Check you are on the same Wi-Fi."
        case .refused:
            "The desktop refused the pairing. It may have expired — pair again."
        case .hostCouldNotProveItself:
            "Something answered, but it is not the machine showing that code. Do not continue."
        }
    }
}

enum PairingService {
    static func parse(_ text: String) -> PairingPayload? {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let data = trimmed.data(using: .utf8),
            let payload = try? JSONDecoder().decode(PairingPayload.self, from: data),
            !payload.hostId.isEmpty,
            !payload.pairingSecret.isEmpty
        else {
            return nil
        }
        return payload
    }

    /// Completes the handshake and returns the host to save plus the key to keep.
    ///
    /// The key is derived here, not received. Nothing secret crosses the
    /// network, so an attacker recording the whole exchange gains nothing.
    static func pair(
        with payload: PairingPayload,
        deviceID: String,
        deviceName: String,
        session: URLSession = .shared
    ) async throws -> (host: SavedHost, secret: String) {
        let body = try JSONEncoder().encode(
            PairRequestBody(
                hostId: payload.hostId,
                deviceId: deviceID,
                deviceName: deviceName,
                platform: "ipados"
            )
        )
        let signature = GoghModeCrypto.pairRequestMac(
            pairingSecret: payload.pairingSecret,
            hostID: payload.hostId,
            deviceID: deviceID,
            deviceName: deviceName
        )

        var lastFailure = PairingError.unreachable
        for address in payload.addresses {
            guard let url = URL(string: address + "/v2/pair") else { continue }
            var request = URLRequest(url: url)
            request.httpMethod = "POST"
            request.setValue("application/json; charset=utf-8", forHTTPHeaderField: "Content-Type")
            request.setValue(signature, forHTTPHeaderField: "X-GoghMode-Pair-Mac")
            request.httpBody = body
            // The desktop waits for a person to tap approve, so this is a slow
            // request by design rather than a stalled one.
            request.timeoutInterval = 90

            guard let (_, response) = try? await session.data(for: request),
                let httpResponse = response as? HTTPURLResponse
            else {
                continue
            }
            guard httpResponse.statusCode == 200 else {
                lastFailure = .refused
                continue
            }

            // Proves the machine that answered holds the code on the screen the
            // user scanned, rather than merely being reachable at that address.
            let expected = GoghModeCrypto.pairResponseMac(
                pairingSecret: payload.pairingSecret,
                hostID: payload.hostId,
                deviceID: deviceID
            )
            let offered = httpResponse.value(forHTTPHeaderField: "X-GoghMode-Pair-Mac") ?? ""
            guard GoghModeCrypto.matches(expected, offered) else {
                throw PairingError.hostCouldNotProveItself
            }

            let host = SavedHost(
                id: payload.hostId,
                name: payload.name,
                platform: payload.platform,
                address: address,
                credential: .paired
            )
            let secret = GoghModeCrypto.deriveDeviceSecret(
                pairingSecret: payload.pairingSecret,
                deviceID: deviceID
            )
            return (host, secret)
        }

        throw lastFailure
    }

    private struct PairRequestBody: Encodable {
        let hostId: String
        let deviceId: String
        let deviceName: String
        let platform: String
    }
}

/// Reads the pairing code off the desktop's screen.
///
/// Typing a 32-character secret on a tablet is the kind of friction that stops
/// people pairing at all, so the camera is the main path and pasting the text is
/// the fallback.
struct PairingScanner: UIViewControllerRepresentable {
    var onScan: (String) -> Void

    func makeUIViewController(context: Context) -> ScannerViewController {
        let controller = ScannerViewController()
        controller.onScan = onScan
        return controller
    }

    func updateUIViewController(_ controller: ScannerViewController, context: Context) {
        controller.onScan = onScan
    }
}

final class ScannerViewController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    var onScan: ((String) -> Void)?

    private let session = AVCaptureSession()
    private var preview: AVCaptureVideoPreviewLayer?
    private var hasScanned = false

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black

        guard let device = AVCaptureDevice.default(for: .video),
            let input = try? AVCaptureDeviceInput(device: device),
            session.canAddInput(input)
        else {
            return
        }
        session.addInput(input)

        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else { return }
        session.addOutput(output)
        output.setMetadataObjectsDelegate(self, queue: .main)
        output.metadataObjectTypes = [.qr]

        let preview = AVCaptureVideoPreviewLayer(session: session)
        preview.videoGravity = .resizeAspectFill
        preview.frame = view.bounds
        view.layer.addSublayer(preview)
        self.preview = preview
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        preview?.frame = view.bounds
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        hasScanned = false
        guard !session.isRunning else { return }
        // Starting the session blocks, so it does not belong on the main thread.
        let session = session
        DispatchQueue.global(qos: .userInitiated).async {
            session.startRunning()
        }
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        guard session.isRunning else { return }
        let session = session
        DispatchQueue.global(qos: .userInitiated).async {
            session.stopRunning()
        }
    }

    func metadataOutput(
        _ output: AVCaptureMetadataOutput,
        didOutput metadataObjects: [AVMetadataObject],
        from connection: AVCaptureConnection
    ) {
        // One code per presentation. The camera fires repeatedly while pointed
        // at the same code, and pairing twice burns the second attempt.
        guard !hasScanned,
            let object = metadataObjects.first as? AVMetadataMachineReadableCodeObject,
            let text = object.stringValue
        else {
            return
        }
        hasScanned = true
        onScan?(text)
    }
}
