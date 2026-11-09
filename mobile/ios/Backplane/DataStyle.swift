import SwiftUI

// The data app's pieces in the desktop client's language (native/gpui, assets/web/style.css): mono type, 1 px lines, square corners, flat panels, one accent.
// The same as the Android app's DBar, DBtn, DField, DDot and DLine (DataViews.kt).

private struct DMonoFont: ViewModifier {
    @ScaledMetric private var size: CGFloat
    private let bold: Bool
    init(size: CGFloat, bold: Bool) { _size = ScaledMetric(wrappedValue: size, relativeTo: .footnote); self.bold = bold }
    func body(content: Content) -> some View { content.font(.mono(size: size, weight: bold ? .bold : .regular)) }
}

extension View {
    /// JetBrains Mono at a desktop size, scaled with this phone's text size.
    func dmono(_ size: CGFloat = 13, bold: Bool = false) -> some View { modifier(DMonoFont(size: size, bold: bold)) }

    /// A screen's bar: the panel colour, its title in mono, a square back arrow (`back: false` for the first screen of a sheet).
    func dataBar(_ title: String, back: Bool = true) -> some View { modifier(DBar(title: title, back: back)) }

    /// A field: a 1 px box on the view colour.
    func dbox(_ line: Color = BackplaneDesign.line, fill: Color = BackplaneDesign.view) -> some View {
        background(fill).overlay(Rectangle().strokeBorder(line, lineWidth: 1))
    }

    /// A sheet: square, on the panel colour.
    func dataSheet() -> some View { presentationBackground(BackplaneDesign.panel).presentationCornerRadius(0) }

    /// A row of a flat list: no insets, no separator, its own background.
    func dataRow(_ fill: Color = BackplaneDesign.panel) -> some View { listRowInsets(EdgeInsets()).listRowSeparator(.hidden).listRowBackground(fill) }
}

private struct DBar: ViewModifier {
    let title: String
    let back: Bool
    @Environment(\.dismiss) private var dismiss
    func body(content: Content) -> some View {
        content.navigationBarTitleDisplayMode(.inline).navigationBarBackButtonHidden(back)
            .toolbarBackground(BackplaneDesign.panel, for: .navigationBar).toolbarBackground(.visible, for: .navigationBar)
            .toolbar {
                if back {
                    ToolbarItem(placement: .topBarLeading) {
                        Button { dismiss() } label: { Image(systemName: "chevron.left").foregroundStyle(BackplaneDesign.dim) }.accessibilityLabel(Tr.t("Back")).accessibilityIdentifier("data.back")
                    }.dflat()
                }
                ToolbarItem(placement: .principal) { Text(title).dmono(14, bold: true).foregroundStyle(BackplaneDesign.text).lineLimit(1) }.dflat()
            }
    }
}

extension ToolbarContent {
    /// The bar's items without iOS 26's rounded glass behind them: the app's corners are square.
    @ToolbarContentBuilder func dflat() -> some ToolbarContent {
        if #available(iOS 26, *) { sharedBackgroundVisibility(.hidden) } else { self }
    }
}

// the system's back arrow is hidden on every screen (DBar), which also takes the edge swipe away: this keeps it
extension UINavigationController: @retroactive UIGestureRecognizerDelegate {
    override open func viewDidLoad() { super.viewDidLoad(); interactivePopGestureRecognizer?.delegate = self }
    public func gestureRecognizerShouldBegin(_ g: UIGestureRecognizer) -> Bool { viewControllers.count > 1 }
}

struct DDot: View {
    let color: Color
    var size: CGFloat = 6
    var body: some View { Rectangle().fill(color).frame(width: size, height: size) }
}

struct DLine: View {
    var body: some View { Rectangle().fill(BackplaneDesign.line).frame(height: 1) }
}

