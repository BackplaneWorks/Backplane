// A declined gesture never consumes its page. Readiness changes alone do not
// request data: only the scroll controller's existing real-drag gate calls here.
struct EarlierPageGate {
    private var enabled = false
    private var began = false
    private var accepted = false
    private var page = ""

    mutating func configure(enabled: Bool, page: String = "") {
        self.enabled = enabled
        // A continuing real fling may prefetch the next displayed page.
        // Readiness or a count-only refresh of the same page cannot rearm it.
        if self.page != page { self.page = page; accepted = false }
    }

    mutating func beginDrag() {
        began = true
        accepted = false
    }

    @discardableResult mutating func request(_ earlier: Int, perform: () -> Bool) -> Bool {
        guard began, enabled, earlier > 0, !accepted, perform() else { return false }
        accepted = true
        return true
    }
}
