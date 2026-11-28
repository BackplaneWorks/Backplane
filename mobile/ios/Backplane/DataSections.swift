import SwiftUI

// The sidebar's sections (docs/ALWAYS-ON.md section 8, the hub's present/sections.rs through data/1 `sidebar.sections`): Needs you, Pinned,
// Active, the groups (by git owner, then folder) and Quiet. The hub decides what is in each; the app draws them as they come.

/// One row of a section: a thread (kind "threads") or a project with its visible threads under it (kind "projects").
struct DSecRow: Codable, Identifiable, Equatable {
    var id: String; var title: String
    // a thread's
    var word: String; var say: String; var want: String; var ago: String; var machine: String; var mark: DMark?; var project: String; var projectKey: String; var off: Bool; var nav: String; var pinned: Bool; var hide: String
    // a project's
    var kind: String
    var key: String; var running: Int; var needsCount: Int; var threads: [DSecRow]; var icon: String; var css: String; var dots: [DSecDot]; var more: Int
    var projOff: Bool { off }
    /// a project's threads with no row under it (finished ones, on its page): counted on its row so none seems lost
    var rest: Int = 0
    /// its menu (SectionMenu.swift), the hub's; an older hub sends none
    var menu: [DMenuItem]
    /// the fold the hub keeps for a project row (`sidebar.fold.<key>`): the action its chevron sends, folded now, the rows it hides or would hide
    var fold: String; var folded: Bool; var under: Int
    /// a swarm row's: what it wants of the person, one line and its buttons (the hub's present/swarm_need.rs); nil when it wants nothing
    var need: DSwarmNeed?
    /// its status as a word (the hub's `label` and `tone`: bad, run, wait, ok; an older hub sends none and `pill` reads `word` and `want`)
    var label: String; var badgeTone: String
    /// the spans it can be snoozed for (`row-snooze` with an option's value), the thread one press settles, and a snoozed row's wake value
    var snooze: [DSnoozeOpt]; var settle: String; var wake: String
    /// a project with a board in its folder
    var hasBoard: Bool
    /// thread provider id (e.g. claude, codex, antigravity)
    var provider: String
    /// collapsed-by-default Done (N) fold under a project
    var doneCount: Int; var doneOpen: Bool; var doneToggle: String; var done: [DSecRow]
    enum CodingKeys: String, CodingKey { case id, title, word, say, want, ago, machine, mark, project, projectKey, off, nav, pinned, hide, kind, key, running, needsCount, threads, icon, css, dots, more, rest, menu, fold, folded, under, need, label, badgeTone = "tone", snooze, settle, wake, hasBoard, provider, doneCount, doneOpen, doneToggle, done }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.str(.id); title = c.str(.title); word = c.str(.word); say = c.str(.say); want = c.str(.want); ago = c.str(.ago); machine = c.str(.machine)
        mark = try? c.decode(DMark.self, forKey: .mark); project = c.str(.project); projectKey = c.str(.projectKey); off = c.bool(.off); nav = c.str(.nav); pinned = c.bool(.pinned); hide = c.str(.hide)
        kind = c.str(.kind); key = c.str(.key); running = c.int(.running); needsCount = c.int(.needsCount); threads = (try? c.decode([DSecRow].self, forKey: .threads)) ?? []
        icon = c.str(.icon); css = c.str(.css); dots = (try? c.decode([DSecDot].self, forKey: .dots)) ?? []; more = c.int(.more); rest = c.int(.rest)
        menu = (try? c.decode([DMenuItem].self, forKey: .menu)) ?? []
        fold = c.str(.fold); folded = c.bool(.folded); under = c.int(.under)
        need = kind == "swarm" ? DSwarmNeed.of(c, .need) : nil
        label = c.str(.label); badgeTone = c.str(.badgeTone); snooze = (try? c.decode([DSnoozeOpt].self, forKey: .snooze)) ?? []; settle = c.str(.settle); wake = c.str(.wake)
        hasBoard = c.bool(.hasBoard); provider = c.str(.provider)
        doneCount = c.int(.doneCount); doneOpen = c.bool(.doneOpen); doneToggle = c.str(.doneToggle); done = (try? c.decode([DSecRow].self, forKey: .done)) ?? []
    }
    var isProject: Bool { !key.isEmpty }
    /// a chevron on the row: something is under it, or it is folded (an older hub sends no fold)
    var canFold: Bool { !fold.isEmpty && (folded || under > 0) }
    /// a live swarm's row (`kind` swarm): it opens the swarm's page, its room
    var swarmRoom: String? { kind == "swarm" ? (nav.hasPrefix("@room:") ? String(nav.dropFirst(6)) : nil) : nil }
    /// the row's status, a word with its icon; nil for a thread with nothing to say
    var pill: HomePill? { isProject ? nil : Home.pill(label: label, tone: badgeTone, word: word, want: want) }
}
struct DSecDot: Codable, Equatable { var short: String; var here: Bool; var dim: Bool
    enum CodingKeys: String, CodingKey { case short, here, dim }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); short = c.str(.short); here = c.bool(.here); dim = c.bool(.dim) } }
