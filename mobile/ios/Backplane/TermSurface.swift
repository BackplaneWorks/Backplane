import UIKit

// Terminal text drawn with UIKit: runs of cells with colours, a block cursor, nothing animated. TermSurface draws what it is given; TermInputSurface (below) is the
// full screen's: it takes the keyboard (soft, with the accessory row, or hardware), paste, pinch and scrolling back.

private func uiColor(_ rgb: UInt32, alpha: CGFloat = 1) -> UIColor {
    UIColor(red: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255, blue: CGFloat(rgb & 255) / 255, alpha: alpha)
}

class TermSurface: UIView {
    var lines: [VLine] = [] { didSet { setNeedsDisplay() } }
    var cursor: (x: Int, y: Int)? { didSet { setNeedsDisplay() } }
    var fontSize: CGFloat = 12 { didSet { if fontSize != oldValue { rebuild(); setNeedsLayout(); setNeedsDisplay() } } }
    private(set) var cw: CGFloat = 7, lh: CGFloat = 15
    private var font = UIFont.monospacedSystemFont(ofSize: 12, weight: .regular), bold = UIFont.monospacedSystemFont(ofSize: 12, weight: .bold)

    /// Width of one cell per point of font size.
    static let aspect: CGFloat = {
        let f = Mono.uiFont(size: 100); return ("M" as NSString).size(withAttributes: [.font: f]).width / 100
    }()

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = true; contentMode = .redraw; backgroundColor = uiColor(vtDefaultBg); clipsToBounds = true
        rebuild()
    }
    required init?(coder: NSCoder) { fatalError() }

    private func rebuild() {
        font = Mono.uiFont(size: fontSize); bold = Mono.uiFont(size: fontSize, weight: .bold)
        cw = ("M" as NSString).size(withAttributes: [.font: font]).width
        lh = ceil(font.lineHeight)
    }
    /// The grid that fits `size` at this font.
    func grid(_ size: CGSize) -> (cols: Int, rows: Int) { (max(Int(size.width / cw), 2), max(Int(size.height / lh), 2)) }

    override func draw(_ rect: CGRect) {
        guard let ctx = UIGraphicsGetCurrentContext() else { return }
        ctx.setFillColor(uiColor(vtDefaultBg).cgColor); ctx.fill(rect)
        for (i, line) in lines.enumerated() {
            let y = CGFloat(i) * lh
            if y + lh < rect.minY || y > rect.maxY { continue }
            var col = 0
            for run in line.runs {
                let w = run.text.unicodeScalars.reduce(0) { $0 + vtWidth($1.value) }
                let x = CGFloat(col) * cw
                if run.bg != vtDefaultBg { ctx.setFillColor(uiColor(run.bg).cgColor); ctx.fill(CGRect(x: x, y: y, width: CGFloat(w) * cw, height: lh)) }
                var a: [NSAttributedString.Key: Any] = [.font: run.bold ? bold : font, .foregroundColor: uiColor(run.fg)]
                if run.underline { a[.underlineStyle] = NSUnderlineStyle.single.rawValue }
                (run.text as NSString).draw(at: CGPoint(x: x, y: y), withAttributes: a)
                col += w
            }
        }
        if let c = cursor {
            ctx.setFillColor(uiColor(vtDefaultFg, alpha: 0.55).cgColor)
            ctx.fill(CGRect(x: CGFloat(c.x) * cw, y: CGFloat(c.y) * lh, width: cw, height: lh))
        }
    }
}

