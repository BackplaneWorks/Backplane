import Base
import ./json.bend as J

# Ordering a project (pure; docs/order.md)
# =========================================
# A project says what it is for in `.backplane/intent.md` (goal, parts,
# notes: the person's words, kept by the agents and edited by the person)
# and what can be made of it in `*.fab.json` files beside the things they
# describe (a board's folder, a part in mech/). The hub reads both for the
# project card every client shows (fab.look): the goal, and, when the
# person allows price estimates, "Buy it · from $X" from the order
# server's estimate of the bare parts (no files leave the machine for it,
# only each part's kind, options and measured features).
#
# Buy it starts a buy thread (setting `buy.<thread>`, its source thread):
# its agent asks what it must, reviews, exports into mfg/<order>/ with a
# manifest, and calls order_quote. The hub (server/orderio.bend) shows the
# person every file that would be sent and asks first; only then does it
# upload, wait for the quote, and post it with a Buy now link. The agent never
# holds the quote's token and never sends anything itself.

# the order server; the setting order.server replaces it
def Order.server() -> String:
  "https://2.29.106.88"

# Instructions
# ------------

def Order.prompt.layout() -> String:
  "Lay out a new project as `boards/<name>/` (one KiCad project per board), `mech/` (mechanical parts: `<part>.py`, `.FCStd`, `.step`), `extra/` (designed here but never ordered: jigs, test fixtures, references) and `mfg/` (generated manufacturing files; never edit by hand). In a project laid out otherwise, offer once to reorganize it; never move files without the person's OK."

def Order.prompt.fab() -> String:
  "Each thing to be ordered has a `<name>.fab.json` beside it (a board's in its folder, a part's beside its STEP in mech/): {\"kind\": \"pcb\" | \"3dp\" | \"cnc\" | \"sheetmetal\", \"name\": \"Main board\", \"source\": \"<its file, from the fab.json's folder>\", \"qty\": 5, \"options\": {...}, \"features\": {...}}; add \"order\": false for one not to be ordered. Features are measured, never guessed, and kept current whenever the design changes, because Backplane shows the person a price from them: a board's `width_mm` and `height_mm` (its Edge.Cuts box) and `min_drill_mm` (its smallest plated hole, usually a via); a solid's `volume_cm3`, `area_cm2`, `size_x_mm`, `size_y_mm`, `size_z_mm` (FreeCAD: shape.Volume/1000, shape.Area/100, the tightest box, largest first); a sheet part's `flat_x_mm`, `flat_y_mm`, `cut_length_mm`, `holes`, `bends` (its flat pattern). Options (any left out take the first value): pcb: layers 1|2|4|6|8|10|12|14|16 (default 2), thickness_mm 1.6|0.4|0.6|0.8|1.0|1.2|2.0, color green|black|white|blue|red|yellow|purple, finish hasl|hasl_lead_free|enig|osp, copper_oz 1|2, castellated, gold_fingers; 3dp: process mjf|sla|sls|fdm|slm, material (mjf: pa12|pa11|pa12_glass; sla: standard_resin|black_resin|tough_resin|high_temp_resin|clear_resin; sls: pa12|pa12_glass; fdm: abs|asa|pla|tpu|pa12_cf|pc_abs|peek; slm: stainless_316l|titanium), color, which the material decides (mjf pa12 and pa11: black|gray, pa12_glass: black|white; sla standard_resin: white, black_resin: black, tough_resin: green, high_temp_resin: yellow, clear_resin: translucent|transparent; sls pa12: black, pa12_glass: white; fdm abs and asa: black|white, pla: black|white|red|blue, tpu: black|white|gray|translucent|green, pa12_cf and pc_abs: black, peek: brown; slm: metal); cnc: material aluminum_6061|aluminum_7075|stainless_304|steel_45|steel_q235|brass|copper|abs_black|abs_white|pom_black|pom_white|pmma|nylon|polycarbonate|fr4, finish none|anodize|bead_blast|bead_blast_anodize|hard_anodize|conductive_anodize|brushed|brushed_anodize|laser_mark, tolerance standard|0.1|0.05|0.02; sheetmetal: material aluminum_5052|stainless_304|stainless_201|steel_cold_rolled|steel_galvanized, thickness_mm (0.5 to 6), finish none|powder_coat|anodize|conductive_anodize|hard_anodize."

def Order.prompt.intent() -> String:
  "`.backplane/intent.md` says what the person is building, in their words: `# Goal` (one or two lines), `# Parts` (a line each: the part, its folder, what it is, whether it is to be ordered) and `# Notes` (only what matters: constraints and choices they made). Keep it short and plain; the person reads and edits it on the project card. At the start of each request that says something new about the goal, the parts or a constraint, have a subagent update it in the background (Claude Code: the Agent tool) while you get on with the request; leave it alone otherwise. Never put progress, plans or todo lists in it."

