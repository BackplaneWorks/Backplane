import Foundation
import QuartzCore
import Darwin

// Start-up phase marks (DEBUG only): milliseconds since the process was created, one NSLog line each ("Backplane boot <name> <ms>").
enum Boot {
    static let origin: Double = {
        var info = kinfo_proc(); var size = MemoryLayout<kinfo_proc>.stride
        var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()]
        guard sysctl(&mib, 4, &info, &size, nil, 0) == 0 else { return Date().timeIntervalSince1970 }
        let t = info.kp_proc.p_un.__p_starttime
        return Double(t.tv_sec) + Double(t.tv_usec) / 1e6
    }()
    static func mark(_ name: String) {
        #if DEBUG || BP_MEASURE
        NSLog("Backplane boot %@ %.0f", name, (Date().timeIntervalSince1970 - origin) * 1000)
        #endif
    }
}