/// The keys above the soft keyboard: Esc, Tab, Ctrl (sticky), the arrows and the characters a phone keyboard hides.
final class TermAccessory: UIInputView {
    var press: ((String) -> Void)?
    private var ctrl: UIButton?
    init() {
        super.init(frame: CGRect(x: 0, y: 0, width: 320, height: 44), inputViewStyle: .keyboard)
        allowsSelfSizing = true
        let scroll = UIScrollView(); scroll.showsHorizontalScrollIndicator = false; scroll.translatesAutoresizingMaskIntoConstraints = false
        let stack = UIStackView(); stack.axis = .horizontal; stack.spacing = 4; stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(scroll); scroll.addSubview(stack)
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: leadingAnchor), scroll.trailingAnchor.constraint(equalTo: trailingAnchor),
            scroll.topAnchor.constraint(equalTo: topAnchor), scroll.bottomAnchor.constraint(equalTo: bottomAnchor), heightAnchor.constraint(equalToConstant: 44),
            stack.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 4), stack.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -4),
            stack.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 4), stack.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -4),
            stack.heightAnchor.constraint(equalTo: scroll.frameLayoutGuide.heightAnchor, constant: -8),
        ])
        for (title, id) in [("Esc", "esc"), ("Tab", "tab"), ("Ctrl", "ctrl"), ("←", "left"), ("↑", "up"), ("→", "right"), ("↓", "down"), ("|", "|"), ("~", "~"), ("/", "/"), ("-", "-")] {
            let b = UIButton(type: .system)
            var c = UIButton.Configuration.filled(); c.title = title; c.cornerStyle = .fixed; c.background.cornerRadius = 0
            c.baseBackgroundColor = uiColor(0x2a2f38); c.baseForegroundColor = uiColor(0xd9dde4); c.contentInsets = NSDirectionalEdgeInsets(top: 0, leading: 12, bottom: 0, trailing: 12)
            c.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { var a = $0; a.font = Mono.uiFont(size: 14, weight: .semibold); return a }
            b.configuration = c; b.accessibilityIdentifier = "term.key." + id; b.accessibilityLabel = title
            b.addAction(UIAction { [weak self] _ in self?.press?(id) }, for: .touchUpInside)
            if id == "ctrl" { ctrl = b }
            stack.addArrangedSubview(b)
            b.widthAnchor.constraint(greaterThanOrEqualToConstant: 40).isActive = true
        }
    }
    required init?(coder: NSCoder) { fatalError() }
    func setCtrl(_ on: Bool) {
        guard var c = ctrl?.configuration else { return }
        c.baseBackgroundColor = on ? uiColor(0x5fb3ff) : uiColor(0x2a2f38); c.baseForegroundColor = on ? uiColor(0x06121f) : uiColor(0xd9dde4); ctrl?.configuration = c
    }
}

final class TermInputSurface: TermSurface, UIKeyInput, UITextInputTraits, UIEditMenuInteractionDelegate {
    /// bytes for the program
    var onBytes: (([UInt8]) -> Void)?
    /// the grid that fits changed
    var onGrid: ((Int, Int) -> Void)?
    var onFontSize: ((CGFloat) -> Void)?
    var vt: () -> Vt? = { nil }
    private(set) var scroll = 0 { didSet { if scroll != oldValue { onScroll?() } } }
    var onScroll: (() -> Void)?
    private var ctrl = false { didSet { accessory.setCtrl(ctrl) } }
    private let accessory = TermAccessory()
    private var panBase = 0
    private var pinchBase: CGFloat = 12
    private var lastGrid = (0, 0)
    private var menu: UIEditMenuInteraction?

    // UITextInputTraits: a terminal types exactly what is pressed
    var autocorrectionType: UITextAutocorrectionType = .no
    var autocapitalizationType: UITextAutocapitalizationType = .none
    var spellCheckingType: UITextSpellCheckingType = .no
    var smartQuotesType: UITextSmartQuotesType = .no
    var smartDashesType: UITextSmartDashesType = .no
    var smartInsertDeleteType: UITextSmartInsertDeleteType = .no
    var keyboardType: UIKeyboardType = .asciiCapable
    var keyboardAppearance: UIKeyboardAppearance = .dark