# how any agent reviews a project for manufacturing (a buy thread starts one
# in its own thread); the hub records it with the design's fingerprint
def Order.prompt.review() -> String:
  "<backplane_manufacturing_review>When asked to review the project for manufacturing, check what makes a fab ask questions or refuse, not the circuit: DRC against the fab's limits (tracks and gaps: 0.10 mm or more on 1 or 2 layers, 0.09 mm on 4 or more, 0.127 mm is comfortable; vias: a 0.3 mm hole in a 0.45 mm pad costs nothing extra, smaller down to a 0.15 mm hole in a 0.25 mm pad is fine but costs more, so say so rather than calling it an error; plated holes for parts: rings of 0.13 mm; copper 0.3 mm from the board edge; silkscreen lines 0.15 mm), every BOM line has a manufacturer part number that is in stock (pcbparts tools; prefer basic/preferred parts, name extended ones), parts on one side for the cheaper assembly, footprint rotations sane, an outline that is closed, every part to order has a current fab file; for printed parts walls 1 mm or more (0.8 for SLA), for sheet metal flanges at least 4x the thickness and holes at least the thickness from a bend, for CNC inside corners with radii of 1 mm or more and no undercuts. Run every check in the foreground (no background commands, monitors or waits) and keep each one short. Write the findings to `.backplane/review.md` (per part: what is fine, what must change and why), then call the backplane tool `order_review` with action \"record\", verdict \"ok\" when nothing must change or \"issues\", a one-line summary and issues: what must change, one short line each (\"Shade: walls 0.6 mm, need 0.8 for SLA\"). Your last message is what the thread that asked hears: make it that list and the link `.backplane/review.md`, then end. Change nothing while reviewing: say what must change; the person decides. Call the manufacturer \"the fab\", never by name.</backplane_manufacturing_review>"

# with ordering off, only where to turn it on (law order_off_says_where)
def Order.prompt.off() -> String:
  "Ordering from Backplane is off. If the person asks to order, buy or price the project, say they can turn it on in Settings → Ordering (a Buy it button then appears once the design is ready), or take the design files to any manufacturer themselves."

# every project thread's block
# the project's files for every agent: the intent and the layout always,
# the fab files and the manufacturing review only with ordering on
def Order.prompt.project.k(on: Bool) -> String:
  match on:
    case True{}:
      "<backplane_project_files>" ++ Order.prompt.intent() ++ "\n" ++ Order.prompt.layout() ++ "\n" ++ Order.prompt.fab()
      ++ " Prefer basic and preferred parts from the fab's own stock when choosing components (the pcbparts tools: jlc_search, jlc_stock_check); every BOM line needs its manufacturer part number (an `MPN` field on the symbol). In what you write, call the manufacturer \"the fab\" and its catalog \"the fab's stock\", never by name (no JLC, LCSC or EasyEDA).</backplane_project_files>\n" ++ Order.prompt.review()
    case False{}:
      "<backplane_project_files>" ++ Order.prompt.intent() ++ "\n" ++ Order.prompt.layout() ++ "</backplane_project_files>\n" ++ Order.prompt.off()

