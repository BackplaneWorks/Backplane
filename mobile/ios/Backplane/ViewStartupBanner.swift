import SwiftUI

struct ViewStartupBanner: View {
    let model: AppModel
    private var key: String? {
        if let hub = model.screen?.hub, !hub.isEmpty {
            return model.startup[hub] == .compatibilityRequired ? hub : nil
        }
        return model.startup.keys.sorted().first { model.startup[$0] == .compatibilityRequired }
    }
    var body: some View {
        if let key {
            VStack(alignment: .leading, spacing: 4) {
                // a hub without views (older than 0.12.0): it needs an
                // update; the app never downloads a hub's whole history
                Text("This hub needs an update · " + key).font(.caption)
                HStack {
                    Button("Retry") { model.retryView(key) }
                    Spacer()
                }.font(.caption)
            }.padding(10).background(.bar)
        }
    }
}

