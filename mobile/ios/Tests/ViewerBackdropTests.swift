// The 3D viewers' background palette (ViewerBackdrop.swift) against docs/VIEWER-BACKGROUND.md. Compiled with ViewerBackdrop.swift alone,
// outside the app: scripts/mobile/ios-unit-tests.sh (run viewer-backdrop).
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }
typealias B = ViewerBackdrop

// auto follows the theme, live: the same setting resolves both ways
check(B.resolve("auto", custom: 0, dark: false) == B(top: 0xf5f6f8, bottom: 0xe1e4e9), "auto light", B.resolve("auto", custom: 0, dark: false))
check(B.resolve("auto", custom: 0, dark: true) == B(top: 0x4d535e, bottom: 0x353a43), "auto dark", B.resolve("auto", custom: 0, dark: true))
// light and dark ignore the theme
check(B.resolve("light", custom: 0x123456, dark: true) == .light, "light in a dark theme")
check(B.resolve("dark", custom: 0x123456, dark: false) == .dark, "dark in a light theme")
// custom: c on top, each channel × 0.88 rounded at the bottom
let c = B.resolve("custom", custom: 0xc8a000, dark: true)
check(c == B(top: 0xc8a000, bottom: 0xb08d00), "custom c8a000 (200*.88=176, 160*.88=140.8)", B.hex(c.bottom))
check(B.custom(0xffffff).bottom == 0xe0e0e0, "white × 0.88 = 224", B.hex(B.custom(0xffffff).bottom))
check(B.custom(0x000000).bottom == 0, "black stays black")
check(B.custom(0x8a94a6).bottom == 0x798292, "slate: 138→121.44→121, 148→130.24→130, 166→146.08→146", B.hex(B.custom(0x8a94a6).bottom))
check(B.custom(0x030303).bottom == 0x030303, "3 × 0.88 = 2.64 rounds to 3", B.hex(B.custom(0x030303).bottom))
check(B.custom(0xff123456).top == 0x123456, "only the low 24 bits", B.hex(B.custom(0xff123456).top))
// anything else is auto
check(B.resolve("neon", custom: 0, dark: true) == .dark && B.resolve("", custom: 0, dark: false) == .light, "unknown is auto")
// the stored hex
check(B.parse("#C8a000") == 0xc8a000 && B.parse("c8a000") == 0xc8a000, "parse")
check(B.parse("fff") == nil && B.parse("#zzzzzz") == nil && B.parse("#1234567") == nil && B.parse("") == nil, "parse refuses")
check(B.hex(0xc8a000) == "#c8a000" && B.parse(B.hex(0x0a0b0c)) == 0x0a0b0c, "hex round trip")
check(B.modes == ["auto", "light", "dark", "custom"], "the four choices")

print("viewer-backdrop: \(passed) passed, \(failed) failed")
exit(failed == 0 ? 0 : 1)
