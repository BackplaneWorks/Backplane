import PhotosUI
import SwiftUI
import UIKit
import UniformTypeIdentifiers

// The pieces of a thread and the sheets over it. Every label, value and
// choice comes from the screen (src/mobile/screen.bend); these only draw
// them and send back the action each names.

// the hub's palette for a thread's phase (src/web/style.css, src/app/theme.bend)
enum PhaseColor {
    static func rgb(_ hex: UInt32) -> Color {
        Color(red: Double(hex >> 16 & 0xff) / 255, green: Double(hex >> 8 & 0xff) / 255, blue: Double(hex & 0xff) / 255)
    }
    static let accent = rgb(0x5fb3ff), ok = rgb(0x58c28a), warn = rgb(0xe3b341), bad = rgb(0xf06a6a)
    static let input = rgb(0xb48cff), queued = rgb(0x4fc1c9), waiting = rgb(0xe07bd0), faint = rgb(0x5c6473)
}

// a thread's dot: its phase (row.status, src/core/model.bend's Phase.name),
// in the same colors as the desktop and web; else its turn
struct StatusDot: View {
    let state: String
    let status: String?

    private func dot(_ c: Color) -> some View {
        Circle().fill(c).frame(width: 7, height: 7)
    }

    var body: some View {
        switch status ?? "" {
        case "approval": Image(systemName: "hand.raised.fill").foregroundStyle(PhaseColor.warn).font(.caption)
        case "input": Image(systemName: "questionmark.bubble.fill").foregroundStyle(PhaseColor.input).font(.caption)
        // ("queued" from an older hub: nothing waits now, it is working)
        case "working", "queued": ProgressView().controlSize(.mini).tint(PhaseColor.accent)
        // the turn is over, its subagents still at work ("waiting" from an older hub)
        case "monitoring", "waiting": Image(systemName: "arrow.triangle.branch").foregroundStyle(PhaseColor.waiting).font(.caption)
                .symbolEffect(.pulse)
        case "failed": Image(systemName: "exclamationmark.circle.fill").foregroundStyle(PhaseColor.bad)
        case "complete": dot(PhaseColor.ok)
        case "stopped", "idle", "ready": dot(PhaseColor.faint)
        default:
            switch state {
            case "run": ProgressView().controlSize(.mini).tint(PhaseColor.accent)
            case "fail": Image(systemName: "exclamationmark.circle.fill").foregroundStyle(PhaseColor.bad)
            case "stop": Image(systemName: "stop.circle").foregroundStyle(PhaseColor.warn)
            default: dot(PhaseColor.faint)
            }
        }
    }
}

// an image the lightbox shows
struct Shown: Identifiable {
    let url: URL
    var id: String { url.absoluteString }
}

// an image from the hub, as a thumbnail; a tap opens it full screen
struct Thumb: View {
    let model: AppModel
    let url: String
    let show: (Shown) -> Void

    var body: some View {
        if let u = model.web(url) {
            AsyncImage(url: u) { phase in
                switch phase {
                case .success(let img): img.resizable().scaledToFit()
                case .failure: Image(systemName: "photo").foregroundStyle(.tertiary).frame(width: 80, height: 60)
                default: ProgressView().frame(width: 80, height: 60)
                }
            }
            .frame(maxWidth: 260, maxHeight: 200, alignment: .leading)
            .clipShape(.rect(cornerRadius: 6))
            .onTapGesture { show(Shown(url: u)) }
            .accessibilityAddTraits(.isButton)
        }
    }
}

// pinch to zoom, drag to pan, double-tap to zoom in or back, tap Done to close
struct Lightbox: View {
    let shown: Shown
    let close: () -> Void
    @State private var scale: CGFloat = 1
    @State private var base: CGFloat = 1
    @State private var offset: CGSize = .zero
    @State private var moved: CGSize = .zero

    var body: some View {
        NavigationStack {
            AsyncImage(url: shown.url) { phase in
                if let img = phase.image {
                    img.resizable().scaledToFit()
                } else if phase.error != nil {
                    Image(systemName: "photo").foregroundStyle(.secondary)
                } else {
                    ProgressView()
                }
            }
            .scaleEffect(scale)
            .offset(offset)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(.black)
            .gesture(MagnifyGesture()
                .onChanged { scale = min(max(base * $0.magnification, 1), 8) }
                .onEnded { _ in base = scale; if scale == 1 { offset = .zero; moved = .zero } })
            .simultaneousGesture(DragGesture()
                .onChanged { offset = CGSize(width: moved.width + $0.translation.width, height: moved.height + $0.translation.height) }
                .onEnded { _ in moved = offset })
            .onTapGesture(count: 2) {
                withAnimation {
                    scale = scale > 1 ? 1 : 2.5
                    base = scale
                    if scale == 1 { offset = .zero; moved = .zero }
                }
            }
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done", action: close) }
                ToolbarItem(placement: .topBarLeading) { ShareLink(item: shown.url) }
            }
            .toolbarBackground(.black, for: .navigationBar)
            .toolbarColorScheme(.dark, for: .navigationBar)
        }
    }
}

