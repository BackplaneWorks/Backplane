// ProviderMarks parsing, bounding-box sanity (including Codex elliptical arcs), aliases and fallback.
// Compiled with Backplane/ProviderMark.swift on macOS.
import Foundation

var failed = 0, passed = 0
func check(_ ok: Bool, _ name: String, _ got: Any = "") { if ok { passed += 1 } else { failed += 1; print("FAIL \(name): got \(got)") } }

let expected = ["claude", "codex", "grok", "muse", "antigravity", "opencode", "pi", "omp"]
check(ProviderMarks.all.map(\.id) == expected, "all 8 providers listed", ProviderMarks.all.map(\.id))

for spec in ProviderMarks.all {
    check(!spec.segs.isEmpty, "\(spec.id) has segments", spec.segs.count)
    var endXs: [CGFloat] = [], endYs: [CGFloat] = [], ctrlXs: [CGFloat] = [], ctrlYs: [CGFloat] = []
    for s in spec.segs {
        switch s {
        case let .move(x, y), let .line(x, y): endXs.append(x); endYs.append(y)
        case let .cubic(x1, y1, x2, y2, x, y): ctrlXs += [x1, x2]; ctrlYs += [y1, y2]; endXs.append(x); endYs.append(y)
        case let .quad(x1, y1, x, y): ctrlXs.append(x1); ctrlYs.append(y1); endXs.append(x); endYs.append(y)
        case .close: break
        }
    }
    let minX = endXs.min() ?? 0, maxX = endXs.max() ?? 0, minY = endYs.min() ?? 0, maxY = endYs.max() ?? 0
    check(minX >= -1 && maxX <= spec.vbW + 1, "\(spec.id) end X inside viewBox", "\(minX)...\(maxX)")
    check(minY >= -1 && maxY <= spec.vbH + 1, "\(spec.id) end Y inside viewBox", "\(minY)...\(maxY)")
    let pad = max(2, spec.vbW * 0.05)
    if !ctrlXs.isEmpty {
        check((ctrlXs.min() ?? 0) >= -pad && (ctrlXs.max() ?? 0) <= spec.vbW + pad, "\(spec.id) ctrl X", ctrlXs)
        check((ctrlYs.min() ?? 0) >= -pad && (ctrlYs.max() ?? 0) <= spec.vbH + pad, "\(spec.id) ctrl Y", ctrlYs)
    }
}

check(ProviderMarks.canonical("agy") == "antigravity", "agy alias")
check(ProviderMarks.canonical("@Gemini") == "antigravity", "gemini alias")
check(ProviderMarks.canonical("openai") == "codex", "openai alias")
check(ProviderMarks.canonical("anthropic") == "claude", "anthropic alias")
check(ProviderMarks.providerOfChip("claude / opus / high") == "claude", "chip parse")
check(ProviderMarks.providerOfChip("openai · gpt-5") == "codex", "chip alias")
check(ProviderMarks.providerOfChip("unknown / m1") == "", "unknown chip")
check(ProviderMarks.spec("mystery") == nil, "unknown spec is nil")
check(ProviderMarks.letter("mystery") == "?", "unknown letter")
check(ProviderMarks.color("mystery") == 0x64748B, "unknown color")
check(!ProviderMarks.parseSegments("M0 0 Z 1 2 ? 3").isEmpty, "malformed path terminates")

print("provider marks: \(passed) passed, \(failed) failed"); exit(failed == 0 ? 0 : 1)
