import SwiftUI
import UIKit

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
            Button(Tr.t(thread.pinned ? "Unpin" : "Pin"), systemImage: thread.pinned ? "pin.slash" : "pin") { Task { await client.threadAct(thread.id, thread.pinned ? "unpin" : "pin") } }
            Button(Tr.t(thread.settled ? "Unsettle" : "Settle"), systemImage: "checkmark.circle") { Task { await client.threadAct(thread.id, thread.settled ? "unsettle" : "settle") } }
            Button(Tr.t(thread.archived ? "Unarchive" : "Archive"), systemImage: "archivebox") { Task { await client.threadAct(thread.id, thread.archived ? "unarchive" : "archive") } }
            if !thread.parent.isEmpty { Button(Tr.t("Move to top level"), systemImage: "arrow.up.left") { Task { await client.threadAct(thread.id, "detach") } } }
            Button(Tr.t("Rename"), systemImage: "pencil") { renaming = thread }
            Button(Tr.t("Delete…"), systemImage: "trash", role: .destructive) { deleting = thread }
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
            .alert(Tr.t("Rename thread"), isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } })) {
                TextField(Tr.t("Title"), text: $title)
                Button(Tr.t("Rename")) { if let t = renaming, !title.trimmingCharacters(in: .whitespaces).isEmpty { Task { await client.threadAct(t.id, "rename", title: title.trimmingCharacters(in: .whitespaces)) } } }
                Button(Tr.t("Cancel"), role: .cancel) {}
            }
            .onChange(of: renaming?.id) { _, _ in title = renaming?.title ?? "" }
            .confirmationDialog(Tr.ios_thread_delete_ask(title: deleting?.title ?? ""), isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }), titleVisibility: .visible) {
                Button(Tr.t("Delete thread"), role: .destructive) { if let t = deleting { Task { await client.threadAct(t.id, "delete"); deleted() } } }
                Button(Tr.t("Keep it"), role: .cancel) {}
            }
    }
}

/// A thread opened at one message (a search hit): it reads older pages until the message is held, scrolls to it and marks it.
struct DThreadAt: Hashable { var id: String; var msg: String }

/// A snippet of what was said with the query's words in bold (ThreadFind.spans).
func dSnippet(_ text: String, _ q: String) -> Text {
    var out = Text("")
    for (run, word) in ThreadFind.spans(text, q) { out = out + (word ? Text(run).bold().foregroundColor(BackplaneDesign.accentText) : Text(run)) }
    return out
}

/// One "Messages" row of the search: the snippet with the words in bold, who said it, the thread and its project, when.
struct DataMessageHit: View {
    let hit: DHit
    let query: String
    private var place: String {
        let who = hit.role == "user" ? "you said" : "agent said"
        let parts: [String] = [who, hit.title.isEmpty ? "Thread" : hit.title, hit.project, dAgo(hit.at)]
        return parts.filter { !$0.isEmpty }.joined(separator: " · ")
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            dSnippet(hit.text, query).dmono(13).foregroundStyle(BackplaneDesign.text).lineLimit(3).multilineTextAlignment(.leading)
            Text(place).dmono(11).foregroundStyle(BackplaneDesign.faint).lineLimit(1)
        }.padding(.horizontal, 12).padding(.vertical, 6).frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
    }
}

