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
    let tag: String
    @State var link: String
    let done: (String) -> Void
    var status: String = ""
    var repair = false
    @State private var code = ""
    @State private var hubAddress = ""
    @State private var scanning = false
    @State private var invalid = false

    init(tag: String, link: String, status: String = "", repair: Bool = false, done: @escaping (String) -> Void) {
        self.tag = tag;self._link = State(initialValue: link);self.status = status;self.repair = repair;self.done = done
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
                    Text("BACKPLANE")
                        .font(.mono(.caption, weight: .bold))
                        .tracking(2).foregroundStyle(BackplaneDesign.accent)
                    Text(repair ? "Re-pair this device" : "Connect to your Backplane hub").font(.largeTitle.weight(.bold))
                    Text("On your paired computer, open Settings → Devices → Add a device.").foregroundStyle(.secondary)
                    if !status.isEmpty { Text(status).foregroundStyle(PhaseColor.warn).accessibilityIdentifier("pair.status") }
                }
                Button { scanning = true } label: {
                    Label(NativeMessages.native_pair_scan(tag), systemImage: "qrcode.viewfinder")
                }
                .buttonStyle(PairActionStyle())
                .accessibilityIdentifier("pair.scan")
                VStack(alignment: .leading, spacing: 8) {
                    Text(tag == "ja" ? "コピーした接続リンクをペーストして接続します。" : "Paste a copied pairing link to connect.")
                        .font(.footnote).foregroundStyle(.secondary)
                    PasteButton(payloadType: String.self) { values in
                        if let value = values.first { connect(value) } else { invalid = true }
                    }
                    .tint(BackplaneDesign.accent)
                    .buttonBorderShape(.roundedRectangle(radius: 0))
                    .accessibilityHint(tag == "ja" ? "ペーストしたリンクで接続します。" : "Connects using the pasted pairing link.")
                    .accessibilityIdentifier("pair.paste")
                }
                VStack(alignment: .leading, spacing: 10) {
                    Text(NativeMessages.settings_pair(tag)).font(.headline)
                    TextField(NativeMessages.settings_pair(tag), text: $link, prompt: Text(verbatim: "http://host:3787/pair#invite=…"))
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
                    Text("Use a fresh link, QR code, or short code from Settings → Devices.").font(.footnote).foregroundStyle(.secondary)
                    if invalid {
                        Text(tag == "ja" ? "有効な接続アドレスを入力してください。" : "Enter a valid hub address or pairing link.")
                            .font(.footnote).foregroundStyle(PhaseColor.bad)
                            .accessibilityIdentifier("pair.address.error")
                    }
                }
                DisclosureGroup("Type a short code") {
                    TextField("Hub address", text: $hubAddress).textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
                    TextField("8-character code", text: $code).textInputAutocapitalization(.characters).autocorrectionDisabled().privacySensitive()
                    Button("Connect with code") {
                        guard let value = Pairing.codeLink(address: hubAddress, code: code) else { invalid = true;return }
                        connect(value)
                    }.buttonStyle(PairActionStyle())
                }
                Button(NativeMessages.native_pair_connect(tag)) { connect(link) }
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
        .fullScreenCover(isPresented: $scanning) { ScanView(tag: tag) { connect($0) } }
    }
}

