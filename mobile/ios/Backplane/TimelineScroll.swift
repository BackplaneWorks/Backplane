import SwiftUI
import Observation
import UIKit

// Entry identity and its distance from the top are retained together. UIKit
// performs the correction in layout, including intrinsic-height changes that
// don't produce a new Bend screen (images, expanded cards and the keyboard).
@MainActor @Observable
final class TimelinePosition {
    private(set) var following = true
    private(set) var unread = 0
    @ObservationIgnored var latest: (() -> Void)?
    @ObservationIgnored var jump: ((String) -> Void)?
    @ObservationIgnored fileprivate var beginning: (() -> Void)?
    // Kept by the owning navigation context, without observing frame changes.
    // A controller may disappear while a linked source is open.
    @ObservationIgnored fileprivate var viewport: TimelineViewport?
    @ObservationIgnored fileprivate(set) var reading: TimelineReading?

    fileprivate func report(following: Bool, unread: Int) {
        if self.following != following { self.following = following }
        if self.unread != unread { self.unread = unread }
    }

    func resetToStart(firstID: String? = nil) {
        viewport = nil
        reading = nil
        if let firstID { jump?(firstID) }
        else { beginning?() }
    }
}

struct TimelineReading: Equatable {
    let id: String
    let offset: Double
    let first: String
    let last: String
    var json: String {
        guard let data = try? JSONSerialization.data(withJSONObject: ["id": id, "offset": offset, "first": first, "last": last]),
              let value = String(data: data, encoding: .utf8) else { return "" }
        return value
    }
}

fileprivate struct TimelineViewport {
    let following: Bool
    let unread: Int
    let anchor: (id: String, offset: CGFloat)?
    let retained: [(id: String, offset: CGFloat)]
}

struct TimelineScroll<Content: View>: UIViewControllerRepresentable {
    let position: TimelinePosition
    let ids: [String]
    let earlier: Int
    let loadEarlier: () -> Bool
    var later = 0
    var loadLater: () -> Bool = { false }
    var earlierEnabled = true
    var startsAtLatest = true
    // Optional diagnostic callback; normal builds install none.
    var onDisplay: (() -> Void)? = nil
    var onViewport: ((TimelineReading) -> Void)? = nil
    @ViewBuilder let content: () -> Content

    func makeUIViewController(context: Context) -> TimelineController<Content> {
        TimelineController(position: position, startsAtLatest: startsAtLatest, content: content())
    }

    func updateUIViewController(_ controller: TimelineController<Content>, context: Context) {
        controller.configure(ids: ids, earlier: earlier, enabled: earlierEnabled, loadEarlier: loadEarlier, onDisplay: onDisplay, later: later, loadLater: loadLater, onViewport: onViewport, content: content())
    }
}

struct TimelineMarker: UIViewRepresentable {
    let id: String
    func makeUIView(context: Context) -> TimelineMark { TimelineMark(id: id) }
    func updateUIView(_ view: TimelineMark, context: Context) { view.entryID = id }
}

final class TimelineMark: UIView {
    var entryID: String
    init(id: String) { entryID = id; super.init(frame: .zero); isUserInteractionEnabled = false }
    required init?(coder: NSCoder) { fatalError("init(coder:)") }
}

private final class TimelineHost<Content: View>: UIHostingController<Content> {
    var laidOut: (() -> Void)?
    override func viewDidLayoutSubviews() { super.viewDidLayoutSubviews(); laidOut?() }
}

