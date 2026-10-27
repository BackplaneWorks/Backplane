import SwiftUI

struct RootView: View {
    @Bindable var model: AppModel
    @State private var pairing = false
    @State private var recoveryKey: String?
    @State private var recoverySheet = false
    @State private var recoverySheetKey: String?
    // the delete (or remove) just answered: its dialog stays down until the screen drops it
    @State private var answered = ""
    @State private var removed = ""
    @State private var renamed = ""
    @State private var newTitle = ""
    // the first-run tour waits until nothing else is presented over the root
    // (see tourWait): its screen model already lacks a step while Bend
    // knows of another overlay, and this covers the rest (the hubs sheet, a
    // picture opened large, menus and sheets still going away)
    @State private var tourReady = false

    var body: some View {
        content
            .safeAreaInset(edge: .top, spacing: 0) { ViewStartupBanner(model: model) }
            .overlay(alignment: .top) { BubMoveOverlay(move: model.contentReady ? model.screen?.bubMove : nil) }
            .environment(\.backplaneLanguage, model.screen?.locale ?? "en")
            .environment(\.locale, Locale(identifier: model.screen?.locale ?? "en"))
            .onChange(of: authKey, initial: true) { _, key in
                if let key, recoveryKey == nil, !hasUsableContent { recoveryKey = key }
            }
            .onChange(of: model.contentReady) { _, ready in
                if ready { recoveryKey = nil; recoverySheet = false }
            }
            .onChange(of: model.links) { _, links in
                let keys = links.compactMap(Pairing.key)
                if let key = recoveryKey, !keys.contains(key) { recoveryKey = nil }
                if let key = recoverySheetKey, !keys.contains(key) {
                    recoverySheetKey = nil
                    recoverySheet = false
                }
            }
    }

    private var authKey: String? {
        guard !model.contentReady else { return nil }
        let keys = model.links.compactMap(Pairing.key)
        if let focused = model.screen?.hub, !focused.isEmpty {
            return keys.contains(focused) && model.conn[focused]?.failure == .authentication ? focused : nil
        }
        return keys.first { model.conn[$0]?.failure == .authentication }
    }

    private var hasUsableContent: Bool {
        guard let s = model.screen else { return false }
        let shown = !s.projects.isEmpty || !(s.active ?? []).isEmpty || !(s.legacy ?? []).isEmpty ||
                    !s.bots.isEmpty || !s.rooms.isEmpty || s.thread != nil || s.bot != nil || s.uber != nil
        let otherOnline = model.links.compactMap(Pairing.key).contains { key in
            key != authKey && (model.conn[key]?.phase == .online || model.conn[key]?.phase == .syncing)
        }
        return shown || otherOnline
    }

    @ViewBuilder private var content: some View {
        if model.links.isEmpty {
            NavigationStack { PairView(tag: model.screen?.locale ?? "en", link: "") { _ = model.pair($0) } }
        } else if !hasUsableContent, let key = recoveryKey ?? authKey {
            ConnectionStartupView(model: model, initialKey: key)
        } else if let s = model.screen {
            asked(s)
                .safeAreaInset(edge: .top, spacing: 0) {
                    if let key = authKey {
                        Button {
                            recoverySheetKey = key
                            recoverySheet = true
                        } label: {
                            HStack {
                                Text(NativeMessages.native_hub_failure(s.locale ?? "en", "authentication", ""))
                                Spacer()
                                Text(s.locale == "ja" ? "再試行" : "Retry")
                            }.padding(12)
                        }
                        .accessibilityIdentifier("pair.connection.error")
                    }
                }
                .sheet(isPresented: $recoverySheet) {
                    ConnectionStartupView(model: model, initialKey: recoverySheetKey)
                }
                .sheet(isPresented: findShown) {
                    if let f = model.screen?.find { FindSheet(model: model, find: f) }
                }
                .sheet(isPresented: $pairing) {
                    NavigationStack {
                        HubsView(model: model, screen: model.screen ?? s)
                            .toolbar { ToolbarItem(placement: .confirmationAction) { Button(NativeMessages.tour_done(model.screen?.locale ?? s.locale ?? "en")) { pairing = false } } }
                    }
                }
                .sheet(isPresented: tourShown) {
                    if let t = model.screen?.tour { TourSheet(model: model, tour: t) }
                }
                .task(id: "\(model.screen?.tour != nil)|\(pairing)") { await tourWait() }
        } else {
            ConnectionStartupView(model: model, initialKey: authKey)
        }
    }

    // the first-run tour (core/onboard.bend): up while the screen carries a
    // step (Bend leaves it out while it knows of another overlay) and
    // tourWait has seen the root clear; the same as Android: dismissing it
    // (swipe down) is Skip
    private var tourShown: Binding<Bool> {
        Binding(get: { model.screen?.tour != nil && tourReady && !pairing },
                set: { if !$0, model.screen?.tour != nil { model.act("ob-skip") } })
    }

    // each time the step appears or the hubs sheet moves: not ready, then
    // ready once no view controller is presented over the root (a sheet or
    // alert still going away counts as presented), polled only while a step
    // waits
    private func tourWait() async {
        tourReady = false
        guard model.screen?.tour != nil, !pairing else { return }
        repeat {
            try? await Task.sleep(nanoseconds: 300_000_000)
            if Task.isCancelled { return }
        } while !Presented.none
        tourReady = true
    }

    private var findShown: Binding<Bool> {
        Binding(get: { model.screen?.find != nil }, set: { if !$0, model.screen?.find != nil { model.act("find-close") } })
    }

    private func stack(_ s: Screen) -> some View {
        AppTabs(model: model, screen: s, pairing: $pairing)
        .alert(s.error, isPresented: Binding(get: { !s.error.isEmpty }, set: { if !$0 { model.act("dismiss") } })) {
            Button(NativeMessages.native_common_ok(s.locale ?? "en")) { model.act("dismiss") }
        }
        .alert(s.deleting?.title ?? "", isPresented: Binding(get: { s.deleting.map { $0.id != answered } ?? false }, set: { _ in }),
               presenting: s.deleting) { d in
            Button(d.yes, role: .destructive) { answered = d.id; model.act("row-delete", d.id) }
            Button(d.no, role: .cancel) { answered = d.id; model.act("delete-no") }
        } message: { d in
            Text(d.body)
        }
        .onChange(of: s.deleting?.id) { answered = "" }
    }

    private func asked(_ s: Screen) -> some View {
        stack(s)
            .alert(s.renaming?.title ?? "", isPresented: Binding(get: { s.renaming.map { $0.id != renamed } ?? false }, set: { _ in }),
                   presenting: s.renaming) { r in
                TextField(NativeMessages.native_common_title(s.locale ?? "en"), text: $newTitle)
                Button(r.yes) { renamed = r.id; model.act("rename-save", r.id + "\u{1f}" + newTitle) }
                Button(r.no, role: .cancel) { renamed = r.id; model.act("rename-no") }
            }
            .onChange(of: s.renaming?.id) { renamed = ""; newTitle = s.renaming?.text ?? "" }
            .alert(s.removing?.title ?? "", isPresented: Binding(get: { s.removing.map { $0.id != removed } ?? false }, set: { _ in }),
                   presenting: s.removing) { d in
                Button(d.yes, role: .destructive) { removed = d.id; model.act("proj-remove", d.id) }
                Button(d.no, role: .cancel) { removed = d.id; model.act("proj-keep") }
            } message: { d in
                Text(d.body)
            }
            .onChange(of: s.removing?.id) { removed = "" }
    }
}

