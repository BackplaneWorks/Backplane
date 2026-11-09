import SwiftUI

struct PairActionStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.body.weight(.semibold))
            .frame(maxWidth: .infinity, minHeight: 48)
            .foregroundStyle(BackplaneDesign.onAccent)
            .background(BackplaneDesign.accent.opacity(configuration.isPressed ? 0.7 : 1))
            .contentShape(Rectangle())
    }
}

struct PairView: View {
    @State var link: String
    let done: (String) -> Void
    var status: String = ""
    var repair = false
    var adding = false
    @State private var code = ""
    @State private var hubAddress = ""
    @State private var scanning = false
    @State private var invalid = false

    init(link: String, status: String = "", repair: Bool = false, adding: Bool = false, done: @escaping (String) -> Void) {
        self._link = State(initialValue: link);self.status = status;self.repair = repair;self.adding = adding;self.done = done
    }

    private func connect(_ text: String) {
        guard Pairing.key(text) != nil else { invalid = true; return }
        invalid = false
        link = ""
        done(text)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                VStack(alignment: .leading, spacing: 10) {
                    BrandMark(size: 48)
                    Text(verbatim: "BACKPLANE")
                        .font(.mono(.caption, weight: .bold))
                        .tracking(2).foregroundStyle(BackplaneDesign.accent)
                    Text(Tr.t(repair ? "Re-pair this device" : adding ? "Add a hub" : "Connect to your Backplane hub")).font(.largeTitle.weight(.bold))
                    Text(Tr.t("On your paired computer, open Settings → Devices → Add a device.")).foregroundStyle(.secondary)
                    if !status.isEmpty { Text(Tr.t(status)).foregroundStyle(PhaseColor.warn).accessibilityIdentifier("pair.status") }
                }
                Button { scanning = true } label: {
                    Label(Tr.k("native.pair.scan"), systemImage: "qrcode.viewfinder")
                }
                .buttonStyle(PairActionStyle())
                .accessibilityIdentifier("pair.scan")
                VStack(alignment: .leading, spacing: 8) {
                    Text(Tr.t("Paste a copied pairing link to connect."))
                        .font(.footnote).foregroundStyle(.secondary)
                    PasteButton(payloadType: String.self) { values in
                        if let value = values.first { connect(value) } else { invalid = true }
                    }
                    .tint(BackplaneDesign.accent)
                    .buttonBorderShape(.roundedRectangle(radius: 0))
                    .accessibilityHint(Tr.t("Connects using the pasted pairing link."))
                    .accessibilityIdentifier("pair.paste")
                }
                VStack(alignment: .leading, spacing: 10) {
                    Text(Tr.k("settings.pair")).font(.headline)
                    TextField(Tr.k("settings.pair"), text: $link, prompt: Text(verbatim: "http://host:3787/pair#invite=…"))
                        .privacySensitive()
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                        .submitLabel(.go)
                        .onSubmit { connect(link) }
                        .padding(14)
                        .background(BackplaneDesign.panel)
                        .overlay(Rectangle().stroke(invalid ? PhaseColor.bad : BackplaneDesign.line, lineWidth: 1))
                        .accessibilityIdentifier("pair.address")
                    Text(Tr.t("Use a fresh link, QR code, or short code from Settings → Devices.")).font(.footnote).foregroundStyle(.secondary)
                    if invalid {
                        Text(Tr.t("Enter a valid hub address or pairing link."))
                            .font(.footnote).foregroundStyle(PhaseColor.bad)
                            .accessibilityIdentifier("pair.address.error")
                    }
                }
                DisclosureGroup(Tr.t("Type a short code")) {
                    TextField(Tr.t("Hub address"), text: $hubAddress).textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
                    TextField(Tr.t("8-character code"), text: $code).textInputAutocapitalization(.characters).autocorrectionDisabled().privacySensitive()
                    Button(Tr.t("Connect with code")) {
                        guard let value = Pairing.codeLink(address: hubAddress, code: code) else { invalid = true;return }
                        connect(value)
                    }.buttonStyle(PairActionStyle())
                }
                Button(Tr.k("native.pair.connect")) { connect(link) }
                    .buttonStyle(PairActionStyle())
                    .disabled(link.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .opacity(link.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? 0.5 : 1)
                    .accessibilityIdentifier("pair.connect")
            }
            .frame(maxWidth: 520, alignment: .leading)
            .padding(24)
        }
        .background(BackplaneDesign.background)
        .navigationBarTitleDisplayMode(.inline)
        .fullScreenCover(isPresented: $scanning) { ScanView { connect($0) } }
    }
}