# a buy thread's own block
def Order.prompt.buy() -> String:
  "<backplane_buy>This thread orders the project's parts; the person pressed Buy it. Be brief: short messages, a small table at most, no walls of text. Steps:\n"
  ++ "1. Read `.backplane/intent.md` and every `*.fab.json` (find . -name '*.fab.json' -not -path './extra/*' -not -path './mfg/*'). If a board or part to order has none yet, write it first, measured as the project's instructions say, and say so in a line. In one short message, list what would be ordered (part, what it is, quantity).\n"
  ++ "2. Call the backplane tool `order_review` with action \"gate\". When the review is current and passed it says so: go on. Otherwise Backplane itself tells the person where the review stands (the issues as a list, with the link to `.backplane/review.md`) and asks them; end your turn, and their answer comes as their next message: \"Review for manufacturing\": call delegate_task with the prompt \"Review this project for manufacturing.\" (the task works in this thread's folder, on the very files to be ordered, and its result comes back here), say in one line that it runs as a task of this thread and the order goes on when it's done, and end your turn; when its result comes, call the gate again. \"Fix them first\": delegate_task again with the exact changes `.backplane/review.md` asks for (in this folder, so the order gets them), and once that is done the review once more, then the gate. \"Order without a review\" or \"Order anyway\": go on. Export nothing until the gate passes or the person says to order anyway. Never review or change the design yourself here; this thread only orders.\n"
  ++ "3. Ask only what is missing, one question at a time with the backplane tool `order_ask` (it never waits: end your turn after each, and the answer comes as the person's next message) so the person can tap the answers (two to four short choices, the likely one first): the country it ships to (two-letter code), quantities, and for each board whether it comes assembled or bare. When an earlier order's `mfg/*/manifest.json` exists, offer its answers first (its ship_to, quantities and assembly) and skip what still holds. Boards come in at least 5 and assembly starts at 2 boards (any number up to the quantity). Bare boards can come with a BOM they order parts with themselves (by manufacturer part number at any distributor).\n"
  ++ "4. Export from the saved files (a fix made through `kicad_ipc` counts once saved with SaveDocument) into `mfg/<YYYYMMDD-HHMM>/`: for each board `<name>/gerbers/` (kicad-cli pcb export gerbers, and kicad-cli pcb export drill with Excellon, mm, PTH and NPTH in separate files, into the same folder), and when it is assembled `<name>/bom.csv` (columns Designator, Quantity, Value, Footprint, MPN, Manufacturer, and LCSC when known; one line per part number) and `<name>/positions.csv` (kicad-cli pcb export pos --format csv --units mm --side both; columns Designator, Mid X, Mid Y, Rotation, Layer); for each mechanical part `<name>/<name>.step`, and for sheet metal also its flat pattern `<name>/<name>.dxf`. Then write `manifest.json` there: {\"spec\": 1, \"title\": \"<the project, in words>\", \"ship_to\": \"US\", \"parts\": [{\"name\" (its fab.json name, e.g. Main board), \"kind\", \"qty\", \"options\", \"features\" (sheet metal: its bends), \"files\": {\"gerbers\": \"<name>/gerbers\", \"bom\", \"positions\"} or {\"step\", \"dxf\"}, \"assemble\": <boards to assemble, 0 for bare>}], \"files\": {\"<path from the folder>\": \"<sha256>\"}}, listing every file (sha256sum).\n"
  ++ "5. Call the backplane tool `order_quote` with the folder (`mfg/<...>`) and end your turn: Backplane shows the person every file it would send and asks them first. Once they send, it posts the quote with its Buy now link (it opens Stripe) in this thread and you get a message with its total; do not repeat its table. Say in a line or two what it covers, and if it lists problems, what fixes each.\n"
  ++ "Only order_quote ever sends anything: never upload files or contact a manufacturer yourself. The person never deals with the fab: there is no fab website, account or preview for them, so never ask them to check anything there; check what can be checked here (the exported files, the positions file, the renders) and say what you checked. Prices are estimates until paid; the person can always take the mfg folder to any manufacturer themselves. Never name the manufacturer.</backplane_buy>"

# The review gate (order_review's action "gate", in a buy thread): the
# hub, not the agent, tells the person where the design's review stands and
# asks them (laws order_gate_*). 0: current and passed, go on; 1: none or
# stale, Review for manufacturing or Order without a review; 2: current
# with issues, those as bullets with the review's link, then Fix them
# first or Order anyway. Each answer comes as the person's message.
def Order.gate.k(+state: String, +verdict: String) -> Nat:
  Bool.pick(Nat, Bool.not(String.eq(state, "current")), 1n, Bool.pick(Nat, String.eq(verdict, "ok"), 0n, 2n))

def Order.gate(+said: J.Json) -> Nat:
  Order.gate.k(J.Json.get_str(said, "state", ""), J.Json.get_str(said, "verdict", ""))

def Order.gate.tidy(+x: String) -> String:
  Bool.pick(String, String.ends_with(String.trim(x), "."), String.trim(String.take(String.trim(x), Nat.sub(String.length(String.trim(x)), 1n))), String.trim(x))

def Order.gate.keep(blank: Bool, +x: String, rest: List<&2, String>) -> List<&2, String>:
  match blank:
    case True{}:
      rest
    case False{}:
      x <> rest

