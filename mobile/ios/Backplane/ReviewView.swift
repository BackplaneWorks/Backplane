import SwiftUI

// The viewer's Review tab (client.bend's Rp, mobile/view.bend's Rv.json): a
// chip per page, the page's title and the report as Markdown (the apps'
// own MarkdownView; its tables scroll sideways), or what to say while
// there is none. Refresh sits at the bottom, under the thumb.
struct ReviewScreen: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let viewer: Viewer
    let page: ReviewPage

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ViewerChoices(model: model, viewer: viewer)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(page.tabs, id: \.page) { t in
                        Button(t.label) { model.act("rv-page", t.page) }
                            .buttonStyle(.bordered)
                            .tint(t.on ? Color.accentColor : .secondary)
                    }
                }
                .padding(.horizontal)
            }
            Text(page.title).font(.headline).lineLimit(2)
                .padding(.horizontal).padding(.vertical, 6)
            Divider()
            if let blocks = page.blocks, !blocks.isEmpty, !page.busy {
                ScrollView {
                    if let c = page.checks { ChecksBoxes(model: model, checks: c).padding([.horizontal, .top]) }
                    if let r = page.release { ReleaseBox(model: model, form: r, busy: page.busy).padding([.horizontal, .top]) }
                    MarkdownView(blocks: blocks).padding()
                }
            } else if let r = page.release {
                ScrollView {
                    ReleaseBox(model: model, form: r, busy: page.busy).padding([.horizontal, .top])
                    VStack {
                        if page.busy { ProgressView() }
                        Text(page.note).foregroundStyle(r.err.isEmpty ? Color.secondary : Color.red)
                            .multilineTextAlignment(.leading).frame(maxWidth: .infinity, alignment: .leading)
                            .accessibilityIdentifier("release-note")
                    }
                    .padding()
                }
                .scrollDismissesKeyboard(.interactively)
            } else {
                VStack {
                    if page.busy { ProgressView() }
                    Text(page.note).foregroundStyle(.secondary).multilineTextAlignment(.center).padding()
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            if page.release == nil {
                Divider()
                Button { model.act("rv-run", "") } label: {
                    Label(NativeMessages.common_refresh(lang), systemImage: "arrow.clockwise").frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .disabled(page.busy)
                .padding()
            }
        }
    }
}

// The Release page's form: the tag (empty releases the proposed one), two
// switches that are off until the person turns them on (Publish on GitHub
// pushes the tag and makes a GitHub release), and the button. The tag goes
// to Bend as the field "rltag" while it is typed.
struct ReleaseBox: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let form: ReleaseForm
    let busy: Bool
    @State private var text = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            TextField(form.next, text: $text)
                .textFieldStyle(.roundedBorder)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .submitLabel(.go)
                .onSubmit { if !busy { model.act("rv-release", "") } }
                .onChange(of: text) { _, t in model.field("rltag", t) }
                .accessibilityIdentifier("release-tag")
            Toggle(NativeMessages.review_publish(lang), isOn: Binding(get: { form.publish }, set: { _ in model.act("rv-release-pub", "") }))
                .accessibilityIdentifier("release-pub")
            Toggle(NativeMessages.review_force(lang), isOn: Binding(get: { form.force }, set: { _ in model.act("rv-release-force", "") }))
                .accessibilityIdentifier("release-force")
            // the field is typed without a new screen, so the label follows the text here
            let tag = text.trimmingCharacters(in: .whitespaces).isEmpty ? form.tag : text.trimmingCharacters(in: .whitespaces)
            Button { model.act("rv-release", "") } label: {
                Text(NativeMessages.review_release_make(lang, tag)).frame(maxWidth: .infinity, minHeight: 32)
            }
            .buttonStyle(.borderedProminent)
            .disabled(busy || tag.isEmpty)
            .accessibilityIdentifier("release-make")
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .onAppear {
            text = form.text ?? ""
            #if DEBUG
            if let t = ProcessInfo.processInfo.environment["BACKPLANE_RLTAG"] { text = t }
            #endif
        }
    }
}

