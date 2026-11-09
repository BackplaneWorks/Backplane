// Outbox (Backplane/Outbox.swift), an XCTest: scripts/mobile/ios-outbox-test.sh builds it as a Swift package of Outbox.swift and this file and runs `swift test`.
import XCTest
@testable import OutboxKit

final class OutboxTests: XCTestCase {
    func testPushKeepsOrderAndSkipsEmptyAndRepeats() {
        var o = Outbox()
        o.push(thread: "t1", text: "first", clientId: "c1")
        o.push(thread: "t2", text: "other thread", clientId: "c2")
        o.push(thread: "t1", text: "  \n ", clientId: "c3")      // blank: nothing kept
        o.push(thread: "t1", text: "again", clientId: "c1")      // the same clientId: kept once
        o.push(thread: "t1", text: "second\n", clientId: "c4")
        XCTAssertEqual(o.items.map(\.clientId), ["c1", "c2", "c4"])
        XCTAssertEqual(o.items.last?.text, "second", "kept trimmed")
        XCTAssertTrue(o.has("t1")); XCTAssertFalse(o.has("t3"))
    }

    func testRemoveAnswersTheItem() {
        var o = Outbox()
        o.push(thread: "t1", text: "a", clientId: "c1"); o.push(thread: "t1", text: "b", clientId: "c2")
        XCTAssertEqual(o.remove("c1")?.text, "a")
        XCTAssertNil(o.remove("c1"), "already gone")
        XCTAssertNil(o.remove("nope"))
        XCTAssertEqual(o.items.map(\.clientId), ["c2"])
    }

    func testRowsForThreadInTheHubsShape() {
        var o = Outbox()
        o.push(thread: "t1", text: "\n  Move the socket  \nsecond line", clientId: "c1")
        o.push(thread: "t2", text: "elsewhere", clientId: "c2")
        o.push(thread: "t1", text: String(repeating: "x", count: 300), clientId: "c3")
        let rows = o.rows("t1")
        XCTAssertEqual(rows.map(\.msg), ["c1", "c3"])
        XCTAssertEqual(rows[0].tag, "Unsent")
        XCTAssertEqual(rows[0].text, "Move the socket", "the first line that is not blank")
        XCTAssertEqual(rows[1].text.count, 200, "200 characters at most")
        XCTAssertEqual(rows[0].buttons, [DQueueButton(label: "Edit", action: "u-edit", value: "c1"), DQueueButton(label: "Remove", action: "u-remove", value: "c1")])
        XCTAssertTrue(o.rows("t3").isEmpty)
        // the same JSON as a hub row: {msg, tag, text, buttons:[{label, action, value}]}
        let j = try! JSONSerialization.jsonObject(with: JSONEncoder().encode(rows[0])) as! [String: Any]
        XCTAssertEqual(Set(j.keys), ["msg", "tag", "text", "buttons"])
        XCTAssertEqual(Set(((j["buttons"] as! [[String: Any]])[0]).keys), ["label", "action", "value"])
        // and a hub row decodes into the same type
        let hub = #"{"msg":"m9","tag":"Refused","text":"no","buttons":[{"label":"Edit","action":"o-edit","value":"m9"}]}"#
        let r = try! JSONDecoder().decode(DQueueRow.self, from: Data(hub.utf8))
        XCTAssertEqual(r, DQueueRow(msg: "m9", tag: "Refused", text: "no", buttons: [DQueueButton(label: "Edit", action: "o-edit", value: "m9")]))
    }

    func testNextToSendIsTheOldestOfAllThreads() {
        var o = Outbox()
        XCTAssertNil(o.next)
        o.push(thread: "t2", text: "one", clientId: "c1")
        o.push(thread: "t1", text: "two", clientId: "c2")
        o.push(thread: "t2", text: "three", clientId: "c3")
        var order: [String] = []
        while let n = o.next { order.append(n.clientId); o.sent(n.clientId) }
        XCTAssertEqual(order, ["c1", "c2", "c3"])
        XCTAssertTrue(o.isEmpty)
    }

    func testSentDropsOnlyThatOne() {
        var o = Outbox()
        o.push(thread: "t1", text: "a", clientId: "c1"); o.push(thread: "t1", text: "b", clientId: "c2")
        o.sent("c2"); o.sent("unknown")
        XCTAssertEqual(o.items.map(\.clientId), ["c1"])
        XCTAssertEqual(o.next?.clientId, "c1")
    }

    func testPersistenceRoundTrip() {
        var o = Outbox()
        o.push(thread: "t1", text: "ünïcode ✓\nline two", clientId: "c1")
        o.push(thread: "peer~t2", text: "far", clientId: "c2")
        let back = Outbox.decoded(o.encoded())
        XCTAssertEqual(back, o)
        XCTAssertEqual(back.rows("t1"), o.rows("t1"))
        XCTAssertEqual(Outbox.decoded(nil), Outbox(), "no file: empty")
        XCTAssertEqual(Outbox.decoded(Data("not json".utf8)), Outbox(), "a broken file: empty")
    }

    func testDraftGetsTheTextBack() {
        XCTAssertEqual(Outbox.draft("", adding: "hello"), "hello")
        XCTAssertEqual(Outbox.draft("  \n", adding: "hello"), "hello")
        XCTAssertEqual(Outbox.draft("typed \n", adding: "hello"), "typed\n\nhello")
    }
}