def Order.gate.clean(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Order.gate.keep(String.is_empty(Order.gate.tidy(x)), Order.gate.tidy(x), Order.gate.clean(t))

def Order.gate.strs(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{+x} <> t:
      x <> Order.gate.strs(t)
    case _ <> t:
      Order.gate.strs(t)

# the bullets: the recorded issues, else the summary cut at its semicolons
def Order.gate.points.k(none: Bool, +summary: String, xs: List<&2, String>) -> List<&2, String>:
  match none:
    case True{}:
      Order.gate.clean(String.split(summary, ';'))
    case False{}:
      xs

def Order.gate.points(+summary: String, +issues: List<&2, J.Json>) -> List<&2, String>:
  Order.gate.points.k(List.is_empty(&2, J.Json, issues), summary, Order.gate.clean(Order.gate.strs(issues)))

def Order.gate.list(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case +x <> t:
      "- " ++ x ++ "\n" ++ Order.gate.list(t)

def Order.gate.md(+summary: String, issues: List<&2, J.Json>) -> String:
  "**The manufacturing review found what to fix before ordering:**\n\n" ++ Order.gate.list(Order.gate.points(summary, issues)) ++ "\nDetails: `.backplane/review.md`"

def Order.gate.review.q() -> String:
  "This design hasn't been checked for manufacturing since it changed. A review finds what a fab would refuse or charge more for."

def Order.gate.fix.q() -> String:
  "Fix these before ordering, or order as it is?"

# what the agent hears (Backplane did the asking)
def Order.gate.said(i: Nat) -> String:
  match i:
    case 0n:
      "The manufacturing review is current and passed: go on with the order."
    case 1n:
      "The design has no current manufacturing review: Backplane asked the person \"Review for manufacturing\" or \"Order without a review\". End your turn now; their answer comes as their next message."
    case _:
      "The current review found issues: Backplane posted them in this thread and asked the person \"Fix them first\" or \"Order anyway\". Don't repeat them. End your turn now; their answer comes as their next message."

# what the order tools answer at once (neither waits on the person: their
# answer comes as their next message, the quote as a message once posted)
def Order.ask.asked() -> String:
  "Asked on the person's screen. End your turn now: their answer comes as their next message, however long they take."

def Order.quote.asked() -> String:
  "The person is asked whether to send these files. End your turn now: once they send, the quote is posted in this thread and you get a message with its total."

def Order.quote.declined() -> String:
  "The person chose not to send the files (Not now). Ask whether anything should change first."

# Ordering is off until the person turns it on in Settings (the setting
# order.enabled): no Buy it, no prices asked, nothing sent to the order
# server, no fab files or reviews asked of agents (laws order_off_*)
def Order.enabled(+v: String) -> Bool:
  String.eq(v, "on")

# Tools
# -----

def Order.names() -> List<&2, String>:
  ["order_quote", "order_status", "order_ask", "order_review"]

def Order.tool.prop(+name: String, +ty: String, desc: String) -> J.KV:
  J.KV{name, J.Json.obj([J.KV{"type", J.Str{ty}}, J.KV{"description", J.Str{desc}}])}

def Order.tool.def(name: String, desc: String, props: List<&2, J.KV>, req: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"name", J.Str{name}}, J.KV{"description", J.Str{desc}},
    J.KV{"inputSchema", J.Json.obj([J.KV{"type", J.Str{"object"}}, J.KV{"properties", J.Json.obj(props)}, J.KV{"required", J.Json.arr(req)}])}])

def Order.tools() -> List<&2, J.Json>:
  [Order.tool.def("order_quote", "Get a quote for an exported order: the folder (from this thread's folder, e.g. mfg/20261017-0930) holding manifest.json and the files it lists. Backplane shows the person every file it would send and asks them first, and answers at once: end your turn. Once they send, it uploads the files, waits for the quote (a minute or two), posts it in this thread with a Buy now link and sends you a message with its total and any problems (or that they chose not to send).",
     [Order.tool.prop("dir", "string", "The order's folder, from this thread's folder.")], [J.Str{"dir"}]),
   Order.tool.def("order_status", "Where this thread's last quote or order stands: waiting, quoted (with its total), paid, placed, shipped (with tracking) or delivered.", Nil{}, Nil{}),
   Order.tool.def("order_ask", "Ask the person one question with choices they can tap (on every device); they may also type their own answer. Answers at once: end your turn, and their answer comes as their next message, however long they take.",
     [Order.tool.prop("question", "string", "The question, short and plain."),
      J.KV{"options", J.Json.obj([J.KV{"type", J.Str{"array"}}, J.KV{"items", J.Json.obj([J.KV{"type", J.Str{"string"}}])}, J.KV{"description", J.Str{"Two to four short choices, the likely one first."}}])}],
     [J.Str{"question"}, J.Str{"options"}])]

# every thread's agent has this one (a buy thread checks, a review records)
def Order.review.tool() -> J.Json:
  Order.tool.def("order_review", "The project's manufacturing review. gate (a buy thread, before exporting): Backplane checks the review and, unless it is current and passed, shows the person why and asks them; answers what it did. check: whether the design (its boards, schematics, models and fab files) was reviewed since it last changed; answers state (none, stale or current), verdict, summary and issues. record: after a manufacturing review, keep its verdict (ok or issues), a one-line summary and the issues, one short line each, with the design as it is now.",
    [Order.tool.prop("action", "string", "gate, check or record"), Order.tool.prop("verdict", "string", "record: ok or issues"), Order.tool.prop("summary", "string", "record: one line"),
     J.KV{"issues", J.Json.obj([J.KV{"type", J.Str{"array"}}, J.KV{"items", J.Json.obj([J.KV{"type", J.Str{"string"}}])}, J.KV{"description", J.Str{"record: what must change, one short line each (\"Shade: walls 0.6 mm, need 0.8\"), at most 8; none when ok."}}])}],
    [J.Str{"action"}])

# A review is current while the design is as it was when the review was
# recorded: the hub hashes it (orderio.bend's Od.print) and keeps the hash in
# .backplane/review.json (laws order_review_*)
def Order.review.state(+fp: String, +rec: J.Json) -> String:
  Bool.pick(String, String.is_empty(J.Json.get_str(rec, "fingerprint", "")), "none",
    Bool.pick(String, String.eq(J.Json.get_str(rec, "fingerprint", ""), fp), "current", "stale"))

# an issues list: its strings, trimmed, blank ones left out, at most n
def Order.review.line(blank: Bool, +x: String, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match blank:
    case True{}:
      rest
    case False{}:
      J.Str{x} <> rest

def Order.review.strs(xs: List<&2, J.Json>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{+x} <> t:
      Order.review.line(String.is_empty(String.trim(x)), String.trim(x), Order.review.strs(t))
    case _ <> t:
      Order.review.strs(t)

def Order.review.lines(xs: List<&2, J.Json>, n: Nat) -> List<&2, J.Json>:
  List.take(&2, J.Json, Order.review.strs(xs), n)

# what check answers
def Order.review.said(+fp: String, +rec: J.Json) -> J.Json:
  J.Json.obj([J.KV{"state", J.Str{Order.review.state(fp, rec)}}, J.KV{"verdict", J.Str{J.Json.get_str(rec, "verdict", "")}},
    J.KV{"summary", J.Str{J.Json.get_str(rec, "summary", "")}}, J.KV{"issues", J.Json.arr(Order.review.lines(J.Json.elems(J.Json.get_obj(rec, "issues")), 8n))},
    J.KV{"at", J.Json.nat(J.Json.get_nat(rec, "at", 0n))}])

# what record keeps: a verdict is ok or issues, nothing else
def Order.review.verdict(+v: String) -> String:
  Bool.pick(String, String.eq(String.trim(v), "ok"), "ok", "issues")

def Order.review.rec(+fp: String, +verdict: String, +summary: String, +at: Nat) -> J.Json:
  J.Json.obj([J.KV{"fingerprint", J.Str{fp}}, J.KV{"verdict", J.Str{Order.review.verdict(verdict)}}, J.KV{"summary", J.Str{String.trim(summary)}}, J.KV{"at", J.Json.nat(at)}])

# with what must change: the issues' lines, at most 8, blank ones left out
# (law order_review_issues_kept)
def Order.review.rec.with(+fp: String, +verdict: String, +summary: String, issues: List<&2, J.Json>, +at: Nat) -> J.Json:
  J.Json.obj([J.KV{"fingerprint", J.Str{fp}}, J.KV{"verdict", J.Str{Order.review.verdict(verdict)}}, J.KV{"summary", J.Str{String.trim(summary)}},
    J.KV{"issues", J.Json.arr(Order.review.lines(issues, 8n))}, J.KV{"at", J.Json.nat(at)}])

def Order.is.name(+n: String, ns: List<&2, String>) -> Bool:
  match ns:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(String.eq(n, h), Order.is.name(n, t))

# an MCP tools/call for one of the order tools
def Order.is(+o: J.Json) -> Bool:
  Order.is.name(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), Order.names())

# Buy threads
# -----------

def Order.buy.key(+thread: String) -> String:
  "buy." ++ thread

def Order.buy.title(+project: String) -> String:
  "Buy: " ++ project

# what the person says to start it
def Order.buy.text() -> String:
  "Buy it"

# The project card
# ----------------

# a fab file's part, as the estimate wants it (none for one marked
# "order": false or that is not an object)
type FabFile is Data:
  FabFile{path: String, text: String}

def Order.part.of(+path: String, +o: J.Json) -> J.Json:
  J.Json.obj([J.KV{"name", J.Str{J.Json.get_str(o, "name", path)}}, J.KV{"kind", J.Str{J.Json.get_str(o, "kind", "")}},
    J.KV{"qty", Maybe.default(&2, J.Json, J.Json.get(o, "qty"), J.Num{"1"})},
    J.KV{"options", J.Json.get_obj(o, "options")}, J.KV{"features", J.Json.get_obj(o, "features")}, J.KV{"path", J.Str{path}}])

def Order.part.k(ordered: Bool, +path: String, +o: J.Json, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match ordered:
    case True{}:
      Order.part.of(path, o) <> rest
    case False{}:
      rest

def Order.obj(o: J.Json) -> Bool:
  match o:
    case J.Obj{_}:
      True{}
    case _:
      False{}

def Order.part.obj(m: Maybe<&2, J.Json>, +path: String, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match m:
    case Some{+o}:
      Order.part.k(Bool.and(Order.obj(o), J.Json.get_bool(o, "order", True{})), path, o, rest)
    case None{}:
      rest

def Order.parts(fs: List<&2, FabFile>) -> List<&2, J.Json>:
  match fs:
    case Nil{}:
      Nil{}
    case FabFile{+path, text} <> t:
      Order.part.obj(J.Json.parse(text), path, Order.parts(t))

# what the hub sends for "from $X": each part without its path
def Order.est.part(+p: J.Json) -> J.Json:
  J.Json.obj([J.KV{"name", Maybe.default(&2, J.Json, J.Json.get(p, "name"), J.Str{""})}, J.KV{"kind", Maybe.default(&2, J.Json, J.Json.get(p, "kind"), J.Str{""})},
    J.KV{"qty", Maybe.default(&2, J.Json, J.Json.get(p, "qty"), J.Num{"1"})},
    J.KV{"options", J.Json.get_obj(p, "options")}, J.KV{"features", J.Json.get_obj(p, "features")}])

def Order.est.parts(ps: List<&2, J.Json>) -> List<&2, J.Json>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      Order.est.part(p) <> Order.est.parts(t)

def Order.est.body(ps: List<&2, J.Json>) -> String:
  J.Json.show(J.Json.obj([J.KV{"parts", J.Json.arr(Order.est.parts(ps))}]))

# the intent's goal: the first line of text under "# Goal", else the
# first line of text at all
def Intent.text(+l: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(String.trim(l))), Bool.not(String.starts_with(String.trim(l), "#")))

def Intent.first(ls: List<&2, String>) -> String:
  match ls:
    case Nil{}:
      ""
    case +l <> t:
      Bool.pick(String, Intent.text(l), String.trim(l), Intent.first(t))

def Intent.goal.go(ls: List<&2, String>, +after: Bool) -> String:
  match ls:
    case Nil{}:
      ""
    case +l <> t:
      Bool.pick(String, Bool.and(after, Intent.text(l)), String.trim(l),
        Intent.goal.go(t, Bool.or(after, String.starts_with(String.to_lower(String.trim(l)), "# goal"))))

def Intent.goal.or(+g: String, +text: String) -> String:
  Bool.pick(String, String.is_empty(g), Intent.first(String.lines(text)), g)

def Intent.goal(+text: String) -> String:
  Intent.goal.or(Intent.goal.go(String.lines(text), False{}), text)

# The quote in the thread
# -----------------------
# The order server writes the quote's text (so it can change without a
# new app); the hub adds the Buy now link (straight to Stripe), which carries the quote's token.

def Order.quote.md(+md: String, +pay: String, +label: String, ok: Bool) -> String:
  match ok:
    case True{}:
      md ++ "\n\n[" ++ label ++ "](" ++ pay ++ ")"
    case False{}:
      md

# What the person approves before anything is sent: how many files, from
# where, to where they ship, and each file (the first 40)
def Order.ask.files(ps: List<&2, String>, n: Nat) -> String:
  match ps n:
    case Nil{} _:
      ""
    case _ 0n:
      "\n…"
    case +p <> t 1n+m:
      "\n" ++ p ++ Order.ask.files(t, m)

def Order.ask.part(+p: J.Json) -> String:
  J.Json.get_str(p, "name", "?") ++ " ×" ++ J.Json.as_raw(J.Json.get(p, "qty"), "1")
    ++ Bool.pick(String, Bool.or(String.eq(J.Json.as_raw(J.Json.get(p, "assemble"), "0"), "0"), String.is_empty(J.Json.as_raw(J.Json.get(p, "assemble"), ""))), "",
      " (" ++ J.Json.as_raw(J.Json.get(p, "assemble"), "") ++ " assembled)")

def Order.ask.parts(ps: List<&2, J.Json>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Order.ask.part(p) <> Order.ask.parts(t)

def Order.ask.detail(+dir: String, +m: J.Json, +ps: List<&2, String>) -> String:
  "Send " ++ Nat.show(List.length(&2, String, ps)) ++ " files from " ++ dir ++ " to Backplane's quote service? They are kept only to make what you order.\n"
    ++ "Ships to " ++ J.Json.get_str(m, "ship_to", "?") ++ " · " ++ String.join(Order.ask.parts(J.Json.elems(J.Json.get_obj(m, "parts"))), ", ") ++ "\n"
    ++ Order.ask.files(ps, 40n)

# what the agent hears back (never the token or the Buy now link)
def Order.said(+q: J.Json) -> String:
  J.Json.show(J.Json.obj([J.KV{"status", J.Str{J.Json.get_str(q, "status", "")}},
    J.KV{"total", Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(q, "quote"), "total"), J.Null{})},
    J.KV{"lead_days", Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(q, "quote"), "lead_days"), J.Null{})},
    J.KV{"problems", Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(q, "quote"), "problems"), J.Json.arr(Nil{}))},
    J.KV{"warnings", Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(q, "quote"), "warnings"), J.Json.arr(Nil{}))},
    J.KV{"order", J.Json.get_obj(q, "order")},
    J.KV{"note", J.Str{"Backplane posted the quote with its Buy now link in this thread; do not repeat its table."}}]))

