import SwiftUI
import UIKit

// The copy button (feature copyable; native/gpui/src/screens/copyable.rs): one small icon wherever text is there to be copied (a code block, an
// inline command, a path, an id, a commit), as GitHub draws on a README's code. On touch it is always shown. One tap copies the exact text
// (no markdown, no trailing newline, a block whole: Copyable.exact) to the pasteboard and turns into a check for a moment (Copyable.shown).
// A shell command gets Run in terminal beside it: the chat's terminal opens with the command typed in, not run (TermStore.runInTerminal).
// VoiceOver reads "Copy" and the text; with a hardware keyboard the button takes focus, Return or Cmd+C copies, Return on Run in terminal runs it.

/// Run in terminal for the chat on screen: set by the thread, bot chat and room screens when the hub has `termChat`; nil hides the button.
private struct RunInTerminalKey: EnvironmentKey { static let defaultValue: ((String) -> Void)? = nil }
extension EnvironmentValues {
    var runInTerminal: ((String) -> Void)? { get { self[RunInTerminalKey.self] } set { self[RunInTerminalKey.self] = newValue } }
}

/// The chat's terminal for the screens that have one: Run in terminal for their content, and the bar's Terminal button (DChatTermButton).
@MainActor func dRunIn(_ client: DataClient, chat: String, title: String) -> ((String) -> Void)? {
    guard client.has("termChat"), !chat.isEmpty else { return nil }
    return { cmd in Task { do { try await client.terms.runInTerminal(cmd, chat: chat, title: title) } catch { client.say(client.conn == .online ? error.localizedDescription : "Not connected to the hub.") } } }
}

/// The copy button for `text`. `word`: "Copy"/"Copied" beside the icon (a code block's bar, a labelled press of the hub's).
struct DCopyButton: View {
    let text: String
    var word = false
    var label: String? = nil          // a hub button's own label (Copy link, Copy code); Copied for a moment either way
    @State private var copied = false
    @FocusState private var focused: Bool

    private func copy() {
        UIPasteboard.general.string = Copyable.exact(text)
        copied = true
        UIAccessibility.post(notification: .announcement, argument: Tr.t("Copied"))
        Task { try? await Task.sleep(nanoseconds: UInt64(Copyable.shown * 1_000_000_000)); copied = false }
    }
    var body: some View {
        Button(action: copy) {
            HStack(spacing: 4) {
                Image(systemName: copied ? "checkmark" : "doc.on.doc").font(.system(size: 12, weight: .medium))
                    .foregroundStyle(copied ? BackplaneDesign.ok : BackplaneDesign.dim)
                if word || label != nil { Text(copied ? Tr.t("Copied") : (label ?? Tr.t("Copy"))).dmono(11).foregroundStyle(copied ? BackplaneDesign.ok : BackplaneDesign.accent) }
            }
            .padding(.horizontal, 6).frame(minWidth: 32, minHeight: 32).contentShape(Rectangle())
            .overlay(Rectangle().strokeBorder(focused ? BackplaneDesign.accent : Color.clear, lineWidth: 1))
        }
        .buttonStyle(.plain)
        .focusable(true).focused($focused)
        .onKeyPress(keys: ["c"]) { k in if k.modifiers.contains(.command) { copy(); return .handled }; return .ignored }
        .onKeyPress(.return) { copy(); return .handled }
        .accessibilityLabel(copied ? Tr.t("Copied") : (label ?? Tr.t("Copy")))
        .accessibilityValue(Copyable.exact(text))
        .accessibilityIdentifier("copy.button")
    }
}

/// Run in terminal beside a command's copy button.
struct DRunButton: View {
    let cmd: String
    var word = false
    @Environment(\.runInTerminal) private var run
    @FocusState private var focused: Bool
    var body: some View {
        if let run {
            Button { run(cmd) } label: {
                HStack(spacing: 4) {
                    Image(systemName: "terminal").font(.system(size: 12, weight: .medium)).foregroundStyle(BackplaneDesign.dim)
                    if word { Text(Tr.t("Run in terminal")).dmono(11).foregroundStyle(BackplaneDesign.accent) }
                }
                .padding(.horizontal, 6).frame(minWidth: 32, minHeight: 32).contentShape(Rectangle())
                .overlay(Rectangle().strokeBorder(focused ? BackplaneDesign.accent : Color.clear, lineWidth: 1))
            }
            .buttonStyle(.plain)
            .focusable(true).focused($focused)
            .onKeyPress(.return) { run(cmd); return .handled }
            .accessibilityLabel(Tr.t("Run in terminal"))
            .accessibilityValue(Copyable.exact(cmd))
            .accessibilityIdentifier("copy.run")
        }
    }
}

/// The pieces of a text to copy (a room's post, a notice, a reply's inline code): each one's first line in code type, its copy button, and
/// Run in terminal beside a command.
struct DCopyPieces: View {
    let pieces: [DCopyPiece]
    var body: some View {
        if !pieces.isEmpty {
            VStack(alignment: .leading, spacing: 2) {
                ForEach(Array(pieces.enumerated()), id: \.offset) { _, p in
                    HStack(spacing: 2) {
                        Text(verbatim: firstLine(p.text)).dmono(12).foregroundStyle(BackplaneDesign.text).lineLimit(1).truncationMode(.tail)
                            .padding(.horizontal, 6).padding(.vertical, 3).background(BackplaneDesign.panel)
                        Spacer(minLength: 4)
                        if p.cmd { DRunButton(cmd: p.text) }
                        DCopyButton(text: p.text)
                    }.accessibilityElement(children: .contain).accessibilityIdentifier("copy.piece")
                }
            }
        }
    }
    private func firstLine(_ s: String) -> String {
        let l = s.split(separator: "\n", maxSplits: 1, omittingEmptySubsequences: false)
        return l.count > 1 ? String(l[0]) + " …" : s
    }
}

/// A value shown to be copied (a Settings row's note that is a path, an id, a link): the note and its copy button.
struct DCopyValue: View {
    let shown: String
    let value: String
    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 4) {
            Text(verbatim: shown).dmono(11).foregroundStyle(BackplaneDesign.dim).textSelection(.enabled)
            DCopyButton(text: value)
        }
    }
}

/// The bar's Terminal button: the chat's own shell (a thread's, a bot's chat, the uberbot's, a room's), as the desktop's Ctrl+J opens it.
struct DChatTermButton: View {
    let client: DataClient
    let chat: String
    let title: String
    var body: some View {
        if client.has("termChat"), !chat.isEmpty {
            Button { Task { do { _ = try await client.terms.openChat(chat, title: title) } catch { client.say(client.conn == .online ? error.localizedDescription : "Not connected to the hub.") } } } label: { Image(systemName: "terminal").foregroundStyle(BackplaneDesign.dim) }
                .accessibilityLabel(Tr.t("Terminal")).accessibilityIdentifier("data.chat.term")
        }
    }
}
