import ActivityKit
import Foundation

private func hex(_ d: Data) -> String { d.map { String(format: "%02x", $0) }.joined() }

// Runs a Live Activity (lock screen + Dynamic Island) per paired hub from
// the screen's islands. One can only start while the app is in front;
// after that its hub keeps it current with pushes to its token, and ends
// it when that hub's work is done. Each activity's token goes to its own
// hub alone, so one machine going idle never ends another's.
@MainActor
final class IslandController {
    private var activities: [String: Activity<IslandAttributes>] = [:]
    private let register: (String, String, String?) -> Void
    // with no word for this long iOS marks it stale (the hubs push at least
    // hourly while any thread works: push's Island.fresh)
    private var stale: Date { Date().addingTimeInterval(3 * 3600) }

    // register(kind, token, hub): "activity" for a hub's activity (to that
    // hub), "start" for starting one by push (iOS 17.2+, to every hub)
    init(register: @escaping (String, String, String?) -> Void) {
        self.register = register
        // one per hub; an older one (no hub, or a second for a hub) goes now
        for a in Activity<IslandAttributes>.activities {
            if a.attributes.hub.isEmpty || activities[a.attributes.hub] != nil {
                Self.end(a, nil)
            } else {
                activities[a.attributes.hub] = a
                watch(a)
            }
        }
        if #available(iOS 17.2, *) {
            Task {
                for await t in Activity<IslandAttributes>.pushToStartTokenUpdates { register("start", hex(t), nil) }
            }
        }
    }

    private func watch(_ a: Activity<IslandAttributes>) {
        let hub = a.attributes.hub
        Task {
            for await t in a.pushTokenUpdates { register("activity", hex(t), hub) }
        }
    }

    private static func end(_ a: Activity<IslandAttributes>, _ s: IslandAttributes.ContentState?) {
        Task { await a.end(s.map { .init(state: $0, staleDate: nil) }, dismissalPolicy: .immediate) }
    }

    // a hub no longer paired takes its activity with it
    func show(_ islands: [HubIsland], foreground: Bool) {
        let hubs = Set(islands.map(\.hub))
        for (hub, a) in activities where !hubs.contains(hub) {
            activities[hub] = nil
            Self.end(a, nil)
        }
        for i in islands { show(i.hub, i.island, foreground: foreground) }
    }

    private func show(_ hub: String, _ s: IslandAttributes.ContentState, foreground: Bool) {
        if let a = activities[hub], a.activityState == .ended || a.activityState == .dismissed { activities[hub] = nil }
        if activities[hub] == nil, s.running > 0 {
            guard foreground, ActivityAuthorizationInfo().areActivitiesEnabled else { return }
            do {
                let a = try Activity.request(attributes: IslandAttributes(hub: hub), content: .init(state: s, staleDate: stale), pushType: .token)
                activities[hub] = a
                watch(a)
            } catch {
                NSLog("live activity: %@", error.localizedDescription)
            }
            return
        }
        // against what the activity shows now (its hub's push may have
        // changed it since), and again whenever its stale date draws near
        guard let a = activities[hub] else { return }
        let due = (a.content.staleDate ?? .distantFuture) < Date().addingTimeInterval(2 * 3600)
        guard s != a.content.state || due else { return }
        if s.running > 0 {
            Task { await a.update(.init(state: s, staleDate: stale)) }
        } else {
            activities[hub] = nil
            Self.end(a, s)
        }
    }
}
