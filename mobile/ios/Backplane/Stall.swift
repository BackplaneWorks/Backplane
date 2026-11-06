import Foundation
import QuartzCore

#if DEBUG
// BACKPLANE_STALL=1: where the main thread blocks. A 5 ms timer notes any gap over 30 ms ("Backplane stall N ms draft=BYTES"), and
// BACKPLANE_ACTS "@keys=abc" types characters 150 ms apart, each logging how long the main run loop took from the edit to idle
// ("Backplane key cost N ms draft=BYTES": the edit, the model, SwiftUI's layout and the commit).
enum Stall {
    static var draftBytes = 0
    static let on = ProcessInfo.processInfo.environment["BACKPLANE_STALL"] == "1"
    private static var last = CACurrentMediaTime()
    static func start() {
        guard on else { return }
        last = CACurrentMediaTime()
        let t = Timer(timeInterval: 0.005, repeats: true) { _ in
            let n = CACurrentMediaTime()
            let gap = (n - last) * 1000
            last = n
            if gap > 30 { NSLog("Backplane stall %.0f ms draft=%d", gap, draftBytes) }
        }
        RunLoop.main.add(t, forMode: .common)
    }
    static func key() {
        guard on else { return }
        let t0 = CACurrentMediaTime()
        let obs = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.beforeWaiting.rawValue, false, Int.max) { _, _ in
            NSLog("Backplane key cost %.1f ms draft=%d", (CACurrentMediaTime() - t0) * 1000, draftBytes)
        }
        CFRunLoopAddObserver(CFRunLoopGetMain(), obs, .commonModes)
    }
}
#endif
