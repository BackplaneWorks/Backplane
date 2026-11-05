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

struct DeviceManagerView: View {
    let link: String
    @Environment(\.dismiss) private var dismiss
    @State private var devices: [PairedDevice] = []
    @State private var invitation: DeviceInvitation?
    @State private var status = ""
    @State private var revokeTarget: PairedDevice?
    @State private var links: [IssuedLink] = []
    @State private var uninviteTarget: IssuedLink?
    struct PairedDevice: Identifiable { let id: String; let name: String; let lastSeen: Date }
    // a pairing link this hub issued (/auth/devices "invitations"): when, used
    // or not and by which device; revoking a used one revokes that device too
    struct IssuedLink: Identifiable {
        // (machine: a hub-to-hub invite made in Bots; its "device" is the link it made)
        let id: String; let state: String; let created: Date; let expires: Date; let used: Date?; let device: String; let deviceName: String; let deviceRevoked: Bool; let machine: Bool
        var revocable: Bool { state == "open" || (state == "used" && !device.isEmpty && !deviceRevoked) }
        var title: String { machine ? "Machine invite" : "Device invitation" }
        var summary: String {
            switch state {
            case "used": return "Used " + (used.map { $0.formatted(date: .abbreviated, time: .shortened) } ?? "") + " by " + (deviceName.isEmpty ? (machine ? "a machine" : "a device") : deviceName) + (deviceRevoked ? (machine ? " (unlinked)" : " (revoked)") : "")
            case "revoked": return device.isEmpty ? "Revoked, never used" : "Revoked with " + (deviceName.isEmpty ? (machine ? "the machine it linked" : "the device it paired") : deviceName)
            case "expired": return "Expired, never used"
            default: return "Not used yet · expires " + expires.formatted(date: .omitted, time: .shortened)
            }
        }
    }
    struct DeviceInvitation { let link: String; let code: String; let address: String; let expires: Date; let qr: [String] }
    private func request(_ path: String, method: String = "GET", device: String? = nil, invitation: String? = nil) async throws -> [String: Any] {
        guard let url = Pairing.http(link, path: path, token: false),
              let token = Pairing.socket(link).flatMap({ URLComponents(url: $0, resolvingAgainstBaseURL: false) })?.queryItems?.first(where: { $0.name == "token" })?.value else { throw URLError(.badURL) }
        var request = URLRequest(url: url);request.httpMethod = method;request.timeoutInterval = 8
        request.setValue("Bearer " + token, forHTTPHeaderField: "Authorization")
        request.setValue("native", forHTTPHeaderField: "X-Backplane-Client")
        request.setValue(url.scheme ?? "http", forHTTPHeaderField: "X-Backplane-Scheme")
        if let device { request.setValue(device, forHTTPHeaderField: "X-Backplane-Device") }
        if let invitation { request.setValue(invitation, forHTTPHeaderField: "X-Backplane-Invitation") }
        let config = URLSessionConfiguration.ephemeral;config.httpShouldSetCookies = false;config.urlCache = nil
        let session = URLSession(configuration: config, delegate: Pairing.NoPairRedirect(), delegateQueue: nil);defer { session.invalidateAndCancel() }
        let (data, response) = try await session.data(for: request)
        guard (response as? HTTPURLResponse)?.statusCode == 200, let result = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { throw URLError(.userAuthenticationRequired) }
        return result
    }
    private func refresh() async {
        do { let result = try await request("/auth/devices")
            devices = (result["devices"] as? [[String: Any]] ?? []).filter { $0["revoked"] as? Bool != true }.compactMap { d in
                guard let id = d["id"] as? String, let name = d["name"] as? String, let seen = d["lastSeen"] as? Double else { return nil }
                return PairedDevice(id: id, name: name, lastSeen: Date(timeIntervalSince1970: seen / 1000))
            }
            links = (result["invitations"] as? [[String: Any]] ?? []).compactMap { x in
                guard let id = x["id"] as? String, let state = x["state"] as? String, let created = x["createdAt"] as? Double, let expires = x["expiresAt"] as? Double else { return nil }
                return IssuedLink(id: id, state: state, created: Date(timeIntervalSince1970: created / 1000), expires: Date(timeIntervalSince1970: expires / 1000),
                    used: (x["usedAt"] as? Double).map { Date(timeIntervalSince1970: $0 / 1000) }, device: x["device"] as? String ?? "", deviceName: x["deviceName"] as? String ?? "", deviceRevoked: x["deviceRevoked"] as? Bool == true, machine: x["kind"] as? String == "machine")
            }
        } catch { status = "Could not load devices. Check your connection and pairing." }
    }
    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    Text("Add another computer or phone. Invitations work once and expire in 10 minutes.")
                    if !status.isEmpty { Text(status).foregroundStyle(.secondary) }
                    Button("Add a device") { Task {
                        status = "Creating invitation…"
                        do { let result = try await request("/auth/invitations", method: "POST")
                            guard let value = result["link"] as? String, let code = result["code"] as? String,
                                  let until = result["expiresAt"] as? Double else { status = "Could not create invitation.";return }
                            invitation = DeviceInvitation(link: value, code: code, address: (result["routes"] as? [String])?.first ?? "", expires: Date(timeIntervalSince1970: until / 1000), qr: (result["qr"] as? String ?? "").components(separatedBy: "\n").filter { !$0.isEmpty })
                            status = "Invitation ready. Share it with the device you want to authorize."
                        } catch { status = "Could not create invitation. Check the hub connection." }
                    } }.buttonStyle(.borderedProminent)
                    if let invitation {
                        TimelineView(.periodic(from: .now, by: 1)) { clock in
                            VStack(alignment: .leading, spacing: 12) {
                                if clock.date < invitation.expires {
                                    Text(invitation.code).font(.mono(.largeTitle, weight: .bold)).privacySensitive()
                                    Text("Hub address: " + invitation.address)
                                    Text("Expires ").font(.footnote) + Text(invitation.expires, style: .relative).font(.footnote)
                                    QRCodeView(rows: invitation.qr).privacySensitive()
                                    HStack {
                                        Button("Copy link") { UIPasteboard.general.string = invitation.link }
                                        ShareLink("Share", item: invitation.link)
                                        Button("Copy code") { UIPasteboard.general.string = invitation.code }
                                    }
                                } else { Text("Expired — create a new invitation.") }
                            }
                        }
                    }
                    Text("Paired devices").font(.headline)
                    if devices.isEmpty { Text("Existing pairings are preserved. New devices appear here after connecting.").foregroundStyle(.secondary) }
                    ForEach(devices) { device in
                        VStack(alignment: .leading) {
                            Text(device.name).font(.headline)
                            Text("Last seen ") + Text(device.lastSeen, style: .relative)
                            Button("Revoke", role: .destructive) { revokeTarget = device }
                        }.padding(.vertical, 8)
                    }
                    Text("Pairing links").font(.headline)
                    if links.isEmpty { Text("Links made with Add a device, and invites made in Bots to link a machine, appear here.").foregroundStyle(.secondary) }
                    ForEach(links) { x in
                        VStack(alignment: .leading) {
                            Text(x.title + " · " + x.created.formatted(date: .abbreviated, time: .shortened)).font(.headline)
                            Text(x.summary).foregroundStyle(.secondary)
                            if x.revocable { Button("Revoke", role: .destructive) { uninviteTarget = x } }
                        }.padding(.vertical, 8)
                    }
                }.frame(maxWidth: 520, alignment: .leading).padding(24)
            }
            .navigationTitle("Devices")
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Done") { dismiss() } } }
            .task { await refresh() }
            .alert("Revoke access?", isPresented: Binding(get: { revokeTarget != nil }, set: { if !$0 { revokeTarget = nil } })) {
                Button("Revoke", role: .destructive) { if let device = revokeTarget { Task { do { _ = try await request("/auth/revoke", method: "POST", device: device.id);await refresh() } catch { status = "Could not revoke this device." } } };revokeTarget = nil }
                Button("Cancel", role: .cancel) { revokeTarget = nil }
            } message: { Text("This device will need a new invitation to reconnect.") }
            .alert("Revoke this link?", isPresented: Binding(get: { uninviteTarget != nil }, set: { if !$0 { uninviteTarget = nil } })) {
                Button("Revoke", role: .destructive) { if let x = uninviteTarget { Task { do { _ = try await request("/auth/revoke", method: "POST", invitation: x.id);await refresh() } catch { status = "Could not revoke this link." } } };uninviteTarget = nil }
                Button("Cancel", role: .cancel) { uninviteTarget = nil }
            } message: { Text(uninviteTarget?.state == "used" ? (uninviteTarget?.machine == true ? "The machine it linked (" : "The device it paired (") + ((uninviteTarget?.deviceName).flatMap { $0.isEmpty ? nil : $0 } ?? "unnamed") + (uninviteTarget?.machine == true ? ") is unlinked too." : ") loses access too.") : "It will no longer pair anything.") }
        }
    }
}