// an attachment chip; an image shows as a thumbnail
struct ChipView: View {
    let model: AppModel
    let chip: Chip
    let show: (Shown) -> Void

    var body: some View {
        if chip.image {
            Thumb(model: model, url: chip.url, show: show)
        } else {
            Label(chip.label, systemImage: "doc")
                .font(.caption)
                .lineLimit(1)
                .padding(.horizontal, 8).padding(.vertical, 4)
                .background(Color(.tertiarySystemFill), in: .rect(cornerRadius: 6))
        }
    }
}

// a message's text where any part of it can be selected (UITextView: a
// SwiftUI Text only copies whole); Copy All takes it whole
private struct SelectableText: UIViewRepresentable {
    let text: String

    func makeUIView(context: Context) -> UITextView {
        let v = UITextView()
        v.isEditable = false
        v.isSelectable = true
        v.font = .preferredFont(forTextStyle: .body)
        v.adjustsFontForContentSizeCategory = true
        v.textContainerInset = UIEdgeInsets(top: 16, left: 12, bottom: 16, right: 12)
        v.backgroundColor = .clear
        return v
    }

    func updateUIView(_ v: UITextView, context: Context) {
        if v.text != text { v.text = text }
    }
}

struct SelectSheet: View {
    let model: AppModel
    let text: String
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            SelectableText(text: text)
                .navigationTitle("Select Text")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) { Button("Copy All") { model.act("copy", text) } }
                    ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
                }
        }
    }
}

struct EntryRow: View {
    let model: AppModel
    let entry: Entry
    let show: (Shown) -> Void
    @State private var selecting = false

    @ViewBuilder private var menu: some View {
        Button("Copy", systemImage: "doc.on.doc") { model.act("copy", entry.text) }
        Button("Select Text", systemImage: "selection.pin.in.out") { selecting = true }
    }

    var body: some View {
        row.sheet(isPresented: $selecting) { SelectSheet(model: model, text: entry.text) }
    }

    // when a message was sent, or a turn finished
    @ViewBuilder private var stamp: some View {
        if let s = entry.stamp, !s.isEmpty {
            Text(s).font(.caption2).monospacedDigit().foregroundStyle(.tertiary)
        }
    }

