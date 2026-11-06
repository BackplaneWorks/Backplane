import Foundation

// This build's URL scheme (Info.plist `BPScheme`, from the URL_SCHEME build
// setting): the GPUI experiment registers its own, so it never takes
// `backplane://` from the production app.
enum AppIdentity {
    static let scheme = (Bundle.main.object(forInfoDictionaryKey: "BPScheme") as? String) ?? "backplane"
}