struct DSection: Codable, Identifiable, Equatable {
    var id: String; var kind: String; var title: String; var count: Int; var fold: Bool; var open: Bool; var toggle: String; var group: String; var rename: Bool; var rows: [DSecRow]
    enum CodingKeys: String, CodingKey { case id, kind, title, count, fold, open, toggle, group, rename, rows }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.str(.id); kind = c.str(.kind); title = c.str(.title); count = c.int(.count); fold = c.bool(.fold); open = c.bool(.open); toggle = c.str(.toggle)
        group = c.str(.group); rename = c.bool(.rename); rows = (try? c.decode([DSecRow].self, forKey: .rows)) ?? []
    }
}
struct DGroupTo: Codable, Equatable { var value: String; var label: String
    enum CodingKeys: String, CodingKey { case value, label }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); value = c.str(.value); label = c.str(.label) } }
struct DProjectPage: Decodable, Equatable { var key: String; var sections: [DSection]
    enum CodingKeys: String, CodingKey { case key, sections }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); key = c.str(.key); sections = (try? c.decode([DSection].self, forKey: .sections)) ?? [] } }
struct DSections: Decodable { var sections: [DSection]; var groupsTo: [DGroupTo]; var page: DProjectPage?
    enum CodingKeys: String, CodingKey { case sections, groupsTo, page }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); sections = (try? c.decode([DSection].self, forKey: .sections)) ?? []; groupsTo = (try? c.decode([DGroupTo].self, forKey: .groupsTo)) ?? []; page = try? c.decode(DProjectPage.self, forKey: .page) } }
// (here, beside DSections: DataModels.swift is compiled without this file by the wire test, scripts/mobile/ios-unit-tests.sh)
struct DShellSyncReply: Decodable {
    let sub: Int
    let projects: [DProject]?
    let threads: [DThread]?
    let removed: [String]?
    let sections: DSections?
    let h: [String: String]?
    enum CodingKeys: String, CodingKey { case sub, projects, threads, removed, sections, h }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        sub = (try? c.decode(Int.self, forKey: .sub)) ?? 0
        projects = try? c.decode([DProject].self, forKey: .projects)
        threads = try? c.decode([DThread].self, forKey: .threads)
        removed = try? c.decode([String].self, forKey: .removed)
        sections = try? c.decode(DSections.self, forKey: .sections)
        h = try? c.decode([String: String].self, forKey: .h)
    }
}
/// A project's page on the navigation stack: its key and name.
struct DProjectRoute: Hashable { var key: String; var title: String }