    @ViewBuilder private var row: some View {
        switch entry.kind {
        case "user":
            VStack(alignment: .trailing, spacing: 6) {
                if !entry.text.isEmpty {
                    Text(entry.text)
                        .padding(.horizontal, 14).padding(.vertical, 10)
                        .background(Color.accentColor.opacity(0.15), in: .rect(cornerRadius: 18))
                        .contextMenu { menu }
                }
                ForEach(entry.attachments ?? [], id: \.self) { ChipView(model: model, chip: $0, show: show) }
                stamp
            }
            .padding(.leading, 48)
            .frame(maxWidth: .infinity, alignment: .trailing)
        case "assistant":
            VStack(alignment: .leading, spacing: 8) {
                MarkdownView(blocks: entry.blocks ?? [])
                ForEach(entry.images ?? [], id: \.self) { Thumb(model: model, url: $0.url, show: show) }
                stamp
            }
            .contextMenu { menu }
        case "fold":
            Button { model.act("fold", entry.value ?? "") } label: {
                HStack(spacing: 6) {
                    Image(systemName: entry.open == true ? "chevron.down" : "chevron.right").font(.caption2)
                    Text(entry.text).lineLimit(1)
                }
                .font(.caption.monospaced())
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .accessibilityLabel((entry.open == true ? "Hide " : "Show ") + entry.text)
        case "link":
            Button { model.act("select", entry.value ?? "") } label: {
                Text(entry.text).lineLimit(1).font(.callout)
            }
            .buttonStyle(.borderless)
        // a reminder the hub delivered: a bell, Reminder, the text and when
        case "reminder":
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Image(systemName: "bell")
                Text("Reminder").foregroundStyle(.tertiary)
                Text(entry.text)
                Spacer()
                Text(entry.when ?? "").foregroundStyle(.tertiary)
            }
            .font(.caption)
            .foregroundStyle(Color.accentColor)
            .frame(maxWidth: .infinity, alignment: .leading)
        // a design step: shows it in the viewer
        case "step":
            Button { model.act("hist-show", entry.value ?? "") } label: {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Image(systemName: "clock.arrow.circlepath")
                    Text(entry.text).lineLimit(2)
                }
                .font(.caption.monospaced())
                .foregroundStyle(Color.accentColor)
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Show in the viewer: " + entry.text)
        default:
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(entry.label ?? "").foregroundStyle(.tertiary)
                Text(entry.text).lineLimit(2)
            }
            .font(.caption.monospaced())
            .foregroundStyle(entry.tone == "error" ? AnyShapeStyle(.red) : AnyShapeStyle(.secondary))
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

private func subColor(_ state: String) -> Color {
    switch state {
    case "running": PhaseColor.accent
    case "done", "reporting", "held": PhaseColor.ok
    case "watching", "launching", "waiting on", "waiting", "needs you": PhaseColor.accent
    // "" is the "N done" toggle row (subs.bend's Subs.done.toggle): neutral
    case "", "starting", "queued": PhaseColor.faint
    case "cancelled", "cancelling": PhaseColor.warn
    default: PhaseColor.bad
    }
}

// the subagent panel: what this thread delegated (a tap opens its thread)
// and the agent's own (a tap opens or shuts its steps); outlined in the
// accent while any are at work
struct SubsView: View {
    let model: AppModel
    let subs: Subs

    var body: some View {
        let busy = !subs.busy.isEmpty
        VStack(alignment: .leading, spacing: 6) {
            // the head opens and shuts the panel; done, it is folded to this line
            Button { if let a = subs.act, !a.isEmpty { model.act(a, subs.value ?? "") } } label: {
                HStack {
                    Text("Subagents").font(.caption.bold()).foregroundStyle(busy ? .primary : .secondary)
                    Spacer(minLength: 0)
                    Text(subs.word ?? subs.busy).font(.caption).foregroundStyle(busy ? PhaseColor.accent : .secondary)
                    Image(systemName: (subs.open ?? true) ? "chevron.down" : "chevron.right").font(.caption).foregroundStyle(.secondary)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            // many subagents scroll within a third of the screen
            ScrollView {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(subs.rows) { r in
                        Button { model.act(r.act, r.value) } label: {
                            VStack(alignment: .leading, spacing: 2) {
                                HStack(spacing: 8) {
                                    Rectangle().fill(subColor(r.state)).frame(width: r.live ? 8 : 6, height: r.live ? 8 : 6)
                                    Text(r.agent ? (r.open ? "▾" : "▸") : r.who).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                                    Text(r.title).lineLimit(1)
                                    Spacer(minLength: 0)
                                    if let c = r.cancel, !c.isEmpty {
                                        Button(c == "wake-cancel" ? "Cancel" : "Stop") { model.act(c, r.id) }.font(.caption2).foregroundStyle(PhaseColor.warn).buttonStyle(.plain)
                                    }
                                    Text(r.stateLabel ?? r.state).font(.caption2).foregroundStyle(subColor(r.state))
                                }
                                if !r.doing.isEmpty {
                                    Text(r.doing).font(.caption2).foregroundStyle(PhaseColor.accent).lineLimit(1).padding(.leading, 16)
                                }
                                ForEach(Array(r.steps.enumerated()), id: \.offset) { _, st in
                                    HStack(spacing: 6) {
                                        if !st.kind.isEmpty { Text(st.kind).foregroundStyle(.tertiary) }
                                        Text(st.text).foregroundStyle(.secondary).lineLimit(1)
                                    }
                                    .font(.caption2.monospaced())
                                    .padding(.leading, 16)
                                }
                            }
                            .contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .frame(maxHeight: 260)
            .fixedSize(horizontal: false, vertical: true)
        }
        .padding(10)
        .background(Color(.secondarySystemBackground))
        .overlay(Rectangle().stroke(busy ? PhaseColor.accent : .clear, lineWidth: 1))
    }
}

// the subagents over the thread, from the top bar's button: the panel
// opened (its rows folded away once all are done) and kept up to date
struct SubsSheet: View {
    let model: AppModel
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            ScrollView {
                if let u = model.screen?.thread?.subs { SubsView(model: model, subs: u).padding() }
            }
            .navigationTitle("Subagents")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
        .presentationDetents([.medium, .large])
        .onAppear {
            if let u = model.screen?.thread?.subs, u.open == false, let a = u.act, !a.isEmpty { model.act(a, u.value ?? "") }
        }
    }
}

// an approval, question or plan waiting on the user
struct AskCard: View {
    let model: AppModel
    let ask: Ask

    var body: some View {
        if ask.kind == "browser" {
            BrowserAskCard(model: model, ask: ask)
        } else {
            standard
        }
    }

    private var standard: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label(ask.head, systemImage: ask.kind == "plan" ? "list.bullet.clipboard" : ask.kind == "input" ? "questionmark.bubble" : "hand.raised")
                .font(.subheadline.bold())
            if !ask.blocks.isEmpty {
                ScrollView { MarkdownView(blocks: ask.blocks).font(.callout) }.frame(maxHeight: 220)
            } else if !ask.detail.isEmpty {
                ScrollView { Text(ask.detail).font(.caption.monospaced()).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading) }.frame(maxHeight: 300)
            }
            FlowButtons(buttons: ask.buttons) { model.act("answer", $0.value) }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.orange.opacity(0.12), in: .rect(cornerRadius: 12))
    }
}

// buttons that wrap onto more lines when they do not fit
struct FlowButtons: View {
    let buttons: [AskButton]
    let tap: (AskButton) -> Void

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) { items }
            VStack(alignment: .leading, spacing: 6) { items }
        }
    }