final class TimelineController<Content: View>: UIViewController, UIScrollViewDelegate {
    let scroll = UIScrollView()
    private let host: TimelineHost<Content>
    private let position: TimelinePosition
    private var anchor: (id: String, offset: CGFloat)?
    private var retained: [(id: String, offset: CGFloat)] = []
    private var onViewport: ((TimelineReading) -> Void)?
    private var reportedSpan: [String] = []
    private var pendingJump: String?
    private var following = true
    private var unread = 0
    private var ids: [String] = []
    private var earlier = 0
    private var earlierGate = EarlierPageGate()
    private var loadEarlier: (() -> Bool)?
    private var later = 0
    private var laterGate = EarlierPageGate()
    private var loadLater: (() -> Bool)?
    private var correcting = false
    private var reportPending = false
    private var initialized = false
    private var awaitingBeginning = false
    private var gestureTop: CGFloat = 0
    private var readingOlder = false
    private var onDisplay: (() -> Void)?
    private var displayLink: CADisplayLink?
    private var displayTicks = 0
    private var displayConfiguration: UInt64 = 0
    private var displayLayout: UInt64?
    private var displayObservation: (configuration: UInt64, callback: () -> Void)?

    init(position: TimelinePosition, startsAtLatest: Bool = true, content: Content) {
        self.position = position
        awaitingBeginning = !startsAtLatest && position.viewport?.anchor == nil &&
            position.viewport?.following != true
        if let kept = position.viewport {
            following = kept.following
            unread = kept.unread
            anchor = kept.anchor
            retained = kept.retained
        } else {
            following = startsAtLatest
        }
        host = TimelineHost(rootView: content)
        super.init(nibName: nil, bundle: nil)
        position.latest = { [weak self] in self?.showLatest() }
        position.jump = { [weak self] id in self?.show(id) }
        position.beginning = { [weak self] in self?.showBeginning() }
    }
    required init?(coder: NSCoder) { fatalError("init(coder:)") }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .clear
        scroll.backgroundColor = .clear
        scroll.delegate = self
        scroll.alwaysBounceVertical = true
        scroll.keyboardDismissMode = .interactive
        scroll.accessibilityIdentifier = "thread.timeline"
        scroll.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(scroll)
        addChild(host)
        host.view.backgroundColor = .clear
        host.sizingOptions = .intrinsicContentSize
        host.safeAreaRegions = []
        host.view.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(host.view)
        host.didMove(toParent: self)
        NSLayoutConstraint.activate([
            scroll.topAnchor.constraint(equalTo: view.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            scroll.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            host.view.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            host.view.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
            host.view.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
            host.view.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
            host.view.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor)
        ])
        host.laidOut = { [weak self] in
            guard let self else { return }
            self.reconcile()
            if self.onDisplay != nil { self.displayLayout = self.displayConfiguration }
            self.scheduleDisplayObservation()
        }
    }

    func configure(ids next: [String], earlier: Int, enabled: Bool, loadEarlier: @escaping () -> Bool, onDisplay: (() -> Void)?, later: Int = 0, loadLater: @escaping () -> Bool = { false }, onViewport: ((TimelineReading) -> Void)? = nil, content: Content) {
        // Count only entries appended after the old tail, never history prepends.
        if !following, let last = ids.last, let at = next.firstIndex(of: last) {
            unread += next.distance(from: next.index(after: at), to: next.endIndex)
        }
        ids = next
        self.earlier = earlier
        earlierGate.configure(enabled: enabled, page: next.first ?? "")
        self.loadEarlier = loadEarlier
        self.later = later
        if later > 0 { following = false }
        self.loadLater = loadLater
        laterGate.configure(enabled: enabled, page: next.last ?? "")
        if onDisplay != nil || self.onDisplay != nil {
            displayLink?.invalidate()
            displayLink = nil
            displayObservation = nil
            displayConfiguration &+= 1
            displayLayout = nil
        }
        self.onDisplay = onDisplay
        self.onViewport = onViewport
        host.rootView = content
        if onDisplay != nil { host.view.setNeedsLayout() }
        view.setNeedsLayout()
        report()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        reconcile()
        scheduleDisplayObservation()
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        scheduleDisplayObservation()
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        displayLink?.invalidate()
        displayLink = nil
        displayObservation = nil
    }

    private var canObserveDisplay: Bool {
        guard onDisplay != nil, displayLayout == displayConfiguration, initialized, let window = view.window,
              view.bounds.width > 0, view.bounds.height > 0,
              view.convert(view.bounds, to: window).intersects(window.bounds) else { return false }
        var ancestor: UIView? = view
        while let current = ancestor {
            if current.isHidden || current.alpha <= 0.01 { return false }
            ancestor = current.superview
        }
        return frames().contains { ids.contains($0.0) && $0.1.intersects(scroll.bounds) }
    }

    private func scheduleDisplayObservation() {
        guard displayLink == nil, canObserveDisplay, let callback = onDisplay else { return }
        displayObservation = (displayConfiguration, callback)
        displayTicks = 0
        let link = CADisplayLink(target: self, selector: #selector(observeDisplay))
        displayLink = link
        link.add(to: .main, forMode: .common)
    }

    @objc private func observeDisplay() {
        guard canObserveDisplay, let observation = displayObservation,
              observation.configuration == displayConfiguration else {
            displayLink?.invalidate(); displayLink = nil; displayObservation = nil; return
        }
        displayTicks += 1
        // Let one complete frame opportunity pass after visible layout. The
        // second tick bounds scheduling; screenshots still verify the pixels.
        guard displayTicks >= 2 else { return }
        displayLink?.invalidate()
        displayLink = nil
        displayObservation = nil
        observation.callback()
    }

    private func marks(_ view: UIView) -> [TimelineMark] {
        if let mark = view as? TimelineMark { return [mark] }
        return view.subviews.flatMap { marks($0) }
    }
    private func frames() -> [(String, CGRect)] {
        marks(host.view).map { ($0.entryID, $0.convert($0.bounds, to: scroll)) }
            .sorted { $0.1.minY < $1.1.minY }
    }
    private var top: CGFloat { scroll.contentOffset.y + scroll.adjustedContentInset.top }
    private var viewportTop: CGFloat {
        scroll.convert(CGPoint(x: 0, y: top), to: view.window).y
    }
    private var end: CGFloat {
        max(-scroll.adjustedContentInset.top,
            scroll.contentSize.height - scroll.bounds.height + scroll.adjustedContentInset.bottom - scroll.contentInset.bottom)
    }
    private func capture(_ rows: [(String, CGRect)]) {
        if let first = rows.first(where: { $0.1.maxY > top + 0.5 }) {
            anchor = (first.0, first.1.minY - top + viewportTop)
            // A folded/removed anchor can fall back to the next surviving
            // visible entry at its own old screen position.
            retained = rows.filter { $0.1.maxY > top && $0.1.minY < top + scroll.bounds.height }
                .map { ($0.0, $0.1.minY - top + viewportTop) }
        }
    }
    private func setOffset(_ y: CGFloat) {
        // While reading an anchor near the end, allow bottom padding rather
        // than clamping it upward when a keyboard or a tool card shrinks.
        let needed = max(0, y - end)
        if abs(scroll.contentInset.bottom - needed) > 0.25 { scroll.contentInset.bottom = needed }
        let target = max(-scroll.adjustedContentInset.top, y)
        if abs(scroll.contentOffset.y - target) > 0.25 {
            scroll.setContentOffset(CGPoint(x: scroll.contentOffset.x, y: target), animated: false)
        }
    }
    private func reconcile() {
        guard isViewLoaded, view.window != nil, scroll.bounds.height > 0, !correcting else { return }
        correcting = true
        defer { correcting = false; gestureTop = top }
        let rows = frames()
        guard !rows.isEmpty else { return }
        if awaitingBeginning {
            // A hosting view can expose its new markers while its bounds
            // still describe the previous empty layout. Capturing that
            // intermediate geometry turns centering into a retained offset.
            // A fresh list starts only after its configured first row is
            // laid out inside the hosting bounds. Existing reading anchors
            // never enter this initial-start branch.
            guard rows.first?.0 == ids.first, host.view.bounds.height > 0,
                  rows.allSatisfy({ $0.1.minY >= -0.5 &&
                      $0.1.maxY <= host.view.bounds.height + 0.5 }) else { return }
            awaitingBeginning = false
            setOffset(-scroll.adjustedContentInset.top)
            capture(rows)
            initialized = true
            report()
            return
        }
        if let id = pendingJump, let row = rows.first(where: { $0.0 == id }) {
            setOffset(row.1.minY - scroll.adjustedContentInset.top)
            pendingJump = nil
            following = false
            capture(rows)
        } else if following {
            if scroll.contentInset.bottom != 0 { scroll.contentInset.bottom = 0 }
            if abs(scroll.contentOffset.y - end) > 0.25 {
                scroll.setContentOffset(CGPoint(x: 0, y: end), animated: false)
            }
            capture(rows)
        } else if let held = anchor, let row = rows.first(where: { $0.0 == held.id }) {
            setOffset(row.1.minY - held.offset + viewportTop - scroll.adjustedContentInset.top)
        } else if let kept = retained.first(where: { old in rows.contains { $0.0 == old.id } }),
                  let row = rows.first(where: { $0.0 == kept.id }) {
            anchor = kept
            setOffset(row.1.minY - kept.offset + viewportTop - scroll.adjustedContentInset.top)
        } else {
            capture(rows)
        }
        initialized = true
        report()
        prefetchEarlier()
    }
    private func report() {
        position.viewport = TimelineViewport(following: following, unread: unread,
                                             anchor: anchor, retained: retained)
        // Keep exact local geometry, but cross the bridge only when the
        // visible identities change. A page captures the latest offset below.
        let visible = frames().filter { ids.contains($0.0) && $0.1.maxY > top && $0.1.minY < top + scroll.bounds.height }
        if let held = anchor, let first = visible.first, let last = visible.last,
           visible.contains(where: { $0.0 == held.id }) {
            let reading = TimelineReading(id: held.id, offset: Double(held.offset - viewportTop), first: first.0, last: last.0)
            position.reading = reading
            let span = [held.id, first.0, last.0]
            if span != reportedSpan { reportedSpan = span; onViewport?(reading) }
        }
        guard !reportPending else { return }
        reportPending = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.reportPending = false
            self.position.report(following: self.following, unread: self.unread)
        }
    }
    func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
        earlierGate.beginDrag()
        laterGate.beginDrag()
        gestureTop = top
        readingOlder = false
        following = false
        capture(frames())
    }
    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        guard !correcting, scroll.isDragging || scroll.isDecelerating else { return }
        if abs(top - gestureTop) > 0.25 { readingOlder = top < gestureTop }
        gestureTop = top
        capture(frames())
        following = later == 0 && abs(end - scroll.contentOffset.y) <= 2
        if following { unread = 0 }
        report()
        // Only a real drag/fling can request another page. Layout, restored
        // anchors, keyboard changes and programmatic search jumps must not.
        prefetchEarlier()
    }
    private func prefetchEarlier() {
        if initialized, readingOlder, scroll.isDragging || scroll.isDecelerating,
           top < scroll.bounds.height * 2, earlier > 0, !following, let loadEarlier {
            earlierGate.request(earlier, perform: loadEarlier)
        }
        if initialized, !readingOlder, scroll.isDragging || scroll.isDecelerating,
           end - top < scroll.bounds.height * 2, later > 0, let loadLater {
            laterGate.request(later, perform: loadLater)
        }
    }
    private func showLatest() {
        awaitingBeginning = false
        following = true
        unread = 0
        anchor = nil
        pendingJump = nil
        reconcile()
    }
    private func showBeginning() {
        awaitingBeginning = false
        following = false
        unread = 0
        anchor = nil
        retained = []
        pendingJump = nil
        setOffset(-scroll.adjustedContentInset.top)
        reconcile()
    }
    private func show(_ id: String) { awaitingBeginning = false; pendingJump = id; reconcile() }
}