/// A section's head: its title, a count, a chevron when it folds.
struct DataSectionHead: View {
    let s: DSection; let toggle: () -> Void
    var body: some View {
        Button(action: { if s.fold { toggle() } }) {
            HStack(spacing: 6) {
                if s.fold { Image(systemName: s.open ? "chevron.down" : "chevron.right").font(.system(size: 10)).foregroundStyle(BackplaneDesign.dim).frame(width: 14) }
                Text(Tr.t(s.title).uppercased()).dmono(11, bold: s.id == "needs").foregroundStyle(s.id == "needs" ? BackplaneDesign.bad : BackplaneDesign.dim).lineLimit(1)
                if s.fold || s.kind == "threads" { Text("\(s.count)").dmono(11).foregroundStyle(BackplaneDesign.faint) }
                Spacer(minLength: 0)
            }.padding(.leading, 14).padding(.trailing, 12).padding(.top, 10).frame(minHeight: s.fold ? 44 : 34).contentShape(Rectangle())
        }.buttonStyle(.plain).accessibilityIdentifier("data.section." + s.id)
    }
}
/// A row's status as a word with its icon, square as the rest of the app: red and solid when the person is wanted or it stopped, the accent
/// while it works, quiet when it waits or is done. It replaced the row's coloured dot, which said almost nothing at a glance.
struct DStatusPill: View {
    let pill: HomePill
    private var color: Color { switch pill.tone { case "bad": BackplaneDesign.bad; case "run": BackplaneDesign.accent; case "ok": BackplaneDesign.ok; default: BackplaneDesign.dim } }
    var body: some View {
        // solid when the person must act (approve, answer, sign in); a stop is red too, as a line: it waits for nobody
        let solid = pill.tone == "bad" && pill.label != "Failed" && pill.label != "Stopped"
        HStack(spacing: 4) {
            Image(systemName: pill.symbol).font(.system(size: 10, weight: .bold))
            Text(Tr.t(pill.label)).dui(11, weight: .semibold).lineLimit(1)
        }
        .foregroundStyle(solid ? BackplaneDesign.background : color)
        .padding(.horizontal, 9).frame(minHeight: 24)
        .background(solid ? color : color.opacity(0.12), in: Capsule())
        .overlay(Capsule().strokeBorder(color, lineWidth: 1))
        .fixedSize().accessibilityElement(children: .combine)
    }
}
/// A project with a board in it: a small chip beside its name.
struct DBoardBadge: View {
    var body: some View {
        HStack(spacing: 3) {
            Image(systemName: "cpu").font(.system(size: 9, weight: .semibold))
            Text(Tr.t("PCB")).dmono(10, bold: true)
        }
        .foregroundStyle(BackplaneDesign.ok).padding(.horizontal, 9).frame(minHeight: 18)
        .overlay(Capsule().strokeBorder(BackplaneDesign.ok.opacity(0.6), lineWidth: 1)).fixedSize().accessibilityElement(children: .combine)
    }
}
/// A thread row of a section: its title, under it where it is and when, and its status as a word at the right. One whose machine is off is
/// listed, dimmed, and opens nothing. A snoozed row (the hub's `wake`) says when it comes back.
struct DataSectionThread: View {
    let r: DSecRow; var indent: CGFloat = 14; var showProject = false; let open: (String) -> Void
    private var sub: String {
        var parts: [String] = []
        if showProject && !r.project.isEmpty { parts.append(r.project) }
        if let m = r.mark { parts.append(dAway(m.short, m.away)) }
        if !r.wake.isEmpty { parts.append(Tr.mobile_snooze_wakes(time: r.ago)) } else if !r.ago.isEmpty { parts.append(r.ago) }
        return parts.joined(separator: " · ")
    }
    var body: some View {
        Button { if !r.off { open(r.nav) } } label: {
            HStack(spacing: 10) {
                VStack(alignment: .leading, spacing: 3) {
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        if !r.provider.isEmpty { ProviderMark(provider: r.provider, size: 12) }
                        Text(r.title).dui(15, weight: r.word == "needs" ? .semibold : .medium).foregroundStyle(BackplaneDesign.text).lineLimit(2)
                        if r.pinned { Image(systemName: "pin.fill").font(.system(size: 9)).foregroundStyle(BackplaneDesign.faint).accessibilityIdentifier("data.section.pinned." + r.id) }
                    }
                    if !sub.isEmpty { Text(sub).dmono(11).foregroundStyle(BackplaneDesign.dim).lineLimit(1).truncationMode(.middle) }
                }
                Spacer(minLength: 0)
                if let p = r.pill { DStatusPill(pill: p).accessibilityIdentifier("data.section.pill." + r.id) }
            }.padding(.leading, indent).padding(.trailing, 12).padding(.vertical, 6).frame(minHeight: 52).contentShape(Rectangle()).opacity(r.off ? 0.55 : 1)
        }.buttonStyle(.plain).accessibilityIdentifier("data.section.thread." + r.id)
    }
}
/// A swarm row that wants the person (its `need`): the row, the need's line, and one small button per action under it. A press on the row or on
/// "board" opens the swarm's page; "open" the member's thread; the others call `swarm.act` (Swarm.press), and its reply may open an account's
/// sign-in or a thread. A refusal shows the hub's words under the buttons. `after` runs once the hub has answered (a project page reads its lists again).
struct DataSwarmNeedRow: View {
    let client: DataClient; let r: DSecRow; let need: DSwarmNeed; let room: String
    let openRoom: () -> Void; let openThread: (String) -> Void; var after: () async -> Void = {}
    @State private var busy = false
    @State private var error = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            DataSectionThread(r: r) { _ in openRoom() }
            Text(Tr.t(need.line)).dmono(11).foregroundStyle(BackplaneDesign.bad).lineLimit(1).truncationMode(.tail)
                .padding(.leading, 14).padding(.trailing, 12).padding(.top, -2).accessibilityIdentifier("data.section.need." + r.id)
            HStack(spacing: 6) {
                ForEach(Swarm.buttons(need, rowRoom: room)) { a in
                    Button(Tr.t(a.label)) { press(a) }.buttonStyle(DButton(primary: false, compact: true)).disabled(busy)
                        .accessibilityIdentifier("data.section.need." + a.act + "." + r.id)
                }
                Spacer(minLength: 0)
            }.padding(.leading, 14).padding(.trailing, 12).padding(.top, 6).padding(.bottom, error.isEmpty ? 8 : 4)
            if !error.isEmpty { Text(error).dmono(11).foregroundStyle(BackplaneDesign.bad).lineLimit(2).padding(.leading, 14).padding(.trailing, 12).padding(.bottom, 8).accessibilityIdentifier("data.section.need.error." + r.id) }
        }
    }
    private func press(_ a: DSwarmNeedAct) {
        switch Swarm.press(a, rowRoom: room) {
        case .board?: openRoom()
        case .thread(let t)?: openThread(t)
        case .request(let p)?:
            busy = true; error = ""
            Task {
                do {
                    switch try await client.swarmNeedAct(p) {
                    case .account(let id)?: client.pushAccount = id
                    case .thread(let t)?: openThread(t)
                    case nil: break
                    }
                } catch { self.error = error.localizedDescription }
                busy = false; await after()
            }
        case nil: break
        }
    }
}
extension View {
    /// A long press on a thread row. Another machine's thread (the hub's `hide`): Hide, here only, until something new happens there (`thread-hide`).
    /// A thread of this hub (the hub's `menu`): Pin or Unpin (a pinned thread stays under its project and never settles by itself), Settle or
    /// Unsettle, Archive. `after` runs once the hub has answered (a project page reads its lists again).
    func farHide(_ r: DSecRow, _ client: DataClient, after: @escaping () async -> Void = {}) -> some View {
        contextMenu {
            if !r.hide.isEmpty { Button(Tr.t("Hide"), systemImage: "eye.slash") { Task { await client.sidebarAct("thread-hide", r.hide) } } }
            if !r.isProject && r.swarmRoom == nil { ForEach(DMenuItem.thread(r.menu)) { m in
                // a chat in no project: Move to project… lists the hub's projects (`sub`, each a `row-move-project`); every other item is a thread.act
                if m.isMove { Menu { ForEach(m.sub.indices, id: \.self) { i in Button(m.sub[i].label) { Task { await client.sidebarAct(m.sub[i].action.isEmpty ? DMenuItem.moveProject : m.sub[i].action, m.sub[i].value); await after() } }.accessibilityIdentifier("data.section.move." + r.id + ".\(i)") } } label: { Label(Tr.t(m.label), systemImage: "folder") }.accessibilityIdentifier("data.section.move." + r.id) }
                else { Button(Tr.t(m.label)) { Task { await client.sectionThreadAct(r.id, m.id); await after() } }.accessibilityIdentifier("data.section." + m.id + "." + r.id) }
            } }
            // what a swipe does, for a finger that would rather press: the spans it can be snoozed for, or Wake
            if !r.snooze.isEmpty { Menu { ForEach(r.snooze) { o in Button(Tr.t(o.label)) { Task { await client.snoozeRow(o.value); await after() } } } } label: { Label(Tr.t("Snooze"), systemImage: "clock") } }
            if !r.wake.isEmpty { Button(Tr.t("Wake"), systemImage: "bell") { Task { await client.snoozeRow(r.wake); await after() } } }
        }
    }
    /// A thread row's swipes (native, in a List): to the left Snooze, one button per span the hub offers (a full swipe takes the first, 1 hour), or
    /// Wake on a snoozed row; to the right Settle, on a row the hub lets one press settle. A row with neither has no swipe.
    func rowSwipes(_ r: DSecRow, _ client: DataClient, after: @escaping () async -> Void = {}) -> some View {
        swipeActions(edge: .trailing, allowsFullSwipe: true) {
            ForEach(r.snooze) { o in
                Button { Task { await client.snoozeRow(o.value); await after() } } label: { Label(Tr.t(o.label), systemImage: "clock") }
                    .tint(o.id == r.snooze.first?.id ? BackplaneDesign.accent : BackplaneDesign.dim).accessibilityIdentifier("data.section.snooze." + r.id)
            }
            if !r.wake.isEmpty { Button { Task { await client.snoozeRow(r.wake); await after() } } label: { Label(Tr.t("Wake"), systemImage: "bell") }.tint(BackplaneDesign.accent).accessibilityIdentifier("data.section.wake." + r.id) }
        }
        .swipeActions(edge: .leading, allowsFullSwipe: true) {
            if !r.settle.isEmpty { Button { Task { await client.sectionThreadAct(r.id, "settle"); await after() } } label: { Label(Tr.t("Settle"), systemImage: "checkmark") }.tint(BackplaneDesign.ok).accessibilityIdentifier("data.section.settle." + r.id) }
        }
    }
}
/// A project row of a section: its badge and name, a PCB chip when it has a board, the machines' dots, what runs and what waits in it; a press
/// opens its page. `tall` is a row of the bench (Pinned, Recent): the ones a thumb goes for.
struct DataSectionProject: View {
    let r: DSecRow; var faded = false; var tall = false; var toggleFold: () -> Void = {}
    var body: some View {
        HStack(spacing: 8) {
            // the fold: a tap on the chevron folds its threads away or back (the hub keeps it); a tap elsewhere opens the page
            if r.canFold {
                Button(action: toggleFold) { Image(systemName: r.folded ? "chevron.right" : "chevron.down").font(.system(size: 10)).foregroundStyle(BackplaneDesign.faint).frame(width: 30, height: 44).contentShape(Rectangle()) }
                    .buttonStyle(.plain).accessibilityLabel(Tr.t(r.folded ? "Show" : "Hide")).accessibilityIdentifier("data.section.fold." + r.key).padding(.trailing, -8)
            } else { Color.clear.frame(width: 6, height: 44) }
            if r.icon == "backplane" { BrandMark(size: tall ? 16 : 14) } else { Image(systemName: dataBadgeSymbol(r.icon)).font(.system(size: tall ? 16 : 14)).foregroundStyle(Color(css: r.css) ?? BackplaneDesign.ok).frame(width: 20) }
            Text(r.title).dui(15, weight: tall ? .semibold : .medium).foregroundStyle(BackplaneDesign.text).lineLimit(1)
            if r.hasBoard { DBoardBadge().accessibilityIdentifier("data.section.board." + r.key) }
            if r.dots.count > 1 { HStack(spacing: 3) { ForEach(Array(r.dots.enumerated()), id: \.offset) { _, d in Circle().fill(d.dim ? BackplaneDesign.faint : (d.here ? BackplaneDesign.accent : BackplaneDesign.dim)).frame(width: 5, height: 5) } }.accessibilityHidden(true) }
            Spacer(minLength: 0)
            if r.rest > 0 { Text("\(r.rest)").dmono(11).foregroundStyle(BackplaneDesign.faint).accessibilityIdentifier("data.section.rest." + r.key) }
            if r.running > 0 { Text(Tr.ios_sections_running(n: r.running)).dmono(11, bold: true).foregroundStyle(BackplaneDesign.accent).lineLimit(1).fixedSize() }
            // what of it waits on the person (its rows are in Needs you, folded or not)
            if r.needsCount > 0 { Text("\(r.needsCount)").dmono(11, bold: true).foregroundStyle(BackplaneDesign.background).padding(.horizontal, 6).frame(minWidth: 20, minHeight: 20).background(BackplaneDesign.bad, in: Capsule()).accessibilityIdentifier("data.section.needs." + r.key) }
            if r.pinned { Image(systemName: "pin.fill").font(.system(size: 10)).foregroundStyle(BackplaneDesign.dim) }
            Image(systemName: "chevron.right").font(.system(size: 10)).foregroundStyle(BackplaneDesign.faint).accessibilityHidden(true)
        }.padding(.leading, 8).padding(.trailing, 12).frame(minHeight: tall ? 52 : 46).contentShape(Rectangle()).opacity(faded || r.projOff ? 0.55 : 1)
    }
}

