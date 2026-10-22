// Native connection policy. The shared bridge owns view RPCs and compact choices.
// A hub without views is compatibilityRequired: it needs an update. The
// whole-log compatibility sync is retired: compatibilityChosen is never set
// (the app loads the recent window and pages back, never all history).
enum StartupPolicy: Equatable {
    case discovering
    case thin
    case compatibilityRequired
    case compatibilityChosen
}

// A bot can change tabs without changing its owner/nav identity. Only an
// observed current presentation request may release that owner's old view.
struct ThinPresentationFence {
    private var epoch: UInt64?
    private var revision: UInt32 = 0
    private var active = false

    mutating func observe(_ update: SyncUpdate, epoch: UInt64) -> Bool {
        guard update.thinPresenting else { return false }
        return begin(epoch: epoch, revision: update.requestRevision!)
    }

    mutating func begin(epoch: UInt64, revision: UInt32) -> Bool {
        guard revision > 0, self.epoch != epoch || revision > self.revision else { return false }
        self.epoch = epoch
        self.revision = revision
        active = true
        return true
    }

    func accepts(epoch: UInt64, revision: UInt32) -> Bool {
        active && self.epoch == epoch && self.revision == revision
    }

    mutating func invalidate() { active = false }
}
