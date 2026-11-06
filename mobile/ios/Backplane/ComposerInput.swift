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
            UIKitField(text: text, placeholder: NativeMessages.native_thread_composer(lang), editable: model.canEditComposer(for: thread.id),
                       onChange: { model.editComposer($0, for: thread.id) },
                       // a hardware keyboard's Return sends (Shift+Return is a newline), as the web's composer does
                       onSend: { if model.canSendComposer(for: thread.id) { model.sendComposer(for: thread.id) } },
                       // an image on the pasteboard is attached, as the paperclip's Paste does
                       onPasteImages: { imgs in
                           for (i, image) in imgs.enumerated() { if let d = image.pngData() { model.attach(d, name: "pasted-\(i + 1).png", for: thread.id) } }
                       })
                .accessibilityLabel(NativeMessages.native_thread_composer(lang))
                // one text view per thread: a composition or an undo history never follows the person to the next thread
                .id(thread.id)
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
final class ComposerUITextView: UITextView {
    var onSend: (() -> Void)?
    var onPasteImages: (([UIImage]) -> Void)?

    // Return on a hardware keyboard sends, Shift+Return (and the software keyboard's Return) inserts a newline
    // A press we consume gets none of the later phases passed to UIKit (all phases of a press, or none).
    private var consumed = Set<UIPress>()
    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        if markedTextRange == nil, onSend != nil,
           let p = presses.first(where: { press in
               guard let key = press.key, key.keyCode == .keyboardReturnOrEnter || key.keyCode == .keypadEnter else { return false }
               return key.modifierFlags.intersection([.shift, .alternate, .control, .command]).isEmpty
           }) {
            consumed.insert(p)
            onSend?()
            let rest = presses.subtracting([p])
            if !rest.isEmpty { super.pressesBegan(rest, with: event) }
            return
        }
        super.pressesBegan(presses, with: event)
    }
    override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let rest = presses.subtracting(consumed); consumed.subtract(presses)
        if !rest.isEmpty { super.pressesEnded(rest, with: event) }
    }
    override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let rest = presses.subtracting(consumed); consumed.subtract(presses)
        if !rest.isEmpty { super.pressesCancelled(rest, with: event) }
    }

    // paste: pictures are attachments; anything else is the system's text paste
    override func paste(_ sender: Any?) {
        let pb = UIPasteboard.general
        if pb.hasImages, !pb.hasStrings, let imgs = pb.images, !imgs.isEmpty { onPasteImages?(imgs); return }
        super.paste(sender)
    }
    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        if action == #selector(UIResponderStandardEditActions.paste(_:)), isEditable, UIPasteboard.general.hasImages { return true }
        return super.canPerformAction(action, withSender: sender)
    }
}

// the composer's text view, for the debug acts that drive an IME composition or an undo (BACKPLANE_ACTS @ime, @imecommit, @undo)
enum ComposerHandle { static weak var view: UITextView? }

struct UIKitField: UIViewRepresentable {
    let text: String
    let placeholder: String
    let editable: Bool
    let onChange: (String) -> Void
    let onSend: () -> Void
    let onPasteImages: ([UIImage]) -> Void
    var font: UIFont? = nil   // the data app's mono; else the system's body

    func makeUIView(context: Context) -> UITextView {
        let v = ComposerUITextView()
        v.onSend = onSend
        v.onPasteImages = onPasteImages
        v.delegate = context.coordinator
        v.font = font ?? UIFont.preferredFont(forTextStyle: .body)
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
        label.font = font ?? UIFont.preferredFont(forTextStyle: .body)
        label.adjustsFontForContentSizeCategory = true
        label.numberOfLines = 1
        label.lineBreakMode = .byTruncatingTail
        label.textColor = UIColor(BackplaneDesign.secondaryText)
        label.tag = 77
        label.isAccessibilityElement = false   // the field's own label says it
        label.translatesAutoresizingMaskIntoConstraints = false
        v.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: v.leadingAnchor), label.topAnchor.constraint(equalTo: v.topAnchor),
            label.trailingAnchor.constraint(lessThanOrEqualTo: v.frameLayoutGuide.trailingAnchor),
        ])
        ComposerHandle.view = v
        return v
    }

    func updateUIView(_ v: UITextView, context: Context) {
        context.coordinator.parent = self
        (v as? ComposerUITextView)?.onSend = onSend
        (v as? ComposerUITextView)?.onPasteImages = onPasteImages
        if let label = v.viewWithTag(77) as? UILabel, label.text != placeholder { label.text = placeholder }   // a language change
        // The model's text differs from the view's only when something other than the person's typing changed it (Send cleared
        // it, a restore, another device): a composition in progress is ended first, the selection is kept (clamped), and the
        // undo history, which points at the old ranges, is dropped (shake-to-undo crashed on those).
        if v.text != text {
            // unmarkText() and the text change both tell the delegate; the model is what is being applied here, so the
            // coordinator must not send either back as an edit (that wrote the old text over the new one)
            context.coordinator.applying = true
            defer { context.coordinator.applying = false }
            if v.markedTextRange != nil { v.unmarkText() }
            let sel = v.selectedRange
            v.text = text
            let n = (text as NSString).length
            v.selectedRange = NSRange(location: min(sel.location, n), length: min(sel.length, max(0, n - min(sel.location, n))))
            v.undoManager?.removeAllActions()
        }
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
        var applying = false
        init(_ p: UIKitField) { parent = p }
        func textViewDidChange(_ v: UITextView) {
            if applying { v.viewWithTag(77)?.isHidden = !v.text.isEmpty; return }
            v.viewWithTag(77)?.isHidden = !v.text.isEmpty
            parent.onChange(v.text)
        }
    }
}
