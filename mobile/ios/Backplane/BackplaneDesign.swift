import SwiftUI
import UIKit

// The same quiet surfaces and accent as the web client. Dynamic colours keep
// the device's Appearance setting effective on every screen.
enum BackplaneDesign {
    private static func color(_ light: UInt32, _ dark: UInt32) -> Color {
        Color(uiColor: UIColor { traits in
            let rgb = traits.userInterfaceStyle == .dark ? dark : light
            return UIColor(red: CGFloat((rgb >> 16) & 255) / 255,
                           green: CGFloat((rgb >> 8) & 255) / 255,
                           blue: CGFloat(rgb & 255) / 255, alpha: 1)
        })
    }
    static let background = color(0xfafaf8, 0x0d0f12)
    static let panel = color(0xf1f0ec, 0x13161b)
    static let raised = color(0xe8e7e2, 0x1a1e25)
    static let secondaryText = color(0x595b61, 0xb7bec9)
    static let line = color(0xd3d1ca, 0x262b34)
    static let accent = color(0x1f6fd1, 0x5fb3ff)
    static let onAccent = color(0xffffff, 0x06121f)
}

// Plain, square rows retain native List swipe and accessibility behaviour.
struct BackplaneListSurface: ViewModifier {
    func body(content: Content) -> some View {
        content.listStyle(.plain)
            .scrollContentBackground(.hidden)
            .background(BackplaneDesign.background)
    }
}
