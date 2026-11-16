import SwiftUI

// Feature `cats` (docs/PHONE-DATA.md): a bot's cat in the lists and Settings' "Your uberbot". The hub draws nothing here: it answers each
// rig (`cat.rig`, already tinted, the look a name gives worked out by it), and this keeps them by (look, colour, mood), which never change.

struct DCatPreset: Decodable, Hashable { var name: String; var color: String }
struct DCatRig: Decodable {
    var look: Int; var color: String; var parts: [CatPart]; var presets: [DCatPreset]
    enum CodingKeys: String, CodingKey { case look, color, parts, presets }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        look = c.int(.look); color = c.str(.color)
        parts = (try? c.decode([CatPart].self, forKey: .parts)) ?? []; presets = (try? c.decode([DCatPreset].self, forKey: .presets)) ?? []
    }
}

@MainActor
@Observable
final class CatStore {
    private(set) var rigs: [String: [CatPart]] = [:]
    /// Settings' colour row, from the last rig read.
    private(set) var presets: [DCatPreset] = []
    /// The look each name gives (lower case: the hub's hash ignores case), from `cat.rig {name}`.
    private(set) var nameLooks: [String: Int] = [:]
    @ObservationIgnored weak var client: DataClient?
    @ObservationIgnored private var asking: Set<String> = []

    func rig(look: Int, color: String, mood: String = "idle") -> [CatPart]? { rigs[CatPick.key(look: look, color: color, mood: mood)] }
    /// A bot's rig, read once.
    func want(look: Int, color: String, mood: String = "idle") async {
        let k = CatPick.key(look: look, color: color, mood: mood)
        guard rigs[k] == nil, !asking.contains(k) else { return }
        asking.insert(k); defer { asking.remove(k) }
        _ = await ask(["look": look, "color": color, "mood": mood])
    }
    /// `cat.rig` with these parameters; the answer is kept under its own look and colour (and its name's look, when asked by name).
    func ask(_ p: [String: Any]) async -> DCatRig? {
        guard let client, client.has("cats"), let r: DCatRig = try? await client.call("cat.rig", p) else { return nil }
        let mood = p["mood"] as? String ?? "idle"
        rigs[CatPick.key(look: r.look, color: r.color, mood: mood)] = r.parts
        if let n = p["name"] as? String { nameLooks[n.lowercased()] = r.look }
        if !r.presets.isEmpty, r.presets != presets { presets = r.presets }
        return r
    }
    /// The look a name gives.
    func lookOf(_ name: String) async -> Int? {
        if let l = nameLooks[name.lowercased()] { return l }
        return await ask(["name": name, "mood": "idle"])?.look
    }
}

/// A bot's cat at `size`, from its `bots.list` row; nothing until its rig is in.
struct DBotCat: View {
    let client: DataClient
    let look: Int
    let color: String
    let size: CGFloat
    var body: some View {
        Group {
            if let r = client.cats.rig(look: look, color: color) { CatView(rig: r, size: size) } else { Color.clear.frame(width: size, height: size) }
        }
        .accessibilityHidden(true)
        .task(id: CatPick.key(look: look, color: color, mood: "idle")) { await client.cats.want(look: look, color: color) }
    }
}

