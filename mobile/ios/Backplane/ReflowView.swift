import SwiftUI

private struct ReflowAlloy: Decodable, Identifiable {
    let id, composition: String
    let solidus, liquidus: Double
}
private struct ReflowPlan: Decodable {
    let error: String?
    let duration: Double?
    let samples: [[Double]]?
}

// The shared Bend planner computes the profile once per edit. Playback only
// interpolates its samples; it never duplicates the thermal model in Swift.
struct ReflowView: View {
    let model: AppModel
    @Environment(\.scenePhase) private var scenePhase
    @State private var profile = ""
    @State private var defaults = ""
    @State private var alloy = "SAC305"
    @State private var alloys: [ReflowAlloy] = []
    @State private var samples: [[Double]] = []
    @State private var duration = 0.0
    @State private var time = 0.0
    @State private var speed = 10.0
    @State private var playing = false
    @State private var overlay = false
    @State private var error = ""
    @State private var revision = 0
    @State private var active = true

    private var sample: [Double] {
        guard !samples.isEmpty else { return [0, 25, 25, 0] }
        let t = min(max(time, 0), duration), a = samples[min(Int(t), samples.count - 1)]
        let b = samples[min(Int(ceil(t)), samples.count - 1)], mix = t - floor(t)
        return zip(a, b).map { pair in pair.0 + (pair.1 - pair.0) * mix }
    }
    private func paint() {
        let s = sample
        guard s.count == 4 else { return }
        model.reflowThermal = SIMD4(overlay ? 1 : 0, Float(s[2]), Float(s[3]), 0)
    }
    @MainActor private func rebuild() async {
        playing = false; revision += 1
        let ticket = revision, text = await model.reflow(profile, alloy: alloy)
        guard active, ticket == revision else { return }
        guard let data = text.data(using: .utf8), let plan = try? JSONDecoder().decode(ReflowPlan.self, from: data) else { error = "Reflow planner unavailable"; return }
        if let why = plan.error { error = why; return }
        guard let rows = plan.samples, !rows.isEmpty, rows.allSatisfy({ $0.count == 4 }), let end = plan.duration else { error = "Invalid planner reply"; return }
        samples = rows; duration = end; time = min(time, end); error = ""; paint()
    }
    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    Text("Illustrative reflow preview").font(.headline)
                    Text("Editable example, not a qualified process. CAD geometry stays unchanged.").font(.caption).foregroundStyle(.secondary)
                    Picker("Solder alloy", selection: $alloy) { ForEach(alloys) { Text($0.id).tag($0.id) } }
                    if let a = alloys.first(where: { $0.id == alloy }) {
                        Text("\(a.composition) · solidus \(a.solidus, specifier: "%.0f") °C / liquidus \(a.liquidus, specifier: "%.0f") °C").font(.caption)
                    }
                    Link("Alloy data · Indium Corporation", destination: URL(string: "https://www.indium.com/products/alloys/solder-alloys/")!)
                    let s = sample
                    Text("Furnace \(s[1], specifier: "%.1f") °C · board mean \(s[2], specifier: "%.1f") °C · liquid \(s[3] * 100, specifier: "%.1f")%")
                        .font(.caption).monospacedDigit()
                    ReflowGraph(samples: samples, duration: duration, time: time, liquid: false)
                    ReflowGraph(samples: samples, duration: duration, time: time, liquid: true)
                    HStack {
                        Button("Restart") { playing = false; time = 0; paint() }
                        Button(playing ? "Pause" : "Play") { if time >= duration { time = 0 }; playing.toggle() }.disabled(samples.isEmpty)
                        Picker("Speed", selection: $speed) { ForEach([1.0, 10, 30, 60], id: \.self) { Text("\(Int($0))×").tag($0) } }
                    }
                    Text("\(time, specifier: "%.1f") / \(duration, specifier: "%.0f") s").monospacedDigit()
                    Slider(value: Binding(get: { time }, set: { playing = false; time = $0; paint() }), in: 0...max(duration, 1))
                        .accessibilityLabel("Reflow replay time in seconds")
                    Text(s[3] >= 0.999 ? "Liquid · illustrative" : s[3] > 0 ? "Partially molten · illustrative" : "Solid · illustrative")
                    Toggle("Illustrative heat overlay", isOn: $overlay).onChange(of: overlay) { _, _ in paint() }
                    DisclosureGroup("Edit profile points") {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("JSON pairs: [seconds, furnace °C]. Apply validates time order and limits in the shared planner.").font(.caption)
                            TextEditor(text: $profile).font(.system(.caption, design: .monospaced)).frame(minHeight: 130).border(.secondary)
                                .accessibilityLabel("Profile points as seconds and furnace Celsius pairs")
                            HStack {
                                Button("Apply") { Task { await rebuild() } }
                                Button("Reset") { profile = defaults; Task { await rebuild() } }
                            }
                        }
                    }
                    if !error.isEmpty { Text(error).foregroundStyle(.red).accessibilityAddTraits(.updatesFrequently) }
                    Text("Model: 24 s first-order lag, 1 s steps; linear fraction between solidus and liquidus. No latent heat, spatial gradients, flux, wetting or settling model.")
                        .font(.caption).foregroundStyle(.secondary)
                }.padding()
            }.navigationTitle("Reflow preview").navigationBarTitleDisplayMode(.inline)
        }
        .task {
            active = true; defaults = await model.reflowDefaults(); profile = defaults
            if let data = await model.reflowAlloys().data(using: .utf8) { alloys = (try? JSONDecoder().decode([ReflowAlloy].self, from: data)) ?? [] }
            await rebuild()
        }
        .task(id: playing) {
            guard playing else { return }
            var last = Date()
            while !Task.isCancelled && playing {
                try? await Task.sleep(for: .milliseconds(100))
                guard !Task.isCancelled, playing else { return }
                let now = Date(); time = min(duration, time + min(now.timeIntervalSince(last), 0.25) * speed); last = now; paint()
                if time >= duration { playing = false }
            }
        }
        .onChange(of: alloy) { _, _ in if !profile.isEmpty { Task { await rebuild() } } }
        .onChange(of: scenePhase) { _, phase in if phase != .active { playing = false } }
        .onDisappear { active = false; revision += 1; playing = false; model.reflowThermal = SIMD4(0, 25, 0, 0) }
    }
}

