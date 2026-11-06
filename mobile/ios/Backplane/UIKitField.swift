import SwiftUI
import UIKit

// The data app's composer text field: a UITextView that handles IME, grows to six lines, sends on Return from a hardware keyboard and takes pasted pictures.
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
