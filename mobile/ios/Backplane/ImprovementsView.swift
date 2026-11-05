import SwiftUI
import UniformTypeIdentifiers
import ImageIO
import SafariServices

struct ImprovementsView: View {
    let model: AppModel
    @Environment(\.dismiss) private var dismiss
    @State private var draft = ImprovementsDraft()
    @State private var preview: ImprovementsPreview?
    @State private var image: UIImage?
    @State private var loaded = false
    @State private var observedText = ""
    @State private var observedReport = ""
    @State private var observedDetails = ""
    @State private var failedLoad = false
    @State private var busy = false
    @State private var choosing = false
    @State private var confirmNew = false
    @State private var confirmRetry = false
    @State private var note = ""
    @State private var browser: ImprovementsBrowserPage?
    private let store = ImprovementsStore()

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Picker("Send to", selection: $draft.target) {
                        Text("Public GitHub issue").tag(ImprovementsDraft.Target.github)
                        Text("Internal agent thread").tag(ImprovementsDraft.Target.internal)
                    }
                    .disabled(busy || failedLoad || !loaded)
                }
                if draft.target == .internal {
                    Section {
                        Text("Continue the existing internal Improvements workflow in your hub’s browser. Its saved drafts stay internal. Your public draft is kept on this device.")
                        Button("Open internal Improvements") { openInternal() }
                            .disabled(busy || failedLoad || !loaded)
                    }
                } else {
                    Section("What would you like to improve?") {
                        TextEditor(text: $draft.text).frame(minHeight: 130)
                            .accessibilityIdentifier("improvements.text")
                            .disabled(busy || failedLoad || !loaded)
                        Text("Optional. This opens a public issue draft on BackplaneWorks/Backplane. You review and press Submit on GitHub. The report is attributed to your GitHub account; sign in there using your own account if needed.")
                        Button("Write report with agent in hub browser") { openInternal(writer: true) }
                            .disabled(busy || failedLoad || !loaded || draft.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        Text("Hands off only this description to your hub’s public report form. Press its writer button to generate a filled template with writing guidance. Review and edit it there; you can also paste the completed body below. No screenshot is handed off.")
                    }
                    Section {
                        Toggle("I may add a screenshot publicly", isOn: $draft.screenshotConsent)
                            .disabled(busy || failedLoad || !loaded)
                            .accessibilityIdentifier("improvements.screenshotConsent")
                        Text("Screenshots may expose private information. Nothing is captured automatically. A chosen image stays on this device; review it and manually attach it on GitHub if you want it public.")
                        if draft.screenshotConsent {
                            Button("Choose screenshot") { choosing = true }.disabled(busy)
                            if let image {
                                Image(uiImage: image).resizable().scaledToFit().frame(maxHeight: 220)
                                    .accessibilityLabel("Local screenshot preview; not uploaded")
                                Button("Remove screenshot") { store.removeImage(); self.image = nil; draft.evidenceReviewed = false; persist() }
                            }
                        }
                    }
                    Section("Thorough behavior description") {
                        TextEditor(text: $draft.evidenceDetails).frame(minHeight: 100).disabled(busy || failedLoad || !loaded)
                        Text("Include steps, expected and actual behavior. Supply a thorough description here or choose an evidence screenshot to attach manually on GitHub.")
                        Button("Fill report from description") { prepare(fill: true) }
                            .disabled(busy || failedLoad || !loaded || draft.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    }
                    Section("Editable complete issue body") {
                        TextEditor(text: $draft.report).font(.mono(.body)).frame(minHeight: 200)
                            .disabled(busy || failedLoad || !loaded).accessibilityIdentifier("improvements.report")
                        Toggle("I reviewed the report’s evidence: an image to attach manually, or a thorough behavior description", isOn: $draft.evidenceReviewed)
                            .disabled(busy || failedLoad || !loaded)
                        Text("The body below is sent exactly as edited. Review its OS family and app version too.")
                    }
                    Section {
                        Button("Preview exact public issue body") { prepare(fill: false) }
                            .disabled(busy || failedLoad || !loaded || draft.report.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        if let preview {
                            Text(verbatim: preview.body).font(.mono(.body)).textSelection(.enabled)
                                .accessibilityIdentifier("improvements.preview")
                            Text("Only this body is prefilled. The selected screenshot is not attached. Your hub, project, paths, thread and account details are not added.")
                            Button(draft.opened ? "GitHub draft already opened" : "Open draft on GitHub") { openPublic(preview) }
                                .disabled(busy || !draft.canOpen(preview, hasImage: image != nil))
                                .accessibilityIdentifier("improvements.open")
                        }
                        if draft.opened {
                            Text("Opening GitHub does not confirm submission. Check GitHub before retrying to avoid duplicate issues.")
                            Button("Retry opening…") { confirmRetry = true }.disabled(busy)
                        }
                    }
                }
                if !note.isEmpty { Section { Text(note).accessibilityIdentifier("improvements.status") } }
                Section { Button("New draft…", role: .destructive) { confirmNew = true }.disabled(busy || failedLoad || !loaded) }
            }
            .navigationTitle("Improvements")
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() }.disabled(busy) } }
            .onAppear {
                guard !loaded else { return }
                do {
                    let saved = try store.read()
                    observedText = saved.text
                    observedReport = saved.report
                    observedDetails = saved.evidenceDetails
                    draft = saved
                    if draft.screenshotConsent { image = thumbnail(store.imageURL) }
                    loaded = true
                } catch { failedLoad = true; note = "Could not restore the saved draft. It has been kept unchanged; close and try again." }
            }
            .onChange(of: draft.text) { _, _ in changed() }
            .onChange(of: draft.report) { _, _ in changed() }
            .onChange(of: draft.evidenceDetails) { _, _ in changed() }
            .onChange(of: draft.evidenceReviewed) { _, _ in persist() }
            .onChange(of: draft.target) { _, _ in persist() }
            .onChange(of: draft.screenshotConsent) { _, consent in
                if !consent { store.removeImage(); image = nil }
                persist()
            }
            .fileImporter(isPresented: $choosing, allowedContentTypes: [.image]) { result in
                guard draft.screenshotConsent else { return }
                do {
                    let url = try result.get()
                    let access = url.startAccessingSecurityScopedResource()
                    defer { if access { url.stopAccessingSecurityScopedResource() } }
                    let size = try url.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? 0
                    guard size > 0, size <= 10 * 1024 * 1024 else { note = "Choose an image up to 10 MB."; return }
                    let data = try Data(contentsOf: url)
                    guard let decoded = thumbnail(url) else { note = "Could not read this image."; return }
                    try store.save(draft)
                    try data.write(to: store.imageURL, options: .atomic)
                    image = decoded
                    draft.evidenceReviewed = false; persist()
                    note = "Local preview only. Attach manually on GitHub after reviewing it."
                } catch { note = "Image selection was not completed. Your draft is kept." }
            }
            .confirmationDialog("Replace the saved draft?", isPresented: $confirmNew, titleVisibility: .visible) {
                Button("New public draft", role: .destructive) { draft = ImprovementsDraft(); preview = nil; image = nil; store.removeImage(); persist() }
            }
            .confirmationDialog("Check GitHub first. Open another copy of this draft?", isPresented: $confirmRetry, titleVisibility: .visible) {
                Button("Allow another open") { draft.opened = false; persist() }
            }
            .sheet(item: $browser) { page in ImprovementsBrowser(url: page.url) }
        }
        .interactiveDismissDisabled(busy)
    }

    private func persist() {
        guard loaded, !failedLoad else { return }
        do { try store.save(draft) }
        catch { note = "Could not save this draft. Free device storage, then try again." }
    }
    private func changed() {
        guard loaded, !failedLoad,
              observedText != draft.text || observedReport != draft.report || observedDetails != draft.evidenceDetails else { return }
        observedText = draft.text; observedReport = draft.report; observedDetails = draft.evidenceDetails
        preview = nil; draft.opened = false; draft.evidenceReviewed = false; persist()
    }
    private func prepare(fill: Bool) {
        guard !busy else { return }
        busy = true; preview = nil
        Task { @MainActor in
            defer { busy = false }
            do {
                try store.save(draft)
                let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "unknown"
                let json: String
                if fill { json = await model.improvementsPreview(draft.text, os: "iOS", version: version, details: draft.evidenceDetails) }
                else { json = await model.improvementsPreviewBody(draft.report) }
                let value = try JSONDecoder().decode(ImprovementsPreview.self, from: Data(json.utf8))
                guard value.url.isEmpty || value.destination != nil else { note = "Public preview is unavailable in this build. Your draft is kept."; return }
                if fill {
                    draft.report = value.body; changed(); try store.save(draft)
                    note = "Report filled. Edit the complete body and review its evidence, then preview it."; return
                }
                guard value.body == draft.report else { note = "The preview did not match your edited report. Try again."; return }
                preview = value
                note = value.url.isEmpty ? "This body is too long for a prefilled GitHub URL. Shorten your request and preview again. Nothing was truncated." : "Review the exact body before opening GitHub."
            } catch { note = "Could not prepare the public preview. Your draft is kept; try again." }
        }
    }
    private func openPublic(_ value: ImprovementsPreview) {
        guard !busy, draft.canOpen(value, hasImage: image != nil), let url = value.destination else { return }
        do {
            draft.opened = true
            try store.save(draft)
        } catch { draft.opened = false; note = "Could not save the opening receipt. Try again after freeing device storage."; return }
        // A browser form supports sign-in without account discovery or an
        // installed GitHub app intercepting and dropping the prefilled body.
        browser = ImprovementsBrowserPage(url: url)
        note = "Continue on GitHub. You must press Submit there."
    }
    private func openInternal(writer: Bool = false) {
        guard let base = model.web("/"), var c = URLComponents(url: base, resolvingAgainstBaseURL: false) else {
            note = "Connect to your hub before opening internal Improvements."; return
        }
        if writer {
            guard let data = try? JSONSerialization.data(withJSONObject: ["description": draft.text]),
                  let json = String(data: data, encoding: .utf8),
                  let encoded = json.addingPercentEncoding(withAllowedCharacters: CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.!~*'()")),
                  encoded.utf8.count < 24000 else { note = "Description is too long for a browser handoff. Shorten it, or paste it into the hub’s Improvements form."; return }
            c.percentEncodedFragment = "improvements-issue=" + encoded
        } else { c.fragment = "improvements-internal" }
        guard let url = c.url else { return }
        busy = true
        UIApplication.shared.open(url) { success in Task { @MainActor in
            busy = false
            note = success ? "Continue in the hub’s Improvements form." : "Could not open the hub browser. Your draft is kept."
        } }
    }
    private func thumbnail(_ url: URL) -> UIImage? {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let cg = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceThumbnailMaxPixelSize: 1200
              ] as CFDictionary) else { return nil }
        return UIImage(cgImage: cg)
    }
}

private struct ImprovementsBrowserPage: Identifiable {
    let id = UUID()
    let url: URL
}
private struct ImprovementsBrowser: UIViewControllerRepresentable {
    let url: URL
    func makeUIViewController(context: Context) -> SFSafariViewController { SFSafariViewController(url: url) }
    func updateUIViewController(_ controller: SFSafariViewController, context: Context) {}
}
