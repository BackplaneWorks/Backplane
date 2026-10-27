import ActivityKit
import SwiftUI
import WidgetKit

@main
struct IslandBundle: WidgetBundle {
    var body: some Widget {
        IslandWidget()
    }
}

private struct Mark: View {
    let running: Int

    var body: some View {
        Image(systemName: running > 0 ? "cpu" : "checkmark.circle.fill")
            .foregroundStyle(running > 0 ? Color.blue : Color.secondary)
    }
}

// a thread to open, or with pick the app asks which (a tap can only open
// the app; the island expands on a long press)
private func link(_ thread: String) -> URL? {
    var c = URLComponents()
    c.scheme = "backplane"
    c.host = "open"
    c.queryItems = [URLQueryItem(name: "thread", value: thread)]
    return c.url
}

// a tap anywhere: the thread when one works, else the app as it was (it
// never asks which: a list of every working thread over the app, shown
// when it was opened from here, was more in the way than a help)
private func tap(_ s: IslandAttributes.ContentState) -> URL? {
    s.lines.count == 1 ? link(s.lines[0].thread) : nil
}

// each thread its own row, a tap on it opens that thread. The hub sends
// only the first few (notice.bend's Notice.island.max) and counts the rest
private struct Lines: View {
    let lines: [IslandAttributes.Line]
    let titles: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(lines, id: \.thread) { l in
                if let u = link(l.thread) {
                    Link(destination: u) { Row(line: l, title: titles) }
                } else {
                    Row(line: l, title: titles)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct Row: View {
    let line: IslandAttributes.Line
    let title: Bool

    // one line a thread, so several fit the lock screen's 160 points
    var body: some View {
        HStack(spacing: 6) {
            if title {
                Text(line.title).font(.caption.weight(.semibold)).lineLimit(1)
                    .frame(maxWidth: 150, alignment: .leading)
            }
            Text(line.doing).font(.caption.monospaced()).foregroundStyle(.secondary).lineLimit(1)
            Spacer(minLength: 4)
            if title { Image(systemName: "chevron.right").font(.caption2).foregroundStyle(.tertiary) }
        }
        .contentShape(.rect)
    }
}

// one line under the rows: what is not shown ("+ N more"), and, with no
// word from the hubs for hours, that this may no longer be so
private struct Foot: View {
    let more: Int
    let stale: Bool

    var body: some View {
        let words = (more > 0 ? ["+ \(more) more"] : []) + (stale ? ["Not updated lately: open Backplane"] : [])
        if !words.isEmpty {
            Text(words.joined(separator: " · ")).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
        }
    }
}

struct IslandWidget: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: IslandAttributes.self) { ctx in
            let s = ctx.state
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Mark(running: s.running)
                    Text(s.headline).font(.headline).lineLimit(1)
                    Spacer()
                    if s.running > 1 { Text("\(s.running)").font(.headline.monospacedDigit()) }
                }
                Lines(lines: s.lines, titles: s.running > 1)
                Foot(more: s.running - s.lines.count, stale: ctx.isStale)
            }
            .opacity(ctx.isStale ? 0.5 : 1)
            .padding()
            .widgetURL(tap(s))
        } dynamicIsland: { ctx in
            let s = ctx.state
            return DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    Mark(running: s.running).font(.title3).padding(.leading, 4)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    Text("\(s.running)").font(.title3.monospacedDigit().bold()).padding(.trailing, 4)
                }
                DynamicIslandExpandedRegion(.center) {
                    Text(s.headline).font(.headline).lineLimit(1)
                }
                DynamicIslandExpandedRegion(.bottom) {
                    VStack(alignment: .leading, spacing: 4) {
                        Lines(lines: s.lines, titles: s.running > 1)
                        Foot(more: s.running - s.lines.count, stale: ctx.isStale)
                    }
                }
            } compactLeading: {
                Mark(running: s.running).opacity(ctx.isStale ? 0.4 : 1)
            } compactTrailing: {
                Text(ctx.isStale ? "?" : "\(s.running)").monospacedDigit()
            } minimal: {
                Mark(running: s.running).opacity(ctx.isStale ? 0.4 : 1)
            }
            .widgetURL(tap(s))
        }
    }
}