/// A project's page: running, then needs you, then recent, then Done (n) folded; every machine's threads.
struct DataProjectPageView: View {
    let client: DataClient; let route: DProjectRoute
    @Binding var path: NavigationPath
    var onNew: (String) -> Void = { _ in }   // New thread in this project (the New thread sheet: where it runs, and the machine picker)
    private var project: DProject? { client.projects.first { !$0.key.isEmpty && $0.key == route.key } }
    @State private var page: DProjectPage?
    @State private var done = false
    // Buy it beside New thread, as a thread's header has it: the hub's button for this project. A buy thread starts an agent's turn at once, so a
    // press asks first (`buyAsk`), and only its answer makes the buy thread and opens it
    @State private var buy: DBuy?
    @State private var buyAsk: DBuy?
    @State private var buying = false
    @State private var buyFailed = ""
    /// a long Done list: its newest rows, the rest behind the hub's "Older (m)" (`page-older`)
    @State private var older = false
    /// what the project is, as the desktop's project page opens: its goal (a tap shows it whole) and its boards
    @State private var card: DProjectCard?
    @State private var whole = false
    @State private var readme: String?
    var body: some View {
        List {
            // as the desktop's project page: the folder (selectable), and the viewer on the project itself without a thread
            // (`project-home-<id>`): Board, Schematic and 3D when it has a board, Files always
            if let p = project {
                VStack(alignment: .leading, spacing: 8) {
                    if !p.root.isEmpty { Text(verbatim: p.root).dmono(11).foregroundStyle(BackplaneDesign.faint).lineLimit(2).textSelection(.enabled).accessibilityIdentifier("data.project.page.path") }
                    HStack(spacing: 8) {
                        ForEach(((card?.boards.isEmpty ?? true) ? [] : ["Board", "Schematic", "3D"]) + ["Files"], id: \.self) { t in
                            Button { path.append(DViewerRoute(id: "project-home-" + p.id, tab: t)) } label: {
                                Text(Tr.t(t)).dui(12, weight: .semibold).lineLimit(1).foregroundStyle(BackplaneDesign.text)
                                    .padding(.horizontal, 10).frame(minHeight: 30)
                                    .background(BackplaneDesign.raised, in: RoundedRectangle(cornerRadius: DRadius.control))
                                    .overlay(RoundedRectangle(cornerRadius: DRadius.control).strokeBorder(BackplaneDesign.line, lineWidth: 1))
                            }.buttonStyle(.plain).accessibilityIdentifier("data.project.page.tool.\(t)")
                        }
                    }
                }.frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 14).padding(.vertical, 8).dataRow()
            }
            if let c = card, !c.goal.isEmpty || !c.boards.isEmpty || c.leadDays > 0 || !c.estimateWarnings.isEmpty {
                Button { whole.toggle() } label: {
                    VStack(alignment: .leading, spacing: 4) {
                        if !c.goal.isEmpty { Text(c.goal).dui(13).foregroundStyle(BackplaneDesign.text).lineLimit(whole ? 20 : 2).multilineTextAlignment(.leading) }
                        if !c.boards.isEmpty { Text(Tr.t("Boards").uppercased() + "  " + c.boards.joined(separator: " · ")).dmono(10).foregroundStyle(BackplaneDesign.dim).lineLimit(2) }
                        if c.leadDays > 0 { Text(Tr.order_lead_days(days: c.leadDays)).dmono(11).foregroundStyle(BackplaneDesign.dim).accessibilityIdentifier("data.project.page.lead") }
                        if !c.estimateWarnings.isEmpty { Text(Tr.order_estimate_warnings(warnings: c.estimateWarnings)).dmono(11).foregroundStyle(BackplaneDesign.warn).fixedSize(horizontal: false, vertical: true).accessibilityIdentifier("data.project.page.warnings") }
                    }.frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 14).padding(.vertical, 8).contentShape(Rectangle())
                }.buttonStyle(.plain).background(BackplaneDesign.panel).accessibilityIdentifier("data.project.page.card").dataRow()
            }
            if !buyFailed.isEmpty { Text(buyFailed).dmono(11).foregroundStyle(BackplaneDesign.bad).padding(.horizontal, 14).padding(.vertical, 6).dataRow() }
            if let page {
                ForEach(page.sections.filter { !$0.rows.isEmpty || ($0.fold && $0.count > 0) }) { s in
                    DataSectionHead(s: s) { if s.toggle == "page-older" { older.toggle() } else { done.toggle() }; Task { await load() } }.dataRow()
                    ForEach(s.rows) { r in
                        if let room = r.swarmRoom, let n = r.need { DataSwarmNeedRow(client: client, r: r, need: n, room: room, openRoom: { path.append(DSwarmRoute(id: room, name: r.title)) }, openThread: { path.append($0) }) { await load() }.dataRow() }
                        else { DataSectionThread(r: r) { nav in if let room = r.swarmRoom { path.append(DSwarmRoute(id: room, name: r.title)) } else { path.append(nav) } }.farHide(r, client) { await load() }.rowSwipes(r, client) { await load() }.dataRow() }
                    }
                }
                if page.sections.allSatisfy({ $0.rows.isEmpty && $0.count == 0 }) { Text(Tr.t("No threads")).dmono(12).foregroundStyle(BackplaneDesign.faint).padding(14).dataRow() }
                // under the threads, as the desktop's project page: its README (when it has one) and its goal and notes, folded, read-only
                if let p = project {
                    if let r = readme { DataProjectNote(title: Tr.k("gpui.readme").uppercased(), tag: "readme", empty: "") { r }.dataRow() }
                    if client.has("projecthome") { DataProjectNote(title: Tr.k("gpui.goal.notes"), tag: "goal", empty: Tr.k("gpui.no.goal.yet.ask.an")) { await client.projectIntent(p.id) }.dataRow() }
                }
            } else { DLoading(text: Tr.t("Loading…")).dataRow() }
        }
        .listStyle(.plain).scrollContentBackground(.hidden).environment(\.defaultMinListRowHeight, 1)
        // opening the page is a visit: the hub's bench (Recent) follows where the person was last, on this phone too
        .navigationTitle(route.title).task { await client.visitProject(route.key); await load() }.refreshable { await load() }
        .toolbar { if let p = project { ToolbarItem(placement: .primaryAction) { Button { onNew(p.id) } label: { Image(systemName: "plus") }.accessibilityIdentifier("data.project.page.new") } } }
        .toolbar { if let b = buy { ToolbarItem(placement: .primaryAction) { Button { buyAsk = b } label: { Image(systemName: "cart") }.disabled(buying).accessibilityLabel(Tr.t(b.label)).accessibilityIdentifier("data.project.page.buy") } } }
        .confirmationDialog(Tr.ios_buy_ask(project: route.title), isPresented: Binding(get: { buyAsk != nil }, set: { if !$0 { buyAsk = nil } }), titleVisibility: .visible) {
            Button(Tr.t("Buy it")) { if let b = buyAsk { startBuy(b) } }.accessibilityIdentifier("data.project.page.buy.confirm")
            Button(Tr.t("Cancel"), role: .cancel) {}
        }
        .task(id: "\(project?.id ?? "")|\(client.conn == .online)") { if client.conn == .online, let p = project { buy = await client.buyOfProject(p.id); card = await client.projectCard(p.id)
            readme = (await client.fileBytes("project-home-" + p.id, "README.md")).flatMap { String(data: $0, encoding: .utf8) } } }
    }
    private func startBuy(_ b: DBuy) {
        guard !buying else { return }; buying = true; buyFailed = ""
        Task { do { let t = try await client.buy(b.thread); if !t.isEmpty { client.linkTarget = DLinkTarget(kind: .thread, id: t) } } catch { buyFailed = error.localizedDescription }; buying = false }
    }
    private func load() async { page = await client.projectPage(route.key, done: done, older: older) }
}

