//! Whether a request is a review, which runs with the design files read-only (agents/snapguard.rs). The rule (#1305):
//! a review is a SHORT request (one paragraph, at most `MAX` characters) that LEADS with a review ask ("review the board", "audit the
//! schematic", "check over the layout", "what's left?") and asks for no change or new work (`asks_change`). Anything else is not one: a
//! design prompt that also asks for "a summary with renders" or a stock check is work, and work may write.
//! The rule fails open on purpose: a request it cannot place is not a review, so the worst miss is a turn that may write (the snapshot
//! before every turn still holds the state before it), never a design turn that cannot.

/// The longest request that can be a review: a plain review ask is a sentence or two; a long prompt is a brief for work.
pub const MAX: usize = 280;

/// Lowercase words, apostrophes kept (typographic ones made plain), everything else a break.
fn words(text: &str) -> Vec<String> {
    text.to_lowercase().replace(['\u{2019}', '\u{2018}'], "'").split(|c: char| !c.is_alphanumeric() && c != '\'').map(|w| w.trim_matches('\'')).filter(|w| !w.is_empty()).map(str::to_string).collect()
}

/// Whether the request is plainly a review (see the module's rule).
pub fn is_review(text: &str) -> bool {
    let t = text.trim();
    t.chars().count() <= MAX && !t.contains("\n\n") && leads_with_review(&words(t)) && !asks_change(t)
}

/// Words a request may open with before its ask ("please", "can you", "ok so now", "give me a quick").
const OPENERS: &[&str] = &["status", "please", "pls", "plz", "hey", "hi", "ok", "okay", "so", "now", "then", "first", "also", "and", "can", "could", "would", "will", "you", "i", "i'd", "we", "like", "want", "need", "to",
    "let's", "lets", "let", "us", "me", "just", "do", "give", "run", "get", "a", "an", "the", "some", "quick", "quickly", "full", "thorough", "final", "proper", "short", "brief", "design", "dfm", "manufacturing", "fab", "careful", "second", "fresh"];
/// The asks a review opens with (one word), and the two-word ones.
/// A status question ("status?", "what's the status of the boards?") is none: asked in the middle of design work, it would leave the thread
/// read-only for the "carry on" after it.
const LEADS: &[&str] = &["review", "reviews", "audit", "assess", "assessment", "critique", "summarize", "summarise", "summary", "evaluate", "evaluation", "inspect", "feedback"];
const LEADS2: &[(&str, &str)] = &[("check", "over"), ("look", "over"), ("check", "through"), ("look", "through"), ("sanity", "check"), ("what's", "left"), ("what", "left"), ("what", "do")];

/// The first words, past the openers, are a review ask.
pub fn leads_with_review(w: &[String]) -> bool {
    let i = w.iter().position(|x| !OPENERS.contains(&x.as_str())).unwrap_or(w.len());
    let Some(a) = w.get(i) else { return false };
    if LEADS.contains(&a.as_str()) { return true }
    let b = w.get(i + 1).map_or("", String::as_str);
    // "what is left", "what do you think", "where are we at"; "what's the status"
    let b = if a == "what" && b == "is" { w.get(i + 2).map_or("", String::as_str) } else { b };
    LEADS2.iter().any(|(x, y)| a == x && b == *y) && !(a == "what" && b == "do" && w.get(i + 2..i + 4).map_or(true, |s| s.join(" ") != "you think"))
}

/// Verbs that ask for a change or for new work: any of them in a request means it is no review, and a follow-up of a review with one ends it.
const CHANGE: &[&str] = &["fix", "fixes", "fixing", "change", "changing", "apply", "applying", "update", "updating", "add", "adding", "remove", "removing", "delete", "deleting",
    "route", "reroute", "rerouting", "place", "placing", "move", "moving", "implement", "implementing", "modify", "modifying", "redo", "rework", "replace", "replacing",
    "rewrite", "refactor", "edit", "editing", "tidy", "continue", "finish", "resume", "yes", "yep", "build", "building", "create", "creating", "generate", "generating", "write", "writing", "draw", "make", "making",
    "put", "commit", "save", "swap", "rename", "rotate", "flip", "connect", "prepare", "proceed", "redesign", "relayout", "regenerate", "convert", "import", "export"];
/// Two-word asks for a change or for files.
const CHANGE2: &[(&str, &str)] = &[("clean", "up"), ("start", "over"), ("lay", "out"), ("wire", "up"), ("go", "ahead"), ("do", "it"), ("go", "for"), ("get", "ready"), ("set", "up"),
    ("keep", "going"), ("carry", "on"), ("fab", "file"), ("fab", "files"), ("fabrication", "files"), ("production", "files"), ("manufacturing", "files"), ("to", "order")];
/// What a "make", "write" or "create" may produce in a review: a list or a report is the review's answer, not a change.
const REPORT: &[&str] = &["list", "summary", "report", "overview", "note", "notes", "review", "writeup", "rundown", "recap", "table", "plan", "assessment", "sure"];
/// Words after which "design" is a verb ("please design", "and then design", "can you design").
const BEFORE_VERB: &[&str] = &["to", "and", "then", "please", "you", "let's", "lets", "also", "now", "can", "could", "would", "will", "i'd", "pls", "just"];