// The Checks page's gate and checklist: a red "Fails: ..." or a green
// "Passes", then the boxes as toggles under their headings. A tap sends
// "rv-tick" index; the hub writes .backplane/checklist.md and answers the
// boxes, so a toggle shows what the hub says.
struct ChecksBoxes: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let checks: ReviewChecks

    private var groups: [(String, [ReviewTask])] {
        var out: [(String, [ReviewTask])] = []
        for t in checks.tasks {
            if let i = out.firstIndex(where: { $0.0 == t.group }) { out[i].1.append(t) } else { out.append((t.group, [t])) }
        }
        return out
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Label(checks.gate.isEmpty ? NativeMessages.review_passes(lang) : NativeMessages.review_fails(lang, checks.gate),
                  systemImage: checks.gate.isEmpty ? "checkmark.circle.fill" : "xmark.octagon.fill")
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(checks.gate.isEmpty ? Color.green : Color.red)
                .accessibilityIdentifier("checks-gate")
            ForEach(groups, id: \.0) { g in
                VStack(alignment: .leading, spacing: 0) {
                    if !g.0.isEmpty { Text(g.0).font(.headline).padding(.vertical, 6) }
                    ForEach(g.1, id: \.index) { t in
                        Toggle(isOn: Binding(get: { t.done }, set: { _ in model.act("rv-tick", String(t.index)) })) {
                            Text(t.text).font(.body)
                        }
                        .toggleStyle(CheckBoxStyle())
                        .padding(.vertical, 6)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

// a square box and its words, the whole row the tap's size
struct CheckBoxStyle: ToggleStyle {
    func makeBody(configuration c: Configuration) -> some View {
        Button { c.isOn.toggle() } label: {
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: c.isOn ? "checkmark.square.fill" : "square")
                    .font(.title2)
                    .foregroundStyle(c.isOn ? Color.accentColor : .secondary)
                c.label.multilineTextAlignment(.leading).foregroundStyle(.primary)
                Spacer(minLength: 0)
            }
            .frame(minHeight: 44)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

// The note for a new comment, as a bottom sheet small enough to leave the
// board (and the place) in view. The text goes to Bend as the field
// "rcnote" while it is typed; Send posts it.
struct NoteSheet: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let at: String
    @State private var text = ""
    @FocusState private var focus: Bool

    private var place: String {
        let p = at.split(separator: "\t", omittingEmptySubsequences: false)
        guard p.count >= 2, let x = Double(p[0]), let y = Double(p[1]) else { return "" }
        return NativeMessages.native_review_place(lang, String(format: "%.2f", x), String(format: "%.2f", y))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text(NativeMessages.native_review_comment(lang)).font(.headline)
                Text(place).font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button(NativeMessages.common_cancel(lang)) { model.act("rv-note-shut", "") }
            }
            TextField(NativeMessages.native_review_write(lang), text: $text, axis: .vertical)
                .lineLimit(1...4)
                .textFieldStyle(.roundedBorder)
                .focused($focus)
                .onChange(of: text) { _, t in model.field("rcnote", t) }
                .accessibilityIdentifier("note-field")
            Button {
                model.act("rv-note-send", "")
            } label: { Text(NativeMessages.composer_send(lang)).frame(maxWidth: .infinity, minHeight: 32) }
                .buttonStyle(.borderedProminent)
                .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .accessibilityIdentifier("note-send")
        }
        .padding()
        .presentationDetents([.height(230)])
        .presentationBackgroundInteraction(.enabled)
        .onAppear {
            focus = true
            #if DEBUG
            if let t = ProcessInfo.processInfo.environment["BACKPLANE_NOTE"] { text = t }
            #endif
        }
    }
}

// The open comment: its words, who and where, the replies, a reply field,
// Resolve or Reopen, Close. A sheet of two heights: the pin stays in view
// at the lower one.
struct CommentSheet: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let open: ReviewOpen
    @State private var text = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(open.by).font(.headline)
                if open.resolved { Text(NativeMessages.native_review_resolved(lang)).font(.caption).foregroundStyle(.green) }
                Spacer()
                Button { model.act("rv-pin", "") } label: { Image(systemName: "xmark.circle.fill").font(.title2) }
                    .accessibilityLabel(NativeMessages.common_close(lang))
            }
            if !open.place.isEmpty { Text(open.place).font(.caption).foregroundStyle(.secondary) }
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    Text(open.text).frame(maxWidth: .infinity, alignment: .leading)
                    ForEach(Array(open.replies.enumerated()), id: \.offset) { _, r in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(r.by).font(.caption).foregroundStyle(.secondary)
                            Text(r.text)
                        }
                        .padding(.leading, 12)
                    }
                }
            }
            HStack {
                TextField(NativeMessages.review_reply(lang), text: $text, axis: .vertical)
                    .lineLimit(1...3)
                    .textFieldStyle(.roundedBorder)
                    .onChange(of: text) { _, t in model.field("rcreply", t) }
                    .accessibilityIdentifier("reply-field")
                Button(NativeMessages.review_reply(lang)) {
                    model.act("rv-reply-send", "")
                    text = ""
                }
                .buttonStyle(.borderedProminent)
                .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .accessibilityIdentifier("reply-send")
            }
            Button(open.resolved ? NativeMessages.review_reopen(lang) : NativeMessages.review_resolve(lang)) { model.act(open.resolved ? "rv-reopen" : "rv-resolve", open.id) }
                .buttonStyle(.bordered)
                .frame(minHeight: 44)
                .accessibilityIdentifier("resolve")
        }
        .padding()
        #if DEBUG
        .onAppear { if let t = ProcessInfo.processInfo.environment["BACKPLANE_REPLY"] { text = t } }
        #endif
        .presentationDetents([.height(320), .large])
        .presentationBackgroundInteraction(.enabled(upThrough: .height(320)))
    }
}