/// A folded note of a project's page (README, Goal & notes): the head as a section's, the text in the app's Markdown when opened; `load` runs then.
struct DataProjectNote: View {
    let title: String
    let tag: String
    let empty: String
    let load: () async -> String
    @State private var open = false
    @State private var text: String?
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Button { open.toggle() } label: {
                HStack(spacing: 6) {
                    Image(systemName: open ? "chevron.down" : "chevron.right").font(.system(size: 11, weight: .semibold)).foregroundStyle(BackplaneDesign.dim)
                    Text(verbatim: title).dmono(11).foregroundStyle(BackplaneDesign.dim)
                    Spacer()
                }.frame(minHeight: 36).contentShape(Rectangle())
            }.buttonStyle(.plain).accessibilityIdentifier("data.project.page.\(tag)")
            if open {
                if let t = text {
                    if t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { Text(verbatim: empty).dmono(12).foregroundStyle(BackplaneDesign.faint) }
                    else { DataMarkdown(text: t).accessibilityIdentifier("data.project.page.\(tag).text") }
                } else { Text(Tr.t("Loading…")).dmono(12).foregroundStyle(BackplaneDesign.faint) }
            }
        }.padding(.horizontal, 14).padding(.vertical, 2)
        .task(id: open) { if open && text == nil { text = await load() } }
    }
}
