import SwiftUI

// All ordering, filters, dates and actions come from the shared Bend model.
struct BoardModel: Decodable {
    let title: String
    let filters: [SetButton]
    let sections: [BoardSection]
    let add: BoardAdd
}
struct BoardAdd: Decodable { let name, hint, text: String }
struct BoardSection: Decodable { let title: String; let count: Int; let rows: [BoardRow] }
struct BoardRow: Decodable, Identifiable {
    let id, title, sub, status, mark, notes, edit, editNotes: String
    let overdue, dueSoon, open: Bool
    let links, buttons: [SetButton]
}

struct BoardScreen: View {
    let model: AppModel
    let board: BoardModel
    let hub: String
    @State private var title = ""

    private func routed(_ value: String) -> String { value.hasPrefix(hub + "|") ? value : hub + "|" + value }
    private func add() {
        model.field(board.add.name, title)
        model.act("tb-add", hub + "|")
        title = ""
    }
    var body: some View {
        NavigationStack {
            List {
                Section {
                    HStack {
                        TextField(board.add.hint, text: $title).onSubmit(add)
                            .onChange(of: title) { _, v in model.field(board.add.name, v) }
                        Button("Add", action: add)
                    }
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 6) {
                            ForEach(board.filters, id: \.self) { b in
                                Button(b.label) { model.act(b.action, routed(b.value)) }
                                    .buttonStyle(.bordered).tint(b.on ? .accentColor : .secondary)
                            }
                        }
                    }
                }
                ForEach(board.sections, id: \.title) { section in
                    Section {
                        ForEach(section.rows) { r in
                            VStack(alignment: .leading, spacing: 8) {
                                HStack(alignment: .top, spacing: 12) {
                                    Button(r.mark) { model.act("tb-tick", routed(r.id)) }
                                        .buttonStyle(.borderless).accessibilityLabel(r.status)
                                    Button { model.act("tb-row", routed(r.id)) } label: {
                                        VStack(alignment: .leading, spacing: 4) {
                                            Text(r.title).foregroundStyle(.primary)
                                            Text(r.sub).font(.caption).foregroundStyle(r.overdue ? Color.red : Color.secondary)
                                        }.frame(maxWidth: .infinity, alignment: .leading)
                                    }.buttonStyle(.borderless)
                                }
                                if r.open { BoardDetail(model: model, row: r) }
                            }
                            .swipeActions {
                                Button("Done") { model.act("tb-status", routed(r.id) + "\tdone") }.tint(.green)
                                Button("Drop") { model.act("tb-status", routed(r.id) + "\tdropped") }.tint(.orange)
                            }
                        }
                    } header: { Text("\(section.title) · \(section.count)") }
                }
            }
            .navigationTitle(board.title).navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Close") { model.act("tb-close", hub + "|") } } }
            .onAppear { title = board.add.text }
        }
    }
}

private struct BoardDetail: View {
    let model: AppModel
    let row: BoardRow
    @State private var title: String
    @State private var notes: String
    init(model: AppModel, row: BoardRow) {
        self.model = model; self.row = row
        _title = State(initialValue: row.edit)
        _notes = State(initialValue: row.editNotes)
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if !row.notes.isEmpty { Text(row.notes).font(.body).textSelection(.enabled) }
            ForEach(row.links, id: \.self) { b in Button(b.label) { model.act(b.action, b.value) } }
            TextField("Title", text: $title).onChange(of: title) { _, v in model.field("tbedit", v) }
                .onSubmit { model.field("tbedit", title); model.act("tb-title", row.id) }
            Button("Rename") { model.field("tbedit", title); model.act("tb-title", row.id) }
            TextEditor(text: $notes).frame(minHeight: 80).onChange(of: notes) { _, v in model.field("tbnotes", v) }
            Button("Save notes") { model.field("tbnotes", notes); model.act("tb-notes", row.id) }
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) { ForEach(row.buttons, id: \.self) { b in Button(b.label) { model.act(b.action, b.value) }.buttonStyle(.bordered) } }
            }
        }
    }
}
