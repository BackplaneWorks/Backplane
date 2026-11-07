import SwiftUI

// The data/1 additions on the phone (hub features actions, search, models, mentions, bots; native/host/src/data/ext.rs), the same as the Android app's DataExtras.kt:
// thread and project actions, search, the model picker, the mention bar over the composer, bots and rooms. (Written without a Mac to compile on; see docs/mobile-desktop-parity.md.)

/// The actions of a thread as the desktop's row menu offers them. `rename` and `delete` ask through the bindings the caller owns.
struct DataThreadActions: View {
    let client: DataClient
    let thread: DThread
    @Binding var renaming: DThread?
    @Binding var deleting: DThread?
    var body: some View {
        Group {
            Button(thread.pinned ? "Unpin" : "Pin", systemImage: thread.pinned ? "pin.slash" : "pin") { Task { await client.threadAct(thread.id, thread.pinned ? "unpin" : "pin") } }
            Button(thread.settled ? "Unsettle" : "Settle", systemImage: "checkmark.circle") { Task { await client.threadAct(thread.id, thread.settled ? "unsettle" : "settle") } }
            Button(thread.archived ? "Unarchive" : "Archive", systemImage: "archivebox") { Task { await client.threadAct(thread.id, thread.archived ? "unarchive" : "archive") } }
            if !thread.parent.isEmpty { Button("Move to top level", systemImage: "arrow.up.left") { Task { await client.threadAct(thread.id, "detach") } } }
            Button("Rename", systemImage: "pencil") { renaming = thread }
            Button("Delete…", systemImage: "trash", role: .destructive) { deleting = thread }
        }
    }
}

/// The alerts for rename and delete, shared by the list and the thread screen.
struct DataThreadDialogs: ViewModifier {
    let client: DataClient
    @Binding var renaming: DThread?
    @Binding var deleting: DThread?
    var deleted: () -> Void = {}
    @State private var title = ""
    func body(content: Content) -> some View {
        content
            .alert("Rename thread", isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } })) {
                TextField("Title", text: $title)
                Button("Rename") { if let t = renaming, !title.trimmingCharacters(in: .whitespaces).isEmpty { Task { await client.threadAct(t.id, "rename", title: title.trimmingCharacters(in: .whitespaces)) } } }
                Button("Cancel", role: .cancel) {}
            }
            .onChange(of: renaming?.id) { _, _ in title = renaming?.title ?? "" }
            .confirmationDialog("Delete “\(deleting?.title ?? "")”? This cannot be undone.", isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }), titleVisibility: .visible) {
                Button("Delete thread", role: .destructive) { if let t = deleting { Task { await client.threadAct(t.id, "delete"); deleted() } } }
                Button("Keep it", role: .cancel) {}
            }
    }
}

/// Search over threads and projects (the hub's `search`).
struct DataSearchResults: View {
    let client: DataClient
    let query: String
    let open: (String) -> Void
    @State private var result: DSearch?
    var body: some View {
        List {
            if let r = result {
                if r.threads.isEmpty && r.projects.isEmpty { Text("Nothing matches “\(query)”").dmono(12).foregroundStyle(BackplaneDesign.dim).padding(12).dataRow() }
                ForEach(r.projects) { p in Label { Text(p.title).dmono(13) } icon: { Image(systemName: "folder").foregroundStyle(BackplaneDesign.ok) }.padding(.horizontal, 12).frame(minHeight: 40).dataRow().accessibilityIdentifier("data.search.project." + p.id) }
                ForEach(r.threads.filter { !$0.history }) { t in Button { open(t.id) } label: { DataThreadRow(thread: t) }.buttonStyle(.plain).dataRow().accessibilityIdentifier("data.search.thread." + t.id) }
                // the old Scratch chats the search found, under their group's name (feature history)
                let old = r.threads.filter(\.history)
                if !old.isEmpty { DHeading(text: "Conversation history").padding(.leading, 12).padding(.top, 12).padding(.bottom, 4).dataRow().accessibilityIdentifier("data.search.history") }
                ForEach(old) { t in Button { open(t.id) } label: { DataThreadRow(thread: t) }.buttonStyle(.plain).dataRow().accessibilityIdentifier("data.search.thread." + t.id) }
            } else { ProgressView().frame(maxWidth: .infinity).dataRow() }
        }
        .listStyle(.plain).scrollContentBackground(.hidden).background(BackplaneDesign.panel)
        .task(id: query) {
            try? await Task.sleep(for: .milliseconds(250))
            guard !Task.isCancelled else { return }
            result = await client.search(query)
        }
    }
}

