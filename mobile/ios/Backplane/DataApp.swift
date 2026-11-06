import SwiftUI

// The app when it speaks data/1 (build with SWIFT_ACTIVE_COMPILATION_CONDITIONS "GPUI_NATIVE DATA_APP"): no engine, no bridge, no server-drawn views.
#if DATA_APP
@main
struct DataApp: App {
    @State private var client = DataClient.shared
    private let monoFonts: Void = Mono.register()
    var body: some Scene {
        WindowGroup {
            #if DEBUG
            if let f = ProcessInfo.processInfo.environment["BACKPLANE_MDPREVIEW"], let t = try? String(contentsOfFile: f, encoding: .utf8) {
                DMdPreview(full: t, stream: ProcessInfo.processInfo.environment["BACKPLANE_MDSTREAM"] == "1").preferredColorScheme(AppSettings.shared.colorScheme)
            } else { DataRoot(client: client).onOpenURL { client.opened($0) } }
            #else
            DataRoot(client: client).onOpenURL { client.opened($0) }
            #endif
        }
    }
}
#endif