// First connection stays actionable while the hub is loading or retrying.
private struct ConnectionStartupView: View {
    let model: AppModel
    var initialKey: String? = nil
    @State private var addingAddress = false
    @State private var chosenKey: String?
    private var pairedKeys: [String] { model.links.compactMap(Pairing.key) }
    private var failedKeys: [String] { pairedKeys.filter { model.conn[$0]?.failure == .authentication } }
    private var key: String {
        if let chosenKey, pairedKeys.contains(chosenKey) { return chosenKey }
        if let initialKey, pairedKeys.contains(initialKey) { return initialKey }
        return failedKeys.first ?? pairedKeys.first ?? ""
    }
    private var connection: HubConn { model.conn[key] ?? HubConn() }
    private var tag: String { model.screen?.locale ?? "en" }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    Text("BACKPLANE").font(.system(.caption, design: .monospaced).weight(.bold))
                        .tracking(2).foregroundStyle(BackplaneDesign.accent)
                    Text(HubDot.word(connection.phase, tag))
                        .font(.title2.weight(.semibold))
                    Text(key).font(.system(.body, design: .monospaced)).textSelection(.enabled)
                    if failedKeys.count > 1 {
                        Menu(NativeMessages.native_settings_hubs(tag)) {
                            ForEach(failedKeys, id: \.self) { failed in
                                Button(failed) { chosenKey = failed }
                                    .accessibilityIdentifier("pair.connection.hub." + failed)
                            }
                        }
                        .accessibilityIdentifier("pair.connection.hubs")
                    }
                    if let failure = connection.failure {
                        Text(NativeMessages.native_hub_failure(tag, failure.reason, failure.code))
                            .foregroundStyle(PhaseColor.bad)
                            .accessibilityIdentifier("pair.connection.error")
                    } else if model.startup[key] == .compatibilityRequired {
                        Text(tag == "ja" ? "このハブは更新が必要です。" : "This hub needs an update.")
                            .foregroundStyle(PhaseColor.warn)
                            .accessibilityIdentifier("pair.connection.updateRequired")
                    } else {
                        ProgressView().accessibilityIdentifier("pair.connection.progress")
                    }
                    Button(tag == "ja" ? "再試行" : "Retry") { model.retryConnection(key) }
                        .buttonStyle(PairActionStyle())
                        .accessibilityIdentifier("pair.connection.retry")
                    Button(NativeMessages.native_pair_another(tag)) { addingAddress = true }
                        .accessibilityIdentifier("pair.connection.change")
                }
                .frame(maxWidth: 520, alignment: .leading)
                .padding(24)
            }
            .background(BackplaneDesign.background)
            .sheet(isPresented: $addingAddress) {
                NavigationStack {
                    PairView(tag: tag, link: "") { address in
                        chosenKey = Pairing.key(address)
                        _ = model.pair(address)
                        addingAddress = false
                    }
                    .toolbar {
                        ToolbarItem(placement: .cancellationAction) {
                            Button(NativeMessages.common_cancel(tag)) { addingAddress = false }
                        }
                    }
                }
            }
        }
    }
}

private struct PairActionStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.body.weight(.semibold))
            .frame(maxWidth: .infinity, minHeight: 48)
            .foregroundStyle(BackplaneDesign.onAccent)
            .background(BackplaneDesign.accent.opacity(configuration.isPressed ? 0.7 : 1))
            .contentShape(Rectangle())
    }
}

struct PairView: View {
    let tag: String
    @State var link: String
    let done: (String) -> Void
    @State private var scanning = false
    @State private var invalid = false

    private func connect(_ text: String) {
        guard Pairing.key(text) != nil else { invalid = true; return }
        invalid = false
        link = ""
        done(text)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                VStack(alignment: .leading, spacing: 10) {
                    Text("BACKPLANE")
                        .font(.system(.caption, design: .monospaced).weight(.bold))
                        .tracking(2).foregroundStyle(BackplaneDesign.accent)
                    Text(NativeMessages.native_pair_title(tag)).font(.largeTitle.weight(.bold))
                    Text(NativeMessages.native_pair_computer(tag)).foregroundStyle(.secondary)
                }
                Button { scanning = true } label: {
                    Label(NativeMessages.native_pair_scan(tag), systemImage: "qrcode.viewfinder")
                }
                .buttonStyle(PairActionStyle())
                .accessibilityIdentifier("pair.scan")
                VStack(alignment: .leading, spacing: 8) {
                    Text(tag == "ja" ? "コピーした接続リンクをペーストして接続します。" : "Paste a copied pairing link to connect.")
                        .font(.footnote).foregroundStyle(.secondary)
                    PasteButton(payloadType: String.self) { values in
                        if let value = values.first { connect(value) } else { invalid = true }
                    }
                    .tint(BackplaneDesign.accent)
                    .buttonBorderShape(.roundedRectangle(radius: 0))
                    .accessibilityHint(tag == "ja" ? "ペーストしたリンクで接続します。" : "Connects using the pasted pairing link.")
                    .accessibilityIdentifier("pair.paste")
                }
                VStack(alignment: .leading, spacing: 10) {
                    Text(NativeMessages.settings_pair(tag)).font(.headline)
                    TextField(NativeMessages.settings_pair(tag), text: $link, prompt: Text(verbatim: "http://host:3787/#token=…"))
                        .privacySensitive()
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                        .submitLabel(.go)
                        .onSubmit { connect(link) }
                        .padding(14)
                        .background(BackplaneDesign.panel)
                        .overlay(Rectangle().stroke(invalid ? PhaseColor.bad : BackplaneDesign.line, lineWidth: 1))
                        .accessibilityIdentifier("pair.address")
                    Text(NativeMessages.native_pair_instructions(tag)).font(.footnote).foregroundStyle(.secondary)
                    if invalid {
                        Text(tag == "ja" ? "有効な接続アドレスを入力してください。" : "Enter a valid hub address or pairing link.")
                            .font(.footnote).foregroundStyle(PhaseColor.bad)
                            .accessibilityIdentifier("pair.address.error")
                    }
                }
                Button(NativeMessages.native_pair_connect(tag)) { connect(link) }
                    .buttonStyle(PairActionStyle())
                    .disabled(link.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .opacity(link.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? 0.5 : 1)
                    .accessibilityIdentifier("pair.connect")
            }
            .frame(maxWidth: 520, alignment: .leading)
            .padding(24)
        }
        .background(BackplaneDesign.background)
        .navigationBarTitleDisplayMode(.inline)
        .fullScreenCover(isPresented: $scanning) { ScanView(tag: tag) { connect($0) } }
    }
}