/// The picker the composer's chip opens: the hub's sections, the one in force marked. On a thread a press is the desktop's own request; before a thread exists it is
/// remembered and sent with `thread.create`. It stays open, so a model and an effort are one visit.
struct DataPickerSheet: View {
    let client: DataClient
    let thread: String?
    @Binding var choice: DNewThreadChoice
    @Environment(\.dismiss) private var dismiss
    @State private var picker: DPicker?
    @State private var error = ""
    private func load() async { do { picker = try await client.picker(thread: thread, choice); error = "" } catch { self.error = error.localizedDescription } }
    private func press(_ s: DPickerSection, _ c: DChoice) {
        Task {
            if let thread { do { try await client.pick(thread, action: s.action, value: c.value) } catch { self.error = error.localizedDescription; return } }
            else { choice.set(s.action, c.value) }
            await load()
        }
    }
    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text(picker?.chip ?? " ").dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(1).accessibilityIdentifier("data.picker.chip")
                Spacer()
                Button("Done") { dismiss() }.dmono(13, bold: true).foregroundStyle(BackplaneDesign.accent).accessibilityIdentifier("data.picker.done")
            }.padding(.horizontal, 14).frame(height: 48)
            DLine()
            if !error.isEmpty { Text(error).dmono(12).foregroundStyle(BackplaneDesign.bad).frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 14).padding(.vertical, 8).accessibilityIdentifier("data.picker.error") }
            ScrollView {
                // (a plain stack: Model's default and Effort's auto both have the value "", and a lazy stack draws the second blank)
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(picker?.sections ?? []) { s in
                        DHeading(text: s.title).padding(.horizontal, 14).padding(.top, 14).padding(.bottom, 4)
                        ForEach(s.choices) { c in
                            Button { press(s, c) } label: {
                                HStack(spacing: 8) {
                                    Text(c.on ? "✓" : "").dmono(13).foregroundStyle(BackplaneDesign.accent).frame(width: 16)
                                    VStack(alignment: .leading, spacing: 1) {
                                        Text(c.label).dmono(13).foregroundStyle(c.ok ? BackplaneDesign.text : BackplaneDesign.faint)
                                        if !c.ok, !c.why.isEmpty { Text(c.why).dmono(11).foregroundStyle(BackplaneDesign.faint) }
                                    }
                                    Spacer(minLength: 0)
                                }.padding(.horizontal, 14).frame(minHeight: 44).background(c.on ? BackplaneDesign.raised : .clear).contentShape(Rectangle())
                            }
                            .buttonStyle(.plain).disabled(!c.ok).accessibilityAddTraits(c.on ? .isSelected : []).accessibilityIdentifier("data.pick." + s.action + "." + c.value)
                        }
                    }
                }.padding(.bottom, 16)
            }
        }
        .background(BackplaneDesign.panel)
        .presentationDetents([.medium, .large]).presentationDragIndicator(.visible).dataSheet()
        .task { await load() }
    }
}

/// The word being typed begins with @ > % # & and the hub offers matches: one tap puts it in the draft.
struct DataMentionBar: View {
    let client: DataClient
    let text: String
    let pick: (String) -> Void
    @State private var items: [DMention] = []
    private var tail: (sigil: String, query: String, range: Range<String.Index>)? {
        let space = text.lastIndex(where: { $0.isWhitespace }).map { text.index(after: $0) } ?? text.startIndex
        let word = text[space...]
        guard let f = word.first, "@>%#&".contains(f) else { return nil }
        return (String(f), String(word.dropFirst()), space..<text.endIndex)
    }
    var body: some View {
        Group {
            if client.has("mentions"), let t = tail, !items.isEmpty {
                VStack(spacing: 0) {
                    ForEach(items.prefix(6)) { m in
                        Button {
                            let name = (m.sigil == "@" || m.sigil == "#" || m.sigil == "&") ? m.label : m.mid
                            pick(String(text[..<t.range.lowerBound]) + (m.word.isEmpty ? m.sigil + name : m.word) + " ")
                        } label: { HStack { Text(m.sigil + m.label).dmono(12); Spacer(); Text(m.detail).dmono(11).foregroundStyle(BackplaneDesign.dim).lineLimit(1) }.padding(.horizontal, 12).frame(minHeight: 40).contentShape(Rectangle()) }
                        .buttonStyle(.plain)
                        DLine()
                    }
                }
                .background(BackplaneDesign.panel).accessibilityIdentifier("data.mentions")
            }
        }
        .task(id: text) {
            guard client.has("mentions"), let t = tail else { items = []; return }
            items = await client.mentions(t.query, kind: t.sigil)
        }
    }
}