# whether a quote can be paid: worked out and nothing to fix
def Order.payable(+q: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(q, "status", ""), "ready"), List.is_empty(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(q, "quote"), "problems"))))

# An order's progress, said once each in the buy thread
def Order.status.text.k(i: Nat, +tracking: String) -> String:
  match i:
    case 0n:
      "**Paid.** Thank you. We're placing the order now; this thread will say when it ships."
    case 1n:
      "**Ordered.** It's in production."
    case 2n:
      "**Shipped.**" ++ Bool.pick(String, String.is_empty(tracking), "", " Tracking: " ++ tracking)
    case 3n:
      "**Delivered.**"
    case 4n:
      "**Refunded.** The money is on its way back to you."
    case 5n:
      "This quote has expired. Ask here for a new one when you're ready."
    case 6n:
      "**Test payment received.** Ordering through Backplane is a pilot: your card was not charged and nothing is ordered. To get the parts made now, order them from any fabricator with the files exported to `mfg/` here."
    case _:
      ""

def Order.status.pos(+s: String, ls: List<&2, String>, +n: Nat) -> Nat:
  match ls:
    case Nil{}:
      n
    case +h <> t:
      Bool.pick(Nat, String.eq(s, h), n, Order.status.pos(s, t, 1n+n))

# the alert an order's progress raises on the person's devices (a "note":
# notice.bend's Alert), plain text, "" for none
def Order.status.note.k(i: Nat, +tracking: String) -> String:
  match i:
    case 0n:
      "Paid: your order is being placed."
    case 1n:
      "Ordered: your order is in production."
    case 2n:
      "Shipped" ++ Bool.pick(String, String.is_empty(tracking), ".", ". Tracking: " ++ tracking)
    case 3n:
      "Delivered."
    case 4n:
      "Refunded: the money is on its way back to you."
    case _:
      ""