// the hubs this phone is paired with (swipe to unpair), the owner's other
// machines they know of (one tap pairs), and a field for a new link
struct HubsView: View {
    let model: AppModel
    let screen: Screen
    private var tag: String { screen.locale ?? "en" }
    @State private var link = ""
    @State private var scanning = false
    @State private var invalid = false

    private func add(_ text: String) {
        guard Pairing.key(text) != nil else { invalid = true; return }
        invalid = false
        _ = model.pair(text)
        link = ""
    }

    var body: some View {
        Form {
            Section(NativeMessages.native_pair_paired(tag)) {
                ForEach(screen.hubs, id: \.key) { h in
                    let c = model.conn[h.key] ?? HubConn()
                    HStack(alignment: .firstTextBaseline) {
                        HubDot(phase: c.phase).font(.caption)
                        VStack(alignment: .leading, spacing: 2) {
                            HStack {
                                Text(h.name)
                                Spacer()
                                Text(HubDot.word(c.phase, tag)).font(.caption).foregroundStyle(HubDot.tint(c.phase))
                            }
                            Text(HubsView.detail(h, c, model.conn.traffic(h.key), tag)).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                        }
                    }
                    .swipeActions { Button(NativeMessages.native_pair_unpair(tag), role: .destructive) { model.unpair(h.key) } }
                }
            }
            if !screen.found.isEmpty {
                Section(NativeMessages.native_pair_tailnet(tag)) {
                    ForEach(screen.found, id: \.url) { f in
                        Button { model.pair(f.url) } label: {
                            HStack {
                                Text(f.name)
                                Spacer()
                                Image(systemName: "plus.circle")
                            }
                        }
                    }
                }
            }
            Section {
                Text(tag == "ja" ? "コピーした接続リンクをペーストして追加します。" : "Paste a copied pairing link to add a hub.")
                    .font(.footnote).foregroundStyle(.secondary)
                PasteButton(payloadType: String.self) { values in
                    if let value = values.first { add(value) } else { invalid = true }
                }
                .tint(BackplaneDesign.accent)
                .buttonBorderShape(.roundedRectangle(radius: 0))
                .accessibilityHint(tag == "ja" ? "ペーストしたリンクでハブを追加します。" : "Adds a hub using the pasted pairing link.")
                .accessibilityIdentifier("hubs.paste")
                TextField(NativeMessages.settings_pair(tag), text: $link, prompt: Text(verbatim: "http://host:3787/#token=…"))
                    .privacySensitive()
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.URL)
                if invalid {
                    Text(tag == "ja" ? "有効な接続アドレスを入力してください。" : "Enter a valid hub address or pairing link.")
                        .foregroundStyle(PhaseColor.bad).font(.footnote)
                        .accessibilityIdentifier("hubs.address.error")
                }
                Button(NativeMessages.native_pair_add(tag)) { add(link) }
                    .disabled(link.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .accessibilityIdentifier("hubs.add")
                Button { scanning = true } label: { Label(NativeMessages.native_pair_scan(tag), systemImage: "qrcode.viewfinder") }
            } header: {
                Text(NativeMessages.native_pair_another(tag))
            } footer: {
                Text(NativeMessages.native_pair_instructions(tag))
            }
        }
        .navigationTitle(NativeMessages.native_settings_hubs(tag))
        .fullScreenCover(isPresented: $scanning) { ScanView(tag: tag) { add($0) } }
    }

    // host:port · version · changes held · when it last said anything
    static func detail(_ h: HubRow, _ c: HubConn, _ traffic: HubTraffic, _ tag: String) -> String {
        var parts = [h.key]
        if let failure = c.failure, c.phase == .offline || c.phase == .connecting {
            parts.append(NativeMessages.native_hub_failure(tag, failure.reason, failure.code))
            return parts.joined(separator: " · ")
        }
        if let v = h.version, !v.isEmpty { parts.append(v) }
        if c.phase == .syncing, traffic.bytes > 0 {
            parts.append(NativeMessages.native_hub_catchup(tag, ByteCountFormatter.string(fromByteCount: Int64(traffic.bytes), countStyle: .file)))
        } else if let n = h.changes, n > 0 {
            parts.append(NativeMessages.native_hub_changes(tag, n.formatted(.number.locale(Locale(identifier: tag)))))
        }
        if let t = traffic.heard { parts.append(NativeMessages.native_hub_heard(tag, t.formatted(.relative(presentation: .named).locale(Locale(identifier: tag))))) }
        return parts.joined(separator: " · ")
    }
}

// a hub's socket as a dot: green online, orange catching up, grey otherwise
struct HubDot: View {
    let phase: HubConn.Phase

    var body: some View {
        Image(systemName: phase == .online ? "circle.fill" : phase == .syncing ? "circle.lefthalf.filled" : "circle.dotted")
            .foregroundStyle(Self.tint(phase))
    }

    static func tint(_ p: HubConn.Phase) -> Color {
        switch p {
        case .online: .green
        case .syncing: .orange
        case .offline: .red
        case .connecting: .secondary
        }
    }

    static func word(_ p: HubConn.Phase, _ tag: String) -> String {
        switch p {
        case .online: NativeMessages.native_hub_state(tag, "online")
        case .syncing: NativeMessages.native_hub_state(tag, "syncing")
        case .offline: NativeMessages.native_hub_state(tag, "offline")
        case .connecting: NativeMessages.native_hub_state(tag, "connecting")
        }
    }
}

// every paired hub at a glance, in the list's toolbar: one dot each (up to
// four), and a tap opens the hubs
struct HubsPill: View {
    let model: AppModel
    let hubs: [HubRow]
    let open: () -> Void

    var body: some View {
        Button(action: open) {
            HStack(spacing: 3) {
                ForEach(hubs.prefix(4), id: \.key) { h in HubDot(phase: model.conn[h.key]?.phase ?? .connecting) }
                if hubs.count > 4 { Text("+\(hubs.count - 4)") }
            }
            .font(.system(size: 10))
            .frame(minWidth: 44, minHeight: 44)
            .contentShape(Rectangle())
        }
        .accessibilityIdentifier("navigation.hubs")
        .accessibilityLabel(hubs.map { NativeMessages.native_hub_accessibility(model.screen?.locale ?? "en", $0.name, HubDot.word(model.conn[$0.key]?.phase ?? .connecting, model.screen?.locale ?? "en")) }.joined(separator: ", "))
    }
}

private struct SwipeButton: View {
    let model: AppModel
    let swipe: Swipe
    let choose: (Swipe) -> Void

    private var icon: String {
        switch swipe.action {
        case "row-delete": "trash"
        case "row-settle": "checkmark.circle"
        case "row-unsettle": "arrow.uturn.backward"
        default: swipe.options.isEmpty ? "sun.max" : "moon.zzz"
        }
    }

    private var tint: Color {
        switch swipe.tone {
        case "danger": .red
        case "settle": .green
        default: .indigo
        }
    }

    var body: some View {
        Button {
            if swipe.options.isEmpty { model.act(swipe.action, swipe.value) } else { choose(swipe) }
        } label: {
            Label(swipe.label, systemImage: icon)
        }
        .tint(tint)
    }
}