/// Settings, Agents: "Your uberbot". Its name with a live cat beside it, the colours, Different cat and The name's cat; each change goes to
/// `bot.cat` and the bots are read again.
struct DataUberSection: View {
    @Bindable var client: DataClient
    @State private var typed = ""
    @State private var custom = ""
    @State private var err = ""
    @State private var nameLook: Int?
    @State private var shown: [CatPart] = []
    @State private var shownKey = ""
    @State private var busy = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var uber: DBot? { client.bots.first { $0.uber } }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            DHeading(text: Tr.t("Your uberbot"))
            if let b = uber { controls(b) } else { Text(Tr.t("No uberbot")).dmono(12).foregroundStyle(BackplaneDesign.dim).accessibilityIdentifier("settings.uber.none") }
        }
    }

    @ViewBuilder private func controls(_ b: DBot) -> some View {
        let picked = CatPick.picked(rowLook: b.look, nameLook: nameLook)
        HStack(alignment: .center, spacing: 12) {
            ZStack {
                if !shown.isEmpty { CatView(rig: shown, size: 72).id(shownKey).transition(.opacity) }
            }.frame(width: 72, height: 72).accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 6) {
                TextField(Tr.t("Name"), text: $typed).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled()
                    .padding(.horizontal, 10).frame(height: 38).dbox().accessibilityIdentifier("settings.uber.name")
                    .onSubmit { save(b) }
                Button(Tr.t("Save")) { save(b) }.buttonStyle(DButton(primary: true))
                    .disabled(busy || typed.trimmingCharacters(in: .whitespaces).isEmpty || typed.trimmingCharacters(in: .whitespaces) == b.name)
                    .accessibilityIdentifier("settings.uber.save")
            }
        }
        if !err.isEmpty { Text(err).dmono(11).foregroundStyle(BackplaneDesign.bad).accessibilityIdentifier("settings.uber.err") }
        DHeading(text: Tr.t("Colour"))
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                Button(Tr.t("Its own")) { send(["color": ""]) }.buttonStyle(DButton(primary: b.color.isEmpty, compact: true))
                    .accessibilityAddTraits(b.color.isEmpty ? .isSelected : []).accessibilityIdentifier("settings.uber.color.own")
                ForEach(client.cats.presets, id: \.self) { p in
                    let on = CatPick.hex(p.color) == b.color
                    Button { send(["color": p.color]) } label: {
                        Rectangle().fill(Color(css: p.color) ?? BackplaneDesign.raised).frame(width: 28, height: 28)
                            .padding(3).overlay(Rectangle().strokeBorder(on ? BackplaneDesign.accent : Color.clear, lineWidth: 2))
                    }.buttonStyle(.plain).accessibilityLabel(p.name).accessibilityAddTraits(on ? .isSelected : []).accessibilityIdentifier("settings.uber.color." + p.name)
                }
            }
        }
        HStack(spacing: 6) {
            TextField(Tr.t("Custom"), text: $custom).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled()
                .padding(.horizontal, 10).frame(height: 38).dbox().accessibilityIdentifier("settings.uber.custom")
            Button(Tr.t("Use")) {
                guard let c = CatPick.hex(custom) else { err = Tr.t("Type a colour like #6fb3f2."); return }
                send(["color": c])
            }.buttonStyle(DButton()).disabled(busy || custom.isEmpty).accessibilityIdentifier("settings.uber.custom.use")
        }
        DHeading(text: Tr.t("Cat"))
        HStack(spacing: 6) {
            Button(Tr.t("Different cat")) { send(["shuffle": true]) }.buttonStyle(DButton()).accessibilityIdentifier("settings.uber.shuffle")
            Button(Tr.t("The name's cat")) { send(["auto": true]) }.buttonStyle(DButton(primary: false)).disabled(!picked).accessibilityIdentifier("settings.uber.auto")
        }.disabled(busy)
        // the name as the hub has it fills the field (again after a save or a rename elsewhere); the look it gives says whether the look was picked
        Color.clear.frame(height: 0)
            .task(id: b.name) { typed = b.name; nameLook = nil; nameLook = await client.cats.lookOf(b.name) }
            // the preview: a moment after the last key, the cat the name typed gives (or the picked one), faded in
            .task(id: "\(typed)|\(b.color)|\(b.look)|\(picked)") {
                if !shown.isEmpty { try? await Task.sleep(for: .milliseconds(120)) }
                guard !Task.isCancelled, let r = await client.cats.ask(CatPick.ask(typed: typed, color: b.color, rowLook: b.look, picked: picked)), !Task.isCancelled else { return }
                let k = CatPick.key(look: r.look, color: r.color, mood: "idle")
                guard k != shownKey else { return }
                if reduceMotion || shown.isEmpty { shown = r.parts; shownKey = k } else { withAnimation(.easeInOut(duration: 0.15)) { shown = r.parts; shownKey = k } }
            }
    }

    private func save(_ b: DBot) {
        let n = typed.trimmingCharacters(in: .whitespaces)
        guard !n.isEmpty, n != b.name else { return }
        send(["name": n])
    }
    private func send(_ p: [String: Any]) {
        guard !busy else { return }
        busy = true
        Task {
            do { _ = try await client.call("bot.cat", p, as: DEmpty.self); err = ""; if p["color"] != nil { custom = "" } }
            catch { err = error.localizedDescription }
            await client.loadBots()
            busy = false
        }
    }
}
