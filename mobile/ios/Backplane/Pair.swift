import AVFoundation
import SwiftUI

// A QR code as the hub drew it (core/qr.bend: rows, "1" dark), on white
// with four modules of quiet zone
struct QRCodeView: View {
    let rows: [String]

    var body: some View {
        Canvas { ctx, size in
            let n = CGFloat(rows.count + 8)
            let m = min(size.width, size.height) / n
            ctx.fill(Path(CGRect(origin: .zero, size: CGSize(width: m * n, height: m * n))), with: .color(.white))
            var dark = Path()
            for (y, row) in rows.enumerated() {
                for (x, c) in row.enumerated() where c == "1" {
                    dark.addRect(CGRect(x: (CGFloat(x) + 4) * m, y: (CGFloat(y) + 4) * m, width: m + 0.5, height: m + 0.5))
                }
            }
            ctx.fill(dark, with: .color(.black))
        }
        .aspectRatio(1, contentMode: .fit)
        .frame(maxWidth: 320)
    }
}

// The camera reading a pairing code: the first QR code it sees goes to
// found (the app pairs with any link Pairing understands)
struct ScanView: View {
    let tag: String
    let found: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var denied = false

    var body: some View {
        NavigationStack {
            ZStack {
                Color.black.ignoresSafeArea()
                if denied {
                    Text(NativeMessages.native_pair_scanner_denied(tag))
                        .foregroundStyle(.white).multilineTextAlignment(.center).padding()
                } else {
                    Scanner { s in found(s); dismiss() }.ignoresSafeArea()
                    VStack {
                        Spacer()
                        Text(NativeMessages.native_pair_scanner_instructions(tag))
                            .font(.footnote).foregroundStyle(.white).multilineTextAlignment(.center).padding()
                    }
                }
            }
            .navigationTitle(NativeMessages.native_pair_scanner_title(tag))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button(NativeMessages.common_cancel(tag)) { dismiss() } } }
            .task {
                switch AVCaptureDevice.authorizationStatus(for: .video) {
                case .notDetermined: denied = !(await AVCaptureDevice.requestAccess(for: .video))
                case .authorized: denied = false
                default: denied = true
                }
            }
        }
    }
}

private struct Scanner: UIViewControllerRepresentable {
    let found: (String) -> Void

    func makeUIViewController(context: Context) -> ScannerController {
        let c = ScannerController()
        c.found = found
        return c
    }

    func updateUIViewController(_ c: ScannerController, context: Context) {}
}

final class ScannerController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    var found: ((String) -> Void)?
    private let session = AVCaptureSession()
    private var preview: AVCaptureVideoPreviewLayer?
    private var done = false

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        guard let cam = AVCaptureDevice.default(for: .video), let input = try? AVCaptureDeviceInput(device: cam),
              session.canAddInput(input) else { return }
        session.addInput(input)
        let out = AVCaptureMetadataOutput()
        guard session.canAddOutput(out) else { return }
        session.addOutput(out)
        out.setMetadataObjectsDelegate(self, queue: .main)
        out.metadataObjectTypes = [.qr]
        let p = AVCaptureVideoPreviewLayer(session: session)
        p.videoGravity = .resizeAspectFill
        view.layer.addSublayer(p)
        preview = p
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        preview?.frame = view.bounds
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        let s = session
        DispatchQueue.global(qos: .userInitiated).async { s.startRunning() }
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        let s = session
        DispatchQueue.global(qos: .userInitiated).async { s.stopRunning() }
    }

    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from connection: AVCaptureConnection) {
        guard !done, let code = objects.compactMap({ $0 as? AVMetadataMachineReadableCodeObject }).first?.stringValue,
              Pairing.key(code) != nil else { return }
        done = true
        Haptics.success()
        found?(code)
    }
}