// A tap opens the thread; a long press asks for its menu ("row-menu"),
// which the screen then carries as rowMenu. Tap and long press are plain
// gestures (not a NavigationLink) so a long press never also opens it.
private struct ThreadRow: View {
    @Environment(\.backplaneLanguage) private var lang
    @Environment(\.dynamicTypeSize) private var typeSize
    let model: AppModel
    let row: Row
    var hierarchy = true
    let choose: (Swipe) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 0) {
                if hierarchy && ((row.children ?? 0) > 0 || row.canChild == true) {
                    Button { model.act("thread-tree", row.id) } label: {
                        Image(systemName: row.expanded == true ? "chevron.down" : "chevron.right")
                            .font(.caption.weight(.semibold))
                            .frame(width: 32, height: 44)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.borderless)
                    .accessibilityLabel((row.expanded == true ? "Collapse subthreads of " : "Expand subthreads of ") + row.title)
                    .accessibilityValue(String(row.children ?? 0) + " subthreads")
                    .accessibilityIdentifier("thread.tree." + row.id)
                } else {
                    Color.clear.frame(width: 32, height: 1).accessibilityHidden(true)
                }
                // Only this sibling area selects/opens the row. A chevron tap
                // never enters the thread or fires its long-press menu.
                HStack(spacing: 10) {
                    StatusDot(state: row.state, status: row.status).frame(width: 18)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(row.title).font(.body.weight(.medium))
                            .lineLimit(typeSize.isAccessibilitySize ? 3 : 2)
                        if let p = row.project, !p.isEmpty {
                            Text(p).font(.caption).foregroundStyle(.secondary).lineLimit(typeSize.isAccessibilitySize ? 2 : 1)
                        }
                        if typeSize.isAccessibilitySize {
                            Text(row.agents.flatMap { $0.isEmpty ? nil : $0 } ?? row.ago)
                                .font(.caption).foregroundStyle(.secondary)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .layoutPriority(1)
                    if row.pinned { Image(systemName: "pin.fill").font(.caption).foregroundStyle(.secondary).accessibilityLabel(lang == "ja" ? "ピン留め" : "Pinned") }
                    if !typeSize.isAccessibilitySize {
                        if let n = row.agents, !n.isEmpty {
                            Text(n).font(.caption).foregroundStyle(PhaseColor.accent).lineLimit(1)
                                .frame(maxWidth: 80, alignment: .trailing)
                        } else {
                            Text(row.ago).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                                .frame(maxWidth: 72, alignment: .trailing)
                        }
                    }
                    Image(systemName: "chevron.right").font(.caption.weight(.semibold)).foregroundStyle(.tertiary)
                }
                .frame(maxWidth: .infinity, minHeight: 52, alignment: .leading)
                .contentShape(Rectangle())
                .onTapGesture { model.open(nav: row.id, action: "select", value: row.id, title: row.title) }
                .onLongPressGesture(minimumDuration: 0.4) {
                    Haptics.impact()
                    model.act("row-menu", row.id)
                }
                .accessibilityElement(children: .combine)
                .accessibilityAddTraits(.isButton)
                .accessibilityAction { model.open(nav: row.id, action: "select", value: row.id, title: row.title) }
                .accessibilityAction(named: NativeMessages.native_thread_menu(lang)) { model.act("row-menu", row.id) }
                .accessibilityIdentifier("thread.row." + row.id)
            }
            if hierarchy && row.expanded == true && row.canChild == true {
                Button { model.act("thread-child", row.id) } label: {
                    Label("New subthread", systemImage: "plus")
                        .font(.caption)
                        .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.borderless)
                .padding(.leading, 60)
                .accessibilityIdentifier("thread.child.row." + row.id)
            }
        }
        .padding(.leading, CGFloat(min(max(row.depth ?? 0, 0), 4) * 12))
        .listRowBackground(BackplaneDesign.background)
        .listRowSeparatorTint(BackplaneDesign.line)
        .opacity(row.faded == true ? 0.45 : 1)
        .swipeActions(edge: .leading) {
            ForEach(row.lead, id: \.self) { SwipeButton(model: model, swipe: $0, choose: choose) }
        }
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            ForEach(row.trail, id: \.self) { SwipeButton(model: model, swipe: $0, choose: choose) }
        }
    }
}

