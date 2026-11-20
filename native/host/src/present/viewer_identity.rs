//! Build the header from the displayed page and verified source facts.
use super::{files_trail, s};
use serde_json::{json, Value};

/// Normalize separators on behalf of Windows peers too. Accept an absolute path
/// only when it names a descendant of this particular project.
pub(super) fn relative(root: &str, path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    let root = root.replace('\\', "/");
    let rel = path.strip_prefix(&format!("{}/", root.trim_end_matches('/')))
        .filter(|_| !root.is_empty()).unwrap_or(&path);
    if rel.is_empty() || rel.split('/').any(|p| p.is_empty() || matches!(p,"."|"..") || p.contains(':') || p.chars().any(char::is_control)) { return None }
    Some(rel.to_owned())
}

pub(super) struct Sources<'a> {
    pub root: &'a str, pub board: &'a str, pub schematic: &'a str,
    pub file: &'a str, pub sheet: &'a str, pub directory: &'a str,
    pub root_sheet: &'a str, pub has_board: Option<bool>, pub has_sheet: Option<bool>, pub has_set: Option<bool>,
    pub set: &'a str, pub made_at: Option<u64>,
}

pub(super) fn identity(page: &str, viewer: &Value, sources: &Sources, exists: &impl Fn(&str) -> Option<bool>) -> Value {
    let label = match page {
        "board"=>"Board", "schematic"=>"Schematic", "3d"=>"3D", "gerbers"=>"Gerbers",
        "split"=>"Split", "mech"=>"Mech", "files"=>"Files", "review"=>"Review", "diff"=>"Diff",
        "image"=>"Image", "pdf"=>"PDF", "text"=>"Text", "browser"=>"Browser", "simulation"=>"Simulation", _=>return Value::Null,
    };
    let mut out = json!({"page":page,"label":label});
    let path = match page {
        "board"|"3d"=>sources.board, "schematic"|"simulation"=>sources.schematic,
        "gerbers"=>sources.set, "mech"=>s(&viewer["mech"], "rel"), "files"=>sources.directory,
        "image"|"pdf"|"text"=>sources.file, _=>"",
    };
    if page == "files" {
        let dir = relative(sources.root, path);
        out["trail"] = json!(files_trail(dir.as_deref().unwrap_or("")));
    }
    if !matches!(page,"review"|"diff"|"browser"|"split") && !(page=="files" && path.is_empty()) {
        match relative(sources.root,path) {
            Some(rel) => match exists(&rel) {
                Some(true) => {
                    out["source"] = json!({"rel":rel, "name":rel.rsplit('/').next()});
                    match page {
                        "schematic" if relative(sources.root, sources.sheet).as_deref() == Some(rel.as_str())
                            && relative(sources.root, sources.root_sheet).is_some_and(|root| root != rel) => out["sheet"] = json!(rel),
                        "mech" => out["part"] = json!(rel),
                        "gerbers" => out["set"] = json!({"rel":rel,"madeAt":sources.made_at}),
                        _ => {}
                    }
                }
                Some(false) => out["missing"] = json!(true),
                None => {}
            },
            None if path.is_empty() => {
                let available = match page {
                    "board" | "3d" => sources.has_board,
                    "schematic" | "simulation" => sources.has_sheet,
                    "gerbers" => sources.has_set,
                    _ => Some(false),
                };
                if available == Some(false) { out["missing"] = json!(true); }
            }
            None => out["missing"] = json!(true),
        }
    }
    match page {
        "split" => out["panes"] = json!(["schematic","board","3d"].map(|p| identity(p,viewer,sources,exists))),
        "diff" => out["scope"] = json!("Thread changes"),
        "review" => {
            let tab = viewer["review"]["tabs"].as_array().and_then(|tabs|
                tabs.iter().find(|tab| tab["on"] == true)).and_then(|tab| tab["page"].as_str());
            match tab {
                Some("bom") => {
                    let versions = viewer["designPanels"]["bom"]["versions"].as_array();
                    let scope = versions.filter(|v| !v.is_empty()).map(|versions| versions.iter()
                        .map(|v| v.as_str().filter(|s| !s.is_empty()).unwrap_or("Working copy"))
                        .collect::<Vec<_>>().join(" → ")).unwrap_or_else(|| "Working copy".into());
                    out["scope"] = json!(scope);
                }
                Some("checks") => out["scope"] = json!("Working copy"),
                _ => {}
            }
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sources() -> Sources<'static> {
        Sources {root:"/design",board:"/design/alternate/main.kicad_pcb",schematic:"/design/child/main.kicad_sch",
            root_sheet:"main.kicad_sch",has_board:None,has_sheet:None,has_set:None,
            sheet:"child/main.kicad_sch",file:"docs/top.pdf",directory:"docs",set:"mfg/one",made_at:Some(123)}
    }

    #[test]
    fn page_sources_are_relative_and_match_each_pane() {
        let facts = sources();
        let viewer=json!({"mech":{"rel":"mech/part.step"},"review":{"tabs":[{"page":"bom","on":true}]},"designPanels":{"bom":{"versions":["a","b"]}}});
        let id=|page|identity(page,&viewer,&facts,&|_|Some(true));
        for (page,path) in [("board","alternate/main.kicad_pcb"),("3d","alternate/main.kicad_pcb"),("schematic","child/main.kicad_sch"),
            ("simulation","child/main.kicad_sch"),("files","docs"),("text","docs/top.pdf"),("image","docs/top.pdf"),("pdf","docs/top.pdf"),("mech","mech/part.step")] {
            assert_eq!(id(page)["source"]["rel"],path);
        }
        assert_eq!(id("files")["trail"][1]["value"],"docs");
        assert_eq!(id("split")["panes"][0],id("schematic"));
        assert_eq!(id("split")["panes"][1],id("board"));
        assert_eq!(id("split")["panes"][2],id("3d"));
        assert_eq!(id("review")["scope"],"a → b");
        assert_eq!(id("diff")["scope"],"Thread changes");
        assert_eq!(id("browser"),json!({"page":"browser","label":"Browser"}));
        assert_eq!(id("gerbers")["set"],json!({"rel":"mfg/one","madeAt":123}));
        assert_eq!(id("schematic")["sheet"], "child/main.kicad_sch");
        let root = Sources { root_sheet:"/design/child/main.kicad_sch", ..facts };
        assert!(identity("schematic", &viewer, &root, &|_| Some(true)).get("sheet").is_none());
        let missing=identity("board",&viewer,&facts,&|_|Some(false));
        assert!(missing["source"].is_null() && missing["missing"]==true);
        for bad in ["/outside/design", "/design-alt/file", "../local", "C:\\folder\\file"] {assert!(relative(facts.root,bad).is_none());}
        assert_ne!(relative("/p","a/x.sch"),relative("/p","b/x.sch"));
    }

    #[test]
    fn review_scope_follows_the_active_tab() {
        let facts = sources();
        for (tab, scope) in [("bom", Some("a → b")), ("checks", Some("Working copy")),
                             ("comments", None), ("release", None)] {
            let mut review = json!({"designPanels":{"bom":{"versions":["a","b"]}}});
            review["review"]["tabs"] = json!(["bom","checks","comments","release"]
                .map(|p| json!({"page":p, "on":p == tab})));
            assert_eq!(identity("review", &review, &facts, &|_| None)["scope"].as_str(), scope);
        }
    }

    #[test]
    fn unnamed_sources_require_confirmed_absence() {
        let facts = sources();
        for available in [None, Some(false), Some(true)] {
            let empty = Sources { board:"", schematic:"", set:"", has_board:available,
                has_sheet:available, has_set:available, ..facts };
            let id = |page| identity(page, &Value::Null, &empty, &|_| panic!("empty path lookup"));
            for page in ["board","3d","schematic","simulation","gerbers"] {
                assert_eq!(id(page)["missing"].as_bool(), (available == Some(false)).then_some(true));
                assert!(id(page).get("source").is_none());
            }
            assert!(id("gerbers").get("set").is_none());
            assert_eq!(id("split"), json!({"page":"split","label":"Split",
                "panes":[id("schematic"),id("board"),id("3d")]}));
        }
    }
}