    @ViewBuilder private var items: some View {
        ForEach(buttons, id: \.self) { b in
            if b.primary {
                Button(b.label) { tap(b) }.buttonStyle(.borderedProminent)
            } else {
                Button(b.label) { tap(b) }.buttonStyle(.bordered)
            }
        }
    }
}

// what the next message attaches (× takes one off), what is uploading,
// and what the word being typed completes to: a `$` skill, an `@` bot,
// a `>` thread, a `%` project or a `#` room, each with its summary
struct ComposerExtras: View {
    let model: AppModel
    let thread: ThreadView
    // an iPhone on its side: little room above the keyboard
    @Environment(\.verticalSizeClass) private var height

    var body: some View {
        let atts = thread.attaching ?? []
        let up = thread.uploading ?? ""
        let skills = thread.skills ?? []
        if let b = thread.btw {
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Text("btw · " + b.q).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    Spacer()
                    Button { model.act("btw-close") } label: { Image(systemName: "xmark") }.buttonStyle(.borderless)
                }
                ScrollView { Text(b.a).font(.callout).frame(maxWidth: .infinity, alignment: .leading) }.frame(maxHeight: 200)
            }
            .padding(10)
            .background(Color(.secondarySystemBackground))
            .padding(.horizontal).padding(.top, 8)
        }
        if !skills.isEmpty {
            // Bend marks the highlighted row; the first one is it only when none is marked
            let hot = skills.firstIndex(where: { $0.on == true }) ?? 0
            VStack(spacing: 0) {
                Divider()
                // up to four rows as they are, more scroll within about five
                // (a ScrollView takes all the height offered it); on its side,
                // a row and a half
                let short = height == .compact
                let rows =
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(Array(skills.enumerated()), id: \.element) { i, k in
                            Button { model.act("skill", k.name) } label: {
                                VStack(alignment: .leading, spacing: 1) {
                                    Text(k.name).font(.subheadline.monospaced().weight(.semibold)).lineLimit(1)
                                    if !k.desc.isEmpty {
                                        Text(k.desc).font(.caption).foregroundStyle(.secondary).lineLimit(2).multilineTextAlignment(.leading)
                                    }
                                }
                                .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
                                .padding(.leading, 12).padding(.trailing, 56).padding(.vertical, 4)
                                .background(i == hot ? Color.accentColor.opacity(0.15) : Color.clear)
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                            .id(i)
                            .accessibilityLabel(k.desc.isEmpty ? k.name : k.name + ", " + k.desc)
                            .accessibilityAddTraits(i == hot ? [.isSelected] : [])
                        }
                    }
                if skills.count <= (short ? 1 : 4) {
                    rows
                } else {
                    ScrollViewReader { proxy in
                        ScrollView { rows }
                            .frame(height: short ? 72 : 5 * 48)
                            .onChange(of: hot) { _, i in withAnimation { proxy.scrollTo(i) } }
                    }
                }
            }
            .overlay(alignment: .topTrailing) {
                Button { model.act("skill-close") } label: {
                    Image(systemName: "xmark").font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
                        .frame(width: 44, height: 44).contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Close suggestions")
            }
            .background(Color(.secondarySystemBackground))
        }
        if !atts.isEmpty || !up.isEmpty {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(atts, id: \.self) { c in
                        Button { model.act("detach", c.path) } label: {
                            Label(c.label, systemImage: c.image ? "photo" : "doc").lineLimit(1)
                            Image(systemName: "xmark").font(.caption2)
                        }
                        .buttonStyle(.bordered)
                        .accessibilityLabel("Remove " + c.label)
                    }
                    if !up.isEmpty {
                        HStack(spacing: 4) {
                            ProgressView().controlSize(.mini)
                            Text(up).lineLimit(1)
                        }
                        .foregroundStyle(.secondary)
                    }
                }
                .font(.caption)
                .padding(.horizontal)
            }
            .padding(.top, 8)
        }
    }
}

