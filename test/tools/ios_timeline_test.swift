import SwiftUI
import UIKit

// Simulator acceptance harness for the production scroll controller. UIKit
// lays out actual hosted SwiftUI rows; no copied scroll algorithm or mocks.
private struct Rows: View {
    let first: Int
    let lines: Int
    let header: CGFloat
    let image: CGFloat
    let tool: CGFloat
    var body: some View {
        VStack(spacing: 14) {
            Color.clear.frame(height: header)
            ForEach(first..<100, id: \.self) { n in
                VStack(spacing: 0) {
                    Text("Message \(n)").frame(maxWidth: .infinity, alignment: .leading)
                    if n == 22 { Text(String(repeating: "text reflows when the viewport width changes ", count: 12)) }
                    if n == 20 { Color.blue.frame(height: image) }
                    if n == 25 { Color.orange.frame(height: tool) }
                }
                .frame(minHeight: 60)
                .background(TimelineMarker(id: "m\(n)"))
            }
            Text(String(repeating: "streamed line\n", count: lines))
                .frame(maxWidth: .infinity, alignment: .leading)
            Color.clear.frame(height: 1)
        }
        .padding(16)
    }
}

@main private final class App: UIResponder, UIApplicationDelegate {
    var window: UIWindow?
    func application(_ application: UIApplication, didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        NSLog("ANCHOR application launched")
        let w = UIWindow(frame: UIScreen.main.bounds)
        window = w
        let p = TimelinePosition()
        let initial = Rows(first: 10, lines: 0, header: 0, image: 0, tool: 0)
        let controller = TimelineController(position: p, content: initial)
        w.rootViewController = controller
        w.makeKeyAndVisible()
        Task { @MainActor in
            var checks = 0
            var maxError: CGFloat = 0
            let file = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0].appendingPathComponent("result.txt")
            try? "".write(to: file, atomically: true, encoding: .utf8)
            func record(_ s: String) {
                print(s)
                let old = (try? String(contentsOf: file, encoding: .utf8)) ?? ""
                try? (old + s + "\n").write(to: file, atomically: true, encoding: .utf8)
            }
            @MainActor func settle() async { try? await Task.sleep(for: .milliseconds(40)); w.layoutIfNeeded(); controller.view.layoutIfNeeded() }
            func marker(_ id: String, _ root: UIView) -> TimelineMark? {
                if let m = root as? TimelineMark, m.entryID == id { return m }
                return root.subviews.lazy.compactMap { marker(id, $0) }.first
            }
            func offset(_ id: String) -> CGFloat {
                guard let m = marker(id, controller.view) else { return .nan }
                return m.convert(m.bounds, to: w).minY
            }
            func expect(_ title: String, _ value: CGFloat, _ expected: CGFloat) {
                checks += 1
                maxError = max(maxError, abs(value - expected))
                if !value.isFinite || abs(value - expected) > 0.5 {
                    record("FAIL \(title): \(value) expected \(expected)")
                }
            }
            func update(first: Int = 10, lines: Int = 0, header: CGFloat = 0, image: CGFloat = 0, tool: CGFloat = 0) {
                controller.configure(ids: (first..<100).map { "m\($0)" }, earlier: 0, loadEarlier: {},
                                     content: Rows(first: first, lines: lines, header: header, image: image, tool: tool))
            }
            record("START")
            update()
            await settle()
            expect("initial follows latest", controller.scroll.contentOffset.y,
                   controller.scroll.contentSize.height - controller.scroll.bounds.height + controller.scroll.adjustedContentInset.bottom)
            // The same drag-start capture used by a real finger, at a partial row.
            let y = marker("m40", controller.view)!.convert(.zero, to: controller.scroll).y
            controller.scroll.setContentOffset(CGPoint(x: 0, y: y + 17), animated: false)
            controller.scrollViewWillBeginDragging(controller.scroll)
            let held = offset("m40")
            for n in 1...200 {
                update(lines: n)
                await settle()
                expect("stream \(n)", offset("m40"), held)
            }
            update(first: 0, lines: 200)
            await settle()
            expect("history prepended", offset("m40"), held)
            update(first: 0, lines: 200, header: 100)
            await settle()
            expect("header expands above", offset("m40"), held)
            update(first: 0, lines: 200, header: 100, image: 275, tool: 190)
            await settle()
            expect("image and tool expand above", offset("m40"), held)
            update(first: 0, lines: 200, header: 0, image: 70, tool: 0)
            await settle()
            expect("content shrinks above", offset("m40"), held)
            controller.view.frame.size.height -= 260
            await settle()
            expect("keyboard reduces viewport", offset("m40"), held)
            expect("keyboard actually reduced height", controller.scroll.bounds.height, w.bounds.height - 260)
            controller.view.frame.origin.y += 44
            controller.view.frame.size.height -= 44
            await settle()
            expect("toolbar moves viewport origin", offset("m40"), held)
            controller.view.frame.size.width -= 80
            await settle()
            expect("width changes/reflow", offset("m40"), held)
            controller.view.frame.size.height += 260
            await settle()
            expect("keyboard dismiss", offset("m40"), held)
            p.latest?()
            await settle()
            expect("intentional latest", controller.scroll.contentOffset.y,
                   controller.scroll.contentSize.height - controller.scroll.bounds.height + controller.scroll.adjustedContentInset.bottom)
            update(first: 0, lines: 250)
            await settle()
            expect("follow resumes", controller.scroll.contentOffset.y,
                   controller.scroll.contentSize.height - controller.scroll.bounds.height + controller.scroll.adjustedContentInset.bottom)
            record("DONE \(checks) real UIKit layout assertions max_error=\(maxError)")
        }
        return true
    }
}