struct ProjectsView: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let screen: Screen
    @Binding var pairing: Bool
    // a swipe whose choices are up (a snooze)
    @State private var choosing: Swipe?
    // the row menu this dialog let go of: it stays down until the screen drops it
    @State private var shutMenu: RowMenu?

    // a folded shelf: its name and count, a tap opens or shuts it
    func foldRow(_ settled: Bool, _ o: Older, _ action: String) -> some View {
        Section {
            Button { model.act(action, "") } label: {
                HStack {
                    Text(settled ? NativeMessages.sidebar_settled_count(lang, String(o.count)) : NativeMessages.sidebar_older(lang, String(o.count))).font(.subheadline)
                    Spacer()
                    Image(systemName: o.open ? "chevron.down" : "chevron.right").font(.caption).foregroundStyle(.tertiary)
                }
            }
            .tint(.secondary)
        }
    }

    var body: some View {
        List {
            // the search is always the list's first row (projects, or in the
            // active view threads by title or project)
            if let f = screen.search {
                Section { SearchField(model: model, search: f) }
            }
            if screen.view == "active" {
                let rows = screen.active ?? []
                if rows.isEmpty && screen.settled == nil {
                    if !screen.projects.isEmpty && (screen.search?.query ?? "").isEmpty {
                        ContentUnavailableView(NativeMessages.sidebar_nothing(lang), systemImage: "tray")
                            .listRowBackground(Color.clear)
                    }
                } else {
                    // live rows; the settled ones follow the fold while it is open
                    Section {
                        ForEach(rows.filter { !($0.faded ?? false) }) { ThreadRow(model: model, row: $0) { choosing = $0 } }
                    }
                    if let o = screen.settled { foldRow(true, o, "side-settled") }
                    let faded = rows.filter { $0.faded ?? false }
                    if !faded.isEmpty {
                        Section {
                            ForEach(faded) { ThreadRow(model: model, row: $0) { choosing = $0 } }
                        }
                    }
                }
            } else {
                ForEach(screen.projects) { p in projectSection(p) }
                if let o = screen.older { foldRow(false, o, "side-older") }
            }
            if let legacy = screen.legacy, !legacy.isEmpty {
                Section("Conversation history") {
                    ForEach(legacy) { ThreadRow(model: model, row: $0, hierarchy: false) { choosing = $0 } }
                }
            }
        }
        .modifier(BackplaneListSurface())
        .overlay {
            if screen.projects.isEmpty && (screen.legacy ?? []).isEmpty && (screen.active ?? []).isEmpty {
                // a hub still catching up has not said what there is yet
                if let h = screen.hubs.first(where: { (model.conn[$0.key]?.phase ?? .connecting) != .online }), !screen.online {
                    VStack(spacing: 12) {
                        if model.conn[h.key]?.failure == nil {
                            ProgressView().accessibilityLabel(HubDot.word(model.conn[h.key]?.phase ?? .connecting, lang))
                        }
                        Text(model.conn[h.key]?.failure.map { NativeMessages.native_hub_failure(lang, $0.reason, $0.code) }
                             ?? NativeMessages.native_hub_connection(lang, HubDot.word(model.conn[h.key]?.phase ?? .connecting, lang), h.name))
                            .foregroundStyle(.secondary).multilineTextAlignment(.center)
                    }
                    .padding()
                    .accessibilityIdentifier("projects.connection.status")
                } else {
                    ContentUnavailableView(screen.empty, systemImage: "folder")
                }
            }
        }
        .navigationTitle(NativeMessages.tour_title_threads(lang))
        .navigationBarTitleDisplayMode(.inline)
        .accessibilityIdentifier("navigation.threads")
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                HubsPill(model: model, hubs: screen.hubs) { pairing = true }
            }
            ToolbarItemGroup(placement: .topBarTrailing) {
                // with several hubs, the picker opens on the one chosen
                if screen.hubs.count > 1 {
                    Menu {
                        ForEach(screen.hubs, id: \.key) { h in Button(h.name) { model.act("picker-open", h.key + "|") } }
                    } label: { Image(systemName: "folder.badge.plus") }
                    .accessibilityLabel(NativeMessages.sidebar_add_project(lang))
                } else {
                    Button { model.act("picker-open") } label: { Image(systemName: "folder.badge.plus") }.accessibilityLabel(NativeMessages.sidebar_add_project(lang))
                }
                Button { model.switchTab(.settings) } label: { Image(systemName: "gearshape") }
                    .accessibilityLabel(NativeMessages.palette_settings(lang))
                Menu {
                    Button(NativeMessages.palette_search(lang), systemImage: "magnifyingglass") { model.act("find-open", "search") }
                } label: { Image(systemName: "ellipsis.circle") }
                .accessibilityLabel(NativeMessages.common_more(lang))
            }
        }
        .confirmationDialog(choosing?.label ?? "", isPresented: Binding(get: { choosing != nil }, set: { if !$0 { choosing = nil } }),
                            titleVisibility: .visible, presenting: choosing) { s in
            ForEach(s.options, id: \.self) { o in Button(o.label) { model.act(s.action, o.value) } }
        }
        // a long-pressed row's or project's menu: an item sends its action
        // (Delete and Remove then ask through the deleting/removing alerts);
        // let go without a choice, the menu is closed ("menu-close")
        .confirmationDialog(screen.rowMenu?.title ?? "", isPresented: menuUp, titleVisibility: .visible, presenting: screen.rowMenu) { m in
            ForEach(m.items, id: \.self) { i in
                Button(i.label, role: i.tone == "danger" ? .destructive : nil) { pick(m, i) }
            }
            Button(NativeMessages.common_cancel(lang), role: .cancel) {}
        }
        .onChange(of: screen.rowMenu) { _, m in if m == nil { shutMenu = nil } }
        .sheet(isPresented: Binding(get: { screen.folders != nil }, set: { if !$0 { model.act("proj-close") } })) {
            if let f = screen.folders { FoldersSheet(model: model, folders: f) }
        }
    }

    // the row menu shows while the screen has one this dialog has not let go
    // of (a file link's shows over its thread)
    private var menuUp: Binding<Bool> {
        Binding(get: { screen.rowMenu.map { $0.kind != "f" && $0 != shutMenu } ?? false }, set: { up in if !up { letGo() } })
    }

    private func pick(_ m: RowMenu, _ i: MenuItem) {
        shutMenu = m
        model.act(i.action, i.value)
    }

    // let go of: an item picked has closed or changed the menu by now; if
    // not, none was, and the menu is closed
    private func letGo() {
        guard let m = screen.rowMenu else { return }
        shutMenu = m
        let model = self.model
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) {
            let still: RowMenu? = model.screen?.rowMenu
            if still == m { model.act("menu-close", "") }
        }
    }

    // a project: its threads, the snoozed and settled shelves, the archived
    // one; its header (faded when quiet) takes a long press for its menu
    @ViewBuilder
    private func projectSection(_ p: Project) -> some View {
        Section {
            ForEach(p.threads) { ThreadRow(model: model, row: $0) { choosing = $0 } }
            if !p.snoozed.isEmpty {
                Text(p.snoozedShelf).font(.subheadline).foregroundStyle(.secondary)
                ForEach(p.snoozed) { ThreadRow(model: model, row: $0) { choosing = $0 } }
            }
            if !p.settled.isEmpty {
                Button { model.act("toggle-settled", p.id) } label: {
                    HStack {
                        Text(p.shelf).font(.subheadline)
                        Spacer()
                        Image(systemName: p.open ? "chevron.down" : "chevron.right").font(.caption).foregroundStyle(.tertiary)
                    }
                }
                .tint(.secondary)
                if p.open { ForEach(p.settled) { ThreadRow(model: model, row: $0) { choosing = $0 } } }
            }
            if let arch = p.archived, !arch.isEmpty, let v = p.value {
                Button { model.act("toggle-settled", v) } label: {
                    HStack {
                        Text(NativeMessages.native_thread_archived(lang, String(arch.count))).font(.subheadline)
                        Spacer()
                        Image(systemName: p.archOpen == true ? "chevron.down" : "chevron.right").font(.caption).foregroundStyle(.tertiary)
                    }
                }
                .tint(.secondary)
                if p.archOpen == true { ForEach(arch) { ThreadRow(model: model, row: $0) { choosing = $0 } } }
            }
        } header: {
            HStack {
                VStack(alignment: .leading) {
                    Text(p.title).font(.subheadline.weight(.semibold)).textCase(nil)
                    Text(p.machine.isEmpty ? p.root : p.machine + ": " + p.root)
                        .font(.caption2).textCase(nil).lineLimit(1).truncationMode(.head)
                }
                .opacity(p.quiet == true ? 0.5 : 1)
                .accessibilityIdentifier("project.row." + p.id)
                Spacer()
                Button { model.act("new-thread", p.id) } label: { Image(systemName: "square.and.pencil") }
                    .accessibilityLabel(NativeMessages.thread_new(lang))
                    .accessibilityIdentifier("project.new-thread." + p.id)
            }
            .padding(.vertical, 8)
            .padding(.horizontal, 12)
            .background(BackplaneDesign.panel)
            .overlay(alignment: .leading) { Rectangle().fill(BackplaneDesign.accent).frame(width: 2) }
            .contentShape(Rectangle())
            .onLongPressGesture(minimumDuration: 0.4) {
                Haptics.impact()
                model.act("proj-menu", p.id)
            }
            .accessibilityAction(named: NativeMessages.native_thread_menu(lang)) { model.act("proj-menu", p.id) }
            .accessibilityIdentifier("project.header." + p.id)
        }
    }
}