// the paperclip: photos or files, each sent up in pieces
struct AttachButton: View {
    let model: AppModel
    @State private var photos: [PhotosPickerItem] = []
    @State private var picking = false
    @State private var importing = false
    @State private var hasImages = UIPasteboard.general.hasImages

    var body: some View {
        Menu {
            Button("Photos", systemImage: "photo.on.rectangle") { picking = true }
            Button("Files", systemImage: "folder") { importing = true }
            if hasImages {
                Button("Paste image", systemImage: "doc.on.clipboard") {
                    // Read only after the person chooses Paste; use the same
                    // upload and size check as the photo picker.
                    for (i, image) in (UIPasteboard.general.images ?? []).enumerated() {
                        if let data = image.pngData() {
                            model.attach(data, name: "pasted-\(i + 1).png")
                        }
                    }
                }
            }
        } label: {
            Image(systemName: "paperclip").font(.system(size: 20)).padding(.bottom, 6)
        }
        .accessibilityLabel("Attach")
        .onAppear { hasImages = UIPasteboard.general.hasImages }
        .onReceive(NotificationCenter.default.publisher(for: UIPasteboard.changedNotification)) { _ in
            hasImages = UIPasteboard.general.hasImages
        }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.didBecomeActiveNotification)) { _ in
            hasImages = UIPasteboard.general.hasImages
        }
        .photosPicker(isPresented: $picking, selection: $photos, maxSelectionCount: 10, matching: .images)
        .onChange(of: photos) { _, items in
            guard !items.isEmpty else { return }
            photos = []
            Task {
                for (i, it) in items.enumerated() {
                    guard let data = try? await it.loadTransferable(type: Data.self) else { continue }
                    let ext = it.supportedContentTypes.first?.preferredFilenameExtension ?? "jpg"
                    // HEIC becomes JPEG, which every agent reads
                    if ext == "heic", let img = UIImage(data: data), let jpg = img.jpegData(compressionQuality: 0.85) {
                        model.attach(jpg, name: "photo-\(i + 1).jpg")
                    } else {
                        model.attach(data, name: "photo-\(i + 1).\(ext)")
                    }
                }
            }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.item], allowsMultipleSelection: true) { r in
            guard case .success(let urls) = r else { return }
            for u in urls {
                let ok = u.startAccessingSecurityScopedResource()
                defer { if ok { u.stopAccessingSecurityScopedResource() } }
                if let data = try? Data(contentsOf: u) { model.attach(data, name: u.lastPathComponent) }
            }
        }
    }
}

// what the thread changed, with the git actions
struct DiffSheet: View {
    let model: AppModel
    let diff: Diff
    @State private var reverting = false

    var body: some View {
        NavigationStack {
            List {
                Section { Text(diff.summary).font(.footnote).foregroundStyle(.secondary) }
                ForEach(diff.files) { f in
                    Section {
                        ForEach(Array(f.lines.enumerated()), id: \.offset) { _, l in line(l) }
                            .listRowInsets(EdgeInsets(top: 0, leading: 8, bottom: 0, trailing: 8))
                    } header: {
                        HStack {
                            Text(f.name).textCase(nil).lineLimit(1).truncationMode(.head)
                            Spacer()
                            Text(f.status).textCase(nil)
                        }
                    }
                }
            }
            .listStyle(.plain)
            .environment(\.defaultMinListRowHeight, 14)
            .navigationTitle("Diff")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Close") { model.act("panel") } }
                ToolbarItem(placement: .primaryAction) {
                    Menu {
                        Button("Commit & push", systemImage: "arrow.up.circle") { model.act("git", "commit_push") }
                        Button("Open PR", systemImage: "arrow.triangle.pull") { model.act("git", "commit_push_pr") }
                        Button("Revert thread", systemImage: "arrow.uturn.backward", role: .destructive) { reverting = true }
                    } label: {
                        Text("Git")
                    }
                }
            }
            .confirmationDialog("Put the files back as they were when this thread began?", isPresented: $reverting, titleVisibility: .visible) {
                Button("Revert thread", role: .destructive) { model.act("revert", "0") }
            }
        }
    }

    private func line(_ l: DiffLine) -> some View {
        let sign = l.k == 1 ? "+" : l.k == 2 ? "-" : " "
        let bg: Color = l.k == 1 ? .green.opacity(0.14) : l.k == 2 ? .red.opacity(0.14) : .clear
        return HStack(alignment: .firstTextBaseline, spacing: 4) {
            if l.k == 4 {
                Text(l.t).foregroundStyle(.blue)
            } else {
                Text(l.k == 2 ? l.o : l.n).foregroundStyle(.tertiary).frame(width: 30, alignment: .trailing)
                Text(sign + l.t).foregroundStyle(l.k == 3 ? .secondary : .primary)
            }
            Spacer(minLength: 0)
        }
        .font(.system(size: 11, design: .monospaced))
        .padding(.vertical, 1)
        .listRowBackground(bg)
        .listRowSeparator(.hidden)
    }
}

