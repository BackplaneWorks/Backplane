import Foundation

/// The 3D viewers' background (Mech and the 3D board share it), docs/VIEWER-BACKGROUND.md:
/// a vertical gradient, top to bottom, in sRGB bytes 0xrrggbb. Pure, so the test compiles it alone.
struct ViewerBackdrop: Equatable {
    var top: UInt32
    var bottom: UInt32

    static let light = ViewerBackdrop(top: 0xf5f6f8, bottom: 0xe1e4e9)
    static let dark = ViewerBackdrop(top: 0x4d535e, bottom: 0x353a43)
    /// the choices, in the Settings order
    static let modes = ["auto", "light", "dark", "custom"]
    /// the custom colour before one is chosen
    static let defaultCustom: UInt32 = 0x8a94a6

    /// a custom colour c: c at the top, each channel × 0.88 (rounded) at the bottom
    static func custom(_ c: UInt32) -> ViewerBackdrop {
        let c = c & 0xffffff
        let b = [16, 8, 0].reduce(UInt32(0)) { a, sh in a | UInt32((Double((c >> UInt32(sh)) & 255) * 0.88).rounded()) << UInt32(sh) }
        return ViewerBackdrop(top: c, bottom: b)
    }

    /// `mode`: auto | light | dark | custom (anything else is auto); `dark`: the app's resolved theme now
    static func resolve(_ mode: String, custom c: UInt32, dark: Bool) -> ViewerBackdrop {
        switch mode {
        case "light": .light
        case "dark": .dark
        case "custom": custom(c)
        default: dark ? .dark : .light
        }
    }

    /// "#rrggbb" or "rrggbb" to 0xrrggbb; nil for anything else
    static func parse(_ s: String) -> UInt32? {
        let h = s.hasPrefix("#") ? String(s.dropFirst()) : s
        guard h.count == 6, h.allSatisfy(\.isHexDigit) else { return nil }
        return UInt32(h, radix: 16)
    }

    static func hex(_ c: UInt32) -> String { String(format: "#%06x", c & 0xffffff) }

    /// a colour's channels, 0 to 1 (sRGB, not decoded)
    static func unit(_ c: UInt32) -> (Float, Float, Float) {
        (Float((c >> 16) & 255) / 255, Float((c >> 8) & 255) / 255, Float(c & 255) / 255)
    }
}