// the list's search field, always shown, never focused on its own:
// typing filters the list ("proj-find-q"), return goes to the first
// project shown ("proj-go"), the x empties it
private struct SearchField: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let search: Search
    @State private var text = ""
    @State private var scope: UInt64?
    @State private var edited = false

    private func edit(_ value: String) {
        text = value
        edited = true
        if let scope { self.scope = model.searchProjects(value, scope: scope) ?? scope }
    }

    var body: some View {
        HStack {
            Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
            TextField(search.hint, text: Binding(get: { text }, set: { edit($0) }), prompt: Text(search.hint).foregroundColor(BackplaneDesign.secondaryText))
                .foregroundColor(.primary)
                .accessibilityIdentifier("projects.search")
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .submitLabel(.go)
                .onSubmit { if let scope { model.submitProjectSearch(text, scope: scope) } }
            if !text.isEmpty {
                Button { edit("") } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary) }
                    .buttonStyle(.plain)
                    .accessibilityLabel(NativeMessages.native_thread_search_clear(lang))
            }
        }
        .onAppear {
            let query = model.beginProjectSearch(search.query)
            scope = query.scope
            text = query.text
            edited = edited || query.edit > 0 || text != search.query
        }
        .onDisappear { if let scope { model.endProjectSearch(scope: scope) }; scope = nil }
        .onChange(of: model.contentReady) { _, ready in
            if ready, text != search.query, let scope {
                self.scope = model.resumeProjectSearch(text, scope: scope) ?? scope
            }
        }
        .onChange(of: model.projectSearchRevision) { _, _ in
            if model.contentReady, let scope {
                self.scope = model.resumeProjectSearch(text, scope: scope) ?? scope
            }
        }
        .onChange(of: search.query) { _, q in
            if !edited { text = q }
        }
    }
}

// the project picker: a path field over the listed folder's rows
private struct FoldersSheet: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let folders: Folders
    @State private var text = ""
    // what was typed here: a screen still echoing it never overwrites the field
    @State private var typed: Set<String> = []

    var body: some View {
        NavigationStack {
            List {
                Section {
                    TextField(folders.hint, text: $text)
                        .font(.body.monospaced())
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .onChange(of: text) { _, t in
                            guard t != folders.text else { return }
                            typed.insert(t)
                            model.act("picker-type", t)
                        }
                    if !folders.error.isEmpty { Text(folders.error).font(.footnote).foregroundStyle(.red) }
                }
                Section {
                    ForEach(folders.items, id: \.self) { r in
                        Button { model.act(r.action, r.value) } label: {
                            Label(r.label, systemImage: Self.icon(r.kind)).lineLimit(1).truncationMode(.head)
                        }
                        .disabled(r.action.isEmpty)
                        .tint(r.kind == "dir" || r.kind == "up" ? .primary : .accentColor)
                    }
                }
            }
            .navigationTitle(NativeMessages.sidebar_add_project(lang))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button(NativeMessages.common_cancel(lang)) { model.act("proj-close") } }
            }
        }
        .onAppear { text = folders.text }
        .onChange(of: folders.text) { _, t in
            if !typed.contains(t) { typed = []; text = t }
        }
    }

    static func icon(_ kind: String) -> String {
        switch kind {
        case "up": "arrow.turn.left.up"
        case "add": "plus.circle"
        case "new": "folder.badge.plus"
        case "mkdir": "folder.badge.plus"
        case "off": "checkmark.circle"
        case "machine": "desktopcomputer"
        default: "folder"
        }
    }
}

// What a pushed id shows. The screen's own thread (or bot) once the client
// has selected it; until then the same thread as it was last shown, or its
// title from the list. Never another thread: the screen may still hold the
// one before for a moment after a tap.
struct ThreadDestination: View {
    @Environment(\.backplaneLanguage) private var lang
    let model: AppModel
    let id: String
    @State private var attempt = 0
    @State private var timedOut = false

    private struct WatchKey: Equatable {
        let id: String
        let attempt: Int
        let active: Bool
        let ready: Bool
        let links: [String]
    }

    private var selected: Bool { model.screen.map { AppModel.nav($0) == id } ?? false }
    private var active: Bool { model.path.last == id }
    private var ready: Bool {
        selected && model.contentReady && (model.screen?.bot != nil || model.screen?.thread != nil)
    }
    private var watchKey: WatchKey {
        WatchKey(id: id, attempt: attempt, active: active, ready: ready, links: model.links)
    }
    private var hub: String? {
        model.links.compactMap(Pairing.key).first { id.hasPrefix($0 + "|") }
            ?? (model.screen?.hub.isEmpty == false ? model.screen?.hub : nil)
    }
    private var failureMessage: String {
        if let hub, let failure = model.conn[hub]?.failure {
            return NativeMessages.native_hub_failure(lang, failure.reason, failure.code)
        }
        if id == AppModel.making {
            return lang == "ja" ? "スレッドの作成に時間がかかっています。" : "Creating this thread timed out."
        }
        return lang == "ja" ? "会話を開くのに時間がかかっています。" : "Opening this conversation timed out."
    }

    private var composerThread: ThreadView? {
        let bot = (selected ? model.screen?.bot : nil) ?? model.seenBots[id]
        if let bot, bot.kind != "bot" || (bot.tab ?? "chat") != "chat" { return nil }
        let thread = (selected ? model.screen?.thread : nil) ?? model.seen[id]
        guard let thread, thread.id == id else { return nil }
        return thread
    }

    var body: some View {
        VStack(spacing: 0) {
        Group {
            if let b = (selected ? model.screen?.bot : nil) ?? model.seenBots[id] {
                BotDestination(model: model, bot: b, live: ready,
                               cachedThread: selected ? model.screen?.thread : model.seen[id],
                               cachedPlane: selected ? model.screen.flatMap { model.plane($0) } : model.seenPlanes[id])
                    .allowsHitTesting(ready)
                    .disabled(!ready)
                    .overlay(alignment: .top) { if !ready { loadingStatus } }
            } else if let t = (selected ? model.screen?.thread : nil) ?? model.seen[id] {
                ThreadScreen(model: model, thread: t, live: ready).id(id)
                    .overlay(alignment: .top) { if !ready { loadingStatus } }
            } else {
                VStack(alignment: .leading, spacing: 20) {
                    HStack(spacing: 12) {
                        if let d = model.destination, d.nav == id, !d.cat.isEmpty {
                            BotCat(model: model, key: d.cat, size: 48)
                        }
                        Text(title).font(.headline)
                    }
                    loadingStatus
                    if !timedOut {
                        ForEach(0..<6) { _ in
                            VStack(alignment: .leading, spacing: 8) {
                                Text(NativeMessages.native_thread_content(lang)).font(.headline)
                                Text(NativeMessages.native_thread_content_note(lang))
                            }.redacted(reason: .placeholder).accessibilityHidden(true)
                        }
                    }
                    Spacer()
                }
                .padding().frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .navigationTitle(title)
                .navigationBarTitleDisplayMode(.inline)
            }
        }
        if let thread = composerThread {
            ThreadComposer(model: model, thread: thread)
        }
        }
        .modifier(ViewerPresentationHost(model: model, thread: id))
        .task(id: watchKey) {
            let started = watchKey
            timedOut = false
            guard started.active, !started.ready else { return }
            do { try await Task.sleep(for: .seconds(20)) } catch { return }
            guard !Task.isCancelled, watchKey == started, active, !ready else { return }
            timedOut = true
        }
    }

    private var loadingLabel: String {
        if id == AppModel.making { return lang == "ja" ? "スレッドを作成中…" : "Creating thread…" }
        return lang == "ja" ? "会話を開いています…" : "Opening conversation…"
    }

