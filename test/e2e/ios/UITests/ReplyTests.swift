import XCTest

// Reply on the iOS simulator (feature `reply`, docs/PHONE-DATA.md), through real taps, against the copy rows' private hub (copy-hub.mjs, run by
// reply-rows.sh). Thread t5 holds the person's message cp0 and the agent's reply cp1. The hub's control port (f-ctl.mjs /count) reads the hub's own
// log, where the stand-in's `zqecho` answer repeats what the agent was given: the reply is checked by what the hub kept and what the agent was given, never by the test's say-so.
final class ReplyRows: XCTestCase {
    var d: Driver!
    override func setUp() { continueAfterFailure = true }

    func count(_ f: String, _ re: String) -> Int {
        guard let base = d.env["HUB_CTL"], var c = URLComponents(string: base + "/count") else { return -1 }
        c.queryItems = [URLQueryItem(name: "f", value: f), URLQueryItem(name: "re", value: re)]
        guard let u = c.url else { return -1 }
        var r = URLRequest(url: u); r.timeoutInterval = 15
        let sem = DispatchSemaphore(value: 0); var n = -1
        URLSession.shared.dataTask(with: r) { data, _, _ in n = ((data.flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any])?["n"] as? Int) ?? -1; sem.signal() }.resume()
        _ = sem.wait(timeout: .now() + 20); return n
    }

    func test_reply_rows() {
        d = Driver("reply-rows"); defer { d.finish() }
        d.app.terminate()
        d.app.launchArguments = ["-app.alerts", "<false/>"]
        d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_SELECT": "t5"]
        d.app.launch()
        let composer = d.el("thread.composer"), strip = d.el("data.reply.strip"), drop = d.el("data.reply.drop")
        _ = composer.waitForExistence(timeout: 40)
        let answer = d.label("Then tell me")
        d.step("the agent's reply is shown", timeout: 30, act: {}, expect: { answer.exists })
        d.shot("thread-open")

        // long-press the agent's reply: Reply in its menu puts the strip over the box, with the sender and the first line
        let menuReply = d.app.buttons["data.entry.reply.menu"]
        d.step("a long-press on the reply offers Reply", timeout: 10, act: { answer.press(forDuration: 1.2) }, expect: { menuReply.exists })
        d.shot("long-press-menu")
        d.step("Reply shows the strip: Agent, its first line", timeout: 10, act: { d.tap(menuReply) },
               expect: { strip.exists && d.label("Replying to: One thing needs you").exists && d.label("Agent").exists })
        d.shot("strip")
        d.step("× drops the reply", timeout: 10, act: { d.tap(drop) }, expect: { !strip.exists })

        // the footer's Reply does the same; a send carries it
        let footer = d.app.buttons.matching(identifier: "data.entry.reply").allElementsBoundByIndex.last ?? d.el("data.entry.reply")
        d.step("Reply under the message puts the strip back", timeout: 10, act: { d.tap(footer) }, expect: { strip.exists })
        d.type(composer, "zqecho make it so")
        // the keyboard's one-time tip ("Speed up your typing…") covers the screen on a fresh simulator
        if d.app.buttons["Continue"].waitForExistence(timeout: 2) { d.app.buttons["Continue"].tap() }
        let before = count("events", "in_reply_to msg=cp1 chars=")
        d.step("the send keeps the desktop's in_reply_to block in the hub's log", timeout: 30, act: { d.tap(d.el("thread.send")) },
               expect: { self.count("events", "in_reply_to msg=cp1 chars=") > before })
        // the stand-in answers `echo: <what it was given>`: the block, then the words
        d.step("the agent is given the block ahead of what was written", timeout: 40, act: {},
               expect: { self.count("events", #""role":"assistant".*echo: .*<in_reply_to msg=cp1 chars=[0-9]*>\\nOne thing needs you.*</in_reply_to>\\n\\nzqecho make it so"#) > 0 })
        d.step("the strip is gone after the send", timeout: 5, act: {}, expect: { !strip.exists })
        // (the chip's identifier is the row's, data.entry.user: found by its words)
        let chip = d.app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "↩ In reply to: One thing needs you")).firstMatch
        d.step("the sent message shows what it answers", timeout: 20, act: {}, expect: { chip.exists })
        d.shot("sent")

        // a swipe right on the person's own message replies to it
        let mine = d.label("can you speed up the builds?")
        // (scrolled by a drag in the conversation, between the header and the composer: the composer is a scroll view too, and the keyboard swipe-types)
        d.step("the person's message is on screen", timeout: 10, act: { var n = 0; while !mine.isHittable && n < 12 {
            let from = d.app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.22)); from.press(forDuration: 0.05, thenDragTo: d.app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.48))); n += 1 } }, expect: { mine.isHittable })
        d.step("a swipe right on it is Reply, sender You", timeout: 10, act: {
            let a = mine.coordinate(withNormalizedOffset: CGVector(dx: 0.2, dy: 0.5)), b = a.withOffset(CGVector(dx: 140, dy: 0))
            a.press(forDuration: 0.05, thenDragTo: b)
        }, expect: { strip.exists && d.label("You").exists && d.label("Replying to: zqshort can you speed up").exists })
        d.shot("swiped")
        d.tap(drop)

        // a tap on the quote goes to the message it answers
        d.step("a tap on the quote goes to the original", timeout: 15, act: { d.app.swipeUp(velocity: .fast); d.tap(chip) }, expect: { answer.isHittable })
        d.shot("went-to-original")
    }
}