/// A room: its posts (read again every few seconds) and a composer.
struct DataRoomView: View {
    let client: DataClient
    let room: DRoom
    @State private var posts: [DPost] = []
    @State private var draft = ""
    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 10) {
                    ForEach(posts) { p in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(p.from.isEmpty ? "you" : p.from).dmono(11, bold: true).foregroundStyle(BackplaneDesign.accent)
                            Text(p.text).dmono(13).textSelection(.enabled)
                        }
                    }
                }.padding()
            }
            DLine()
            HStack(alignment: .bottom, spacing: 8) {
                TextField("Post to #\(room.name)", text: $draft, axis: .vertical).dmono(13).padding(10).dbox(BackplaneDesign.accent).accessibilityIdentifier("data.room.composer")
                Button("Post") { let t = draft; draft = ""; Task { try? await client.roomPost(room.id, t); posts = await client.roomPosts(room.id) } }
                    .buttonStyle(DButton(primary: true)).disabled(draft.trimmingCharacters(in: .whitespaces).isEmpty).accessibilityIdentifier("data.room.post")
            }.padding(12)
        }
        .background(BackplaneDesign.background)
        .dataBar("#" + room.name)
        .task { while !Task.isCancelled { posts = await client.roomPosts(room.id); try? await Task.sleep(for: .seconds(2.5)) } }
    }
}

/// The SF Symbol drawn for a project's icon name (the hub's list, present/badge.rs); anything else is the folder.
func dataBadgeSymbol(_ icon: String) -> String {
    switch icon {
    case "circuit-board": "cpu.fill"
    case "cpu": "cpu"
    case "zap": "bolt"
    case "radio": "antenna.radiowaves.left.and.right"
    case "box": "shippingbox"
    case "layers": "square.3.layers.3d"
    case "code": "chevron.left.forwardslash.chevron.right"
    case "terminal": "terminal"
    case "globe": "globe"
    case "wrench": "wrench"
    case "flask-conical": "flask"
    case "book": "book"
    case "star": "star"
    default: "folder"
    }
}

/// A project's badge: the hub's tints and icons (`badge.list`); a press is `project.act color|icon`.
struct DataBadgeSheet: View {
    let client: DataClient
    let project: DProject
    @State private var list: DBadgeList?
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            Form {
                if let l = list {
                    Section("Colour") {
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 40))], spacing: 12) {
                            ForEach(l.colors, id: \.name) { t in
                                Button { Task { await client.projectAct(project.id, "color", value: t.name) } } label: {
                                    Rectangle().fill(Color(css: t.css) ?? .gray).frame(width: 32, height: 32)
                                        .overlay(Rectangle().stroke(t.css.caseInsensitiveCompare(project.css) == .orderedSame ? Color.primary : .clear, lineWidth: 2))
                                }.buttonStyle(.plain).accessibilityIdentifier("badge.color." + t.name)
                            }
                        }
                    }
                    Section("Icon") {
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 40))], spacing: 12) {
                            ForEach(l.icons, id: \.self) { n in
                                Button { Task { await client.projectAct(project.id, "icon", value: n) } } label: {
                                    Image(systemName: dataBadgeSymbol(n)).frame(width: 32, height: 32)
                                        .overlay(Rectangle().stroke(n == project.icon ? Color.primary : .clear, lineWidth: 2))
                                }.buttonStyle(.plain).accessibilityIdentifier("badge.icon." + n)
                            }
                        }
                    }
                } else { ProgressView() }
            }
            .navigationTitle(project.title).navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
        .task { list = await client.badgeList() }
    }
}

extension Color {
    /// "#rrggbb" as a colour; nil for anything else.
    init?(css: String) {
        guard css.hasPrefix("#"), css.count == 7, let v = UInt32(css.dropFirst(), radix: 16) else { return nil }
        self.init(red: Double((v >> 16) & 255) / 255, green: Double((v >> 8) & 255) / 255, blue: Double(v & 255) / 255)
    }
}