    private var loadingStatus: some View {
        VStack(alignment: .leading, spacing: 8) {
            if timedOut {
                Text(failureMessage).accessibilityIdentifier("thread.loading.error")
                if id != AppModel.making {
                    Button(lang == "ja" ? "再試行" : "Retry") {
                        guard active, !ready, let hub, model.links.contains(where: { Pairing.key($0) == hub }) else { return }
                        timedOut = false
                        attempt += 1
                        model.act("select", id)
                    }
                    .accessibilityIdentifier("thread.loading.retry")
                }
            } else {
                ProgressView(loadingLabel).accessibilityIdentifier("thread.loading")
            }
        }
        .font(.caption)
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(BackplaneDesign.panel)
    }

    // the row's title in the list, while nothing of the thread is known
    private var title: String {
        if id == AppModel.making { return NativeMessages.thread_new(lang) }
        if let d = model.destination, d.nav == id { return d.title }
        for p in model.screen?.projects ?? [] {
            for r in p.threads + p.snoozed + p.settled + (p.archived ?? []) where r.id == id { return r.title }
        }
        for r in model.screen?.active ?? [] where r.id == id { return r.title }
        return ""
    }
}

// a message on its way: at once when sent, until the hub has it
private struct SendingBubble: View {
    let text: String
    let note: String

    var body: some View {
        VStack(alignment: .trailing, spacing: 2) {
            Text(text)
                .padding(.horizontal, 14).padding(.vertical, 10)
                .background(Color.accentColor.opacity(0.08), in: .rect(cornerRadius: 18))
            HStack(spacing: 4) {
                ProgressView().controlSize(.mini)
                Text(note)
            }
            .font(.caption2).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .trailing)
        .opacity(0.7)
    }
}

struct ThreadScreen: View {
    @Environment(\.backplaneLanguage) private var lang
    @Bindable var model: AppModel
    let thread: ThreadView
    // A last-seen thread remains scrollable while selection or reconnect is
    // pending. Entry actions and hub controls require the fresh selection.
    var live = true
    // the image open in the lightbox
    @State private var shown: Shown?
    @State private var position = TimelinePosition()
    // the entry a jump showed, tinted for a moment
    @State private var lit: String?
    // the file menu this dialog let go of
    @State private var shutFile: RowMenu?
    // the subagents sheet (the top bar's button) is up
    @State private var showSubs = false

