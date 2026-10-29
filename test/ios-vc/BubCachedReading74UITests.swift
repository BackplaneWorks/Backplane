import XCTest
final class BubParkedScrollUITests: XCTestCase {
    func testRealBubParkedReading() {
        let app = XCUIApplication()
        app.launchEnvironment["BACKPLANE_QUALIFICATION"] = "1"
                app.launch()
        let ready = app.descendants(matching: .any)["qualification.connection.ready"]
        XCTAssertTrue(ready.waitForExistence(timeout: 60))
        let home = app.buttons["tab.home"]; XCTAssertTrue(home.waitForExistence(timeout: 15)); home.tap()
        if app.navigationBars["Bub"].exists { app.navigationBars.buttons.firstMatch.tap() }
        let card = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@ AND label CONTAINS[cd] %@", "home.uber.", "Bub")).firstMatch
        XCTAssertTrue(card.waitForExistence(timeout: 30)); card.tap()
        let bot = app.descendants(matching: .any)["bot.screen.b1790292962-8017"]
        guard bot.waitForExistence(timeout: 60), ready.waitForExistence(timeout: 60) else { XCTFail("Real Bub bot screen and readiness required"); return }
        let chat=app.buttons["bot.tab.chat"]
        guard chat.waitForExistence(timeout: 20) else { XCTFail("Real Bub Chat tab missing"); return }
        if !chat.isSelected { chat.tap() }
        XCTAssertTrue(ready.waitForExistence(timeout: 60))
        let status=app.descendants(matching: .any).matching(NSPredicate(format:"identifier BEGINSWITH %@", "qualification.connection.")).firstMatch
        func fields() -> [String:String] { guard let v=status.value as? String, let d=v.data(using:.utf8), let x=(try? JSONSerialization.jsonObject(with:d)) as? [String:String] else {return [:]}; return x }
        let current=fields(); guard let thread=current["thread"], thread.hasSuffix("|t1790292962-8017"),current["nav"] == thread,current["composerOwner"] == thread,current["bot"] == "b1790292962-8017",current["botTab"] == "chat",current["contentReady"] == "true",chat.isSelected else {XCTFail("Exact real Bub owner, selected Chat and fresh readiness required");return}
        let timeline=app.descendants(matching:.any)["thread.timeline"]
        guard timeline.waitForExistence(timeout:20) else {XCTFail("Bub timeline missing");return}
        let entries=app.descendants(matching:.any).matching(NSPredicate(format:"identifier BEGINSWITH %@","thread.entry.")).allElementsBoundByIndex
        guard let entry=entries.first(where:{$0.exists && $0.isHittable && !$0.frame.intersection(timeline.frame).isEmpty}) else {XCTFail("Visible identified Bub entry unavailable");return}
        let id=entry.identifier
        XCTContext.runActivity(named:"QUALIFICATION_PARK_NOW") {_ in}
        let parked=app.descendants(matching:.any)["qualification.connection.parked"]
        let didPark=parked.waitForExistence(timeout:30)
        let before=entry.frame, vp=timeline.frame
        let visible = !before.isNull && !before.isInfinite && before.width > 0 && before.height > 0 && !before.intersection(vp).isEmpty
        let now=fields(); let sameOwner=now["thread"] == thread && now["nav"] == thread && now["composerOwner"] == thread
        let send=app.buttons.matching(identifier:"thread.send").allElementsBoundByIndex
        let tools=app.buttons.matching(identifier:"thread.tools").allElementsBoundByIndex
        let tabs=app.buttons.matching(NSPredicate(format:"identifier BEGINSWITH %@","bot.tab.")).allElementsBoundByIndex
        let sections=app.buttons.matching(NSPredicate(format:"identifier BEGINSWITH %@","bot.section.")).allElementsBoundByIndex
        let guarded = !send.isEmpty && !tools.isEmpty && !tabs.isEmpty && !send.contains{$0.isEnabled} && !tools.contains{$0.isEnabled} && !tabs.contains{$0.isEnabled} && !sections.contains{$0.isEnabled}
        print("safe-bub-section count=\(sections.count) enabled=\(sections.filter{$0.isEnabled}.count) guarded=\(guarded)")
        print("safe-bub-park parked=\(didPark) owner=\(sameOwner) visible=\(visible) entry-frame=\(before) timeline-frame=\(vp) send-count=\(send.count) send-enabled=\(send.contains{$0.isEnabled}) tools-count=\(tools.count) tools-enabled=\(tools.contains{$0.isEnabled}) bot-tabs-count=\(tabs.count) bot-tabs-enabled=\(tabs.filter{$0.isEnabled}.count)")
        if didPark && visible {timeline.coordinate(withNormalizedOffset:CGVector(dx:0.5,dy:0.32)).press(forDuration:0.1,thenDragTo:timeline.coordinate(withNormalizedOffset:CGVector(dx:0.5,dy:0.78)))}
        let after=entry.frame; let moved=entry.exists && entry.identifier == id && abs(after.minY-before.minY)>5
        print("safe-bub-after parked=\(parked.exists) moved=\(moved) before-y=\(before.minY) after-y=\(after.minY)")
        XCTContext.runActivity(named:"QUALIFICATION_RECONNECT_NOW") {_ in}
        let recovered=ready.waitForExistence(timeout:60)
        let resumed=fields(); let ownerRestored=resumed["thread"] == thread && resumed["nav"] == thread && resumed["composerOwner"] == thread && resumed["contentReady"] == "true"
        print("safe-bub-reconnect ready=\(recovered) parked=\(parked.exists) owner=\(ownerRestored)")
        let tabsNow=app.buttons.matching(NSPredicate(format:"identifier BEGINSWITH %@","bot.tab.")).allElementsBoundByIndex
        print("safe-bub-restored tab-count=\(tabsNow.count) tab-enabled=\(tabsNow.filter{$0.isEnabled}.count)")
        XCTAssertTrue(ownerRestored && !tabsNow.isEmpty && tabsNow.contains{$0.isEnabled})
        XCTAssertTrue(recovered && !parked.exists); XCTAssertTrue(didPark && sameOwner && visible); XCTAssertTrue(moved,"Cached Bub timeline must scroll")
        XCTAssertTrue(guarded,"Actual buttons must be disabled while parked")
    }
}
