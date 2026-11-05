import SwiftUI
import UIKit

// The composer's text input. The SwiftUI multi-line TextField lays the whole draft out on the main thread whenever its view
// is re-evaluated, which every screen update did (1.2 s at 4 KB, 1.7 s at 8 KB, 8-22 s from 20 KB: results/ios/stuck*).
// Three implementations, picked by BACKPLANE_COMPOSER (debug builds; the default is `uikit`, measured: results/ios/keys):
//   swiftui   the original: the field is rebuilt with every ThreadComposer update
//   isolated  the same TextField in an Equatable child that no screen update rebuilds (only a change of the draft, thread,
//             language or editability does)
//   uikit     a UITextView (TextKit lays out the visible lines only), sized to at most six lines
enum ComposerKind {
    case swiftui, isolated, uikit
    static let current: ComposerKind = {
        #if DEBUG
        switch ProcessInfo.processInfo.environment["BACKPLANE_COMPOSER"] {
        case "swiftui": return .swiftui
        case "uikit": return .uikit
        case "isolated": return .isolated
        default: break
        }
        #endif
        return .uikit
    }()
}

struct ComposerInput: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let thread: ThreadView
    var focus: FocusState<Bool>.Binding

    var body: some View {
        let text = model.composerText(for: thread)
        switch ComposerKind.current {
        case .swiftui:
            TextField("", text: Binding(get: { model.composerText(for: thread) }, set: { model.editComposer($0, for: thread.id) }),
                      prompt: Text(NativeMessages.native_thread_composer(lang)).foregroundStyle(BackplaneDesign.secondaryText), axis: .vertical)
                .disabled(!model.canEditComposer(for: thread.id))
                .lineLimit(1...6)
                .focused(focus)
                .accessibilityLabel(NativeMessages.native_thread_composer(lang))
                .accessibilityIdentifier("thread.composer")
        case .isolated:
            IsolatedField(model: model, thread: thread.id, lang: lang, shown: text, editable: model.canEditComposer(for: thread.id), focus: focus)
                .equatable()
        case .uikit:
            UIKitField(text: text, placeholder: NativeMessages.native_thread_composer(lang), editable: model.canEditComposer(for: thread.id)) {
                model.editComposer($0, for: thread.id)
            }
            .accessibilityLabel(NativeMessages.native_thread_composer(lang))
        }
    }
}

// The TextField in a child whose body runs only when its own inputs change (not at every screen update). The text is read
// through the binding's getter, so typing still round-trips through the model.
struct IsolatedField: View, Equatable {
    let model: AppModel
    let thread: String
    let lang: String
    let shown: String
    let editable: Bool
    var focus: FocusState<Bool>.Binding

    static func == (a: IsolatedField, b: IsolatedField) -> Bool {
        a.thread == b.thread && a.lang == b.lang && a.editable == b.editable && a.shown.utf8.count == b.shown.utf8.count && a.shown == b.shown
    }

    var body: some View {
        TextField("", text: Binding(get: { shown }, set: { model.editComposer($0, for: thread) }),
                  prompt: Text(NativeMessages.native_thread_composer(lang)).foregroundStyle(BackplaneDesign.secondaryText), axis: .vertical)
            .disabled(!editable)
            .lineLimit(1...6)
            .focused(focus)
            .accessibilityLabel(NativeMessages.native_thread_composer(lang))
            .accessibilityIdentifier("thread.composer")
    }
}

// A UITextView: lays out only what is visible, handles marked text (IME) and grows to six lines, then scrolls.
struct UIKitField: UIViewRepresentable {
    let text: String
    let placeholder: String
    let editable: Bool
    let onChange: (String) -> Void

    func makeUIView(context: Context) -> UITextView {
        let v = UITextView()
        v.delegate = context.coordinator
        v.font = UIFont.preferredFont(forTextStyle: .body)
        v.adjustsFontForContentSizeCategory = true
        v.backgroundColor = .clear
        v.textContainerInset = UIEdgeInsets(top: 0, left: 0, bottom: 0, right: 0)
        v.textContainer.lineFragmentPadding = 0
        v.isScrollEnabled = false
        // TextKit lays out only the visible part of a long text
        v.layoutManager.allowsNonContiguousLayout = true
        v.autocorrectionType = .default
        v.accessibilityIdentifier = "thread.composer"
        let label = UILabel()
        label.text = placeholder
        label.font = v.font
        label.textColor = UIColor(BackplaneDesign.secondaryText)
        label.tag = 77
        label.translatesAutoresizingMaskIntoConstraints = false
        v.addSubview(label)
        NSLayoutConstraint.activate([label.leadingAnchor.constraint(equalTo: v.leadingAnchor), label.topAnchor.constraint(equalTo: v.topAnchor)])
        return v
    }

    func updateUIView(_ v: UITextView, context: Context) {
        context.coordinator.parent = self
        // a change made by the person (marked text, selection) is never overwritten while composing
        if v.markedTextRange == nil, v.text != text { v.text = text }
        v.isEditable = editable
        v.viewWithTag(77)?.isHidden = !text.isEmpty
    }

    // BACKPLANE_UIKIT_FAST=0 (debug) restores the full measurement, for the A/B in results/ios/keys
    static let fast: Bool = {
        #if DEBUG
        return ProcessInfo.processInfo.environment["BACKPLANE_UIKIT_FAST"] != "0"
        #else
        return true
        #endif
    }()

    func sizeThatFits(_ proposal: ProposedViewSize, uiView v: UITextView, context: Context) -> CGSize? {
        let width = proposal.width ?? 280
        let line = v.font?.lineHeight ?? 20
        #if DEBUG
        let t0 = CACurrentMediaTime()
        defer { if Stall.on { NSLog("Backplane field sizeThatFits %.1f ms bytes=%d fast=%@", (CACurrentMediaTime() - t0) * 1000, text.utf8.count, String(Self.fast)) } }
        #endif
        // Laying out the whole text to find its height costs time in proportion to its length at every edit. Beyond about
        // 600 bytes the field is simply six lines tall and scrolls, and only the visible lines are laid out.
        if Self.fast, text.utf8.count > 600 {
            if !v.isScrollEnabled { v.isScrollEnabled = true }
            return CGSize(width: width, height: line * 6)
        }
        let fit = v.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude)).height
        let h = min(max(fit, line), line * 6)
        let scroll = fit > line * 6
        if v.isScrollEnabled != scroll { v.isScrollEnabled = scroll }
        return CGSize(width: width, height: h)
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    final class Coordinator: NSObject, UITextViewDelegate {
        var parent: UIKitField
        init(_ p: UIKitField) { parent = p }
        func textViewDidChange(_ v: UITextView) {
            v.viewWithTag(77)?.isHidden = !v.text.isEmpty
            parent.onChange(v.text)
        }
    }
}
