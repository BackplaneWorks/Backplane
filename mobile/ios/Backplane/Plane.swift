import SwiftUI

// The status plane (docs/uber.md): the uberbot's card, and what waits on
// the person, what runs, the bots, rooms and routines under it, on the home
// list and as the uberbot's Status tab. All of it is the screen's
// (src/mobile/plane.bend, merged over hubs by hubs.bend); these views only
// draw it and send each row's act and value back.

// the uberbot's card: a bot's row with the plane's counts
struct UberCard: Decodable {
    let nav: String?
    let id, name, mood, note, cat: String
    let counts: String?
    let look: Int
    let sel: Bool
}

struct PlaneRow: Decodable, Identifiable {
    let nav: String?
    let kind, rid, act, value, title, sub, state, ago: String
    let machine: String?
    let away: Bool
    var id: String { kind + ":" + rid }

    private enum CodingKeys: String, CodingKey {
        case nav, kind, rid = "id", act, value, title, sub, state, ago, machine, away
    }
}

struct PlaneMachine: Decodable, Hashable {
    let name, state: String
}

// where the uberbot's conversation is held; Bend says whether to offer
// "Make this the home" (button), and what a tap sends (act, value)
struct PlaneHome: Decodable {
    let name, line, label, act, value: String
    let here, button: Bool
}

struct PlaneModel: Decodable {
    let need, run, idle, bots, rooms, routines: [PlaneRow]
    let locks: [PlaneRow]?
    let idleN, working: Int
    let machines: [PlaneMachine]
    let home: PlaneHome?
}

// a thread row's dot, from the plane's states
private func planeStatus(_ s: String) -> String {
    switch s {
    case "question": "input"
    case "finished": "complete"
    case "away": "idle"
    default: s
    }
}

@MainActor private func tap(_ model: AppModel, _ r: PlaneRow) {
    if let nav = r.nav, !nav.isEmpty {
        model.open(nav: nav, action: r.act, value: r.value, title: r.title, kind: r.kind)
    } else if r.kind == "thread" {
        model.open(nav: r.value, action: r.act, value: r.value, title: r.title)
    } else { model.act(r.act, r.value) }
}

// The home list's card
// --------------------