/// A thread's activity, one colour each (native/gpui/src/ui.rs `side_thread`): green working; red wanting the person or failed; grey otherwise.
func dStateColor(_ state: String) -> Color {
    switch state { case "working": BackplaneDesign.ok; case "asking", "failed": BackplaneDesign.bad; default: BackplaneDesign.faint }
}
/// A row's dot as the desktop's sidebar draws it (gpui siderow.rs `dot_of`) from the hub's word (`status`): finished is the green dot; an older hub sends none and the turn's state decides.
struct DStatusDot: View {
    let thread: DThread
    var size: CGFloat = 7
    /// colour, round, hollow
    private var look: (Color, Bool, Bool) {
        switch thread.status {
        case "blocked": return (BackplaneDesign.bad, false, false)
        case "failed": return (BackplaneDesign.bad, false, true)
        case "stalled": return (BackplaneDesign.warn, false, false)
        case "active", "working": return (BackplaneDesign.accent, true, false)
        case "waiting", "monitoring": return (BackplaneDesign.accent.opacity(0.55), true, true)
        case "finished": return (BackplaneDesign.ok, true, false)
        case "":
            switch thread.state {
            case "working": return (BackplaneDesign.accent, true, false)
            case "asking": return (BackplaneDesign.bad, false, false)
            case "failed": return (BackplaneDesign.bad, false, true)
            default: return (BackplaneDesign.faint, true, false)
            }
        default: return (BackplaneDesign.faint, true, false)
        }
    }
    var body: some View {
        let l = look
        Group {
            if l.1 { if l.2 { Circle().stroke(l.0, lineWidth: 1.5) } else { Circle().fill(l.0) } }
            else { if l.2 { Rectangle().stroke(l.0, lineWidth: 1.5) } else { Rectangle().fill(l.0) } }
        }.frame(width: size, height: size)
    }
}
/// The STATE strip's word: the hub's when it sends one, else the turn's.
func dStatusWord(_ t: DThread) -> String { switch t.status { case "": dStateWord(t.state); case "active": "working"; case "blocked": "needs you"; default: t.status } }
func dStateWord(_ state: String) -> String {
    switch state { case "working": "working"; case "asking": "needs you"; case "failed": "failed"; default: "inactive" }
}
func dAgo(_ ms: Int) -> String {
    let s = max(0, Int(Date().timeIntervalSince1970) - ms / 1000)
    if ms <= 0 { return "" }; if s < 60 { return Tr.k("ios.ago.now") }; if s < 3600 { return Tr.ios_ago_m(n: String(s / 60)) }; if s < 86400 { return Tr.ios_ago_h(n: String(s / 3600)) }; return Tr.ios_ago_d(n: String(s / 86400))
}

/// The desktop's buttons: a solid accent one for the action of a screen, the rest a line on the raised colour.
struct DButton: ButtonStyle {
    var primary = false
    var danger = false
    @Environment(\.isEnabled) private var enabled
    func makeBody(configuration: Configuration) -> some View {
        let solid = primary && enabled
        configuration.label.dmono(13, bold: true).lineLimit(1)
            .foregroundStyle(solid ? BackplaneDesign.onAccent : !enabled ? BackplaneDesign.faint : danger ? BackplaneDesign.bad : BackplaneDesign.text)
            .padding(.horizontal, 14).frame(minHeight: 40)
            .background(solid ? BackplaneDesign.accent : BackplaneDesign.raised)
            .overlay(Rectangle().strokeBorder(solid ? BackplaneDesign.accent : BackplaneDesign.line, lineWidth: 1))
            .opacity(configuration.isPressed ? 0.7 : 1).contentShape(Rectangle())
    }
}

/// Uppercase and faint, the desktop's small headings ("BOTS", "STATE", "MODEL").
struct DHeading: View {
    let text: String
    var body: some View { Text(text.uppercased()).dmono(11).foregroundStyle(BackplaneDesign.faint) }
}

/// One of a few choices, side by side: the chosen one in accent.
struct DChoices<V: Hashable>: View {
    let options: [(label: String, value: V)]
    @Binding var value: V
    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(options, id: \.value) { o in
                    Button(o.label) { value = o.value }.buttonStyle(DButton(primary: o.value == value)).accessibilityAddTraits(o.value == value ? .isSelected : [])
                }
            }
        }
    }
}

/// The working square: breathes while the turn runs (the one animation on screen, and only while this row is).
struct DWorkingRow: View {
    @State private var up = false
    var body: some View {
        HStack(spacing: 8) {
            ZStack { Rectangle().fill(BackplaneDesign.ok).frame(width: up ? 12 : 5, height: up ? 12 : 5).opacity(up ? 1 : 0.45) }.frame(width: 12, height: 12)
            Text(Tr.t("Working…")).dmono(13).foregroundStyle(BackplaneDesign.dim)
        }
        .padding(.vertical, 6)
        .onAppear { withAnimation(.easeInOut(duration: 0.55).repeatForever(autoreverses: true)) { up = true } }
        .accessibilityIdentifier("data.thinking")
    }
}