    override init(frame: CGRect) {
        super.init(frame: frame)
        isAccessibilityElement = true; accessibilityIdentifier = "term.screen"; accessibilityLabel = "Terminal"
        accessory.press = { [weak self] in self?.accessoryKey($0) }
        addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(tapped)))
        addGestureRecognizer(UIPinchGestureRecognizer(target: self, action: #selector(pinched(_:))))
        let pan = UIPanGestureRecognizer(target: self, action: #selector(panned(_:))); pan.maximumNumberOfTouches = 1; addGestureRecognizer(pan)
        let hold = UILongPressGestureRecognizer(target: self, action: #selector(held(_:))); addGestureRecognizer(hold)
        let m = UIEditMenuInteraction(delegate: self); addInteraction(m); menu = m
    }
    required init?(coder: NSCoder) { fatalError() }

    override var canBecomeFirstResponder: Bool { true }
    override var inputAccessoryView: UIView? { accessory }
    var hasText: Bool { true }
    override func didMoveToWindow() { super.didMoveToWindow(); if window != nil { DispatchQueue.main.async { [weak self] in _ = self?.becomeFirstResponder() } } }
    override func layoutSubviews() {
        super.layoutSubviews()
        let g = grid(bounds.size)
        if bounds.width > 0, bounds.height > 0, (g.cols, g.rows) != (lastGrid.0, lastGrid.1) { lastGrid = (g.cols, g.rows); onGrid?(g.cols, g.rows) }
    }
    /// The grid is told again (the screen opened over a pty that may hold another size).
    func announceGrid() { let g = grid(bounds.size); if bounds.width > 0 { onGrid?(g.cols, g.rows) } }

    // MARK: typing
    private func send(_ b: [UInt8]) { if b.isEmpty { return }; scroll = 0; onBytes?(b) }
    private func key(_ sym: UInt32, _ text: UInt32 = 0) -> [UInt8] {
        let mods: UInt32 = ctrl ? 4 : 0; ctrl = false
        return vt()?.key(sym, text, mods) ?? []
    }
    func insertText(_ text: String) {
        var out: [UInt8] = []
        for s in text.unicodeScalars { out += s.value == 10 ? key(65293, 13) : key(0, s.value) }
        send(out)
    }
    func deleteBackward() { send(key(65288, 8)) }
    private func accessoryKey(_ id: String) {
        switch id {
        case "ctrl": ctrl.toggle()
        case "esc": send(key(65307, 27))
        case "tab": send(key(65289, 9))
        case "left": send(key(65361))
        case "up": send(key(65362))
        case "right": send(key(65363))
        case "down": send(key(65364))
        default: if let s = id.unicodeScalars.first { send(key(0, s.value)) }
        }
    }
    override func paste(_ sender: Any?) {
        guard var t = UIPasteboard.general.string, !t.isEmpty, let v = vt() else { return }
        if t.utf8.count > 200_000 { t = String(decoding: Array(t.utf8.prefix(200_000)), as: UTF8.self) }
        send(v.paste(t))
    }
    override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
        action == #selector(paste(_:)) ? UIPasteboard.general.hasStrings : false
    }
    /// A hardware keyboard: arrows, Escape, Tab, Return, Backspace and Ctrl with a letter; everything else comes through insertText.
    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        var rest = presses
        for p in presses {
            guard let k = p.key else { continue }
            let m = k.modifierFlags; let mods: UInt32 = (m.contains(.shift) ? 1 : 0) | (m.contains(.control) ? 4 : 0) | (m.contains(.alternate) ? 8 : 0)
            var sym: UInt32?
            switch k.keyCode {
            case .keyboardUpArrow: sym = 65362
            case .keyboardDownArrow: sym = 65364
            case .keyboardLeftArrow: sym = 65361
            case .keyboardRightArrow: sym = 65363
            case .keyboardEscape: sym = 65307
            case .keyboardTab: sym = 65289
            case .keyboardHome: sym = 65360
            case .keyboardEnd: sym = 65367
            case .keyboardPageUp: sym = 65365
            case .keyboardPageDown: sym = 65366
            case .keyboardDeleteForward: sym = 65535
            default: break
            }
            if let sym { send(vt()?.key(sym, 0, mods) ?? []); rest.remove(p) }
            else if m.contains(.control), let c = k.charactersIgnoringModifiers.unicodeScalars.first { send(vt()?.key(0, c.value, mods) ?? []); rest.remove(p) }
        }
        if !rest.isEmpty { super.pressesBegan(rest, with: event) }
    }

    // MARK: gestures
    @objc private func tapped() { _ = becomeFirstResponder() }
    @objc private func pinched(_ g: UIPinchGestureRecognizer) {
        if g.state == .began { pinchBase = fontSize }
        guard g.state == .changed || g.state == .ended else { return }
        let size = min(max((pinchBase * g.scale * 2).rounded() / 2, 7), 24)
        if size != fontSize { onFontSize?(size) }
    }
    @objc private func panned(_ g: UIPanGestureRecognizer) {
        guard let v = vt(), !v.alt else { return }
        if g.state == .began { panBase = scroll }
        let rows = Int((g.translation(in: self).y / lh).rounded())
        scroll = min(max(panBase + rows, 0), v.history)
    }
    @objc private func held(_ g: UILongPressGestureRecognizer) {
        guard g.state == .began, UIPasteboard.general.hasStrings else { return }
        menu?.presentEditMenu(with: UIEditMenuConfiguration(identifier: nil, sourcePoint: g.location(in: self)))
    }
    func editMenuInteraction(_ i: UIEditMenuInteraction, menuFor c: UIEditMenuConfiguration, suggestedActions: [UIMenuElement]) -> UIMenu? {
        UIMenu(children: [UIAction(title: "Paste") { [weak self] _ in self?.paste(nil) }])
    }
    func pasteFromButton() { paste(nil) }
    func scrollToBottom() { scroll = 0 }
}
