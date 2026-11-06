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
    @State private var result: DSearch?
    var body: some View {
        List {
            if let r = result {
                if r.threads.isEmpty && r.projects.isEmpty { Text("Nothing matches “\(query)”").foregroundStyle(.secondary) }
                ForEach(r.projects) { p in Label(p.title, systemImage: "folder").accessibilityIdentifier("data.search.project." + p.id) }
                ForEach(r.threads) { t in NavigationLink(value: t.id) { DataThreadRow(thread: t) }.accessibilityIdentifier("data.search.thread." + t.id) }
            } else { ProgressView() }
        }
        .listStyle(.plain)
        .task(id: query) {
            try? await Task.sleep(for: .milliseconds(250))
            guard !Task.isCancelled else { return }
            result = await client.search(query)
        }
    }
}

/// The model picker: provider, then its models, from `models.list`; the choice is `thread.model`.
struct DataModelSheet: View {
    let client: DataClient
    let thread: DThread
    @Environment(\.dismiss) private var dismiss
    @State private var groups: [DModelGroup] = []
    @State private var error = ""
    var body: some View {
        NavigationStack {
            List {
                if !error.isEmpty { Text(error).foregroundStyle(.red).font(.footnote) }
                ForEach(groups) { g in
                    Section(g.provider) {
                        ForEach(g.rows.isEmpty ? [DModelRow.defaultRow] : g.rows) { r in
                            Button {
                                Task { do { try await client.setModel(thread.id, provider: g.provider, model: r.value); dismiss() } catch { self.error = error.localizedDescription } }
                            } label: {
                                HStack { Text(r.label); Spacer(); if thread.provider == g.provider && thread.model == r.value { Image(systemName: "checkmark") } }
                            }
                            .disabled(!r.ok).accessibilityIdentifier("data.model." + g.provider + "." + r.value)
                        }
                    }
                }
            }
            .navigationTitle("Model · \(thread.provider) / \(thread.model.isEmpty ? "default" : thread.model)")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } } }
        }
        .task { groups = await client.models() }
    }
}
extension DModelRow {
    // a provider that lists no models still offers its default
    static var defaultRow: DModelRow { try! JSONDecoder().decode(DModelRow.self, from: Data(#"{"value":"","label":"default","ok":true}"#.utf8)) }
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
                            pick(String(text[..<t.range.lowerBound]) + m.sigil + name + " ")
                        } label: { HStack { Text(m.sigil + m.label).font(.system(.footnote, design: .monospaced)); Spacer(); Text(m.detail).font(.caption).foregroundStyle(.secondary) }.padding(.horizontal).padding(.vertical, 8) }
                        .buttonStyle(.plain)
                        Divider()
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
                            Text(p.from.isEmpty ? "you" : p.from).font(.system(.caption, design: .monospaced).bold()).foregroundStyle(BackplaneDesign.accent)
                            Text(p.text).font(.system(.footnote, design: .monospaced))
                        }
                    }
                }.padding()
            }
            HStack {
                TextField("Post to #\(room.name)", text: $draft, axis: .vertical).textFieldStyle(.roundedBorder).accessibilityIdentifier("data.room.composer")
                Button("Post") { let t = draft; draft = ""; Task { try? await client.roomPost(room.id, t); posts = await client.roomPosts(room.id) } }
                    .buttonStyle(.borderedProminent).disabled(draft.trimmingCharacters(in: .whitespaces).isEmpty).accessibilityIdentifier("data.room.post")
            }.padding().background(BackplaneDesign.panel)
        }
        .navigationTitle("#" + room.name).navigationBarTitleDisplayMode(.inline)
        .task { while !Task.isCancelled { posts = await client.roomPosts(room.id); try? await Task.sleep(for: .seconds(2.5)) } }
    }
}