def Order.status.note(+status: String, +tracking: String) -> String:
  Order.status.note.k(Order.status.pos(status, ["paid", "placed", "shipped", "delivered", "refunded"], 0n), tracking)

def Order.status.text(+status: String, +tracking: String) -> String:
  Order.status.text.k(Order.status.pos(status, ["paid", "placed", "shipped", "delivered", "refunded", "expired", "pilot"], 0n), tracking)

# the order's state once there is one; a checkout that expired unpaid
# ("cancelled") is no order, so the quote's own state counts (ready, it
# can still be paid, or expired)
def Order.state.k(+order: String, +quote: String) -> String:
  Bool.pick(String, Bool.or(String.is_empty(order), String.eq(order, "cancelled")), quote, order)

# a pilot's order server (checkout on a Stripe test key: order/fabsrv's
# config.PILOT) takes test payments: paid there is "pilot" here, nothing
# was charged or ordered (laws order_pilot_*)
def Order.state.pilot(pilot: Bool, +st: String) -> String:
  Bool.pick(String, Bool.and(pilot, String.eq(st, "paid")), "pilot", st)

def Order.state(+q: J.Json) -> String:
  Order.state.pilot(J.Json.get_bool(q, "pilot", False{}), Order.state.k(J.Json.get_str(J.Json.get_obj(q, "order"), "status", ""), J.Json.get_str(q, "status", "")))

