import SwiftUI
import UIKit

// A tap on a link in a message (docs/PHONE-DATA.md, "Links in messages"). Web addresses open in Safari, emails and phone numbers in the system's Mail and Phone
// (SwiftUI's openURL; a long press shows the system's Open / Copy / Share menu). A backplane:// link or a path is asked of the hub (link.resolve), which says
// what it opens: a thread (also one on a linked machine), a project, a room, a file at a line, or a short note why not. The root screen follows `linkTarget`.
struct DLinkTarget: Equatable {
    enum Kind { case thread, project, room, file }
    var kind: Kind; var id: String; var path = ""; var line = 0
    var n = UUID()
}

struct DLinkReply: Decodable {
    var kind: String; var thread: String; var project: String; var room: String; var path: String; var line: Int; var note: String
    enum CodingKeys: String, CodingKey { case kind, thread, project, room, path, line, note }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        kind = c.str(.kind); thread = c.str(.thread); project = c.str(.project); room = c.str(.room); path = c.str(.path); line = c.int(.line); note = c.str(.note)
    }
}

extension DataClient {
    /// What to ask the hub for a tapped link: a backplane:// address (this build's own scheme counts as it), or the path a bppath: link carries.
    private func linkText(_ url: URL) -> String? {
        if let p = dPathFrom(url) { return p }
        let s = url.absoluteString
        if url.scheme == "backplane" { return s }
        if url.scheme == AppIdentity.scheme { return "backplane" + s.dropFirst(AppIdentity.scheme.count) }
        return nil
    }

    func openLink(_ url: URL, thread: String) {
        guard let text = linkText(url) else { return }
        Task {
            // a link tapped before the hub has said hello (a cached list, just launched) waits a moment for it: only the hub knows what `desktop~t9` is
            for _ in 0..<50 where !(conn == .online && has("links")) { try? await Task.sleep(nanoseconds: 100_000_000) }
            // a hub that does not know link.resolve: a thread link as before, the rest says so
            guard has("links") else {
                if let u = URL(string: text), u.host == "thread", let id = u.pathComponents.dropFirst().first, !id.isEmpty { linkTarget = DLinkTarget(kind: .thread, id: id) }
                else { say("Update the hub to open this link.") }
                return
            }
            do {
                let r: DLinkReply = try await call("link.resolve", ["url": text, "thread": thread])
                switch r.kind {
                case "thread": linkTarget = DLinkTarget(kind: .thread, id: r.thread)
                case "project": linkTarget = DLinkTarget(kind: .project, id: r.project)
                case "room": linkTarget = DLinkTarget(kind: .room, id: r.room)
                case "file": linkTarget = DLinkTarget(kind: .file, id: r.thread, path: r.path, line: r.line)
                default: say(r.note.isEmpty ? "That link does not open here." : r.note)
                }
            } catch { say(conn == .online ? error.localizedDescription : "Not connected to the hub.") }
        }
    }

    /// A short note over the screen, gone by itself.
    func say(_ note: String) {
        linkNote = note
        Task { try? await Task.sleep(nanoseconds: 3_500_000_000); if linkNote == note { linkNote = "" } }
    }
}

/// The note a link that opens nothing leaves.
struct DLinkNote: View {
    let client: DataClient
    var body: some View {
        #if DEBUG
        Text(client.lastLink).font(.system(size: 1)).frame(width: 2, height: 2).opacity(0.02).accessibilityIdentifier("link.last").allowsHitTesting(false)
        #endif
        if !client.linkNote.isEmpty {
            Text(client.linkNote).dmono(12).foregroundStyle(BackplaneDesign.text).padding(.horizontal, 12).padding(.vertical, 9)
                .background(BackplaneDesign.raised).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1))
                .padding(.bottom, 70).padding(.horizontal, 16).transition(.opacity).accessibilityIdentifier("link.note")
        }
    }
}

/// Plain text with its links tappable (a user's message, a room post, a note): not Markdown, so `*` and `_` stay as typed. Selectable.
struct DLinkText: View {
    let text: String
    var size: CGFloat = 13
    var thread = ""
    var body: some View {
        Text(dLinked(text, size: size)).textSelection(.enabled)
            .environment(\.openURL, OpenURLAction { url in dMdOpen(url, thread: thread) })
    }
}

private final class DLinkedCache { nonisolated(unsafe) static let c = NSCache<NSString, Box>(); final class Box { let v: AttributedString; init(_ v: AttributedString) { self.v = v } } }
func dLinked(_ text: String, size: CGFloat) -> AttributedString {
    let key = "\(size)|" + text
    if let c = DLinkedCache.c.object(forKey: key as NSString) { return c.v }
    var out = AttributedString()
    for run in dLinkRuns(text) {
        var a = AttributedString(run.text); a.font = Font.mono(size: size, weight: .regular)
        if let u = run.url { a.link = u; a.foregroundColor = BackplaneDesign.accent; a.underlineStyle = .single }
        out.append(a)
    }
    DLinkedCache.c.setObject(DLinkedCache.Box(out), forKey: key as NSString); return out
}
