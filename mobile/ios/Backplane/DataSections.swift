import SwiftUI

// The sidebar's sections (docs/ALWAYS-ON.md section 8, the hub's present/sections.rs through data/1 `sidebar.sections`): Needs you, Pinned,
// Active, the groups (by git owner, then folder) and Quiet. The hub decides what is in each; the app draws them as they come.

/// One row of a section: a thread (kind "threads") or a project with its visible threads under it (kind "projects").
struct DSecRow: Decodable, Identifiable, Equatable {
    var id: String; var title: String
    // a thread's
    var word: String; var say: String; var want: String; var ago: String; var machine: String; var mark: DMark?; var project: String; var projectKey: String; var off: Bool; var nav: String; var pinned: Bool; var hide: String
    // a project's
    var kind: String
    var key: String; var projOff: Bool; var running: Int; var needsCount: Int; var threads: [DSecRow]; var icon: String; var css: String; var dots: [DSecDot]; var more: Int
    /// its menu (SectionMenu.swift), the hub's; an older hub sends none
    var menu: [DMenuItem]
    /// the fold the hub keeps for a project row (`sidebar.fold.<key>`): the action its chevron sends, folded now, the rows it hides or would hide
    var fold: String; var folded: Bool; var under: Int
    enum CodingKeys: String, CodingKey { case id, title, word, say, want, ago, machine, mark, project, projectKey, off, nav, pinned, hide, kind, key, running, needsCount, threads, icon, css, dots, more, menu, fold, folded, under }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.str(.id); title = c.str(.title); word = c.str(.word); say = c.str(.say); want = c.str(.want); ago = c.str(.ago); machine = c.str(.machine)
        mark = try? c.decode(DMark.self, forKey: .mark); project = c.str(.project); projectKey = c.str(.projectKey); off = c.bool(.off); nav = c.str(.nav); pinned = c.bool(.pinned); hide = c.str(.hide)
        kind = c.str(.kind); key = c.str(.key); projOff = (try? c.decode(Bool.self, forKey: .off)) ?? false; running = c.int(.running); needsCount = c.int(.needsCount); threads = (try? c.decode([DSecRow].self, forKey: .threads)) ?? []
        icon = c.str(.icon); css = c.str(.css); dots = (try? c.decode([DSecDot].self, forKey: .dots)) ?? []; more = c.int(.more)
        menu = (try? c.decode([DMenuItem].self, forKey: .menu)) ?? []
        fold = c.str(.fold); folded = c.bool(.folded); under = c.int(.under)
    }
    var isProject: Bool { !key.isEmpty }
    /// a chevron on the row: something is under it, or it is folded (an older hub sends no fold)
    var canFold: Bool { !fold.isEmpty && (folded || under > 0) }
    /// a live swarm's row (`kind` swarm): it opens the swarm's page, its room
    var swarmRoom: String? { kind == "swarm" ? (nav.hasPrefix("@room:") ? String(nav.dropFirst(6)) : nil) : nil }
    /// what a thread row's dot is: red when it asks, the accent while it works, dim when stopped
    var tone: Color { switch word { case "needs": return BackplaneDesign.bad; case "running", "monitoring", "limit": return BackplaneDesign.accent; case "failed", "cut", "interrupted": return BackplaneDesign.bad.opacity(0.6); default: return BackplaneDesign.faint } }
}
struct DSecDot: Decodable, Equatable { var short: String; var here: Bool; var dim: Bool
    enum CodingKeys: String, CodingKey { case short, here, dim }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); short = c.str(.short); here = c.bool(.here); dim = c.bool(.dim) } }
struct DSection: Decodable, Identifiable, Equatable {
    var id: String; var kind: String; var title: String; var count: Int; var fold: Bool; var open: Bool; var toggle: String; var group: String; var rename: Bool; var rows: [DSecRow]
    enum CodingKeys: String, CodingKey { case id, kind, title, count, fold, open, toggle, group, rename, rows }
    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = c.str(.id); kind = c.str(.kind); title = c.str(.title); count = c.int(.count); fold = c.bool(.fold); open = c.bool(.open); toggle = c.str(.toggle)
        group = c.str(.group); rename = c.bool(.rename); rows = (try? c.decode([DSecRow].self, forKey: .rows)) ?? []
    }
}
struct DGroupTo: Decodable, Equatable { var value: String; var label: String
    enum CodingKeys: String, CodingKey { case value, label }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); value = c.str(.value); label = c.str(.label) } }
struct DProjectPage: Decodable, Equatable { var key: String; var sections: [DSection]
    enum CodingKeys: String, CodingKey { case key, sections }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); key = c.str(.key); sections = (try? c.decode([DSection].self, forKey: .sections)) ?? [] } }