# nothing more will happen to it: delivered, refunded, or a quote that
# expired or failed unpaid
def Order.done(+st: String) -> Bool:
  Bool.or(Bool.or(Bool.or(String.eq(st, "delivered"), String.eq(st, "refunded")), String.eq(st, "pilot")), Bool.or(Bool.or(String.eq(st, "expired"), String.eq(st, "failed")), String.eq(st, "gone")))

# while unpaid it is checked each minute, after that every ten
def Order.every(+st: String) -> U32:
  Bool.pick(U32, Bool.or(String.eq(st, "ready"), Bool.or(String.eq(st, "quoted"), String.eq(st, ""))), 60000, 600000)

# The card every client draws (from the hub's fab.look answer, kept by the
# client as info fab:<thread>)
# boards: KiCad boards in the project when no part has a fab file yet (the
# buy thread writes the fab files first)
type Card is Data:
  Card{goal: String, intent: String, label: String, parts: Nat, boards: Nat, error: String, estimates: Bool}

def Card.of.j(+j: J.Json) -> Card:
  Card{J.Json.get_str(j, "goal", ""), J.Json.get_str(j, "intent", ""), J.Json.get_str(j, "label", ""),
    List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(j, "parts"))), J.Json.get_nat(j, "boards", 0n), J.Json.get_str(j, "error", ""), J.Json.get_bool(j, "estimates", True{})}

