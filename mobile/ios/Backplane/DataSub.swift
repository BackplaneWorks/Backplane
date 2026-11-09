import SwiftUI

/// The thread a subagent row opens its view over: set on a thread's screen when the hub has feature `subview` ("" elsewhere: no row opens one).
private struct DataSubThreadKey: EnvironmentKey { static let defaultValue = "" }
extension EnvironmentValues { var dataSubThread: String { get { self[DataSubThreadKey.self] } set { self[DataSubThreadKey.self] = newValue } } }

/// One subagent's own view over its thread: what it was asked, what it said, each tool call with its result and how it ended; live while it works.
/// The navigation bar's back returns to the thread.
struct DataSubView: View {
    @Bindable var client: DataClient
    let route: DSubRoute
    private var v: DSubView? { client.subView.flatMap { $0.key == route.key && $0.thread == route.thread ? $0 : nil } }
    private var threadTitle: String { client.threads.first { $0.id == route.thread }?.title ?? Tr.k("ios.sub.thread") }
    var body: some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 8) {
                    Text(Tr.ios_sub_of(thread: threadTitle)).dmono(11).foregroundStyle(BackplaneDesign.faint).lineLimit(1)
                    Spacer(minLength: 0)
                    if let v { Text(v.took.isEmpty ? Tr.t(v.state) : "\(Tr.t(v.state)) · \(v.took)").dmono(12).foregroundStyle(v.live ? BackplaneDesign.ok : BackplaneDesign.faint).accessibilityIdentifier("data.sub.state") }
                    if let v, let label = v.label, !label.isEmpty {
                        Button { client.workAct(route.thread, DWorkRow(title: "", kind: "", state: "", doing: "", act: v.act, value: v.value, label: label)) } label: { Text(Tr.t(label)).dmono(12) }.accessibilityIdentifier("data.sub.stop")
                    }
                }
                if let v { ForEach([v.meta, v.place].filter { !$0.isEmpty }, id: \.self) { Text(Tr.t($0)).dmono(11).foregroundStyle(BackplaneDesign.faint).lineLimit(2).accessibilityIdentifier("data.sub.meta") } }
            }.padding(.horizontal, 14).padding(.vertical, 6).background(BackplaneDesign.panel)
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 2) {
                        if v == nil { Text(client.subGone ? Tr.k("ios.sub.gone") : Tr.t("Loading…")).dmono(13).foregroundStyle(BackplaneDesign.dim).padding(14) }
                        if let v, v.more > 0 { Text(Tr.ios_sub_more(n: v.more)).dmono(11).foregroundStyle(BackplaneDesign.faint) }
                        if let v, v.entries.isEmpty { Text(Tr.k("ios.sub.none")).dmono(13).foregroundStyle(BackplaneDesign.dim).padding(.vertical, 8) }
                        ForEach(v?.entries ?? []) { e in DataSubEntry(thread: route.thread, entry: e).id(e.id) }
                        Color.clear.frame(height: 1).id("sub.end")
                    }.padding(.horizontal, 14).padding(.vertical, 8)
                }
                .accessibilityIdentifier("data.sub.list")
                // live: each new step is followed (the view opens at its newest)
                .onChange(of: v?.entries.count ?? 0) { _, _ in withAnimation(.easeOut(duration: 0.2)) { proxy.scrollTo("sub.end", anchor: .bottom) } }
                .onAppear { proxy.scrollTo("sub.end", anchor: .bottom) }
            }
        }
        .background(BackplaneDesign.view)
        .navigationTitle(v?.title ?? Tr.k("ios.sub.title")).navigationBarTitleDisplayMode(.inline)
        .toolbarBackground(BackplaneDesign.panel, for: .navigationBar).toolbarBackground(.visible, for: .navigationBar)
        .onAppear { client.openSub(route) }
        .onDisappear { client.closeSub() }
    }
}

private struct DataSubEntry: View {
    let thread: String
    let entry: DEntry
    @State private var open = false
    var body: some View {
        switch entry.kind {
        case "user":
            VStack(alignment: .leading, spacing: 4) {
                Text(Tr.k("ios.ask.asked")).dmono(11).foregroundStyle(BackplaneDesign.faint)
                DLinkText(text: entry.text, thread: thread).foregroundStyle(BackplaneDesign.text)
            }.padding(10).frame(maxWidth: .infinity, alignment: .leading).dbox(fill: BackplaneDesign.raised).padding(.vertical, 4).accessibilityIdentifier("data.sub.asked")
        case "assistant":
            DataMarkdown(text: entry.text, thread: thread).padding(.vertical, 4).frame(maxWidth: .infinity, alignment: .leading)
        case "tool":
            let st = entry.tool?.state ?? "done"
            let (res, cut) = dSubResultLines(entry.tool?.result ?? "", open: open)
            let tone = st == "running" ? BackplaneDesign.ok : st == "failed" ? BackplaneDesign.bad : BackplaneDesign.faint
            VStack(alignment: .leading, spacing: 2) {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(st == "running" ? "▸" : st == "failed" ? "✗" : "✓").dmono(12).foregroundStyle(tone)
                    Text(entry.tool?.name ?? Tr.k("ios.sub.tool")).dmono(12, bold: true).foregroundStyle(BackplaneDesign.text)
                    Text(entry.tool?.summary ?? "").dmono(12).foregroundStyle(BackplaneDesign.dim).lineLimit(2)
                    Spacer(minLength: 0)
                }
                if !res.isEmpty { Text(cut ? "\(res)\n…" : res).dmono(11).foregroundStyle(st == "failed" ? BackplaneDesign.bad : BackplaneDesign.faint).padding(.leading, 18) }
            }.padding(.vertical, 3).contentShape(Rectangle()).onTapGesture { if cut || open { open.toggle() } }.accessibilityIdentifier("data.sub.tool")
        default:
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(Tr.t(entry.label)).dmono(11).foregroundStyle(BackplaneDesign.faint)
                Text(entry.text).dmono(12).foregroundStyle(entry.tone.contains("error") ? BackplaneDesign.bad : BackplaneDesign.dim)
                Spacer(minLength: 0)
            }.padding(.vertical, 4).accessibilityIdentifier("data.sub.note")
        }
    }
}
