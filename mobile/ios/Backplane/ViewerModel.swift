import Foundation

// What the Metal viewer reads of a data/1 viewer: plain data, no decisions. (Everything else the old server-drawn screens carried is gone.)

struct Choice: Decodable, Hashable {
    let label, value: String
    let on: Bool
}

struct CardRow: Decodable, Hashable {
    let k, v: String
}

struct Card: Decodable {
    let info, title: String
    let rows: [CardRow]
}

struct SheetRow: Decodable, Hashable {
    let label, value: String
    let on, loop: Bool
    // its sheet symbol on the sheet on show ("x0 y0 x1 y1" mm), when it is
    // directly under it: a tap there opens it
    let box: String?
}

struct LayerRow: Decodable, Hashable {
    let layer: Int
    let name: String
    let on: Bool
    // the active layer of the focus
    let active: Bool?
}

struct ReviewPin: Decodable, Hashable {
    let id: String
    let x, y: Double
    let resolved: Bool
    let count: Int
}

struct ReviewComments: Decodable {
    let pins: [ReviewPin]
}

struct Viewer: Decodable {
    let open, key, layers: String
    let panes: [Viewer]?
    let choices: [Choice]
    let bg: UInt32
    let fade: Int
    let margin, zmin, zmax, tap: Float
    let top, bottom: [Int]
    let slab: UInt32
    let fov: Float
    let picked: String
    let card: Card?
    // the viewer's own light ground, and each layer's colour on it (by layer)
    let light: Bool?
    let look: [UInt32]?
    // the schematic's sheets ("view-sheet" value), the layers the user can
    // turn off ("view-layer" layer) and those off (a bit each), and
    // whether the 3D model shows its parts ("view-parts")
    let sheets: [SheetRow]?
    let layerList: [LayerRow]?
    let off: UInt32?
    // the layer focus: the layers drawn dimmed (a bit
    // each), the one drawn last (255 none), the list's All/Dim/Only (0-2)
    // and whether it has one (a board), and the Layers button's words
    let dim: UInt32?
    let lift: Int?
    let layerMode: Int?
    let layerModes: Bool?
    let layerHead: String?
    let parts: Bool?
    // what the hub says about the source (parts with no 3D model)
    let note: String?
    // the review comments' pins on the file on show
    let comments: ReviewComments?
}