private struct ReflowGraph: View {
    let samples: [[Double]]
    let duration, time: Double
    let liquid: Bool
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(liquid ? "Liquid fraction · %" : "Temperature °C · furnace dashed / board solid").font(.caption)
            Canvas { context, size in
                func point(_ row: [Double], _ index: Int) -> CGPoint {
                    CGPoint(x: CGFloat(row[0] / max(duration, 1)) * size.width,
                            y: CGFloat(1 - row[index] / (liquid ? 1 : 350)) * size.height)
                }
                for index in liquid ? [3] : [1, 2] {
                    var p = Path()
                    for (n, row) in samples.enumerated() { if n == 0 { p.move(to: point(row, index)) } else { p.addLine(to: point(row, index)) } }
                    context.stroke(p, with: .color(index == 1 ? .orange : .cyan), style: StrokeStyle(lineWidth: 2, dash: index == 1 ? [5, 4] : []))
                }
                var cursor = Path(); let x = CGFloat(time / max(duration, 1)) * size.width
                cursor.move(to: CGPoint(x: x, y: 0)); cursor.addLine(to: CGPoint(x: x, y: size.height))
                context.stroke(cursor, with: .color(.secondary), lineWidth: 1)
            }.frame(height: 120).border(.secondary).accessibilityLabel(liquid ? "Liquid fraction graph" : "Furnace and board temperature graph")
        }
    }
}
