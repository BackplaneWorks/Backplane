import SwiftUI

// A transient presentation event, never part of the kept canonical state.
struct BubMove: Decodable {
    let id, from, to: String
    let title: String?
    let rig: [CatPart]
    let duration: Int
    let expires: Double
}

struct BubMoveOverlay: View {
    let move: BubMove?
    @State private var active: BubMove?
    @State private var seen = ""

    var body: some View {
        VStack {
            if let active { BubMoveCard(move: active).id(active.id) }
            Spacer(minLength: 0)
        }
        .padding(.top, 12)
        .allowsHitTesting(false)
        .onChange(of: move?.id, initial: true) {
            guard let move, move.id != seen else { return }
            seen = move.id
            guard move.expires > Date().timeIntervalSince1970 * 1000 else { return }
            active = move
        }
        .task(id: active?.id) {
            guard let event = active else { return }
            let remaining = min(Double(event.duration), event.expires - Date().timeIntervalSince1970 * 1000)
            try? await Task.sleep(for: .milliseconds(max(0, remaining)))
            if !Task.isCancelled, active?.id == event.id { active = nil }
        }
    }
}

private struct BubMoveCard: View {
    let move: BubMove
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private func machine(_ name: String) -> some View {
        VStack(spacing: 4) {
            Image(systemName: "macwindow").font(.system(size: 28, weight: .light))
            Text(name).font(.system(size: 11)).lineLimit(2).multilineTextAlignment(.center)
        }.frame(width: 88, height: 60, alignment: .top)
    }

    var body: some View {
        VStack(spacing: 0) {
            Text(move.title ?? "Bub’s moving").font(.system(size: 12)).foregroundStyle(.secondary).lineLimit(2)
            ZStack(alignment: .topLeading) {
                HStack(alignment: .top) {
                    machine(move.from)
                    Spacer(minLength: 0)
                    machine(move.to)
                }.padding(.top, 36)
                CatView(rig: move.rig, size: 48)
                    .keyframeAnimator(initialValue: CGPoint(x: reduceMotion ? 160 : 0, y: 10), repeating: false) { cat, value in
                        cat.offset(x: 20 + value.x, y: value.y)
                    } keyframes: { _ in
                        KeyframeTrack(\.x) {
                            LinearKeyframe(reduceMotion ? 160 : 0, duration: 0.3)
                            CubicKeyframe(reduceMotion ? 160 : 80, duration: 0.6)
                            CubicKeyframe(160, duration: 0.6)
                            LinearKeyframe(160, duration: 0.5)
                        }
                        KeyframeTrack(\.y) {
                            LinearKeyframe(10, duration: 0.3)
                            CubicKeyframe(reduceMotion ? 10 : -10, duration: 0.6)
                            CubicKeyframe(10, duration: 0.6)
                            LinearKeyframe(10, duration: 0.5)
                        }
                    }
            }.frame(width: 248, height: 96)
        }
        .padding(.horizontal, 16).padding(.vertical, 12)
        .background(.regularMaterial)
        .overlay { Rectangle().stroke(.secondary.opacity(0.35), lineWidth: 1) }
        .keyframeAnimator(initialValue: 0.0, repeating: false) { card, opacity in card.opacity(opacity) } keyframes: { _ in
            KeyframeTrack {
                LinearKeyframe(1.0, duration: 0.24)
                LinearKeyframe(1.0, duration: 1.46)
                LinearKeyframe(0.0, duration: 0.30)
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(move.title ?? "Bub’s moving"): \(move.from) to \(move.to)")
    }
}
