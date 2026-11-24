import SwiftUI

// Add project's Clone from git (docs/CLONE.md; the desktop's commands/clone/view.rs): the repository with the person's GitHub repositories
// suggested as they type, where it goes (clone.dest: the hub's default, a folder from dirs.list, a name), Start a thread, then the job's
// progress from the hub's `clone` pushes. The clone runs on the hub's machine; nothing secret passes through here (gh's device code only).

/// Open folder | Clone from git, square, the hub's two tabs.
struct DataProjectModes: View {
    @Binding var clone: Bool
    var body: some View {
        HStack(spacing: 0) {
            tab(Tr.t("Open folder"), on: !clone, id: "data.project.mode.folder") { clone = false }
            tab(Tr.t("Clone from git"), on: clone, id: "data.project.mode.clone") { clone = true }
        }.dbox().padding(.horizontal, 12).padding(.top, 12)
    }
    private func tab(_ label: String, on: Bool, id: String, _ act: @escaping () -> Void) -> some View {
        Button(action: act) {
            Text(label).dmono(12.5, bold: on).foregroundStyle(on ? BackplaneDesign.onAccent : BackplaneDesign.dim).frame(maxWidth: .infinity).frame(height: 34)
                .background(on ? BackplaneDesign.accent : Color.clear).contentShape(Rectangle())
        }.buttonStyle(.plain).accessibilityIdentifier(id).accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// What the sheet's Clone button needs from the form (the form owns the fields; the sheet's toolbar presses it).
@MainActor @Observable final class CloneForm {
    var repoText = ""
    var picked: DCloneRepo?
    var parent = ""           // chosen with Change folder; empty: the hub's default
    var name = ""             // the folder's name; filled with the hub's default until edited
    var nameEdited = false
    var thread = true
    var text = ""             // the first thread's first message (from `/project <address> <message>`)
    var dest: DCloneDest?
    var repos = DCloneRepos()
    var busy = false
    var error = ""
    var jobId = ""
    var job: DCloneJob?
    var notesRead = false
    var repo: String { Clone.repo(typed: repoText, picked: picked) }
    var canStart: Bool { job == nil && Clone.canStart(dest, busy: busy) && dest?.repo.isEmpty == false }

    private var startedRepo = ""   // what clone.start was given, for Try again

    /// The sheet's Clone button: into the folder clone.dest last agreed to (its parent and name), with Start a thread and the first message.
    func start(_ client: DataClient) async {
        var p: [String: Any] = ["repo": repo, "thread": thread]
        if let d = dest, !d.parent.isEmpty, !d.folder.isEmpty { p["parent"] = d.parent; p["name"] = d.folder }
        if !text.isEmpty { p["text"] = text }
        startedRepo = repo
        await send(client, p)
    }
    /// Try again: a new job with a new id, for the same repository, into the folder the failed one had.
    func retry(_ client: DataClient, _ j: DCloneJob) async {
        await send(client, Clone.retryParams(j, repo: startedRepo, thread: thread, text: text))
    }
    /// The job's id is the app's own and known before anything is sent: a push that beats the answer is this job's, and the answer cannot undo it.
    private func send(_ client: DataClient, _ params: [String: Any]) async {
        busy = true; error = ""; defer { busy = false }
        let id = Clone.newId(); var p = params; p["id"] = id
        notesRead = false; jobId = id
        job = DCloneJob(id: id, repo: (p["repo"] as? String) ?? "")
        do { let j = try await client.cloneJob("clone.start", p); see(client.cloneJobs[id]); see(j) }
        catch {
            // refused: the hub keeps a failed job under this id with the words; an older hub keeps none, so the words make one here
            let words = (error as? DError)?.message ?? error.localizedDescription
            if let j = try? await client.cloneJob("clone.status", ["id": id]) { see(j) }
            if job?.running != false { job = DCloneJob(id: id, state: "failed", repo: job?.repo ?? "", error: words) }
        }
    }
    /// clone.status for the job on show: after a reconnect, back in the foreground, and now and then while it runs (a push may have been missed).
    func refresh(_ client: DataClient) async {
        guard !jobId.isEmpty, job?.running == true else { return }
        if let j = try? await client.cloneJob("clone.status", ["id": jobId]) { see(j) }
    }
    func see(_ j: DCloneJob?) { job = Clone.follow(job, j, id: jobId) }
}

struct DataCloneView: View {
    let client: DataClient
    @Bindable var form: CloneForm
    let finish: (Clone.After) -> Void
    @State private var picking = false
    @Environment(\.openURL) private var openURL
    @Environment(\.scenePhase) private var scene

    var body: some View {
        Group {
            if form.job != nil { jobView } else { formView }
        }
        .navigationDestination(isPresented: $picking) {
            DataFolderPick(client: client, start: form.dest?.parent ?? form.parent) { p in form.parent = p; picking = false }
        }
        // the job on show follows the hub's pushes (and a status answer, below)
        .onChange(of: client.cloneJobs[form.jobId]) { _, j in form.see(j) }
        .onChange(of: client.conn) { _, c in if c == .online { Task { await form.refresh(client) } } }
        .onChange(of: scene) { _, p in if p == .active { Task { await form.refresh(client) } } }
        .onChange(of: form.job) { _, j in if let j { let a = Clone.after(j, read: form.notesRead); if a != .stay { finish(a) } } }
        .onChange(of: client.cloneReposGen) { _, _ in Task { await suggest(wait: false) } }
    }

    // MARK: the form
    private var formView: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                TextField(Tr.t("https or ssh URL, or owner/repo"), text: $form.repoText).dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
                    .padding(.horizontal, 10).frame(height: 40).dbox().accessibilityIdentifier("data.clone.repo")
                suggestions
                if let d = form.dest, d.isRepo { destination(d) }
                if !form.error.isEmpty { Text(Tr.t(form.error)).dmono(12).foregroundStyle(BackplaneDesign.bad).accessibilityIdentifier("data.clone.error") }
            }.padding(12)
        }
        .scrollDismissesKeyboard(.interactively)
        .task(id: form.repoText) { await suggest(wait: true) }
        .task(id: destKey) { await askDest() }
    }
    private var suggestions: some View {
        let typed = form.repoText.trimmingCharacters(in: .whitespaces)
        let rows = form.repos.repos.filter { $0 != form.picked || typed != $0.name }
        return VStack(alignment: .leading, spacing: 0) {
            ForEach(rows.prefix(8), id: \.self) { r in
                Button { pick(r) } label: {
                    VStack(alignment: .leading, spacing: 2) {
                        HStack(spacing: 6) {
                            Text(r.name).dmono(13).foregroundStyle(BackplaneDesign.text).lineLimit(1)
                            if r.priv { Text(Tr.t("private")).dmono(10).foregroundStyle(BackplaneDesign.dim).padding(.horizontal, 4).padding(.vertical, 1).overlay(Rectangle().strokeBorder(BackplaneDesign.line, lineWidth: 1)) }
                            Spacer(minLength: 0)
                        }
                        if !r.description.isEmpty { Text(r.description).dmono(11).foregroundStyle(BackplaneDesign.faint).lineLimit(2) }
                    }.padding(.horizontal, 10).padding(.vertical, 7).frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
                }.buttonStyle(.plain).accessibilityIdentifier("data.clone.suggest." + r.name)
                DLine()
            }
            if form.repos.loading && form.repos.repos.isEmpty { Text(Tr.t("Reading your repositories…")).dmono(11).foregroundStyle(BackplaneDesign.faint).padding(.vertical, 6) }
            if !form.repos.shownHint.isEmpty {
                Text(Tr.t(form.repos.shownHint)).dmono(11.5).foregroundStyle(BackplaneDesign.dim).padding(.vertical, 6).accessibilityIdentifier("data.clone.hint")
                if form.repos.offersSignIn { signInButton }
            }
            if let s = client.cloneSignin, !s.state.isEmpty, s.state != "cancelled", form.repos.offersSignIn || (s.state == "done" && form.repos.repos.isEmpty) { signIn(s) }
        }
    }
    private func destination(_ d: DCloneDest) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            DLine().padding(.vertical, 4)
            HStack(alignment: .firstTextBaseline) {
                Text(Tr.ios_clone_into(dest: d.destShown)).dmono(12.5).foregroundStyle(BackplaneDesign.text).lineLimit(2).accessibilityIdentifier("data.clone.dest")
                Spacer(minLength: 6)
                Button(Tr.t("Change folder")) { picking = true }.buttonStyle(DButton(compact: true)).accessibilityIdentifier("data.clone.change")
            }
            TextField(Tr.t("Folder name"), text: Binding(get: { form.name }, set: { form.name = $0; form.nameEdited = true }))
                .dmono(13).textInputAutocapitalization(.never).autocorrectionDisabled().padding(.horizontal, 10).frame(height: 38).dbox().accessibilityIdentifier("data.clone.name")
            if !d.ok && !d.error.isEmpty { Text(Tr.t(d.error)).dmono(12).foregroundStyle(BackplaneDesign.bad).accessibilityIdentifier("data.clone.dest.error") }
            if d.ok && d.same { Text(Tr.t("That folder is a clone of it already: it is added as it is.")).dmono(11.5).foregroundStyle(BackplaneDesign.dim) }
            Toggle(isOn: $form.thread) { Text(Tr.t("Start a thread")).dmono(13).foregroundStyle(BackplaneDesign.text) }.tint(BackplaneDesign.accent).accessibilityIdentifier("data.clone.thread")
        }
    }
    private func pick(_ r: DCloneRepo) {
        form.picked = r; form.repoText = r.name
        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
    }
    // the suggestions for what is typed, a moment after the typing stops
    private func suggest(wait: Bool) async {
        if wait { try? await Task.sleep(for: .milliseconds(300)); if Task.isCancelled { return } }
        let q = form.repoText.trimmingCharacters(in: .whitespaces)
        guard let r: DCloneRepos = try? await client.call("clone.repos", ["q": q, "limit": 20]), q == form.repoText.trimmingCharacters(in: .whitespaces) else { return }
        form.repos = r
    }
    private var destKey: String { [form.repo, form.parent, form.nameEdited ? form.name : ""].joined(separator: "\u{1f}") }
    // where it goes: the hub's default, then whatever the person chose; a new repository takes its own default name again
    private func askDest() async {
        try? await Task.sleep(for: .milliseconds(250)); if Task.isCancelled { return }
        let repo = form.repo
        guard !repo.isEmpty else { form.dest = nil; return }
        let p = Clone.destParams(repo: repo, parent: form.parent, name: form.nameEdited ? form.name : "")
        guard let d: DCloneDest = try? await client.call("clone.dest", p), repo == form.repo, !Task.isCancelled else { return }
        form.dest = d
        if !form.nameEdited { form.name = d.folder }
    }
    // MARK: the job
    private var jobView: some View {
        let j = form.job ?? DCloneJob(id: "")
        return ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                Text(j.state == "done" ? Tr.ios_clone_cloned(repo: j.repo) : Tr.ios_clone_title(repo: j.repo)).dmono(14, bold: true).foregroundStyle(BackplaneDesign.text).accessibilityIdentifier("data.clone.job.title")
                if !j.destShown.isEmpty { Text(Tr.ios_clone_into(dest: j.destShown)).dmono(11.5).foregroundStyle(BackplaneDesign.faint) }
                switch j.state {
                case "running":
                    if !j.phase.isEmpty { Text(Tr.t(j.phase)).dmono(12.5).foregroundStyle(BackplaneDesign.dim).accessibilityIdentifier("data.clone.job.phase") }
                    Group { if let f = j.fraction { ProgressView(value: f) } else { ProgressView().frame(maxWidth: .infinity, alignment: .leading) } }.tint(BackplaneDesign.accent).accessibilityIdentifier("data.clone.job.progress")
                    if !j.detail.isEmpty { Text(j.detail).dmono(11).foregroundStyle(BackplaneDesign.faint) }
                    Button(Tr.t("Cancel")) { Task { _ = try? await client.cloneJob("clone.cancel", ["id": j.id]) } }.buttonStyle(DButton(danger: true)).accessibilityIdentifier("data.clone.cancel")
                case "done":
                    ForEach(j.notes, id: \.self) { n in Text(Tr.t(n)).dmono(12).foregroundStyle(BackplaneDesign.text).accessibilityIdentifier("data.clone.note") }
                    Button(Tr.t(j.thread.isEmpty ? "Done" : "Open thread")) { form.notesRead = true; finish(Clone.after(j, read: true)) }.buttonStyle(DButton(primary: true)).accessibilityIdentifier("data.clone.open")
                default:
                    Text(j.state == "cancelled" ? Tr.t("Cancelled") : Tr.t(j.error)).dmono(12.5).foregroundStyle(j.state == "cancelled" ? BackplaneDesign.dim : BackplaneDesign.bad).accessibilityIdentifier("data.clone.job.error")
                    HStack(spacing: 8) {
                        Button(Tr.t("Try again")) { Task { await form.retry(client, j) } }.buttonStyle(DButton(primary: true)).accessibilityIdentifier("data.clone.retry")
                        if j.signin { signInButton }
                        Button(Tr.t("Back")) { form.job = nil; form.jobId = "" }.buttonStyle(DButton()).accessibilityIdentifier("data.clone.back")
                    }
                    if j.signin, let s = client.cloneSignin, !s.state.isEmpty, s.state != "cancelled" { signIn(s) }
                }
                if !form.error.isEmpty { Text(Tr.t(form.error)).dmono(12).foregroundStyle(BackplaneDesign.bad) }
            }.padding(12).frame(maxWidth: .infinity, alignment: .leading)
        }
        // a push missed while the socket was away: ask once more now and then while it runs
        .task(id: form.jobId) {
            while !Task.isCancelled, form.job?.running == true {
                try? await Task.sleep(for: .seconds(3))
                await form.refresh(client)
            }
        }
    }

    // MARK: GitHub sign-in (gh's device code, entered on any device)
    private var signInButton: some View {
        Button(Tr.t("Sign in to GitHub")) { Task { do { try await client.cloneSignIn() } catch { form.error = (error as? DError)?.message ?? error.localizedDescription } } }
            .buttonStyle(DButton(compact: true)).accessibilityIdentifier("data.clone.signin")
    }
    private func signIn(_ s: DCloneSignin) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            switch s.state {
            case "starting": Text(Tr.t("Starting the sign-in…")).dmono(12).foregroundStyle(BackplaneDesign.dim)
            case "waiting":
                Text(Tr.ios_clone_code(url: s.url)).dmono(12).foregroundStyle(BackplaneDesign.dim)
                HStack(spacing: 8) {
                    Text(s.code).dmono(18, bold: true).foregroundStyle(BackplaneDesign.text).textSelection(.enabled).accessibilityIdentifier("data.clone.signin.code")
                    Button(Tr.t("Copy")) { UIPasteboard.general.string = s.code }.buttonStyle(DButton(compact: true)).accessibilityIdentifier("data.clone.signin.copy")
                    if let u = URL(string: s.url) { Button(Tr.t("Open the page")) { openURL(u) }.buttonStyle(DButton(primary: true, compact: true)).accessibilityIdentifier("data.clone.signin.open") }
                }
            case "done": Text(Tr.t("Signed in to GitHub.")).dmono(12).foregroundStyle(BackplaneDesign.ok)
            default: if !s.error.isEmpty { Text(Tr.t(s.error)).dmono(12).foregroundStyle(BackplaneDesign.bad) }
            }
        }.padding(.vertical, 6)
    }
}

