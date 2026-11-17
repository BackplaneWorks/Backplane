import XCTest

// The copy button and Run in terminal on the iOS simulator, through real taps, against a private hub on a temp home (copy-hub.mjs) run by copy-rows.sh,
// which also serves HUB_CTL (f-ctl.mjs: /find?name= says where a Run in terminal command wrote its marker, from the hub's disk, not the test's say-so).
// The same reply (gpui-copy-rows.seed.py) is in a project's thread (t5), the uberbot Bub's chat (tb), the bot Pip's chat (tc) and the room bringup (r1).
// The pasteboard is read back the way a person would: pasted into the app's own composer (a test runner reading another app's pasteboard is asked
// "Allow Paste?" by iOS), and the composer's text compared with the exact command.
final class CopyRows: XCTestCase {
    var d: Driver!
    let inline = "sudo apt-get install -y rootlesskit slirp4netns"
    func block(_ marker: String) -> String { "echo zq > \(marker)" }
    override func setUp() { continueAfterFailure = true }

    func get(_ path: String) -> [String: Any] {
        guard let base = d.env["HUB_CTL"], let u = URL(string: base + path) else { return [:] }
        var r = URLRequest(url: u); r.timeoutInterval = 20
        let sem = DispatchSemaphore(value: 0); var out: [String: Any] = [:]
        URLSession.shared.dataTask(with: r) { data, _, _ in out = (data.flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any]) ?? [:]; sem.signal() }.resume()
        _ = sem.wait(timeout: .now() + 25); return out
    }
    func found(_ marker: String) -> [String] { get("/find?name=\(marker)")["paths"] as? [String] ?? [] }

    func launch(select: String) {
        d.app.terminate()
        d.app.launchArguments = ["-app.alerts", "<false/>"]
        d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_SELECT": select]
        d.app.launch()
    }
    /// The composer's text now (a UITextView's value; its placeholder when empty is not the text).
    func composerText(_ c: XCUIElement) -> String { (c.value as? String) ?? "" }
    /// Paste into the composer through its edit menu, then read what it holds; the composer is emptied again after.
    func pasted(_ composer: XCUIElement) -> String {
        guard composer.waitForExistence(timeout: 5) else { return "<no composer>" }
        composer.tap(); sleep(1); composer.press(forDuration: 1.2)
        let paste = d.app.menuItems["Paste"].exists ? d.app.menuItems["Paste"] : d.app.buttons["Paste"]
        if !paste.waitForExistence(timeout: 4) { composer.tap(); sleep(1); composer.tap() }   // a focused field's callout comes on the next tap
        if paste.waitForExistence(timeout: 4) { paste.tap() } else { return "<no Paste in the edit menu>" }
        sleep(1)
        let got = composerText(composer)
        // empty it for the next check
        if !got.isEmpty { composer.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: got.count + 2)) }
        return got
    }
    /// The copy button of the piece/code whose value is `text` (its accessibility value is the exact text it copies).
    func copyButton(_ text: String) -> XCUIElement {
        d.app.descendants(matching: .any).matching(NSPredicate(format: "identifier == 'copy.button' AND value == %@", text)).firstMatch
    }
    func runButton(_ text: String) -> XCUIElement {
        d.app.descendants(matching: .any).matching(NSPredicate(format: "identifier == 'copy.run' AND value == %@", text)).firstMatch
    }
    func scrollTo(_ e: XCUIElement) { var n = 0; while !e.isHittable && n < 6 { d.app.swipeUp(velocity: .slow); n += 1 } }

    /// Copy on one chat surface: the inline command's button, then the fenced block's; each pasted back into the composer is the exact text.
    func copies(_ name: String, marker: String, composer: String) {
        let c = d.el(composer)
        let bin = copyButton(inline), bbl = copyButton(block(marker))
        d.step("\(name): the inline command and the fenced block have copy buttons", timeout: 30, act: {}, expect: { bin.exists && bbl.exists })
        scrollTo(bin)
        d.step("\(name): the inline command's button copies it", timeout: 25, act: { d.tap(bin) }, expect: { d.label("Copied").exists })
        d.shot("\(name)-copied")
        let g1 = pasted(c)
        d.step("\(name): pasted back, it is the exact command", timeout: 1, act: {}, expect: { g1 == self.inline })
        d.note("\(name): inline pasted \(g1.debugDescription)")
        scrollTo(bbl)
        d.tap(bbl); sleep(1)
        let g2 = pasted(c)
        d.step("\(name): the fenced block's button copies it whole, no newline after", timeout: 1, act: {}, expect: { g2 == self.block(marker) })
        d.note("\(name): block pasted \(g2.debugDescription)")
    }

    /// Run in terminal on one chat surface: the chat's terminal opens with the command typed and not run; Return runs it, where `want` says.
    func runs(_ name: String, marker: String, want: String) {
        let run = runButton(block(marker))
        scrollTo(run)
        d.step("\(name): Run in terminal is beside the fenced command", timeout: 20, act: {}, expect: { run.exists })
        let screen = d.el("term.close")
        d.step("\(name): Run in terminal opens the chat's terminal", timeout: 25, act: { d.tap(run) }, expect: { screen.exists })
        // typed in: the terminal's screen holds the command (its accessibility value is the screen's text)
        let term = d.app.descendants(matching: .any).matching(NSPredicate(format: "identifier == 'term.screen' AND value CONTAINS %@", block(marker))).firstMatch
        d.step("\(name): the command is typed at the prompt", timeout: 20, act: {}, expect: { term.exists })
        sleep(3)
        d.shot("\(name)-typed")
        let early = found(marker)
        d.step("\(name): not run before Return (no \(marker) yet)", timeout: 1, act: {}, expect: { early.isEmpty })
        d.app.typeText("\n")
        var places: [String] = []
        d.step("\(name): Return runs it", timeout: 15, act: {}, expect: { places = self.found(marker); return !places.isEmpty })
        d.step("\(name): in \(want)", timeout: 1, act: {}, expect: { places.contains { ($0 as NSString).deletingLastPathComponent == want } })
        d.note("\(name): \(marker) written at \(places)")
        d.tap(screen); sleep(1)
    }

    func test_copy_rows() {
        d = Driver("copy-rows"); defer { d.finish() }

        // a project's thread: copy, and Run in terminal (its shell works in the project's folder)
        launch(select: "t5")
        _ = d.el("thread.composer").waitForExistence(timeout: 40)
        d.shot("thread-open")
        copies("thread", marker: "zq-thread.marker", composer: "thread.composer")
        runs("thread", marker: "zq-thread.marker", want: "proj")

        // the uberbot Bub's chat: copy, and Run in terminal (no project: the home folder)
        launch(select: "tb")
        _ = d.el("thread.composer").waitForExistence(timeout: 40)
        copies("bub", marker: "zq-bub.marker", composer: "thread.composer")
        runs("bub", marker: "zq-bub.marker", want: "home")

        // a bot's chat (Pip): copy
        launch(select: "tc")
        _ = d.el("thread.composer").waitForExistence(timeout: 40)
        copies("pip", marker: "zq-pip.marker", composer: "thread.composer")

        // the room bringup: its post's pieces copy; its own terminal opens (home)
        launch(select: "t5")
        _ = d.el("thread.composer").waitForExistence(timeout: 40)
        let back = d.app.navigationBars.buttons.element(boundBy: 0); if back.exists { back.tap() }
        let room = d.el("data.room.r1"); var n = 0
        while !room.isHittable && n < 6 { d.app.swipeUp(velocity: .slow); n += 1 }
        d.step("room: it opens", timeout: 30, act: { d.tap(room) }, expect: { d.el("data.room.composer").waitForExistence(timeout: 10) })
        copies("room", marker: "zq-room.marker", composer: "data.room.composer")
        runs("room", marker: "zq-room.marker", want: "home")
    }

    /// The picture for the report: a reply's copy buttons and Run in terminal, nothing pressed.
    func test_copy_shot() {
        d = Driver("copy-shot"); defer { d.finish() }
        launch(select: "t5")
        _ = d.el("thread.composer").waitForExistence(timeout: 40)
        let bl = runButton(block("zq-thread.marker"))
        d.step("the reply's Run in terminal is drawn", timeout: 30, act: {}, expect: { bl.exists })
        scrollTo(bl); sleep(1)
        d.shot("reply")
    }
}
