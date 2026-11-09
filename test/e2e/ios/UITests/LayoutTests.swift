import XCTest

// The size matrix on iOS (docs/e2e/SIZES.md): walks the app's screens on whatever simulator and orientation it runs on, and writes each screen's accessibility tree
// (element frames in points) to $OUT/layout-<screen>.json plus a screenshot. test/e2e/layout/ios_check.py judges the trees with the rules the Android check uses (overlap,
// clip, 44 pt touch targets, popups on screen); this file only records. layout.sh runs it per simulator, orientation and Dynamic Type size.
// env (TEST_RUNNER_*): HUB_HOST HUB_PORT HUB_TOKEN_FILE (the fixture hub, as run.sh), OUT, LAYOUT_ORIENT=portrait|landscape, LAYOUT_TEXT=<UICTContentSizeCategory…>.
final class LayoutAudit: XCTestCase {
    var d: Driver!
    var out = ""

    static let types: [XCUIElement.ElementType: String] = [
        .other: "other", .window: "window", .staticText: "staticText", .button: "button", .textField: "textField", .secureTextField: "secureTextField", .textView: "textView",
        .scrollView: "scrollView", .table: "table", .collectionView: "collectionView", .cell: "cell", .link: "link", .switch: "switch", .tab: "tab", .tabBar: "tabBar",
        .navigationBar: "navigationBar", .searchField: "searchField", .segmentedControl: "segmentedControl", .sheet: "sheet", .alert: "alert", .popover: "popover", .menuItem: "menuItem",
        .image: "image", .icon: "icon", .webView: "webView", .slider: "slider", .stepper: "stepper", .toggle: "toggle", .key: "key", .keyboard: "keyboard", .application: "application",
    ]
    func node(_ s: XCUIElementSnapshot) -> [String: Any] {
        let f = s.frame
        return ["type": Self.types[s.elementType] ?? "other", "label": s.label, "id": s.identifier, "frame": [f.minX, f.minY, f.width, f.height].map { ($0 * 10).rounded() / 10 },
                "enabled": s.isEnabled, "children": s.children.map { node($0) }]
    }
    /// the screen as the audit records it: the whole app tree, so a popover or sheet (another window of the app) is in it
    func record(_ name: String) {
        guard let snap = try? d.app.snapshot() else { d.note("no snapshot for \(name)"); return }
        let w = d.app.windows.firstMatch.frame
        var screen = [w.width, w.height]
        if screen.contains(where: { $0 <= 0 }) { screen = [d.app.frame.width, d.app.frame.height] }
        let j: [String: Any] = ["name": name, "screen": screen, "tree": node(snap), "orient": d.env["LAYOUT_ORIENT"] ?? "portrait", "text": d.env["LAYOUT_TEXT"] ?? "default",
                                "device": UIDevice.current.name]
        if let data = try? JSONSerialization.data(withJSONObject: j, options: [.sortedKeys]) { try? data.write(to: URL(fileURLWithPath: "\(out)/layout-\(name).json")) }
        try? XCUIScreen.main.screenshot().pngRepresentation.write(to: URL(fileURLWithPath: "\(out)/layout-\(name).png"))
    }
    func back() { let b = d.app.buttons["data.back"]; if b.exists { b.tap(); sleep(1) } }

    func testLayout() {
        d = Driver("layout"); out = d.out
        XCUIDevice.shared.orientation = d.env["LAYOUT_ORIENT"] == "landscape" ? .landscapeLeft : .portrait
        // Dynamic Type for this run: the launch argument the system honours (UIPreferredContentSizeCategoryName)
        d.app.launchArguments += ["-UIPreferredContentSizeCategoryName", d.env["LAYOUT_TEXT"] ?? "UICTContentSizeCategoryL"]
        d.launch()
        sleep(2)
        record("home")
        // thread: the first row of the list
        let row = d.app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH 'thread.row'")).firstMatch
        if row.waitForExistence(timeout: 10) {
            row.tap(); sleep(2); record("thread")
            // composer with the keyboard up
            let c = d.el("thread.composer")
            if c.waitForExistence(timeout: 5) { c.tap(); sleep(1); d.app.typeText("layout check"); record("composer-keyboard"); }
            // the model picker: the control that shows the model
            let model = d.app.buttons.matching(NSPredicate(format: "label CONTAINS[c] 'model' OR identifier CONTAINS 'model'")).firstMatch
            if model.exists { model.tap(); sleep(1); record("picker"); d.app.swipeDown(); sleep(1) } else { d.note("no model control found") }
            // a streaming turn
            let s = d.el("thread.send"); if s.exists { s.tap(); sleep(3); record("streaming") }
            // the viewer
            let v = d.el("data.thread.viewer"); if v.waitForExistence(timeout: 2) { v.tap(); sleep(3); record("viewer"); back() }
            back()
        } else { d.block("no thread row") }
        // settings and the hub picker, by label (their identifiers are not stable across the app's screens)
        for (name, label) in [("settings", "Settings"), ("hub-picker", "Hubs"), ("accounts", "Accounts")] {
            let b = d.app.buttons.matching(NSPredicate(format: "label CONTAINS[c] %@", label)).firstMatch
            if b.waitForExistence(timeout: 3) { b.tap(); sleep(2); record(name); back() } else { d.note("no \(label) control") }
        }
        // pairing: a fresh app on the pair screen
        d.launch(paired: false); sleep(2); record("pair")
        d.finish()
    }
}
