import ActivityKit
import Foundation

// The Dynamic Island's content: exactly the "island" object
// src/mobile/notify emits, and the content-state the hub pushes.
struct IslandAttributes: ActivityAttributes {
    // the hub this activity belongs to (Pairing.key): each paired hub keeps
    // its own, and only that hub pushes to it. "" on an activity started
    // before there was one per hub
    var hub: String = ""

    init(hub: String) { self.hub = hub }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        hub = try c.decodeIfPresent(String.self, forKey: .hub) ?? ""
    }

    private enum Keys: String, CodingKey { case hub }

    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Keys.self)
        try c.encode(hub, forKey: .hub)
    }

    struct Line: Codable, Hashable {
        let thread: String
        let title: String
        let doing: String
    }

    struct ContentState: Codable, Hashable {
        let running: Int
        let headline: String
        let lines: [Line]
    }
}