/// Search over threads and projects (the hub's `search`), and under them what was said ("Messages": a tap opens the thread at the message).
struct DataSearchResults: View {
    let client: DataClient
    let query: String
    let open: (String) -> Void
    var openAt: (DThreadAt) -> Void = { _ in }
    @State private var result: DSearch?
    var body: some View {
        List {
            if let r = result {
                if r.threads.isEmpty && r.projects.isEmpty && r.messages.isEmpty { Text(Tr.ios_search_none(query: query)).dmono(12).foregroundStyle(BackplaneDesign.dim).padding(12).dataRow() }
                ForEach(r.projects) { p in Label { Text(p.title).dmono(13) } icon: { Image(systemName: "folder").foregroundStyle(BackplaneDesign.ok) }.padding(.horizontal, 12).frame(minHeight: 40).dataRow().accessibilityIdentifier("data.search.project." + p.id) }
                ForEach(r.threads.filter { !$0.history }) { t in Button { open(t.id) } label: { DataThreadRow(thread: t) }.buttonStyle(.plain).dataRow().accessibilityIdentifier("data.search.thread." + t.id) }
                // the old Scratch chats the search found, under their group's name (feature history)
                let old = r.threads.filter(\.history)
                if !old.isEmpty { DHeading(text: Tr.t("Conversation history")).padding(.leading, 12).padding(.top, 12).padding(.bottom, 4).dataRow().accessibilityIdentifier("data.search.history") }
                ForEach(old) { t in Button { open(t.id) } label: { DataThreadRow(thread: t) }.buttonStyle(.plain).dataRow().accessibilityIdentifier("data.search.thread." + t.id) }
                if !r.messages.isEmpty { DHeading(text: Tr.k("ios.search.messages")).padding(.leading, 12).padding(.top, 12).padding(.bottom, 4).dataRow().accessibilityIdentifier("data.search.messages") }
                ForEach(r.messages) { h in
                    Button { openAt(DThreadAt(id: h.thread, msg: h.msg)) } label: { DataMessageHit(hit: h, query: query) }
                        .buttonStyle(.plain).dataRow().accessibilityIdentifier("data.search.message." + h.thread + "." + h.msg)
                }
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
                Button(Tr.t("Done")) { dismiss() }.dmono(13, bold: true).foregroundStyle(BackplaneDesign.accent).accessibilityIdentifier("data.picker.done")
            }.padding(.horizontal, 14).frame(height: 48)
            DLine()
            if !error.isEmpty { Text(Tr.t(error)).dmono(12).foregroundStyle(BackplaneDesign.bad).frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 14).padding(.vertical, 8).accessibilityIdentifier("data.picker.error") }
            ScrollView {
                // (a plain stack: Model's default and Effort's auto both have the value "", and a lazy stack draws the second blank)
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(picker?.sections ?? []) { s in
                        DHeading(text: Tr.t(s.title)).padding(.horizontal, 14).padding(.top, 14).padding(.bottom, 4)
                        ForEach(s.choices) { c in
                            Button { press(s, c) } label: {
                                HStack(spacing: 8) {
                                    Text(c.on ? "✓" : "").dmono(13).foregroundStyle(BackplaneDesign.accent).frame(width: 16)
                                    VStack(alignment: .leading, spacing: 1) {
                                        Text(Tr.t(c.label)).dmono(13).foregroundStyle(c.ok ? BackplaneDesign.text : BackplaneDesign.faint)
                                        if !c.ok, !c.why.isEmpty { Text(Tr.t(c.why)).dmono(11).foregroundStyle(BackplaneDesign.faint) }
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
                        } label: { HStack { Text(m.sigil + m.label).dmono(12); Spacer(); Text(Tr.t(m.detail)).dmono(11).foregroundStyle(BackplaneDesign.dim).lineLimit(1) }.padding(.horizontal, 12).frame(minHeight: 40).contentShape(Rectangle()) }
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
                            Text(p.from.isEmpty ? Tr.t("you") : p.from).dmono(11, bold: true).foregroundStyle(BackplaneDesign.accent)
                            DLinkText(text: p.text)
                        }
                    }
                }.padding()
            }
            DLine()
            HStack(alignment: .bottom, spacing: 8) {
                TextField(Tr.ios_room_post_to(room: room.name), text: $draft, axis: .vertical).dmono(13).padding(10).dbox(BackplaneDesign.accent).accessibilityIdentifier("data.room.composer")
                Button(Tr.t("Post")) { let t = draft; draft = ""; Task { try? await client.roomPost(room.id, t); posts = await client.roomPosts(room.id) } }
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
                    Section(Tr.t("Colour")) {
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 40))], spacing: 12) {
                            ForEach(l.colors, id: \.name) { t in
                                Button { Task { await client.projectAct(project.id, "color", value: t.name) } } label: {
                                    Rectangle().fill(Color(css: t.css) ?? .gray).frame(width: 32, height: 32)
                                        .overlay(Rectangle().stroke(t.css.caseInsensitiveCompare(project.css) == .orderedSame ? Color.primary : .clear, lineWidth: 2))
                                }.buttonStyle(.plain).accessibilityIdentifier("badge.color." + t.name)
                            }
                        }
                    }
                    Section(Tr.t("Icon")) {
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
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button(Tr.t("Done")) { dismiss() } } }
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

/// "Search in thread" (the thread's menu): what was said here with every word typed, newest first (the hub's `search` with `thread`); a tap goes to the message.
struct DataFindInThread: View {
    let client: DataClient
    let thread: String
    let go: (String) -> Void
    @State private var q = ""
    @State private var hits: [DHit]?
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 0) {
                TextField(Tr.t("Words said in this thread"), text: $q).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled()
                    .padding(.horizontal, 10).frame(height: 40).dbox().padding(12).accessibilityIdentifier("data.find.field")
                if let h = hits {
                    if h.isEmpty { Text(Tr.ios_find_none(query: q.trimmingCharacters(in: .whitespaces))).dmono(12).foregroundStyle(BackplaneDesign.dim).padding(14) }
                    List(h) { x in
                        Button { go(x.msg) } label: {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(Tr.t(x.role == "user" ? "you" : "agent") + " · " + dAgo(x.at)).dmono(11).foregroundStyle(BackplaneDesign.faint)
                                Text(x.text).dmono(13).foregroundStyle(BackplaneDesign.text).lineLimit(3)
                            }
                        }.buttonStyle(.plain).accessibilityIdentifier("data.find.hit")
                    }.listStyle(.plain)
                }
                Spacer(minLength: 0)
            }
            .background(BackplaneDesign.background)
            .navigationTitle(Tr.t("Search in thread")).navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button(Tr.t("Done")) { dismiss() } } }
            .task(id: q) {
                let w = q.trimmingCharacters(in: .whitespaces)
                if w.isEmpty { hits = nil; return }
                try? await Task.sleep(for: .milliseconds(250)); if Task.isCancelled { return }
                hits = await client.searchIn(thread, w)
            }
        }
    }
}

/// The camera, for a photo to attach (Attach, Take a photo): the system's own capture screen; `done` gets the photo as JPEG, or nil when cancelled.
struct DCamera: UIViewControllerRepresentable {
    let done: (Data?) -> Void
    func makeUIViewController(context: Context) -> UIImagePickerController {
        let p = UIImagePickerController(); p.sourceType = .camera; p.delegate = context.coordinator; return p
    }
    func updateUIViewController(_ c: UIImagePickerController, context: Context) {}
    func makeCoordinator() -> Coordinator { Coordinator(done) }
    final class Coordinator: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate {
        let done: (Data?) -> Void
        init(_ done: @escaping (Data?) -> Void) { self.done = done }
        func imagePickerController(_ picker: UIImagePickerController, didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]) {
            done((info[.originalImage] as? UIImage)?.jpegData(compressionQuality: 0.85))
        }
        func imagePickerControllerDidCancel(_ picker: UIImagePickerController) { done(nil) }
    }
}