struct DSections: Decodable { var sections: [DSection]; var groupsTo: [DGroupTo]; var page: DProjectPage?
    enum CodingKeys: String, CodingKey { case sections, groupsTo, page }
    init(from d: Decoder) throws { let c = try d.container(keyedBy: CodingKeys.self); sections = (try? c.decode([DSection].self, forKey: .sections)) ?? []; groupsTo = (try? c.decode([DGroupTo].self, forKey: .groupsTo)) ?? []; page = try? c.decode(DProjectPage.self, forKey: .page) } }
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
            }.padding(.leading, 10).padding(.top, 10).frame(minHeight: 30).contentShape(Rectangle())
        }.buttonStyle(.plain).accessibilityIdentifier("data.section." + s.id)
    }
}
/// A thread row of a section; one whose machine is off is listed, dimmed, and opens nothing.
struct DataSectionThread: View {
    let r: DSecRow; var indent: CGFloat = 12; var showProject = false; var quiet = false; let open: (String) -> Void
    var body: some View {
        Button { if !r.off { open(r.nav) } } label: {
            HStack(spacing: 8) {
                Circle().fill(r.tone).frame(width: 7, height: 7)
                Text(r.title).dmono(13).foregroundStyle(BackplaneDesign.text).lineLimit(1)
                if showProject { Text(r.project).dmono(11).foregroundStyle(BackplaneDesign.faint).lineLimit(1) }
                Spacer(minLength: 0)
                if let m = r.mark { Text(dAway(m.short, m.away)).dmono(11).foregroundStyle(BackplaneDesign.faint) }
                if quiet { Text(r.ago).dmono(11).foregroundStyle(BackplaneDesign.faint) }
                else { Text(r.want.isEmpty ? Tr.t(r.say.isEmpty ? r.ago : r.say) : Tr.t(r.want)).dmono(11).foregroundStyle(r.want.isEmpty ? BackplaneDesign.faint : BackplaneDesign.bad) }
            }.padding(.leading, indent).padding(.trailing, 12).frame(minHeight: 36).contentShape(Rectangle()).opacity(r.off ? 0.55 : 1)
        }.buttonStyle(.plain).accessibilityIdentifier("data.section.thread." + r.id)
    }
}
extension View {
    /// Another machine's thread (the hub's `hide`): a long press hides it here until something new happens there (`thread-hide`).
    func farHide(_ r: DSecRow, _ client: DataClient) -> some View {
        contextMenu { if !r.hide.isEmpty { Button(Tr.t("Hide"), systemImage: "eye.slash") { Task { await client.sidebarAct("thread-hide", r.hide) } } } }
    }
}
/// A project row of a section: its badge and name, the machines' dots, its running count; a press opens its page.
struct DataSectionProject: View {
    let r: DSecRow; var faded = false; var toggleFold: () -> Void = {}
    var body: some View {
        HStack(spacing: 8) {
            // the fold: a tap on the chevron folds its threads away or back (the hub keeps it); a tap elsewhere opens the page
            Group {
                if r.canFold {
                    Button(action: toggleFold) { Image(systemName: r.folded ? "chevron.right" : "chevron.down").font(.system(size: 10)).foregroundStyle(BackplaneDesign.faint).frame(width: 22, height: 38).contentShape(Rectangle()) }
                        .buttonStyle(.plain).accessibilityLabel(Tr.t(r.folded ? "Show" : "Hide")).accessibilityIdentifier("data.section.fold." + r.key)
                } else { Color.clear.frame(width: 22, height: 38) }
            }.padding(.trailing, -4)
            if r.icon == "backplane" { BrandMark(size: 13) } else { Image(systemName: dataBadgeSymbol(r.icon)).font(.system(size: 13)).foregroundStyle(Color(css: r.css) ?? BackplaneDesign.ok) }
            Text(r.title).dmono(13).foregroundStyle(BackplaneDesign.text).lineLimit(1)
            if r.dots.count > 1 { HStack(spacing: 3) { ForEach(Array(r.dots.enumerated()), id: \.offset) { _, d in Circle().fill(d.dim ? BackplaneDesign.faint : (d.here ? BackplaneDesign.accent : BackplaneDesign.dim)).frame(width: 5, height: 5) } } }
            Spacer(minLength: 0)
            if r.running > 0 { Text(Tr.ios_sections_running(n: r.running)).dmono(11).foregroundStyle(BackplaneDesign.accent) }
            // what of it waits on the person (its rows are in Needs you, folded or not)
            if r.needsCount > 0 { Text("\(r.needsCount)").dmono(10).foregroundStyle(BackplaneDesign.background).padding(.horizontal, 5).background(BackplaneDesign.bad).accessibilityIdentifier("data.section.needs." + r.key) }
            if r.pinned { Image(systemName: "pin.fill").font(.system(size: 10)).foregroundStyle(BackplaneDesign.dim) }
        }.padding(.trailing, 12).frame(minHeight: 38).contentShape(Rectangle()).opacity(faded || r.projOff ? 0.55 : 1)
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
    var body: some View {
        List {
            if let page {
                ForEach(page.sections.filter { !$0.rows.isEmpty || ($0.fold && $0.count > 0) }) { s in
                    DataSectionHead(s: s) { done.toggle(); Task { await load() } }.dataRow()
                    ForEach(s.rows) { r in DataSectionThread(r: r) { nav in if let room = r.swarmRoom { path.append(DSwarmRoute(id: room, name: r.title)) } else { path.append(nav) } }.farHide(r, client).dataRow() }
                }
                if page.sections.allSatisfy({ $0.rows.isEmpty && $0.count == 0 }) { Text(Tr.t("No threads")).dmono(12).foregroundStyle(BackplaneDesign.faint).padding(14).dataRow() }
            } else { DLoading(text: Tr.t("Loading…")).dataRow() }
        }
        .listStyle(.plain).scrollContentBackground(.hidden).environment(\.defaultMinListRowHeight, 1)
        .navigationTitle(route.title).task { await load() }.refreshable { await load() }
        .toolbar { if let p = project { ToolbarItem(placement: .primaryAction) { Button { onNew(p.id) } label: { Image(systemName: "plus") }.accessibilityIdentifier("data.project.page.new") } } }
    }
    private func load() async { page = await client.projectPage(route.key, done: done) }
}