/// The parent folder for a clone, from the hub's dirs.list (as Open folder browses).
struct DataFolderPick: View {
    let client: DataClient
    let start: String
    let chosen: (String) -> Void
    @State private var dirs: DDirs?
    @State private var error = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(dirs?.path ?? "").dmono(12).foregroundStyle(BackplaneDesign.dim).lineLimit(1).truncationMode(.head)
                Spacer(minLength: 6)
                Button(Tr.t("Use this folder")) { if let d = dirs { chosen(d.path) } }.buttonStyle(DButton(primary: true, compact: true)).disabled(dirs == nil).accessibilityIdentifier("data.clone.folder.use")
            }.padding(12)
            if !error.isEmpty { Text(Tr.t(error)).dmono(12).foregroundStyle(BackplaneDesign.bad).padding(.horizontal, 12).padding(.bottom, 8) }
            DLine()
            List {
                if let d = dirs {
                    if d.parent != d.path { DataDirRow(name: "..", icon: "arrow.up.left") { Task { await go(d.parent) } } }
                    ForEach(d.dirs, id: \.self) { n in DataDirRow(name: n, icon: "folder") { Task { await go(d.path.hasSuffix("/") ? d.path + n : d.path + "/" + n) } } }
                }
            }.listStyle(.plain).scrollContentBackground(.hidden).environment(\.defaultMinListRowHeight, 1)
        }
        .background(BackplaneDesign.panel)
        .dataBar(Tr.t("Change folder"))
        .task { await go(start.isEmpty ? nil : start) }
    }
    private func go(_ p: String?) async {
        do { dirs = try await client.dirs(p); error = "" } catch { self.error = (error as? DError)?.message ?? error.localizedDescription }
    }
}

/// A folder row of a dirs.list browser (Open folder and Change folder).
struct DataDirRow: View {
    let name: String
    let icon: String
    let act: () -> Void
    var body: some View {
        Button(action: act) {
            HStack(spacing: 10) { Image(systemName: icon).font(.system(size: 13)).foregroundStyle(BackplaneDesign.dim).frame(width: 18); Text(name).dmono(13).foregroundStyle(BackplaneDesign.text).lineLimit(1); Spacer() }
                .padding(.horizontal, 14).frame(minHeight: 40).contentShape(Rectangle())
        }.buttonStyle(.plain).dataRow()
    }
}