    var body: some View {
      let displayRevision = model.performanceRevision
      VStack(spacing: 0) {
        TimelineScroll(position: position, ids: thread.entries.map(\.id), earlier: thread.earlier ?? 0,
                       loadEarlier: { live && model.loadEarlier(thread.id) },
                       earlierEnabled: live && model.contentReady && model.screen?.thread?.id == thread.id,
                       onDisplay: PerformanceTrace.shared.begin() == nil ? nil : {
                           model.performanceDisplayed(thread: thread.id, fresh: live, revision: displayRevision)
                       }) {
                VStack(alignment: .leading, spacing: 14) {
                    if let p = thread.parent { EntryRow(model: model, entry: p) { shown = $0 } }
                    ForEach(thread.entries) { e in
                        VStack(alignment: .leading, spacing: 6) {
                            if e.kind == "emb" {
                                EmbRow(model: model, entry: e, emb: thread.embs?.first { $0.key == e.key })
                            } else {
                                EntryRow(model: model, entry: e) { shown = $0 }
                            }
                            if let partial = e.partial {
                                PartialMessageFooter(model: model, hub: model.screen?.hub ?? "", thread: thread.id,
                                                     entry: e.id, partial: partial, selected: live)
                            }
                        }
                        .background(Color.accentColor.opacity(lit == e.id ? 0.18 : 0))
                        .id(e.id)
                        .background(TimelineMarker(id: e.id))
                        .accessibilityElement(children: .contain)
                        .accessibilityIdentifier("thread.entry." + e.id)
                    }
                    // the client's sending rows, then those tapped here it has
                    // not answered yet, by place: one handed over keeps its place
                    ForEach(Array(sending.enumerated()), id: \.offset) { _, text in
                        SendingBubble(text: text, note: sendNote)
                            .transition(.asymmetric(insertion: .move(edge: .bottom).combined(with: .opacity), removal: .opacity))
                    }
                    if !thread.live.isEmpty {
                        MarkdownView(blocks: thread.live)
                    } else if !thread.working.isEmpty {
                        HStack(spacing: 8) {
                            StatusDot(state: thread.state, status: thread.phase)
                            Text(thread.working).foregroundStyle(.secondary)
                        }
                        .font(thread.state == "run" ? .body : .caption)
                    }
                    Color.clear.frame(height: 1).id("end")
                }
                .padding()
                .environment(\.backplaneLanguage, lang)
                // Gate the hosted controls, leaving the enclosing UIKit
                // scroll view available for reading the matching cached thread.
                .disabled(!live)
                .allowsHitTesting(live)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .overlay(alignment: .bottomTrailing) {
            if !position.following {
                Button { position.latest?() } label: {
                    Label(position.unread > 0 ? NativeMessages.native_thread_new(lang, String(position.unread)) : NativeMessages.native_thread_latest(lang), systemImage: "arrow.down")
                        .font(.caption.bold()).padding(.horizontal, 12).padding(.vertical, 8)
                        .background(.regularMaterial, in: Capsule())
                }
                .accessibilityIdentifier("thread.latest")
                .padding(12)
            }
        }
        .onAppear { if live { model.hold(false, thread: thread) } }
        .onChange(of: position.following) { _, following in
            if live { model.hold(!following, thread: thread) }
        }
        .onChange(of: live) { _, selected in
            if selected { model.hold(!position.following, thread: thread) }
        }
        .onChange(of: model.jump) { _, j in
            guard live, let j else { return }
            position.jump?(j.id)
            lit = j.id
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(400))
                withAnimation(.easeOut(duration: 2)) { if lit == j.id { lit = nil } }
            }
        }
      }
        .navigationTitle(thread.title)
        .navigationBarTitleDisplayMode(.inline)
        // a file link tapped in a message asks for its menu (Open and Show
        // in folder act on the hub's machine, Copy path here)
        .environment(\.openURL, OpenURLAction { url in
            guard live else { return .discarded }
            if model.open(url, sourceHub: model.screen?.hub) { return .handled }
            guard let p = FileLink.path(url) else { return .systemAction }
            model.act("file-menu", p)
            return .handled
        })
        .confirmationDialog(fileMenu?.title ?? "", isPresented: fileUp, titleVisibility: .visible, presenting: fileMenu) { m in
            ForEach(m.items, id: \.self) { i in
                Button(i.label) { shutFile = m; model.act(i.action, i.value) }.disabled(!live)
            }
            Button(NativeMessages.common_cancel(lang), role: .cancel) {}
        }
        .onChange(of: fileMenu) { _, m in if m == nil { shutFile = nil } }
        .fullScreenCover(item: $shown) { s in Lightbox(shown: s) { shown = nil } }
        .sheet(isPresented: $showSubs) { SubsSheet(model: model).disabled(!live) }
        .sheet(isPresented: Binding(get: { thread.diff != nil }, set: {
            if !$0, live, model.contentReady, model.screen?.thread?.id == thread.id,
               model.screen?.thread?.diff != nil { model.act("panel") }
        })) {
            if let d = thread.diff { DiffSheet(model: model, diff: d).disabled(!live) }
        }
        .sheet(isPresented: Binding(get: { thread.term != nil }, set: {
            if !$0, live, model.contentReady, model.screen?.thread?.id == thread.id,
               model.screen?.thread?.term != nil { model.act("term-toggle") }
        })) {
            if let t = thread.term { TermSheet(model: model, term: t).disabled(!live).presentationDetents([.large]) }
        }
        .toolbar {
            if model.mailroomReturn != nil {
                ToolbarItem(placement: .topBarLeading) {
                    Button { model.backToMailroom() } label: {
                        Label(NativeMessages.native_mailroom_title(lang), systemImage: "chevron.left")
                    }.accessibilityIdentifier("mailroom.return")
                }
            }
            if !thread.branch.isEmpty || !(thread.boat ?? "").isEmpty {
                ToolbarItem(placement: .principal) {
                    VStack(spacing: 0) {
                        Text(thread.title).font(.headline).lineLimit(1)
                        if !thread.branch.isEmpty {
                            Text(thread.branch).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                        }
                        // the thread's Boat chip: a tap opens Settings (Usage)
                        if let b = thread.boat, !b.isEmpty {
                            Button(b) { model.act("usage-open") }.font(.caption2).lineLimit(1).disabled(!live)
                        }
                    }
                }
            }
            ToolbarItemGroup(placement: .topBarTrailing) {
                // the subagents sit in the top bar, a sheet a tap away, so
                // they never cover the thread
                if let u = thread.subs {
                    Button { showSubs = true } label: {
                        HStack(spacing: 3) {
                            Image(systemName: "person.2")
                            Text(String((u.word ?? u.busy).prefix { $0.isNumber })).font(.caption.monospacedDigit())
                        }
                        .foregroundStyle(u.busy.isEmpty ? Color.secondary : PhaseColor.accent)
                    }
                    .accessibilityLabel(NativeMessages.native_thread_subagents(lang, u.word ?? u.busy))
                    .disabled(!live)
                }
                ForEach(thread.tools.filter { $0.action == "interrupt" }, id: \.self) { t in
                    Button(t.label, systemImage: "stop.fill") { model.act(t.action) }
                        .disabled(!live)
                        .accessibilityIdentifier("thread.interrupt")
                }
                Menu {
                    if thread.canChild == true {
                        Button("New subthread", systemImage: "plus.bubble") { model.act("thread-child", thread.id) }
                            .accessibilityIdentifier("thread.child." + thread.id)
                    }
                    ForEach(thread.tools.filter { $0.action != "interrupt" }, id: \.self) { t in
                        Button { model.act(t.action, t.value ?? "") } label: {
                            Label(t.label, systemImage: t.on ? "checkmark" : Self.icon(t.icon))
                        }
                    }
                    Divider()
                    ForEach(thread.menu ?? [], id: \.self) { t in
                        if let os = t.options, !os.isEmpty {
                            Menu(t.label) {
                                ForEach(os, id: \.self) { o in Button(o.label) { model.act(t.action, o.value) } }
                            }
                        } else {
                            Button(role: t.danger == true ? .destructive : nil) {
                                // the terminal opens at the size this phone has room for
                                model.act(t.action, t.action == "term-toggle" ? TermSheet.size() : t.value ?? "")
                            } label: {
                                Label(t.label, systemImage: t.on ? "checkmark" : Self.icon(t.icon))
                            }
                        }
                    }
                } label: {
                    Image(systemName: "ellipsis.circle")
                }
                .disabled(!live)
                .accessibilityIdentifier("thread.tools")
            }
        }
    }

    // the screen's menu when it is a file link's
    private var fileMenu: RowMenu? {
        guard live, model.screen?.thread?.id == thread.id else { return nil }
        return model.screen?.rowMenu.flatMap { $0.kind == "f" ? $0 : nil }
    }

    // shown until picked from or let go of; let go with no pick, it is closed
    private var fileUp: Binding<Bool> {
        Binding(get: { fileMenu.map { $0 != shutFile } ?? false }, set: { up in
            guard !up, let m = fileMenu else { return }
            shutFile = m
            let model = self.model, owner = thread.id
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) {
                if model.contentReady, model.screen?.thread?.id == owner,
                   model.screen?.rowMenu == m { model.act("menu-close", "") }
            }
        })
    }

    // this thread's messages tapped here and not yet answered
    private var mine: [Outgoing] { model.outgoing.filter { $0.thread == thread.id } }

    private var sending: [String] { thread.sending + mine.map(\.text) }

    // what a message on its way waits for
    private var sendNote: String {
        guard let s = model.screen else { return NativeMessages.native_thread_sending(lang) }
        let c = model.conn[s.hub]?.phase ?? .connecting
        return c == .online || c == .syncing ? NativeMessages.native_thread_sending(lang) : NativeMessages.native_thread_waiting(lang, s.hubs.first { $0.key == s.hub }?.name ?? NativeMessages.native_thread_hub(lang))
    }

    // a tool's icon (core/icons.bend's name) as an SF Symbol
    static func icon(_ name: String?) -> String {
        switch name ?? "" {
        case "pin": "pin"
        case "circle-check": "checkmark.circle"
        case "clock": "clock"
        case "archive": "archivebox"
        case "trash": "trash"
        case "git-fork": "arrow.triangle.branch"
        case "undo": "arrow.uturn.backward"
        case "terminal": "terminal"
        case "panel-right": "sidebar.right"
        case "diff": "plusminus"
        case "search": "doc.text.magnifyingglass"
        case "stop": "stop.fill"
        case "x": "xmark"
        case "box": "cart"
        default: "circle"
        }
    }
}

// the first-run tour: one step (its text comes from core/onboard.bend),
// Skip, Back and Next; Skip and the last step's Done end it for every
// client of the hub
struct TourSheet: View {
    let model: AppModel
    let tour: Tour

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    Text(tour.counter).font(.caption).foregroundStyle(.secondary)
                    Text(tour.title).font(.title3.weight(.semibold))
                    Text(tour.body).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            ForEach(tour.choices, id: \.label) { c in
                Button(c.label) { model.act(c.action, c.value) }.buttonStyle(.bordered)
            }
            HStack {
                Button(tour.skip) { model.act("ob-skip") }
                Spacer()
                if !tour.first { Button(tour.back) { model.act("ob-back") } }
                Button(tour.next) { model.act("ob-next") }.buttonStyle(.borderedProminent)
            }
        }
        .padding(20)
        .presentationDetents([.medium, .large])
    }
}

// whether nothing is presented over the app's root view controller
// (sheets, alerts, dialogs and full-screen covers, going away included)
enum Presented {
    @MainActor static var none: Bool {
        let root = UIApplication.shared.connectedScenes
            .compactMap { ($0 as? UIWindowScene)?.keyWindow?.rootViewController }.first
        return root?.presentedViewController == nil
    }
}
