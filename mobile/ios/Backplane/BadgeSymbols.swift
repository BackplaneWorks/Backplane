import Foundation

/// A project's icon as the hub names it (present/badge.rs ICONS, the desktop's icons.json) as an SF Symbol; any other name is the folder, so a mark is never blank.
/// Tests/BadgeSymbolsTests.swift checks every name of the hub's list has its own symbol and that each symbol exists.
func dataBadgeSymbol(_ icon: String) -> String {
    switch icon {
    case "circuit-board": "cpu.fill"
    case "cpu": "cpu"
    case "zap": "bolt"
    case "radio": "antenna.radiowaves.left.and.right"
    case "box": "shippingbox"
    case "layers": "square.3.layers.3d"
    case "code": "chevron.left.forwardslash.chevron.right"
    case "terminal": "terminal"
    case "globe": "globe"
    case "wrench": "wrench"
    case "flask-conical": "flask"
    case "book": "book"
    case "star": "star"
    case "usb": "cable.connector"
    case "plug": "powerplug"
    case "presentation": "play.rectangle"
    case "gauge": "gauge.with.dots.needle.50percent"
    case "file-text": "doc.text"
    case "image": "photo"
    default: "folder"
    }
}
