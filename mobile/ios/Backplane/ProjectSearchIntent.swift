import Foundation

// Search edits and a committed Return have separate lifetimes. The caller
// supplies only authoritative results that match the query and owner token.
struct ProjectSearchIntent {
    struct Owner: Equatable {
        // The caller changes generation on pairing or search-context changes.
        let generation: UInt64
    }
    struct Query: Equatable {
        let scope: UInt64
        let edit: UInt64
        let text: String
        let owner: Owner
    }
    struct Go: Equatable {
        let query: Query
        let project: String
    }

    private(set) var editing: Query?
    private(set) var committed: Query?
    private var serial: UInt64 = 0
    private var resumable: Query?

    // Begin a new visible editing lifetime. A committed Return is independent
    // and cannot be cancelled by the old field disappearing.
    mutating func begin(text: String, owner: Owner) -> Query {
        serial += 1
        let q = Query(scope: serial, edit: 0, text: text, owner: owner)
        editing = q
        resumable = nil
        return q
    }

    // An edit after Return starts a new scope, while the committed Return
    // remains ordered ahead of it. Old owners cannot edit the new field.
    mutating func edit(_ text: String, scope: UInt64?, owner: Owner) -> Query? {
        if let current = editing {
            guard current.scope == scope, current.owner == owner else { return nil }
            let q = Query(scope: current.scope, edit: current.edit + 1, text: text, owner: owner)
            editing = q
            return q
        }
        // A post-submit edit must name that submitted scope, not a stale view.
        guard let pending = committed ?? resumable, pending.scope == scope, pending.owner == owner else { return nil }
        return begin(text: text, owner: owner)
    }

    // Return captures the exact query, never a project from the current UI.
    // The caller must order this committed query before subsequent edits.
    mutating func submit(scope: UInt64, owner: Owner) -> Query? {
        guard let q = editing, q.scope == scope, q.owner == owner, committed == nil else { return nil }
        editing = nil
        committed = q
        resumable = q
        return q
    }

    // An accepted result must be the exact committed request and reflect the
    // complete merged filtered rows in display order. Empty rows are a no-op.
    mutating func accept(_ query: Query, projects: [String]) -> Go? {
        guard committed == query else { return nil }
        committed = nil
        guard let project = projects.first, !project.isEmpty else { return nil }
        resumable = nil
        return Go(query: query, project: project)
    }

    mutating func disappear(scope: UInt64) {
        if editing?.scope == scope { editing = nil }
    }

    // Explicit navigation, pairing replacement or leaving the search context.
    mutating func cancel() {
        editing = nil
        committed = nil
        resumable = nil
    }
}