private func termColor(_ c: UInt32) -> Color {
    Color(red: Double((c >> 16) & 255) / 255, green: Double((c >> 8) & 255) / 255, blue: Double(c & 255) / 255)
}

// a terminal's screen: its rows of styled runs and the cursor, at a size
struct TermScreen: View {
    let term: Term
    var size: CGFloat = 11

    // a cell's width and a row's height at a size
    static func cell(_ size: CGFloat) -> (w: CGFloat, h: CGFloat) {
        let f = UIFont.monospacedSystemFont(ofSize: size, weight: .regular)
        return (("M" as NSString).size(withAttributes: [.font: f]).width, ceil(f.lineHeight))
    }

    // the largest size (by halves, 7 to 11) whose cols fit in width
    static func fit(cols: Int, width: CGFloat) -> CGFloat {
        let unit = cell(11).w / 11
        guard unit > 0, cols > 0 else { return 11 }
        return min(max((width / CGFloat(cols) / unit * 2).rounded(.down) / 2, 7), 11)
    }

    private func row(_ runs: [TermRun]) -> AttributedString {
        var out = AttributedString()
        for r in runs {
            var a = AttributedString(r.t)
            a.foregroundColor = termColor(r.fg)
            if r.bg != term.bg { a.backgroundColor = termColor(r.bg) }
            if r.b { a.font = .system(size: size, weight: .bold, design: .monospaced) }
            if r.u { a.underlineStyle = .single }
            out += a
        }
        return out
    }

    var body: some View {
        let c = Self.cell(size)
        ZStack(alignment: .topLeading) {
            VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(term.lines.enumerated()), id: \.offset) { _, l in
                    Text(row(l)).frame(height: c.h, alignment: .leading).fixedSize()
                }
            }
            if term.cursor.on {
                Rectangle().fill(termColor(term.fg).opacity(0.6))
                    .frame(width: c.w, height: c.h)
                    .offset(x: CGFloat(term.cursor.x) * c.w, y: CGFloat(term.cursor.y) * c.h)
            }
        }
        .font(.system(size: size, design: .monospaced))
        .foregroundStyle(termColor(term.fg))
    }
}

// the hidden field a terminal's keys are typed in: it keeps one space, so
// a backspace always has something to take
struct TermField: View {
    var typing: FocusState<Bool>.Binding
    let key: (String, Int) -> Void
    let paste: (String) -> Void
    @State private var buf = " "

    var body: some View {
        TextField("", text: $buf)
            .focused(typing)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .keyboardType(.asciiCapable)
            .frame(width: 1, height: 1)
            .opacity(0.01)
            .onChange(of: buf) { old, new in
                if new == " " { return }
                if new.count < old.count || new.isEmpty {
                    key("Backspace", 0)
                } else if new.hasPrefix(" ") {
                    let typed = String(new.dropFirst())
                    if !typed.isEmpty { paste(typed) }
                }
                if buf != " " { buf = " " }
            }
            .onSubmit { key("Enter", 0); typing.wrappedValue = true }
    }
}

// the keys a phone keyboard lacks, and the keyboard's own toggle
struct TermKeys: View {
    var typing: FocusState<Bool>.Binding
    let key: (String, Int) -> Void

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                Button("esc") { key("Escape", 0) }
                Button("tab") { key("Tab", 0) }
                Button("^C") { key("c", 4) }
                Button("^D") { key("d", 4) }
                Button("^Z") { key("z", 4) }
                Button("^L") { key("l", 4) }
                Button { key("ArrowLeft", 0) } label: { Image(systemName: "arrow.left") }
                Button { key("ArrowUp", 0) } label: { Image(systemName: "arrow.up") }
                Button { key("ArrowDown", 0) } label: { Image(systemName: "arrow.down") }
                Button { key("ArrowRight", 0) } label: { Image(systemName: "arrow.right") }
                Button { typing.wrappedValue.toggle() } label: {
                    Image(systemName: typing.wrappedValue ? "keyboard.chevron.compact.down" : "keyboard")
                }
            }
            .buttonStyle(.bordered)
            .font(.caption.monospaced())
            .padding(.horizontal).padding(.vertical, 6)
        }
        .background(.bar)
    }
}

