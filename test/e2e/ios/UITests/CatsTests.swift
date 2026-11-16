import XCTest

// Feature cats on the iOS simulator, through real taps, against a temp-home hub (test/e2e/ios/cats.sh): the bot rows' cats, then Settings › Agents ›
// Your uberbot: a colour, a name typed (the preview), Save, Different cat, The name's cat, a custom colour refused and one taken, a taken name.
// What the hub kept is read from its events (HUB_CTL /botset: the uberbot's last BotSet), never from the driver's say-so.
final class Cats: XCTestCase {
    var d: Driver!
    override func setUp() { continueAfterFailure = true }

    /// the uberbot as the hub last wrote it: name, look, auto, color
    func uber() -> [String: Any] {
        guard let base = d.env["HUB_CTL"], let u = URL(string: base + "/botset") else { return [:] }
        let sem = DispatchSemaphore(value: 0); var out: [String: Any] = [:]
        URLSession.shared.dataTask(with: u) { data, _, _ in out = (data.flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any]) ?? [:]; sem.signal() }.resume()
        _ = sem.wait(timeout: .now() + 10); return out
    }
    func field(_ id: String, _ text: String) {
        let f = d.el(id); d.tap(f)
        if let v = f.value as? String, !v.isEmpty, v != f.placeholderValue { f.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: v.count + 2)) }
        d.type(f, text)
    }

    func test_cats() {
        d = Driver("cats"); defer { d.finish() }
        d.app.terminate()
        d.app.launchArguments = ["-app.alerts", "<false/>"]
        d.app.launchEnvironment = ["BACKPLANE_LINK": d.link]
        d.app.launch(); d.skipTour()
        let back = d.app.navigationBars.buttons.element(boundBy: 0)
        if !d.el("data.settings").waitForExistence(timeout: 15), back.exists { back.tap() }   // just paired: the uberbot's chat opens; back to the list
        d.step("the bot rows are listed", timeout: 30, act: {}, expect: { d.el("data.bot.b0").waitForExistence(timeout: 25) && d.el("data.bot.b1").exists })
        sleep(2); d.shot("list")
        d.step("Settings › Agents", timeout: 20, act: { d.tap(d.el("data.settings")); d.tap(d.el("settings.agents")) },
               expect: { d.el("settings.uber.name").waitForExistence(timeout: 15) })
        sleep(2); d.shot("uber")
        d.step("Sky colours it", timeout: 20, act: { d.tap(d.el("settings.uber.color.Sky")) }, expect: { (self.uber()["color"] as? String) == "#6fb3f2" })
        field("settings.uber.name", "Mog"); sleep(2); d.shot("typed")
        d.step("Save renames it", timeout: 20, act: { d.tap(d.el("settings.uber.save")) }, expect: { (self.uber()["name"] as? String) == "Mog" && (self.uber()["auto"] as? Bool) == true })
        let look = uber()["look"] as? Int ?? -1
        d.step("Different cat picks another look", timeout: 20, act: { d.tap(d.el("settings.uber.shuffle")) },
               expect: { let u = self.uber(); return (u["auto"] as? Bool) == false && (u["look"] as? Int ?? -1) != look })
        sleep(1); d.shot("shuffled")
        d.step("The name's cat gives it back", timeout: 20, act: { d.tap(d.el("settings.uber.auto")) },
               expect: { let u = self.uber(); return (u["auto"] as? Bool) == true && (u["look"] as? Int) == look })
        field("settings.uber.custom", "nope")
        d.step("a custom colour that is not one is refused here", timeout: 10, act: { d.tap(d.el("settings.uber.custom.use")) }, expect: { d.el("settings.uber.err").exists })
        field("settings.uber.custom", "#E5484D")
        d.step("a custom colour is taken", timeout: 20, act: { d.tap(d.el("settings.uber.custom.use")) }, expect: { (self.uber()["color"] as? String) == "#e5484d" })
        field("settings.uber.name", "ada")
        d.step("a taken name is the hub's error, shown", timeout: 20, act: { d.tap(d.el("settings.uber.save")) },
               expect: { d.label("already exists").waitForExistence(timeout: 10) && (self.uber()["name"] as? String) == "Mog" })
        d.shot("taken")
        field("settings.uber.name", "Mog")
        d.step("Its own takes the colour off", timeout: 20, act: { d.tap(d.el("settings.uber.color.own")) }, expect: { (self.uber()["color"] as? String) == "" })
        d.tap(back); d.tap(d.app.navigationBars.buttons.element(boundBy: 0))
        sleep(2); d.shot("list-after")
    }
}
