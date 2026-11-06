import SwiftUI
import UIKit
import CoreText

// JetBrains Mono 2.304 (SIL OFL 1.1, assets/fonts/jetbrains-mono/), the app's face for code blocks, inline code and
// mono labels, with ligatures and contextual alternates off. Body text stays the system font. CJK and other scripts fall
// back to the system font (the face has none). The OFL notice is shown under Settings > Licences.
enum Mono {
    static let regular = "JetBrainsMono-Regular"
    static let bold = "JetBrainsMono-Bold"

    // the bundled .ttf files, registered for this process once at launch
    static func register() {
        for sub in [nil, "Fonts"] as [String?] {
            for url in Bundle.main.urls(forResourcesWithExtension: "ttf", subdirectory: sub) ?? [] {
                CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
            }
        }
    }

    static func uiFont(size: CGFloat, weight: UIFont.Weight = .regular) -> UIFont {
        let name = weight.rawValue >= UIFont.Weight.semibold.rawValue ? bold : regular
        // OpenType ligatures (liga) and contextual alternates (calt) off
        let off: [[UIFontDescriptor.FeatureKey: Int]] = [
            [.type: kLigaturesType, .selector: kCommonLigaturesOffSelector],
            [.type: kContextualAlternatesType, .selector: kContextualAlternatesOffSelector],
        ]
        let d = UIFontDescriptor(fontAttributes: [.name: name, .featureSettings: off])
        let f = UIFont(descriptor: d, size: size)
        // not registered (a test host): the system's mono face, so nothing renders blank
        return f.familyName.hasPrefix("JetBrains") ? f : UIFont.monospacedSystemFont(ofSize: size, weight: weight)
    }

    static func uiStyle(_ s: Font.TextStyle) -> UIFont.TextStyle {
        switch s {
        case .largeTitle: .largeTitle
        case .title: .title1
        case .title2: .title2
        case .title3: .title3
        case .headline: .headline
        case .subheadline: .subheadline
        case .callout: .callout
        case .footnote: .footnote
        case .caption: .caption1
        case .caption2: .caption2
        default: .body
        }
    }
}

extension Font {
    // JetBrains Mono at a text style's size, scaled with Dynamic Type
    static func mono(_ style: Font.TextStyle, weight: UIFont.Weight = .regular) -> Font {
        let ui = Mono.uiStyle(style)
        let base = UIFont.preferredFont(forTextStyle: ui).pointSize
        return Font(UIFontMetrics(forTextStyle: ui).scaledFont(for: Mono.uiFont(size: base, weight: weight)))
    }
    static func mono(size: CGFloat, weight: UIFont.Weight = .regular) -> Font { Font(Mono.uiFont(size: size, weight: weight)) }
}

// Settings > Licences: what the app ships that asks for a notice
struct LicencesView: View {
    private var ofl: String {
        for sub in [nil, "Fonts"] as [String?] {
            if let u = Bundle.main.url(forResource: "OFL", withExtension: "txt", subdirectory: sub),
               let s = try? String(contentsOf: u, encoding: .utf8) { return s }
        }
        return "SIL Open Font License 1.1: https://openfontlicense.org"
    }
    var body: some View {
        List {
            Section("JetBrains Mono 2.304") {
                Text("Code blocks, inline code and mono labels. Copyright 2020 The JetBrains Mono Project Authors (https://github.com/JetBrains/JetBrainsMono). Licensed under the SIL Open Font License 1.1.")
                    .font(.footnote)
                Text(ofl).font(.mono(.caption2)).textSelection(.enabled)
            }
        }
        .navigationTitle("Licences")
        .navigationBarTitleDisplayMode(.inline)
    }
}

#if DEBUG
// BACKPLANE_ACTS "@fonts": the font probe for test/visual/check_fonts_ios.py: five lines at x=16, y=120+24i (below the status bar), 13 pt,
// line 4 bold, the screen's own background (the same lines as the desktop's `--fonts`)
struct FontsProbe: View {
    @Environment(\.colorScheme) private var scheme
    private static let lines = ["The quick brown fox 0123456789 {}[]()<>", "=> != -> == <= >= :: ... |> ++ www",
                                "┌─┬─┐│ ├─┼─┤ └─┴─┘ → ← ✓ ⌘", "日本語 中文 fallback", "Bold: The quick brown fox"]
    var body: some View {
        ZStack(alignment: .topLeading) {
            (scheme == .dark ? Color(red: 0x0d / 255, green: 0x0f / 255, blue: 0x12 / 255) : Color(red: 0xfa / 255, green: 0xfa / 255, blue: 0xf8 / 255))
            VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(Self.lines.enumerated()), id: \.offset) { i, t in
                    Text(t).font(Font(Mono.uiFont(size: 13, weight: i == 4 ? .bold : .regular)))
                        .foregroundStyle(scheme == .dark ? Color(red: 0xd9 / 255, green: 0xdd / 255, blue: 0xe4 / 255) : Color(red: 0x1b / 255, green: 0x1d / 255, blue: 0x21 / 255))
                        .lineLimit(1).fixedSize().frame(height: 24, alignment: .leading)
                }
            }
            .padding(.leading, 16).padding(.top, 120)
        }
        .ignoresSafeArea()
    }
}
#endif
