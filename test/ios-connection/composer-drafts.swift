import Foundation

@main struct ComposerDraftsTests {
    static func main() {
        func check(_ condition: @autoclosure () -> Bool, _ name: String) {
            guard condition() else { fputs("FAIL \(name)\n", stderr); exit(1) }
            print("PASS \(name)")
        }
        var drafts = ComposerDrafts()
        check(drafts.text(for: "h|a") == nil && drafts.edit(for: "h|a") == 0, "missing draft")
        check(drafts.receive("h|a", "server", edit: nil) && drafts.edit(for: "h|a") == 0, "first unversioned draft")
        let a1 = drafts.change("h|a", "local A")
        let b1 = drafts.change("other|b", "local B")
        check(a1 == 1 && b1 == 1 && drafts.text(for: "h|a") == "local A" && drafts.text(for: "other|b") == "local B", "owners isolated")
        check(!drafts.receive("h|a", "echo", edit: a1) && drafts.pending(for: "h|a") == a1, "pending local edit blocks echo")
        drafts.acknowledged("h|a", edit: 0)
        check(drafts.pending(for: "h|a") == a1, "stale acknowledgment does not clear pending")
        drafts.acknowledged("h|a", edit: a1)
        check(drafts.pending(for: "h|a") == nil && drafts.receive("h|a", "accepted", edit: a1), "same revision authoritative completion")
        check(!drafts.receive("h|a", "older", edit: 0) && drafts.text(for: "h|a") == "accepted", "older response ignored")
        check(drafts.receive("h|a", "new server", edit: 5) && drafts.edit(for: "h|a") == 5, "newer authoritative revision")
        let sent = drafts.change("h|a", "sent")
        let clear = drafts.change("h|a", "")
        check(clear == sent + 1 && drafts.failed("h|a", "sent", sentEdit: sent) == clear + 1 && drafts.text(for: "h|a") == "sent", "failure restores untouched clear")
        let old = drafts.edit(for: "h|a")
        _ = drafts.change("h|a", "clear")
        let newest = drafts.change("h|a", "new typing")
        check(drafts.failed("h|a", "stale", sentEdit: old) == nil && drafts.edit(for: "h|a") == newest && drafts.text(for: "h|a") == "new typing", "failure preserves newer typing")
        drafts.forget("h")
        check(drafts.text(for: "h|a") == nil && drafts.edit(for: "h|a") == 0 && drafts.pending(for: "h|a") == nil, "forget resets owner")
        check(drafts.text(for: "other|b") == "local B" && drafts.pending(for: "other|b") == b1, "forget preserves other hub")
    }
}