// the thread's shell: the screen the hub's emulator keeps, keys typed in a
// hidden field, and a row of keys a phone keyboard lacks
struct TermSheet: View {
    let model: AppModel
    let term: Term
    @FocusState private var typing: Bool

    static let font = UIFont.monospacedSystemFont(ofSize: 11, weight: .regular)
    static let cw = ("M" as NSString).size(withAttributes: [.font: font]).width
    static let lh = ceil(font.lineHeight)

    // the size a terminal has room for on this phone, as "<cols>x<rows>"
    static func size() -> String {
        let b = UIScreen.main.bounds
        return "\(max(Int((b.width - 16) / cw), 20))x\(max(Int((b.height * 0.5) / lh), 8))"
    }

    private func key(_ k: String, _ mods: Int = 0) { model.act("term-key", "\(k)\t\(mods)") }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                ScrollView([.horizontal, .vertical]) {
                    TermScreen(term: term).padding(8)
                }
                .defaultScrollAnchor(.bottomLeading)
                .background(termColor(term.bg))
                .onTapGesture { typing = true }
                TermField(typing: $typing, key: { key($0, $1) }, paste: { model.act("term-paste", $0) })
                TermKeys(typing: $typing, key: { key($0, $1) })
            }
            .navigationTitle(term.title.isEmpty ? "Terminal" : term.title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(.visible, for: .navigationBar)
            .toolbarBackground(termColor(term.bg), for: .navigationBar)
            .toolbarColorScheme(.dark, for: .navigationBar)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Close") { model.act("term-toggle") } }
            }
            .onAppear { typing = true }
        }
    }
}

// a terminal in the chat, live: a header (its title, the pin) over its
// screen, the font shrunk so its columns fit (7 pt at the least, then it
// scrolls sideways); a tap types into it ("emb-key", "emb-paste" with its
// key first). At the top of the thread (pinned) the pin is in the accent
// and the screen is held to a height, kept at the bottom.
struct EmbView: View {
    let model: AppModel
    let emb: Emb
    var top = false
    @FocusState private var typing: Bool
    @State private var width = UIScreen.main.bounds.width - 32

    private func key(_ k: String, _ mods: Int) { model.act("emb-key", "\(emb.key)\t\(k)\t\(mods)") }

    var body: some View {
        let fg = emb.term.map { termColor($0.fg) } ?? .primary
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 6) {
                Image(systemName: "terminal").foregroundStyle(fg.opacity(0.7))
                Text(emb.title).lineLimit(1).foregroundStyle(fg)
                Spacer(minLength: 0)
                Button { model.act("emb-pin", emb.key) } label: {
                    Image(systemName: emb.pinned ? "pin.fill" : "pin")
                        .foregroundStyle(emb.pinned ? AnyShapeStyle(PhaseColor.accent) : AnyShapeStyle(fg.opacity(0.5)))
                        .frame(width: 32, height: 28)
                        .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(emb.pinned ? "Unpin" : "Pin to the top")
            }
            .font(.caption)
            .padding(.leading, 8)
            if let t = emb.term {
                let size = TermScreen.fit(cols: emb.cols, width: width - 8)
                let h = TermScreen.cell(size).h * CGFloat(max(emb.rows, t.lines.count)) + 8
                Group {
                    if top {
                        ScrollView([.horizontal, .vertical]) { TermScreen(term: t, size: size).padding(4) }
                            .defaultScrollAnchor(.bottomLeading)
                            .frame(height: min(h, 240))
                    } else {
                        ScrollView(.horizontal) { TermScreen(term: t, size: size).padding(4) }
                            .frame(height: h)
                    }
                }
                .onTapGesture { typing = true }
                .background(TermField(typing: $typing, key: { key($0, $1) }, paste: { model.act("emb-paste", "\(emb.key)\t\($0)") }))
            }
            if typing { TermKeys(typing: $typing, key: { key($0, $1) }) }
        }
        .background(emb.term.map { termColor($0.bg) } ?? Color(.secondarySystemBackground))
        .background(GeometryReader { g in
            Color.clear
                .onAppear { width = g.size.width }
                .onChange(of: g.size.width) { _, w in width = w }
        })
    }
}

// a terminal's row in the timeline: live, its screen; pinned, a faint row
// that unpins it; else a gray row that starts it again
struct EmbRow: View {
    let model: AppModel
    let entry: Entry
    let emb: Emb?

