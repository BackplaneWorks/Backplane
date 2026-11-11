import Foundation

// A sections project row's menu, the hub's (native/host/src/present/sections.rs `project_menu`): Pin, New swarm, Rename, Badge, Open folder,
// Move to group (the other groups, then New group…), Archive, Remove project. The hub decides the items and their order; the app draws them.

/// One item: `id` says what it does here, `value` what it acts on, `sub` Move to group's list; `confirm` asks first (Remove).
struct DMenuItem: Decodable, Equatable, Identifiable {
    var id: String; var label: String; var action: String; var value: String; var confirm: Bool; var sub: [DMenuItem]
    enum CodingKeys: String, CodingKey { case id, label, action, value, confirm, sub }
    init(id: String, label: String, action: String = "", value: String = "", confirm: Bool = false, sub: [DMenuItem] = []) {
        self.id = id; self.label = label; self.action = action; self.value = value; self.confirm = confirm; self.sub = sub
    }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        func s(_ k: CodingKeys) -> String { (try? c.decode(String.self, forKey: k)) ?? "" }
        id = s(.id); label = s(.label); action = s(.action); value = s(.value)
        confirm = (try? c.decode(Bool.self, forKey: .confirm)) ?? false; sub = (try? c.decode([DMenuItem].self, forKey: .sub)) ?? []
    }
    /// What a phone's long press lists: the hub's menu without Open folder (the folder is on the hub's machine) and New swarm when swarms are off.
    static func phone(_ menu: [DMenuItem], swarms: Bool) -> [DMenuItem] { menu.filter { $0.id != "open" && (swarms || $0.id != "swarm") && !$0.label.isEmpty } }
}
