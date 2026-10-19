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

    fileprivate func report(following: Bool, unread: Int) {
        if self.following != following { self.following = following }
        if self.unread != unread { self.unread = unread }
    }
}

struct TimelineScroll<Content: View>: UIViewControllerRepresentable {
    let position: TimelinePosition
    let ids: [String]
    let earlier: Int
    let loadEarlier: () -> Void
    @ViewBuilder let content: () -> Content

    func makeUIViewController(context: Context) -> TimelineController<Content> {
        TimelineController(position: position, content: content())
    }

    func updateUIViewController(_ controller: TimelineController<Content>, context: Context) {
        controller.configure(ids: ids, earlier: earlier, loadEarlier: loadEarlier, content: content())
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
    private var pendingJump: String?
    private var following = true
    private var unread = 0
    private var ids: [String] = []
    private var earlier = 0
    private var requestedEarlier: Int?
    private var loadEarlier: (() -> Void)?
    private var correcting = false
    private var reportPending = false
    private var initialized = false

    init(position: TimelinePosition, content: Content) {
        self.position = position
        host = TimelineHost(rootView: content)
        super.init(nibName: nil, bundle: nil)
        position.latest = { [weak self] in self?.showLatest() }
        position.jump = { [weak self] id in self?.show(id) }
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
        host.laidOut = { [weak self] in self?.reconcile() }
    }

    func configure(ids next: [String], earlier: Int, loadEarlier: @escaping () -> Void, content: Content) {
        // Count only entries appended after the old tail, never history prepends.
        if !following, let last = ids.last, let at = next.firstIndex(of: last) {
            unread += next.distance(from: next.index(after: at), to: next.endIndex)
        }
        ids = next
        self.earlier = earlier
        self.loadEarlier = loadEarlier
        host.rootView = content
        view.setNeedsLayout()
        report()
    }

    override func viewDidLayoutSubviews() { super.viewDidLayoutSubviews(); reconcile() }

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
        guard isViewLoaded, scroll.bounds.height > 0, !correcting else { return }
        correcting = true
        defer { correcting = false }
        let rows = frames()
        guard !rows.isEmpty else { return }
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
    }
    private func report() {
        guard !reportPending else { return }
        reportPending = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.reportPending = false
            self.position.report(following: self.following, unread: self.unread)
            if self.initialized, self.top < 400, self.earlier > 0,
               self.requestedEarlier != self.earlier, !self.following {
                self.requestedEarlier = self.earlier
                self.loadEarlier?()
            }
        }
    }
    func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
        following = false
        capture(frames())
    }
    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        guard !correcting, scroll.isDragging || scroll.isDecelerating else { return }
        capture(frames())
        following = abs(end - scroll.contentOffset.y) <= 2
        if following { unread = 0 }
        report()
    }
    private func showLatest() {
        following = true
        unread = 0
        anchor = nil
        pendingJump = nil
        reconcile()
    }
    private func show(_ id: String) { pendingJump = id; reconcile() }
}
