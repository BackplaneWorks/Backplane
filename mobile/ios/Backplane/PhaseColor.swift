import SwiftUI

// The pairing screen's colours (the palette of a thread's phase).
enum PhaseColor {
    static func rgb(_ hex: UInt32) -> Color {
        Color(red: Double(hex >> 16 & 0xff) / 255, green: Double(hex >> 8 & 0xff) / 255, blue: Double(hex & 0xff) / 255)
    }
    static let accent = BackplaneDesign.accent
    static let ok = BackplaneDesign.color(0x1c8550, 0x58c28a)
    static let warn = BackplaneDesign.color(0x9a6700, 0xe3b341)
    static let bad = BackplaneDesign.color(0xc93434, 0xf06a6a)
    static let input = BackplaneDesign.color(0x7446d0, 0xb48cff)
    static let queued = BackplaneDesign.color(0x127c84, 0x4fc1c9)
    static let waiting = BackplaneDesign.color(0xa3399a, 0xe07bd0)
    static let faint = BackplaneDesign.color(0x72777f, 0x5c6473)
}
