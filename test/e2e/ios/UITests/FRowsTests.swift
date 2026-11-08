import XCTest
import UIKit

// The phone halves of F06, F07, F17 and F21 on the iOS simulator, through real taps, against a private hub on a temp home (test/e2e/pixel-hub.mjs, stand-in
// agent lib/standin_rows.py) run by test/e2e/ios/f-rows.sh, which also serves HUB_CTL (f-ctl.mjs): counts in the hub's events.jsonl and the stand-in's log,
// and killing and starting the hub. A check passes from those logs, or from the viewer's own pixels; never from the driver's say-so. The same rows as
// test/e2e/pixel-f-rows.sh does on the Pixel.
final class FRows: XCTestCase {
    var d: Driver!
    override func setUp() { continueAfterFailure = true }

    func get(_ path: String) -> [String: Any] {
        guard let base = d.env["HUB_CTL"], let u = URL(string: base + path) else { return [:] }
        var r = URLRequest(url: u); r.timeoutInterval = 30
        let sem = DispatchSemaphore(value: 0); var out: [String: Any] = [:]
        URLSession.shared.dataTask(with: r) { data, _, _ in out = (data.flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any]) ?? [:]; sem.signal() }.resume()
        _ = sem.wait(timeout: .now() + 35); return out
    }
    /// lines of the hub's events (`events`) or the stand-in's log (`fake`) matching a regular expression
    func count(_ file: String, _ re: String) -> Int {
        let q = re.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? ""
        return get("/count?f=\(file)&re=\(q)")["n"] as? Int ?? -1
    }
    func say(_ text: String) {
        let c = d.el("thread.composer"); d.tap(c); d.type(c, text); d.tap(d.el("thread.send"))
    }
    /// an ask card's button by its label (the card's buttons carry no identifier of their own in the accessibility tree)
    func button(_ label: String) -> XCUIElement { d.app.buttons.matching(NSPredicate(format: "label == %@", label)).firstMatch }
    /// bright pixels in an element's own screenshot: a drawn board, schematic or model, not an empty page
    func lit(_ e: XCUIElement) -> (Int, Data) {
        let png = e.screenshot().pngRepresentation
        guard let img = UIImage(data: png)?.cgImage else { return (0, png) }
        let w = img.width, h = img.height; var px = [UInt8](repeating: 0, count: w * h * 4)
        guard let ctx = CGContext(data: &px, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpaceCreateDeviceRGB(),
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return (0, png) }
        ctx.draw(img, in: CGRect(x: 0, y: 0, width: w, height: h))
        var n = 0; var i = 0
        while i < px.count { if (Int(px[i]) + Int(px[i + 1]) + Int(px[i + 2])) / 3 > 60 { n += 1 }; i += 4 }
        return (n, png)
    }

    func test_f_rows() {
        d = Driver("f-rows"); defer { d.finish() }
        d.app.terminate()
        d.app.launchArguments = ["-app.alerts", "<false/>"]   // no notification question over the screens
        d.app.launchEnvironment = ["BACKPLANE_LINK": d.link, "BACKPLANE_PERFORMANCE": "1"]
        d.app.launch()
        let row = d.el("data.thread.row.t1")
        guard d.step("open Thread 1 (approval-required)", timeout: 40, act: { _ = row.waitForExistence(timeout: 30); d.tap(row) },
                     expect: { d.el("thread.composer").waitForExistence(timeout: 10) }) else { return }

        // F06: an approval on the phone; Allow and Decline reach the agent
        let a0 = count("events", "\"AskOpened\"")
        let accept = button("Allow")
        d.step("F06 an approval is drawn on the phone", timeout: 45, act: { say("zqapprove now") }, expect: { self.count("events", "\"AskOpened\"") > a0 && accept.exists })
        d.shot("approval")
        d.step("F06 Allow reaches the agent as allow", timeout: 30, act: { d.tap(accept) }, expect: { self.count("fake", "PERM Bash.*allow") > 0 })
        sleep(4)
        d.step("F06 a second approval", timeout: 45, act: { say("zqapprove again") }, expect: { self.count("events", "\"AskOpened\"") > a0 + 1 && self.button("Decline").exists })
        d.step("F06 Decline reaches the agent as deny", timeout: 30, act: { d.tap(self.button("Decline")) }, expect: { self.count("fake", "PERM Bash.*deny") > 0 })
        sleep(3)

        // F07: Stop on a running turn
        let r0 = count("events", "\"state\":\"running\"")
        let stop = button("Stop")   // the send button while a turn runs (identifier thread.send, label Stop)
        d.step("F07 a long turn runs", timeout: 30, act: { say("a long turn") }, expect: { self.count("events", "\"state\":\"running\"") > r0 && stop.exists })
        d.shot("running")
        d.step("F07 Stop interrupts it", timeout: 30, act: { d.tap(stop) }, expect: { self.count("events", "\"state\":\"interrupted\"") > 0 })
        sleep(3)

        // F06: a question after the stop; the tapped answer reaches the agent
        let barrel = d.app.buttons.matching(NSPredicate(format: "label == 'Barrel'")).firstMatch
        d.step("F06 a question is drawn (Which port?)", timeout: 45, act: { say("zqask") }, expect: { barrel.exists })
        d.shot("question")
        d.step("F06 the tapped answer reaches the agent", timeout: 30, act: { d.tap(barrel) }, expect: { self.count("fake", "PERM AskUserQuestion.*Barrel") > 0 })
        sleep(2)

        // F17: each viewer draws, and a swipe changes it
        if d.step("F17 the viewer opens", timeout: 20, act: { d.tap(d.el("data.thread.viewer")) }, expect: { d.el("data.vtab.Board").waitForExistence(timeout: 15) }) {
            for kind in ["Board", "Schematic", "3D", "Mech"] {
                let tabb = d.el("data.vtab." + kind)
                guard tabb.waitForExistence(timeout: 5) else { d.step("F17 \(kind): its tab", timeout: 1, act: {}, expect: { false }); continue }
                d.tap(tabb)
                let surface = d.el(kind == "Mech" ? "data.mech.surface" : "data.viewer.surface")   // Mech's solid has its own surface
                var before = (0, Data())
                // drawn: lit, and no longer saying it is loading (a loading card is bright too, and its spinner moves)
                let loading = d.label("Loading 3D model")
                d.step("F17 \(kind) is drawn", timeout: 120, act: {}, expect: { guard surface.exists, surface.frame.height > 150, !loading.exists else { return false }; before = self.lit(surface); return before.0 > 6000 })
                d.shot("v-\(kind)")
                if surface.exists { surface.swipeLeft() } else { d.note("F17 \(kind): no surface to swipe") }; sleep(2)
                let after = surface.exists ? lit(surface) : (0, Data())
                d.step("F17 \(kind) moves under a swipe", timeout: 1, act: {}, expect: { after.1 != before.1 && after.0 != before.0 && !loading.exists })
                d.note("F17 \(kind): \(before.0) lit pixels before the swipe, \(after.0) after")
            }
            if d.el("viewer.close").exists { d.tap(d.el("viewer.close")) }
            let back = d.app.navigationBars.buttons.element(boundBy: 0)
            if !d.el("thread.composer").exists && back.exists { back.tap() }
            if !d.el("thread.composer").exists { _ = row.waitForExistence(timeout: 10); d.tap(row) }
        }

        // F21: the hub is killed and started again on the same home; the phone reconnects and its next message reaches the new hub
        _ = d.el("thread.composer").waitForExistence(timeout: 10)
        d.step("F21 the hub is killed: the phone says it is offline", timeout: 40, act: { _ = self.get("/kill") },
               expect: { d.label("Offline").exists || d.label("reconnecting").exists || d.label("Cannot reach").exists })
        d.shot("offline")
        d.step("F21 the hub starts again on the same home", timeout: 60, act: { _ = self.get("/restart") }, expect: { self.get("/up")["up"] as? Bool == true })
        d.step("F21 the phone reconnects", timeout: 90, act: {}, expect: { !d.label("Offline").exists && !d.label("Cannot reach").exists && !d.label("reconnecting").exists })
        let n = count("events", "after the reconnect")
        d.step("F21 a message sent afterwards reaches the new hub", timeout: 60, act: { self.say("zqshort after the reconnect") }, expect: { self.count("events", "after the reconnect") > n })
        d.shot("final")
    }
}
