import SwiftUI

// The app when it speaks data/1 (build with SWIFT_ACTIVE_COMPILATION_CONDITIONS "GPUI_NATIVE DATA_APP"): no engine, no bridge, no server-drawn views.
#if DATA_APP
@main
struct DataApp: App {
    @State private var client = DataClient.shared
    private let monoFonts: Void = Mono.register()
    var body: some Scene {
        WindowGroup { DataRoot(client: client) }
    }
}
#endif