def Card.of(+text: String) -> Card:
  Card.of.j(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}))

def Card.intent(c: Card) -> String:
  match c:
    case Card{_, i, _, _, _, _, _}:
      i

# whether there is a card at all: an intent, something to order, or a board
def Card.shown(c: Card) -> Bool:
  match c:
    case Card{_, +i, _, +n, +b, _, _}:
      Bool.or(Bool.not(String.is_empty(String.trim(i))), Nat.is_gt(Nat.add(n, b), 0n))

# the Buy button's words: "Buy it · from $43.20" with an estimate, "Buy it"
# when the person turned estimates off or there are boards but no fab files
# yet, none when there is nothing to order or the order server cannot be
# reached (laws order_buy_*)
def Card.buy.none(boards: Bool, +err: String) -> String:
  match boards:
    case True{}:
      Bool.pick(String, String.is_empty(err), "Buy it", "")
    case False{}:
      ""

def Card.buy.k(none: Bool, +label: String, on: Bool, boards: Bool, +err: String) -> String:
  match none:
    case True{}:
      Card.buy.none(boards, err)
    case False{}:
      Bool.pick(String, String.is_empty(label), Bool.pick(String, on, "", "Buy it"), "Buy it · " ++ label)

def Card.buy(c: Card) -> String:
  match c:
    case Card{_, _, +label, +n, +b, +err, on}:
      Card.buy.k(Nat.is_eq(n, 0n), label, on, Nat.is_gt(b, 0n), err)

# the parts a ready line names, " · " between them ("LED board · Lamp shade")
def Card.names.one(blank: Bool, +name: String, rest: List<&2, String>) -> List<&2, String>:
  match blank:
    case True{}:
      rest
    case False{}:
      String.trim(name) <> rest

def Card.names.go(ps: List<&2, J.Json>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Card.names.one(String.is_empty(String.trim(J.Json.get_str(p, "name", ""))), J.Json.get_str(p, "name", ""), Card.names.go(t))

def Card.boards.words(+n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 1n), "the board", Nat.show(n) ++ " boards")

def Card.ready.k(+names: String, +boards: Nat) -> String:
  Bool.pick(String, String.is_empty(names), Bool.pick(String, Nat.is_eq(boards, 0n), "", "Ready to order · " ++ Card.boards.words(boards)),
    "Ready to order · " ++ names)

def Card.strs(js: List<&2, J.Json>) -> List<&2, String>:
  match js:
    case Nil{}:
      Nil{}
    case +x <> t:
      J.Json.as_str(Some{x}, "") <> Card.strs(t)

# the project's boards, one path a line (from the hub's card)
def Card.boards.j(+j: J.Json) -> String:
  String.join(Card.strs(J.Json.elems(J.Json.get_obj(j, "board_files"))), "\n")

# once something of the project is paid for, the bar says so in place of
# what there is to order, and Buy it reads Buy again (laws order_ordered_*)
def Order.ordered(+st: String) -> Bool:
  Bool.or(Bool.or(String.eq(st, "paid"), String.eq(st, "placed")), Bool.or(String.eq(st, "shipped"), String.eq(st, "delivered")))

def Order.ordered.word(+st: String) -> String:
  Bool.pick(String, String.eq(st, "paid"), "paid, being placed", Bool.pick(String, String.eq(st, "placed"), "in production", st))

def Card.ready.or(+ready: String, +st: String) -> String:
  Bool.pick(String, Order.ordered(st), "Ordered · " ++ Order.ordered.word(st), ready)

# the line beside Buy it above the message box, from the hub's card
# (laws order_ready_*): what there is to order, by name
def Card.ready.j(+j: J.Json) -> String:
  Card.ready.k(String.join(Card.names.go(J.Json.elems(J.Json.get_obj(j, "parts"))), " · "), J.Json.get_nat(j, "boards", 0n))

# the line the card shows shut: the goal, else a nudge
def Card.line(c: Card) -> String:
  match c:
    case Card{+g, _, _, _, _, _, _}:
      Bool.pick(String, String.is_empty(g), "No goal yet. Edit to say what this project is for.", g)

# what Edit puts in the composer: /intent and the text to change
def Intent.template() -> String:
  "# Goal\n\n# Parts\n\n# Notes\n"

def Intent.edit(+intent: String) -> String:
  "/intent\n" ++ Bool.pick(String, String.is_empty(String.trim(intent)), Intent.template(), intent)

# "/intent ..." sent: the rest is the new intent (law intent_edit_round_trip)
def Intent.is(+draft: String) -> Bool:
  Bool.or(String.eq(String.trim(draft), "/intent"), Bool.or(String.starts_with(draft, "/intent "), String.starts_with(draft, "/intent\n")))

def Intent.body(+draft: String) -> String:
  String.trim(String.drop(String.trim(draft), 7n))