/// Whether a request asks for a change or for new work (files written, a design made). A review's follow-up with one is no review.
pub fn asks_change(text: &str) -> bool {
    let w = words(text);
    w.iter().enumerate().any(|(i, x)| {
        let x = x.as_str(); let next = w.get(i + 1).map_or("", String::as_str);
        if CHANGE2.iter().any(|(a, b)| x == *a && next == *b) { return true }
        if x == "design" || x == "designs" { return i == 0 || BEFORE_VERB.contains(&w[i - 1].as_str()) }
        if !CHANGE.contains(&x) { return false }
        // "make a task list", "write me a summary", "make sure": the answer itself, not a change
        !(matches!(x, "make" | "making" | "write" | "writing" | "create" | "creating" | "generate" | "put" | "prepare") && w[i + 1..w.len().min(i + 5)].iter().any(|y| REPORT.contains(&y.as_str())))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real prompts (from the person's threads; names and machines removed) and what each is. `true`: a review (read-only).
    const TABLE: &[(bool, &str)] = &[
        // plain reviews
        (true, "review my project"), (true, "Review the board"), (true, "Can you audit the schematic?"), (true, "check over the layout"), (true, "what's left?"),
        (true, "status? what's left?"), (true, "what's left to do on this board?"), (true, "what is left on the schematic?"), (true, "Give me feedback on the layout"),
        (true, "Review this board for manufacturing"), (true, "Please review the design and create a task list of things to work on"),
        (true, "Do a review of where we are at and summarize the progress towards our objective"), (true, "review my project and make a task list"),
        (true, "Summarize our performance improvements ?"), (true, "review what Grok did and see if it looks right."), (true, "review qucikly for me."),
        (true, "Do a design review of the power section"), (true, "sanity check the USB pair before I order"), (true, "Look over the schematic and tell me what you think"),
(true, "Assess the thermal design of the regulator"), (true, "Please do a quick DFM review of the board"),
        (true, "Review the changes you made in the last turn"), (true, "Ok, now review the footprint for J1 and make sure the pin 1 marker is right"),
        (true, "Critique the placement around U3"), (true, "inspect the ground pour on the bottom layer"), (true, "What do you think of the layout?"),
        (true, "Review the routing and the placement"), (true, "Summarize the F-007 and F-008 issues"),
        // not reviews: asks for a change, even when it starts with a review ask
        (false, "Review the board and fix what you find"), (false, "review the DRC errors then reroute the USB pair"), (false, "delete the old footprints"), (false, "route the board"),
        (false, "Make the changes from the review"), (false, "add a decoupling cap to U3"), (false, "go ahead"), (false, "start over with a new layout"),
        (false, "review this board and get the fabrication files together for qty of 20"),
        (false, "rip up and throw away the existing routing/placement, review the entire schematic, you need to do this yourself manually. ensure the traces are all 0-45 degrees. get ready to order."),
        (false, "Review the schematic, then design a new LDO stage for the 3.3 V rail"), (false, "audit the BOM and replace anything out of stock"),
        (false, "review the board and export a STEP"), (false, "Check over the layout and move C4 closer to U1"), (false, "review and write the fab.json for the board"),
        // not reviews: questions and requests that never lead with a review ask
        (false, "Testing 123"), (false, "You in here with the GPUI branch?"), (false, "Please add a haiku generated summary of what the build adds when we get the green bar for build ready."),
        (false, "Design a USB-C power board with a summary of the parts"), (false, "Make a footprint for the TPS54331 and report a summary of what you made"),
        (false, "Create the KiCad symbol and footprint for this connector, then give me a summary"), (false, "Build a STEP assembly of the enclosure and report a summary with renders"),
        (false, "status?"), (false, "What's the status of the boards?"), (false, "Where are we at with the routing?"), (false, "what's the status? the bar keeps rolling me up here"),
        (false, "Write the fab files for the board and check the parts are in stock"), (false, "Is the performance overlay on main, and at which sha? If it isn't, what's left before it lands?"),
    ];

    #[test] fn the_table_of_real_prompts() {
        let wrong: Vec<String> = TABLE.iter().filter(|(r, t)| is_review(t) != *r).map(|(r, t)| format!("{t:?} should {}be a review", if *r { "" } else { "not " })).collect();
        assert!(wrong.is_empty(), "{} of {}:\n{}", wrong.len(), TABLE.len(), wrong.join("\n"));
    }

    /// #1305's prompt: 3.5 kB asking for footprints, symbols, boards, a sheet-metal part, a STEP assembly and fab files, ending "Report a summary
    /// with renders": work, never a review. And other long design briefs alike, whatever review words they carry.
    #[test] fn long_design_prompts_are_never_reviews() {
        let pogo = include_str!("../../../../test/fixtures/review/i1305-pogo.txt");
        assert!(pogo.len() > 3000 && pogo.contains("Report a summary"));
        assert!(!is_review(pogo), "the pogo prompt");
        assert!(asks_change(pogo), "and a review's follow-up of it ends the review");
        for p in include_str!("../../../../test/fixtures/review/design-prompts.txt").split("\n---\n").filter(|p| !p.trim().is_empty()) {
            assert!(!is_review(p), "{p}");
            assert!(asks_change(p), "a design prompt asks for work: {p}");
        }
        // what was cut, read on its own: still no review (it no longer leads with a review ask)
        assert!(!is_review(&pogo[pogo.len() - 300..]));
        assert!(!is_review("Report a summary with renders, the footprint area comparison, the BOM and what's ready to order."));
    }
    #[test] fn a_review_ask_must_lead_and_be_short() {
        assert!(is_review("Review the board"));
        assert!(!is_review(&format!("Review the board. {}", "Look at every net and every part carefully. ".repeat(8))), "past the length: a brief, not a review ask");
        assert!(!is_review("Review the board.\n\nThen we talk about the enclosure."), "two paragraphs");
        assert!(!is_review("The board has a problem; review it"), "a review ask that does not lead");
        assert!(!is_review(""), "nothing");
    }
}