    private func line(_ icon: String, _ text: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Image(systemName: icon)
            Text(text).lineLimit(1)
        }
        .font(.caption.monospaced())
        .foregroundStyle(.secondary)
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(.rect)
    }

    var body: some View {
        if let e = emb, e.pinned {
            Button { model.act("emb-pin", e.key) } label: { line("pin", e.title + " · pinned at the top") }
                .buttonStyle(.plain)
        } else if let e = emb, e.live {
            EmbView(model: model, emb: e)
        } else if let e = emb {
            Button { model.act("emb-restart", e.restart) } label: { line("play.fill", e.title + " · inactive, tap to restart") }
                .buttonStyle(.plain)
        } else {
            line("terminal", entry.text)
        }
    }
}

// thread search or the file picker: a field over the rows; a row sends its
// action with its value, then the sheet closes
struct FindSheet: View {
    let model: AppModel
    let find: Find
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        NavigationStack {
            List {
                ForEach(find.rows, id: \.self) { r in
                    Button {
                        model.act(r.action, r.value)
                        model.act("find-close")
                    } label: {
                        Label(r.label, systemImage: r.kind == "file" ? "doc" : r.kind == "thread" ? "bubble.left" : "bolt")
                            .lineLimit(2)
                    }
                    .tint(.primary)
                }
            }
            .overlay {
                if find.rows.isEmpty {
                    ContentUnavailableView(find.query.isEmpty ? (find.mode == "files" ? "Type to find a file" : "Type to search threads") : "Nothing found",
                                           systemImage: "magnifyingglass")
                }
            }
            .safeAreaInset(edge: .top) {
                TextField(find.mode == "files" ? "File name" : "Titles and messages", text: $text)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .focused($focused)
                    .padding(10)
                    .background(Color(.secondarySystemBackground), in: .rect(cornerRadius: 10))
                    .padding(.horizontal).padding(.vertical, 8)
                    .background(.bar)
                    .onChange(of: text) { _, t in model.act("find-q", t) }
            }
            .navigationTitle(find.mode == "files" ? "Find file" : "Search threads")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { model.act("find-close") } }
            }
            .onAppear { text = find.query; focused = true }
        }
    }
}

// the hub's settings, as the desktop has them
// a Settings field: typed text goes to the client as "bfield"
private struct SetFieldView: View {
    let model: AppModel
    let field: SetField
    @State private var text = ""

    var body: some View {
        Group {
            if field.secret {
                SecureField(field.hint, text: $text)
            } else {
                TextField(field.hint, text: $text)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
            }
        }
        .onAppear { text = field.value }
        .onChange(of: text) { _, t in model.field(field.name, t) }
    }
}

// The hub's settings rows (src/mobile/screen.bend's Mob.settings.of), as
// the Hub settings page draws them. Rows about this device (`local`) are
// App settings' business and are left out.
struct HubSettingsRows: View {
    let model: AppModel
    let rows: [SetRow]
    // the rows App settings draws itself are left out; with `notify`, only the notification policy's
    var notify = false

    // a row is a section of its own on the hub's page, a plain row in App settings' list
    @ViewBuilder private func wrapped<C: View>(@ViewBuilder _ c: () -> C) -> some View {
        if notify { c() } else { Section { c() } }
    }

    var body: some View {
        ForEach(rows.filter { notify ? ($0.notify ?? false) : !(($0.local ?? false) || ($0.notify ?? false)) }, id: \.self) { r in
            wrapped {
                VStack(alignment: .leading, spacing: 8) {
                    Text(r.label)
                    if !r.note.isEmpty { Text(r.note).font(.footnote).foregroundStyle(.secondary) }
                    ForEach(r.fields ?? [], id: \.self) { f in SetFieldView(model: model, field: f) }
                    if let q = r.qr, !q.isEmpty {
                        VStack(spacing: 6) {
                            QRCodeView(rows: q)
                            if let n = r.qrnote { Text(n).font(.footnote).foregroundStyle(.secondary) }
                        }
                        .frame(maxWidth: .infinity)
                        .onTapGesture { model.act("flag", "pairqr") }
                    }
                    if !r.buttons.isEmpty {
                        ScrollView(.horizontal, showsIndicators: false) {
                            HStack(spacing: 6) {
                                ForEach(r.buttons, id: \.self) { b in
                                    if b.on {
                                        Button(b.label) { model.act(b.action, b.value) }.buttonStyle(.borderedProminent)
                                    } else {
                                        Button(b.label) { model.act(b.action, b.value) }.buttonStyle(.bordered)
                                    }
                                }
                            }
                        }
                    }
                }
                .padding(.vertical, 4)
            }
        }
    }
}
