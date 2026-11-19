//! The ordered entries for a thread's right rail.
use serde_json::{json, Value};

#[derive(Default)]
pub struct RailInput<'a> {
    pub board: bool,
    pub schematic: bool,
    pub mech: bool,
    pub gerbers: bool,
    pub simulation: bool,
    pub altium: bool,
    pub page: &'a str,
    pub diff: bool,
}

pub fn rail(input: RailInput<'_>) -> Value {
    let mut entries = Vec::new();
    let page = if input.diff { "diff" } else { input.page };
    let mut add = |id, label, icon, group, key, why: Option<&str>| {
        let state = if why.is_some() { "disabled" } else if page == id { "on" } else { "off" };
        let mut entry = json!({ "id": id, "label": label, "icon": icon, "group": group, "key": key, "state": state });
        if let Some(why) = why { entry["why"] = json!(why); }
        entries.push(entry);
    };
    let no_board = (!input.board).then_some("No board in this project");
    add("board", "Board", "circuit-board", "design", "b", no_board);
    add("schematic", "Schematic", "cpu", "design", "s", (!input.schematic).then_some("No schematic in this project"));
    add("3d", "3D", "box", "design", "3", no_board);
    if input.gerbers { add("gerbers", "Gerbers", "file-stack", "design", "", None); }
    if input.simulation && input.schematic && !input.altium { add("simulation", "Simulation", "gauge", "design", "", None); }
    add("split", "Split", "layout-panel-left", "layout", "v", (!(input.board && input.schematic)).then_some("Split needs a board and a schematic"));
    add("mech", "Mech", "layers", "mech", "m", (!input.mech).then_some("No mechanical parts in this project"));
    add("files", "Files", "folder", "work", "f", None);
    add("diff", "Diff", "diff", "work", "d", None);
    add("review", "Review", "circle-check", "work", "r", None);
    add("browser", "Browser", "globe", "work", "w", None);
    add("terminal", "Terminal", "terminal", "tools", "`", None);
    add("commands", "Commands", "search", "tools", "secondary-k", None);
    Value::Array(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rail_availability_and_selection() {
        for board in [false, true] { for schematic in [false, true] { for mech in [false, true] {
            for page in ["", "board", "schematic", "3d", "split", "mech", "files", "diff", "review", "browser", "terminal", "commands"] {
                for diff in [false, true] {
                    let rows = rail(RailInput { board, schematic, mech, page, diff, ..Default::default() });
                    let ids = ["board", "schematic", "3d", "split", "mech", "files", "diff", "review", "browser", "terminal", "commands"];
                    let enabled = [board, schematic, board, board && schematic, mech, true, true, true, true, true, true];
                    assert_eq!(rows.as_array().unwrap().len(), ids.len());
                    for ((row, id), enabled) in rows.as_array().unwrap().iter().zip(ids).zip(enabled) {
                        assert_eq!(row["id"], id);
                        assert_eq!(row["state"], if !enabled { "disabled" } else if id == if diff { "diff" } else { page } { "on" } else { "off" });
                        assert_eq!(row.get("why").is_some(), !enabled);
                    }
                }
            }
        }}}
    }

    #[test]
    fn rail_optional_design_entries() {
        for gerbers in [false, true] { for simulation in [false, true] { for altium in [false, true] { for schematic in [false, true] {
            for page in ["gerbers", "simulation"] {
                let rows = rail(RailInput { gerbers, simulation, altium, schematic, page, ..Default::default() });
                let optional: Vec<_> = rows.as_array().unwrap().iter().skip(3).take_while(|r| r["group"] == "design").collect();
                let expected: Vec<_> = [("gerbers", gerbers), ("simulation", simulation && schematic && !altium)].into_iter().filter_map(|(id, show)| show.then_some(id)).collect();
                assert_eq!(optional.len(), expected.len());
                for (row, id) in optional.iter().zip(expected) {
                    assert_eq!(row["id"], id);
                    assert_eq!(row["state"], if id == page { "on" } else { "off" });
                }
            }
        }}}}
    }
}
