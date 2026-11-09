import SwiftUI

// The app's language ("en" or "ja", Tr.lang from Settings, Language), set at the root (DataRoot). UIKit pieces read it to redraw their words on a change.
private struct BackplaneLanguageKey: EnvironmentKey {
    static let defaultValue = "en"
}

extension EnvironmentValues {
    var backplaneLanguage: String {
        get { self[BackplaneLanguageKey.self] }
        set { self[BackplaneLanguageKey.self] = newValue }
    }
}