struct UberCardRow: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let card: UberCard

    var body: some View {
        Button { model.open(nav: card.nav ?? "", action: "uber", value: card.id, title: card.name, cat: card.cat, kind: "bot") } label: {
            HStack(spacing: 12) {
                BotCat(model: model, key: card.cat, size: 40)
                VStack(alignment: .leading, spacing: 2) {
                    Text(card.name).font(.headline).foregroundStyle(.primary)
                    HStack(spacing: 5) {
                        MoodDot(mood: card.mood)
                        Text((card.counts ?? "").isEmpty ? NativeMessages.native_plane_waiting(lang) : card.counts ?? "").lineLimit(1)
                    }
                    .font(.caption)
                    .foregroundStyle(.secondary)
                }
                Spacer()
                Image(systemName: "chevron.right").font(.caption).foregroundStyle(.tertiary)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .listRowBackground(Color.accentColor.opacity(card.sel ? 0.22 : 0.10))
        .accessibilityLabel(NativeMessages.native_plane_card(lang, card.name, card.counts ?? ""))
        .accessibilityIdentifier("home.uber." + card.id)
    }
}

// Rows
// ----

private struct PlaneThread: View {
    let model: AppModel
    let row: PlaneRow

    var body: some View {
        Button { tap(model, row) } label: {
            HStack(spacing: 10) {
                StatusDot(state: "", status: planeStatus(row.state)).frame(width: 18)
                VStack(alignment: .leading, spacing: 1) {
                    Text(row.title).foregroundStyle(.primary).lineLimit(1)
                    let line = [row.sub, row.machine ?? ""].filter { !$0.isEmpty }.joined(separator: " · ")
                    if !line.isEmpty { Text(line).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
                }
                Spacer()
                Text(row.ago).font(.caption).foregroundStyle(BackplaneDesign.secondaryText)
                    .accessibilityIdentifier("home.row.age")
                Image(systemName: "chevron.right").font(.caption.weight(.semibold)).foregroundStyle(.tertiary)
            }
        }
        .buttonStyle(.plain)
        .opacity(row.away ? 0.5 : 1)
    }
}

// a bot at work or waiting in the running sections: its cat's mood, its work
private struct PlaneBot: View {
    let model: AppModel
    let row: PlaneRow
    // in the Bots section its state is a mood; among the threads, a phase
    let mood: Bool

    var body: some View {
        Button { tap(model, row) } label: {
            HStack(spacing: 10) {
                Group {
                    if mood { MoodDot(mood: row.state) } else { StatusDot(state: "", status: planeStatus(row.state)) }
                }
                .frame(width: 18)
                VStack(alignment: .leading, spacing: 1) {
                    Text(row.title).foregroundStyle(.primary).lineLimit(1)
                    let line = [row.sub, row.machine ?? ""].filter { !$0.isEmpty }.joined(separator: " · ")
                    if !line.isEmpty { Text(line).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
                }
                Spacer()
                if !row.ago.isEmpty {
                    Text(row.ago).font(.caption).foregroundStyle(BackplaneDesign.secondaryText)
                        .accessibilityIdentifier("home.row.age")
                }
                Image(systemName: "chevron.right").font(.caption.weight(.semibold)).foregroundStyle(.tertiary)
            }
        }
        .buttonStyle(.plain)
        .opacity(row.away ? 0.5 : 1)
    }
}

private struct PlaneRoom: View {
    let model: AppModel
    let row: PlaneRow

    var body: some View {
        Button { tap(model, row) } label: {
            HStack(spacing: 10) {
                Image(systemName: "bubble.left.and.bubble.right").font(.caption).foregroundStyle(.secondary).frame(width: 18)
                Text(row.title).foregroundStyle(.primary).lineLimit(1)
                Spacer()
                Text(row.sub).font(.caption.weight(.semibold)).foregroundStyle(PhaseColor.accent)
                Image(systemName: "chevron.right").font(.caption.weight(.semibold)).foregroundStyle(.tertiary)
            }
        }
        .buttonStyle(.plain)
    }
}

private struct PlaneRoutine: View {
    let model: AppModel
    let row: PlaneRow

    var body: some View {
        Button { tap(model, row) } label: {
            HStack(spacing: 10) {
                Group {
                    if row.state == "working" { ProgressView().controlSize(.mini).tint(PhaseColor.accent) }
                    else { Image(systemName: row.state == "reminder" ? "bell" : "clock").font(.caption).foregroundStyle(.secondary) }
                }
                .frame(width: 18)
                VStack(alignment: .leading, spacing: 1) {
                    Text(row.title).foregroundStyle(.primary).lineLimit(1)
                    let line = [row.sub, row.machine ?? ""].filter { !$0.isEmpty }.joined(separator: " · ")
                    if !line.isEmpty { Text(line).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
                }
                Spacer()
                Image(systemName: "chevron.right").font(.caption.weight(.semibold)).foregroundStyle(.tertiary)
            }
        }
        .buttonStyle(.plain)
        .opacity(row.away ? 0.5 : 1)
    }
}

private struct PlaneAny: View {
    let model: AppModel
    let row: PlaneRow
    var mood = false

    var body: some View {
        switch row.kind {
        case "bot": PlaneBot(model: model, row: row, mood: mood)
        case "room": PlaneRoom(model: model, row: row)
        case "routine": PlaneRoutine(model: model, row: row)
        default: PlaneThread(model: model, row: row)
        }
    }
}

// a section label: small, dim, capitals, with its count
private struct PlaneLabel: View {
    let text: String
    let count: Int?

    var body: some View {
        HStack(spacing: 6) {
            Text(text).accessibilityIdentifier("home.section.title")
            if let c = count { Text("\(c)").monospacedDigit().accessibilityIdentifier("home.section.count") }
        }
        .font(.caption.weight(.semibold))
        .textCase(.uppercase)
        .foregroundStyle(BackplaneDesign.secondaryText)
    }
}

// Sections
// --------

// the plane as list sections. On the home list (full false) it is what
// waits and what runs, then the bots, rooms and routines compactly; on the
// Status tab (full true) the idle threads follow too.
struct PlaneSections: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let plane: PlaneModel
    let full: Bool
    @State private var idleOpen = true

    private func section(_ name: String, _ rows: [PlaneRow], mood: Bool = false) -> some View {
        Section {
            ForEach(rows) { PlaneAny(model: model, row: $0, mood: mood) }
        } header: {
            PlaneLabel(text: name, count: rows.count)
        }
    }

    var body: some View {
        if !plane.need.isEmpty { section(NativeMessages.plane_needs(lang), plane.need) }
        if !plane.run.isEmpty { section(NativeMessages.plane_running(lang), plane.run) }
        if full && plane.idleN > 0 {
            Section {
                if idleOpen { ForEach(plane.idle) { PlaneAny(model: model, row: $0) } }
                if idleOpen && plane.idleN > plane.idle.count {
                    Text(NativeMessages.native_plane_more(lang, String(plane.idleN - plane.idle.count))).font(.caption).foregroundStyle(.secondary)
                }
            } header: {
                Button { idleOpen.toggle() } label: {
                    HStack(spacing: 6) {
                        PlaneLabel(text: NativeMessages.web_plane_idle(lang), count: plane.idleN)
                        Image(systemName: idleOpen ? "chevron.down" : "chevron.right").font(.caption2).foregroundStyle(.tertiary)
                    }
                }
            }
        }
        // the home list has its own Bots section (the cats)
        if full && !plane.bots.isEmpty { section(NativeMessages.tour_title_bots(lang), plane.bots, mood: true) }
        if !plane.rooms.isEmpty { section(NativeMessages.bots_rooms(lang), plane.rooms) }
        if let locks = plane.locks, !locks.isEmpty { section(NativeMessages.plane_locks(lang), locks) }
        if !plane.routines.isEmpty { section(NativeMessages.bots_tab_routines(lang), plane.routines) }
        if full && plane.need.isEmpty && plane.run.isEmpty && plane.idleN == 0 && plane.bots.isEmpty && plane.rooms.isEmpty && plane.routines.isEmpty {
            Section { Text(NativeMessages.sidebar_nothing(lang)).foregroundStyle(.secondary) }
        }
    }
}

// The Status tab
// --------------

// counters, then the machines and where the conversation is held, in one card
private struct PlaneStrip: View {
    @Environment(\.backplaneLanguage) private var lang
    @Environment(\.dynamicTypeSize) private var typeSize
    @Environment(\.colorScheme) private var colorScheme
    let model: AppModel
    let plane: PlaneModel

    private func counter(_ n: Int, _ label: String, _ tint: Color, id: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text("\(n)").font(.title2.weight(.semibold)).monospacedDigit()
                .foregroundStyle(n > 0 ? metricTint(tint, id: id) : BackplaneDesign.secondaryText)
                .accessibilityIdentifier("home.metric.value." + id)
            Text(label).font(.caption2).textCase(.uppercase).foregroundStyle(BackplaneDesign.secondaryText)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityIdentifier("home.metric.label." + id)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func metricTint(_ dark: Color, id: String) -> Color {
        guard colorScheme != .dark else { return dark }
        // The phase hues stay recognizable on the light List card while
        // reaching readable contrast for both counts and enlarged text.
        switch id {
        case "need": return PhaseColor.rgb(0x795513)
        case "running": return PhaseColor.rgb(0x205da8)
        case "working": return PhaseColor.rgb(0x27693f)
        case "rooms": return PhaseColor.rgb(0x7440ad)
        default: return BackplaneDesign.secondaryText
        }
    }

    private func dot(_ s: String) -> Color {
        switch s {
        case "here": PhaseColor.ok
        case "syncing": PhaseColor.accent
        default: PhaseColor.faint
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if typeSize.isAccessibilitySize {
                LazyVGrid(columns: [GridItem(.flexible(), spacing: 8), GridItem(.flexible())], alignment: .leading, spacing: 12) {
                    counter(plane.need.count, NativeMessages.plane_need(lang), PhaseColor.warn, id: "need")
                    counter(plane.run.count, NativeMessages.plane_running(lang), PhaseColor.accent, id: "running")
                    counter(plane.working, NativeMessages.plane_bots_working(lang), PhaseColor.ok, id: "working")
                    counter(plane.rooms.count, NativeMessages.plane_rooms_unread(lang), PhaseColor.input, id: "rooms")
                }
            } else {
                HStack(alignment: .top, spacing: 8) {
                    counter(plane.need.count, NativeMessages.plane_need(lang), PhaseColor.warn, id: "need")
                    counter(plane.run.count, NativeMessages.plane_running(lang), PhaseColor.accent, id: "running")
                    counter(plane.working, NativeMessages.plane_bots_working(lang), PhaseColor.ok, id: "working")
                    counter(plane.rooms.count, NativeMessages.plane_rooms_unread(lang), PhaseColor.input, id: "rooms")
                }
            }
            if plane.machines.count > 1 {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(plane.machines, id: \.self) { m in
                            HStack(spacing: 5) {
                                Circle().fill(dot(m.state)).frame(width: 6, height: 6)
                                Text(m.name).font(.caption)
                            }
                            .padding(.horizontal, 8).padding(.vertical, 4)
                            .background(.quaternary, in: .rect(cornerRadius: 2))
                            .opacity(m.state == "away" ? 0.55 : 1)
                        }
                    }
                }
            }
            if let h = plane.home {
                HStack(spacing: 10) {
                    Text(h.line).font(.caption).foregroundStyle(.secondary)
                    if h.button {
                        Button(h.label) { model.act(h.act, h.value) }.buttonStyle(.bordered).controlSize(.small)
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }
}

struct StatusTab: View {
    let model: AppModel
    let plane: PlaneModel?

    var body: some View {
        List {
            if let p = plane {
                Section { PlaneStrip(model: model, plane: p) }
                PlaneSections(model: model, plane: p, full: true)
            }
        }
        .overlay { if plane == nil { ProgressView() } }
    }
}

// The Home tab: the uberbot's card, then the plane in full (what needs you,
// what runs, the bots, rooms and routines, the idle threads). With no
// uberbot there is no plane: the tab offers to make one.
struct HomeTab: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let screen: Screen

    var body: some View {
        let card = model.uber(screen)
        List {
            if let u = card {
                Section { UberCardRow(model: model, card: u) }
                if let p = model.plane(screen) {
                    Section { PlaneStrip(model: model, plane: p) }
                    PlaneSections(model: model, plane: p, full: true)
                }
            }
        }
        .overlay {
            if card == nil {
                if screen.hubs.isEmpty || screen.projects.isEmpty && screen.bots.isEmpty {
                    ProgressView().opacity(screen.online ? 0 : 1)
                } else {
                    ContentUnavailableView {
                        Label(NativeMessages.native_home_none(lang), systemImage: "pawprint")
                    } description: {
                        Text(NativeMessages.native_home_hint(lang))
                    } actions: {
                        Button(NativeMessages.native_home_make(lang)) { model.act("uber-new") }.buttonStyle(.borderedProminent)
                    }
                }
            } else if model.plane(screen) == nil {
                ProgressView()
            }
        }
        .navigationTitle(NativeMessages.native_tab_home(lang))
    }
}
