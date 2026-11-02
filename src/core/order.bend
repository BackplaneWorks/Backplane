import Base
import ./json.bend as J
import ./model.bend as M
import ./refs.bend as RF

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
  "Each thing to be ordered has a `<name>.fab.json` beside it (a board's in its folder, a part's beside its STEP in mech/): {\"kind\": \"pcb\" | \"flex\" | \"hdi\" | \"aluminum\" | \"copper_core\" | \"rogers\" | \"ptfe\" | \"3dp\" | \"cnc\" | \"sheetmetal\", \"name\": \"Main board\", \"source\": \"<its file, from the fab.json's folder>\", \"qty\": 5, \"options\": {...}, \"features\": {...}}; add \"order\": false for one not to be ordered. Features are measured, never guessed, and kept current whenever the design changes, because Backplane shows the person a price from them: a board's `width_mm` and `height_mm` (its Edge.Cuts box) and `min_drill_mm` (its smallest plated hole, usually a via); a solid's `volume_cm3`, `area_cm2`, `size_x_mm`, `size_y_mm`, `size_z_mm` (FreeCAD: shape.Volume/1000, shape.Area/100, the tightest box, largest first); a sheet part's `flat_x_mm`, `flat_y_mm`, `cut_length_mm`, `holes`, `bends` (its flat pattern). Boards come in kinds: pcb is a rigid FR-4 board; flex a flexible polyimide one; hdi one with microvias and build-up layers; aluminum and copper_core single-layer metal-core boards (LEDs, power); rogers (RO4350B) and ptfe two-layer high-frequency boards. Every board kind is measured and exported the same way; only pcb boards come assembled. Options (any left out take the first value): pcb: layers 1|2|4|6|8|10|12|14|16 (default 2), thickness_mm 1.6|0.4|0.6|0.8|1.0|1.2|2.0, color green|black|white|blue|red|yellow|purple, finish hasl|hasl_lead_free|enig|osp, copper_oz 1|2, castellated, gold_fingers; flex: layers 2|1|4, thickness_mm (the total, which sets the substrate and copper; 1 layer: 0.07|0.11|0.12|0.14, 2 layers: 0.11|0.12|0.19|0.2|0.24, 4 layers: 0.2|0.25|0.3|0.35|0.45; left out, the first for its layers), coverlay yellow|black|white, stiffener none|polyimide_0.1|polyimide_0.15|polyimide_0.2|polyimide_0.225|polyimide_0.25|fr4_0.1|fr4_0.2|fr4_0.4|fr4_0.6|fr4_0.8|fr4_1.0|fr4_1.2|fr4_1.6|steel_0.1|steel_0.2|steel_0.3|tape_3m468|tape_tesa8854|tape_3m9077 (mm), emi_shield none|one_side|both_sides, gold_uin 1|2 (its ENIG gold), gold_fingers; hdi: layers 4|6|8|10|12|14|16, thickness_mm 1.6 (0.6 to 3.0 as the layers allow), steps 1|2|3 (build-up steps: 1 at 4 layers, up to 2 at 6, up to 3 from 8), laminate np175f|s1000_2m (only s1000_2m from 8 layers), color as pcb, finish hasl|hasl_lead_free|enig|osp (only enig|osp from 6 layers), via_fill plugged|untented|epoxy_filled|copper_filled (only epoxy_filled from 6 layers), castellated, gold_fingers, edge_plating; aluminum: thickness_mm 1.6|0.8|1.0|1.2, color white|black, finish hasl|hasl_lead_free; copper_core: thickness_mm 1.6|1.0|1.2, color white|black (OSP); rogers: thickness_mm 0.51|0.76|1.52, color green|purple, finish enig|osp, gold_uin 1|2, via_fill tented|untented; ptfe: thickness_mm 0.76|1.52, laminate dk2.94|dk3.0|dk2.65|dk2.55 (its dielectric constant), finish enig|osp, gold_uin 1|2, via_fill tented|untented; 3dp: process mjf|sla|sls|fdm|slm, material (mjf: pa12|pa11|pa12_glass; sla: standard_resin|black_resin|tough_resin|high_temp_resin|clear_resin; sls: pa12|pa12_glass; fdm: abs|asa|pla|tpu|pa12_cf|pc_abs|peek; slm: stainless_316l|titanium), color, which the material decides (mjf pa12 and pa11: black|gray, pa12_glass: black|white; sla standard_resin: white, black_resin: black, tough_resin: green, high_temp_resin: yellow, clear_resin: translucent|transparent; sls pa12: black, pa12_glass: white; fdm abs and asa: black|white, pla: black|white|red|blue, tpu: black|white|gray|translucent|green, pa12_cf and pc_abs: black, peek: brown; slm: metal); cnc: material aluminum_6061|aluminum_7075|stainless_304|steel_45|steel_q235|brass|copper|abs_black|abs_white|pom_black|pom_white|pmma|nylon|polycarbonate|fr4, finish none|anodize|bead_blast|bead_blast_anodize|hard_anodize|conductive_anodize|brushed|brushed_anodize|laser_mark, tolerance standard|0.1|0.05|0.02; sheetmetal: material aluminum_5052|stainless_304|stainless_201|steel_cold_rolled|steel_galvanized, thickness_mm (0.5 to 6), finish none|powder_coat|anodize|conductive_anodize|hard_anodize."

def Order.prompt.intent() -> String:
  "`.backplane/intent.md` says what the person is building, in their words: `# Goal` (one or two lines), `# Parts` (a line each: the part, its folder, what it is, whether it is to be ordered) and `# Notes` (only what matters: constraints and choices they made). Keep it short and plain; the person reads and edits it on the project card. At the start of each request that says something new about the goal, the parts or a constraint, update it inline; leave it alone otherwise. Delegate substantial work through Backplane subthreads, never provider-native subagents. Never put progress, plans or todo lists in it."

# how any agent reviews a project for manufacturing (a buy thread starts one
# in its own thread); the hub records it with the design's fingerprint
def Order.prompt.review() -> String:
  "<backplane_manufacturing_review>When asked to review the project for manufacturing, check what makes a fab ask questions or refuse, not the circuit: DRC against the fab's limits (tracks and gaps: 0.10 mm or more on 1 or 2 layers, 0.09 mm on 4 or more, 0.127 mm is comfortable; vias: a 0.3 mm hole in a 0.45 mm pad costs nothing extra, smaller down to a 0.15 mm hole in a 0.25 mm pad is fine but costs more, so say so rather than calling it an error; plated holes for parts: rings of 0.13 mm; copper 0.3 mm from the board edge; silkscreen lines 0.15 mm), every BOM line has a manufacturer part number that is in stock (pcbparts tools; prefer basic/preferred parts, name extended ones), parts on one side for the cheaper assembly, footprint rotations sane, an outline that is closed, every part to order has a current fab file; for printed parts walls 1 mm or more (0.8 for SLA), for sheet metal flanges at least 4x the thickness and holes at least the thickness from a bend, for CNC inside corners with radii of 1 mm or more and no undercuts. Run every check in the foreground (no background commands, monitors or waits) and keep each one short. Write the findings to `.backplane/review.md` (per part: what is fine, what must change and why), then call the backplane tool `order_review` with action \"record\", verdict \"ok\" when nothing must change or \"issues\", a one-line summary and issues: what must change, one short line each (\"Shade: walls 0.6 mm, need 0.8 for SLA\"). Your last message is what the thread that asked hears: make it that list and the link `.backplane/review.md`, then end. Change nothing while reviewing: say what must change; the person decides. Call the manufacturer \"the fab\", never by name.</backplane_manufacturing_review>"

# with ordering on, how ordering happens: only through Buy it and its buy
# thread; an agent once opened the fab's own website in the Browser tab and
# filled in an order there, and another launched a "Buy:" thread itself.
# Only on its own: one the person asked to work the fab's site in their own
# browser refused, so a request in their message lifts it (not payment)
def Order.prompt.on() -> String:
  "Ordering happens only through Backplane: once a part's fab file is ready, the person presses Buy it (in the Ready to order bar above the composer, or Buy it in the thread's header when that bar is put away). It opens a buy thread that checks the design, gets the exact quote from Backplane's order server and takes payment. When the person asks to order, buy or price the project, tell them that. Never order yourself unless the person specifically asks for it in their message: on your own, never open, upload to or fill in a manufacturer's website (the Browser tab included) and never start a thread to order with (thread_launch). When they do ask (say, to work the fab's site in their own browser), do it. Never ask for their payment details: they enter those themselves."

# with ordering off, only where to turn it back on (law order_off_says_where)
def Order.prompt.off() -> String:
  "Ordering from Backplane is off. If the person asks to order, buy or price the project, say they can turn it on in Settings → Ordering (a Buy it button then appears once the design is ready), or take the design files to any manufacturer themselves."

# every project thread's block
# the project's files for every agent: the intent and the layout always,
# the fab files and the manufacturing review only with ordering on
def Order.prompt.project.k(on: Bool) -> String:
  match on:
    case True{}:
      "<backplane_project_files>" ++ Order.prompt.intent() ++ "\n" ++ Order.prompt.layout() ++ "\n" ++ Order.prompt.fab()
      ++ " Prefer basic and preferred parts from the fab's own stock when choosing components (the pcbparts tools: jlc_search, jlc_stock_check); every BOM line needs its manufacturer part number (an `MPN` field on the symbol). In what you write, call the manufacturer \"the fab\" and its catalog \"the fab's stock\", never by name (no JLC, LCSC or EasyEDA). " ++ Order.prompt.on() ++ "</backplane_project_files>\n" ++ Order.prompt.review()
    case False{}:
      "<backplane_project_files>" ++ Order.prompt.intent() ++ "\n" ++ Order.prompt.layout() ++ "</backplane_project_files>\n" ++ Order.prompt.off()

# a buy thread's own block
def Order.prompt.buy() -> String:
  "<backplane_buy>This thread orders the project's parts; the person pressed Buy it. Be brief: short messages, a small table at most, no walls of text. Steps:\n"
  ++ "1. Read `.backplane/intent.md` and every `*.fab.json` (find . -name '*.fab.json' -not -path './extra/*' -not -path './mfg/*'). If a board or part to order has none yet, write it first, measured as the project's instructions say, and say so in a line. In one short message, list what would be ordered (part, what it is, quantity).\n"
  ++ "2. Call the backplane tool `order_review` with action \"gate\". When the review is current and passed it says so: go on. Otherwise Backplane itself tells the person where the review stands (the issues as a list, with the link to `.backplane/review.md`) and asks them; end your turn, and their answer comes as their next message: \"Review for manufacturing\": call delegate_task with the prompt \"Review this project for manufacturing.\" (the task works in this thread's folder, on the very files to be ordered, and its result comes back here), say in one line that it runs as a task of this thread and the order goes on when it's done, and end your turn; when its result comes, call the gate again. \"Fix them first\": delegate_task again with the exact changes `.backplane/review.md` asks for (in this folder, so the order gets them), and once that is done the review once more, then the gate. \"Order without a review\" or \"Order anyway\": go on. Export nothing until the gate passes or the person says to order anyway. Never review or change the design yourself here; this thread only orders.\n"
  ++ "3. Ask only what is missing, one question at a time with the backplane tool `order_ask` (it never waits: end your turn after each, and the answer comes as the person's next message), never in plain words in your reply: Backplane shows each question in its own card above the message box. Questions with choices get two to four short ones, the likely one first: quantities, and for each rigid board (kind pcb) whether it comes assembled or bare; other board kinds come bare. Where it ships is typed: `order_ask` with the question \"Ship to which country?\", `typed` true and no options, so the person gets a box to type it in; when an earlier order's `mfg/*/manifest.json` exists, add `last` with its country's name (it shows as a quick choice). Use the country's two-letter ISO code as ship_to. A region (EU, Europe, Asia) is not a country: ask which country. When an earlier order exists, offer its answers first (its quantities and assembly as choices, its country as `last`) and skip what still holds. Boards come in at least 5 and assembly starts at 2 boards (any number up to the quantity). Bare boards can come with a BOM they order parts with themselves (by manufacturer part number at any distributor).\n"
  ++ "4. Export from the saved files (a fix made through `kicad_ipc` counts once saved with SaveDocument) into `mfg/<YYYYMMDD-HHMM>/`: for each board `<name>/gerbers/` (kicad-cli pcb export gerbers, and kicad-cli pcb export drill with Excellon, mm, PTH and NPTH in separate files, into the same folder), and when it is assembled `<name>/bom.csv` (columns Designator, Quantity, Value, Footprint, MPN, Manufacturer, and LCSC when known; one line per part number) and `<name>/positions.csv` (kicad-cli pcb export pos --format csv --units mm --side both; columns Designator, Mid X, Mid Y, Rotation, Layer); for each mechanical part `<name>/<name>.step`, and for sheet metal also its flat pattern `<name>/<name>.dxf`. Then write `manifest.json` there: {\"spec\": 1, \"title\": \"<the project, in words>\", \"ship_to\": \"US\", \"parts\": [{\"name\" (its fab.json name, e.g. Main board), \"kind\", \"qty\", \"options\", \"features\" (sheet metal: its bends), \"files\": {\"gerbers\": \"<name>/gerbers\", \"bom\", \"positions\"} or {\"step\", \"dxf\"}, \"assemble\": <boards to assemble, 0 for bare>}], \"files\": {\"<path from the folder>\": \"<sha256>\"}}, listing every file (sha256sum).\n"
  ++ "5. Call the backplane tool `order_quote` with the folder (`mfg/<...>`) and end your turn: Backplane shows the person every file it would send and asks them first. Once they send, the files go to the order server and the quote is made (usually 1-2 minutes); meanwhile the thread shows that the quote is on its way. Backplane then posts it with its Buy now link (it opens Stripe) in this thread and you get a message with its total; do not repeat its table. Say in a line or two what it covers, and if it lists problems, what fixes each. If the person writes before the quote comes, call `order_status`: it says whether the files were sent and the quote is being made; never ask them to send or approve again unless it says the files were not sent, and never guess at problems.\n"
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
  "The person is asked on their screen whether to send these files (Send or Not now). End your turn now. Once they send, the files go to the order server and the quote is made (usually 1-2 minutes); the thread shows the person it is on its way, Backplane posts the quote here and you get a message with its total. There is nothing more for them to press: never ask them to send or approve again."

# order_status while the quote is on its way (the files sent, the quote not
# posted yet: the setting Order.wait.key) or before the person sent them
# (the approval still open); "" otherwise, and the order server is asked
def Order.status.sent() -> String:
  "The files are sent; the quote is being made (usually 1-2 minutes). Backplane posts it in this thread and tells you. Nothing is wrong and there is nothing for the person to press: say it is on its way and end your turn."

def Order.status.unsent() -> String:
  "The files are not sent yet: Backplane's question whether to send them (Send or Not now) is open on the person's screen. Say so in a line and end your turn; you get a message once they answer."

def Order.status.now(unsent: Bool, sent: Bool) -> String:
  Bool.pick(String, unsent, Order.status.unsent(), Bool.pick(String, sent, Order.status.sent(), ""))

def Order.quote.declined() -> String:
  "The person chose not to send the files (Not now). Ask whether anything should change first."

# The hub's words to the agent once the quote is settled (Hub.order.wake
# posts them as a "[Backplane] " system note): only the agent reads them;
# the conversation hides them (fold.bend's Fold.note, which keeps their
# openings; law order_wake_hidden)
def Order.wake.posted(+said: String) -> String:
  "The quote is posted in this thread. " ++ said

def Order.wake.failed(+error: String) -> String:
  "The quote could not be worked out: " ++ error ++ "."

def Order.wake.untaken(+error: String) -> String:
  "The files were not taken: " ++ error ++ "."

# A buy thread's questions show in a "Backplane needs" card above the
# composer. One whose answer is typed (where it ships) is the tool
# "order_ask.typed": a box with Continue on every client, its quick choice
# the earlier order's answer as "<it> · last time" (the answer sent is the
# choice without it). Every "order_ask…" is an order's question
# (Order.ask.is: posted as the person's message, never retired; laws
# order_ask_*)
def Order.ask.tool(typed: Bool) -> String:
  Bool.pick(String, typed, "order_ask.typed", "order_ask")

def Order.ask.is(+tool: String) -> Bool:
  String.starts_with(tool, "order_ask")

def Order.ask.typed(+tool: String) -> Bool:
  String.eq(tool, "order_ask.typed")

def Order.ask.last.mark() -> String:
  " · last time"

def Order.ask.value(+choice: String) -> String:
  Bool.pick(String, String.ends_with(choice, Order.ask.last.mark()), String.trim(String.take(choice, Nat.sub(String.length(choice), String.length(Order.ask.last.mark())))), choice)

def Order.ask.keep(skip: Bool, +x: String, rest: List<&2, String>) -> List<&2, String>:
  match skip:
    case True{}:
      rest
    case False{}:
      x <> rest

# the choices but blank ones and the one given
def Order.ask.but(xs: List<&2, String>, +x: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +h <> t:
      Order.ask.keep(Bool.or(String.is_empty(String.trim(h)), String.eq(String.trim(h), x)), String.trim(h), Order.ask.but(t, x))

def Order.ask.choices.k(none: Bool, +last: String, xs: List<&2, String>) -> List<&2, String>:
  match none:
    case True{}:
      Order.ask.but(xs, "")
    case False{}:
      (last ++ Order.ask.last.mark()) <> Order.ask.but(xs, last)

# the ask's options, a line each: the earlier answer first
def Order.ask.choices(+last: String, xs: List<&2, String>) -> String:
  String.join(Order.ask.choices.k(String.is_empty(String.trim(last)), String.trim(last), xs), "\n")

# Ordering is on unless the person turns it off in Settings (the setting
# order.enabled, "on" when never set: on for people updating too, since
# nothing but their own Off ever wrote "off"). Off: no Buy it, no prices
# asked, nothing sent to the order server, no fab files or reviews asked of
# agents (laws order_off_*, order_on_by_default). Every client and the hub
# ask Order.enabled.of, never the setting itself.
def Order.enabled(+v: String) -> Bool:
  Bool.not(String.eq(v, "off"))

def Order.enabled.default() -> String:
  "on"

def Order.enabled.of(+s: M.State) -> Bool:
  Order.enabled(M.Setting.get(s, "order.enabled", Order.enabled.default()))

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
   Order.tool.def("order_ask", "Ask the person one question, shown in its own card above the message box on every device: choices they can tap, or with typed a box to type the answer in (where it ships); they may always type their own answer. Answers at once: end your turn, and their answer comes as their next message, however long they take.",
     [Order.tool.prop("question", "string", "The question, short and plain."),
      J.KV{"options", J.Json.obj([J.KV{"type", J.Str{"array"}}, J.KV{"items", J.Json.obj([J.KV{"type", J.Str{"string"}}])}, J.KV{"description", J.Str{"Two to four short choices, the likely one first; none with typed."}}])},
      J.KV{"typed", J.Json.obj([J.KV{"type", J.Str{"boolean"}}, J.KV{"description", J.Str{"true when the answer is typed (the country it ships to): the person gets a box with Continue."}}])},
      Order.tool.prop("last", "string", "With typed: the earlier order's answer (its country's name), offered as a quick choice.")],
     [J.Str{"question"}])]

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

# A fab object needs both a kind and measured features before it can be
# called ready. Keep the file name in the correction shown to the person.
def Order.part.validation(+path: String, +o: J.Json) -> String:
  Bool.pick(String, String.is_empty(String.trim(J.Json.get_str(o, "kind", ""))), path ++ ": missing kind",
    Bool.pick(String, String.eq(J.Json.show(J.Json.get_obj(o, "features")), "null"), path ++ ": missing measured features", ""))

def Order.part.of(+path: String, +o: J.Json) -> J.Json:
  J.Json.obj([J.KV{"name", J.Str{J.Json.get_str(o, "name", path)}}, J.KV{"kind", J.Str{J.Json.get_str(o, "kind", "")}},
    J.KV{"qty", Maybe.default(&2, J.Json, J.Json.get(o, "qty"), J.Num{"1"})},
    J.KV{"options", J.Json.get_obj(o, "options")}, J.KV{"features", J.Json.get_obj(o, "features")}, J.KV{"path", J.Str{path}}, J.KV{"validation", J.Str{Order.part.validation(path, o)}}])

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

def Order.part.bad(+path: String, +why: String) -> J.Json:
  J.Json.obj([J.KV{"name", J.Str{path}}, J.KV{"kind", J.Str{""}}, J.KV{"path", J.Str{path}}, J.KV{"validation", J.Str{path ++ ": " ++ why}}])

def Order.part.parsed(obj: Bool, +o: J.Json, +path: String, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match obj:
    case True{}:
      Order.part.k(J.Json.get_bool(o, "order", True{}), path, o, rest)
    case False{}:
      Order.part.bad(path, "invalid fab JSON: expected an object") <> rest

def Order.part.obj(m: Maybe<&2, J.Json>, +path: String, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match m:
    case Some{+o}:
      Order.part.parsed(Order.obj(o), o, path, rest)
    case None{}:
      Order.part.bad(path, "could not read fab JSON") <> rest

def Order.parts(fs: List<&2, FabFile>) -> List<&2, J.Json>:
  match fs:
    case Nil{}:
      Nil{}
    case FabFile{+path, text} <> t:
      Order.part.obj(J.Json.parse(text), path, Order.parts(t))

# A combined estimate covers every requested part. Missing rows and per-part
# problems must never make an incomplete sum look like the project's price.
def Estimate.priced(p: J.Json) -> Bool:
  match p:
    case J.Num{_}: True{}
    case _: False{}

def Estimate.strings(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case +x <> t: J.Json.as_str(Some{x}, "") <> Estimate.strings(t)

def Estimate.problem(+p: J.Json, +why: String) -> String:
  "No estimate for " ++ J.Json.get_str(p, "name", J.Json.get_str(p, "path", "part"))
    ++ " (" ++ J.Json.get_str(p, "path", "fab file") ++ ")"
    ++ Bool.pick(String, String.is_empty(why), "", ": " ++ why)

def Estimate.row(+p: J.Json, +r: J.Json) -> String:
  Bool.pick(String, Bool.and(Bool.and(String.eq(String.take(J.Json.get_str(p, "name", ""), 64n), J.Json.get_str(r, "name", "")),
    String.eq(J.Json.get_str(p, "kind", ""), J.Json.get_str(r, "kind", ""))),
    Bool.and(Estimate.priced(J.Json.get_obj(r, "from")), List.is_empty(&2, J.Json, J.Json.elems(J.Json.get_obj(r, "problems"))))), "",
    Estimate.problem(p, String.join(Estimate.strings(J.Json.elems(J.Json.get_obj(r, "problems"))), "; ")))

def Estimate.join(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_empty(a), b, a ++ Bool.pick(String, String.is_empty(b), "", "; " ++ b))

def Estimate.problems(ps: List<&2, J.Json>, rs: List<&2, J.Json>) -> String:
  match ps rs:
    case Nil{} Nil{}: ""
    case Nil{} _ <> _: "Unexpected estimate parts"
    case +p <> t Nil{}:
      Estimate.join(Estimate.problem(p, "not returned by the estimate service"), Estimate.problems(t, Nil{}))
    case +p <> t +r <> more:
      Estimate.join(Estimate.row(p, r), Estimate.problems(t, more))

def Estimate.validation(ps: List<&2, J.Json>) -> String:
  match ps:
    case Nil{}: ""
    case +p <> t:
      Estimate.join(J.Json.get_str(p, "validation", ""), Estimate.validation(t))

def Estimate.note(+ps: List<&2, J.Json>, +est: J.Json, on: Bool) -> String:
  Estimate.join(Estimate.validation(ps), Bool.pick(String, Bool.and(on, String.is_empty(J.Json.get_str(est, "error", ""))), Estimate.problems(ps, J.Json.elems(J.Json.get_obj(est, "parts"))), ""))

# A cached number describes the files at the last check. Refresh is explicit
# because a person or another program can edit those files between turns.
def Estimate.snapshot(+label: String) -> String:
  Bool.pick(String, String.starts_with(label, "from "), "last estimate: " ++ label, label)

def Estimate.label(+label: String, +note: String) -> String:
  Bool.pick(String, String.is_empty(note), label, "estimate incomplete")

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

# what the price covers about customs, said where the total is: a
# duty-paid quote (its shipping's ddp) has nothing left to pay
def Order.customs(ddp: Bool) -> String:
  match ddp:
    case True{}:
      "Includes import VAT and duties: nothing to pay on delivery."
    case False{}:
      "Excludes customs: the courier will contact you to pay any duties and taxes before delivery."

def Order.ddp(+q: J.Json) -> Bool:
  J.Json.get_bool(J.Json.get_obj(J.Json.get_obj(q, "quote"), "shipping"), "ddp", False{})

# the posted quote says what it covers about customs (its shipping's ddp:
# Order.customs), whatever the order server wrote above it
def Order.quote.customs(+md: String, ddp: Bool) -> String:
  Bool.pick(String, String.is_empty(String.trim(md)), md, md ++ "\n\n" ++ Order.customs(ddp))

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

def Order.ask.count(+n: Nat) -> String:
  Nat.show(n) ++ Bool.pick(String, Nat.is_eq(n, 1n), " file", " files")

def Order.ask.detail(+dir: String, +m: J.Json, +ps: List<&2, String>) -> String:
  "Send " ++ Order.ask.count(List.length(&2, String, ps)) ++ " from " ++ dir ++ " to Backplane's quote service? They are kept only to make what you order.\n"
    ++ "Ships to " ++ J.Json.get_str(m, "ship_to", "?") ++ " · " ++ String.join(Order.ask.parts(J.Json.elems(J.Json.get_obj(m, "parts"))), ", ") ++ "\n"
    ++ Order.ask.files(ps, 40n)

# whether a quote can be paid: worked out and nothing to fix
def Order.payable(+q: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(q, "status", ""), "ready"), List.is_empty(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(q, "quote"), "problems"))))

# a quote the order server is still working out
def Order.pending(+q: J.Json) -> Bool:
  Bool.or(String.eq(J.Json.get_str(q, "status", ""), "received"), String.eq(J.Json.get_str(q, "status", ""), "analysing"))

# what the agent is told the thread shows: the Buy now link only when the
# quote can be paid; otherwise there is none, and the agent says why. A
# quote still being made is neither (test/ordercard_test.bend): an agent told
# "no Buy now link" while it was invented problems
def Order.said.note.k(pay: Bool) -> String:
  match pay:
    case True{}:
      "Backplane posted the quote with its Buy now link in this thread; do not repeat its table."
    case False{}:
      "Backplane posted the quote in this thread with NO Buy now link: it cannot be paid until its problems are fixed. Do not say a link is there. Tell the person each problem in plain words and what fixes it (for a destination that cannot be priced, ask for another country), then make a new quote."

def Order.said.note.p(pending: Bool, pay: Bool) -> String:
  match pending:
    case True{}:
      Order.status.sent()
    case False{}:
      Order.said.note.k(pay)

# what the agent hears back (never the token or the Buy now link)
def Order.said(+q: J.Json) -> String:
  J.Json.show(J.Json.obj([J.KV{"status", J.Str{J.Json.get_str(q, "status", "")}},
    J.KV{"total", Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(q, "quote"), "total"), J.Null{})},
    J.KV{"lead_days", Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(q, "quote"), "lead_days"), J.Null{})},
    J.KV{"problems", Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(q, "quote"), "problems"), J.Json.arr(Nil{}))},
    J.KV{"warnings", Maybe.default(&2, J.Json, J.Json.get(J.Json.get_obj(q, "quote"), "warnings"), J.Json.arr(Nil{}))},
    J.KV{"order", J.Json.get_obj(q, "order")},
    J.KV{"note", J.Str{Order.said.note.p(Order.pending(q), Order.payable(q))}}]))

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

# The order card
# --------------
# A buy thread's card, pinned at its top on every client: its steps,
# Details → Files → Quote → Pay → Made → Shipped, the one at work lit, and
# a line saying what happens now or what it needs; once quoted the total,
# what it covers about customs and Buy now (laws order_card_*). From the
# thread's state alone: an order question open (asked), the files'
# approval open (files), the quote on its way (the setting
# Order.wait.key, set from the person's Send until the quote or why there
# is none is posted: the thread waits, never Finished) and the posted
# quote as the hub keeps it (the setting Order.quote.key, Order.sum).
def Order.wait.key(+thread: String) -> String:
  M.Work.order.key(thread)

def Order.quote.key(+thread: String) -> String:
  "order.quote." ++ thread

def Order.steps() -> List<&2, String>:
  ["Details", "Files", "Quote", "Pay", "Made", "Shipped"]

# the total in words: the Buy now label's price ("Buy now · $112.40")
def Order.total.of(+label: String, +raw: String) -> String:
  Bool.pick(String, String.starts_with(label, "Buy now · "), String.drop(label, 10n), Bool.pick(String, String.is_empty(raw), "", "$" ++ raw))

# what the hub keeps of a quote for the card (q: the order server's answer,
# pay: its Buy now link)
def Order.sum(+q: J.Json, +pay: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"id", J.Str{J.Json.get_str(q, "id", "")}}, J.KV{"state", J.Str{Order.state(q)}},
    J.KV{"total", J.Str{Order.total.of(J.Json.get_str(J.Json.get_obj(q, "quote"), "pay_label", ""), J.Json.as_raw(J.Json.get(J.Json.get_obj(q, "quote"), "total"), ""))}},
    J.KV{"ddp", J.Flag{Order.ddp(q)}}, J.KV{"payable", J.Flag{Order.payable(q)}}, J.KV{"pay", J.Str{pay}},
    J.KV{"label", J.Str{J.Json.get_str(J.Json.get_obj(q, "quote"), "pay_label", "Buy now")}},
    J.KV{"tracking", J.Str{J.Json.get_str(J.Json.get_obj(q, "order"), "tracking", "")}}]))

# a newer quote always takes the card; a quote's own progress only while it
# is the card's (an older quote's watcher never overwrites a newer one)
def Order.sum.keeps(fresh: Bool, +cur: String, +id: String) -> Bool:
  Bool.or(fresh, Bool.or(String.is_empty(cur), String.eq(J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(cur), J.Null{}), "id", ""), id)))

def Order.sum.set.k(keep: Bool, +thread: String, +sum: String, +now: Nat) -> List<&2, M.Change>:
  match keep:
    case True{}:
      [M.SettingSet{Order.quote.key(thread), sum, now}]
    case False{}:
      Nil{}

# what the hub keeps of a quote (server.bend's Hub.order.quoted)
def Order.sum.set(+s: M.State, +thread: String, +sum: String, +id: String, fresh: Bool, +now: Nat) -> List<&2, M.Change>:
  Order.sum.set.k(Order.sum.keeps(fresh, M.Setting.get(s, Order.quote.key(thread), ""), id), thread, sum, now)

# the quote on its way from the person's Send (server.bend's Od.go) ...
def Order.wait.set(+thread: String, +now: Nat) -> M.Change:
  M.SettingSet{Order.wait.key(thread), Nat.show(now), now}

# ... until the hub tells the agent what came of it (the quote, why there
# is none, the files not taken: Hub.order.wake)
def Order.unwait(+s: M.State, +thread: String, +now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, String.is_empty(M.Setting.get(s, Order.wait.key(thread), "")), Nil{}, [M.SettingSet{Order.wait.key(thread), "", now}])

type OrderCard is Data:
  OrderCard{step: Nat, line: String, total: String, customs: String, pay: String, label: String}

# the step a quote's state is at (6: delivered, every step done)
def Order.card.at(+st: String) -> Nat:
  Bool.pick(Nat, String.eq(st, "delivered"), 6n, Bool.pick(Nat, String.eq(st, "shipped"), 5n,
    Bool.pick(Nat, Bool.or(String.eq(st, "paid"), String.eq(st, "placed")), 4n, 3n)))

def Order.card.step.k(files: Bool, waiting: Bool, quoted: Bool, +st: String) -> Nat:
  match files waiting quoted:
    case True{} _ _:
      1n
    case False{} True{} _:
      2n
    case False{} False{} True{}:
      Order.card.at(st)
    case False{} False{} False{}:
      0n

# what a quote's state says once it is posted
def Order.card.said(+st: String, payable: Bool, +tracking: String) -> String:
  Bool.pick(String, String.eq(st, "expired"), "This quote expired. Ask here for a new one.",
    Bool.pick(String, String.eq(st, "failed"), "The quote could not be worked out. Ask here to try again.",
      Bool.pick(String, String.eq(st, "pilot"), "Test payment: nothing was charged or ordered.",
        Bool.pick(String, String.eq(st, "refunded"), "Refunded: the money is on its way back to you.",
          Bool.pick(String, Nat.is_ge(Order.card.at(st), 4n), Order.status.note(st, tracking),
            Bool.pick(String, payable, "Quoted. Buy now to place the order.", "The quote has something to fix first: see it below."))))))

def Order.card.line.k(+i: Nat, running: Bool, +st: String, payable: Bool, +tracking: String) -> String:
  match i:
    case 0n:
      Bool.pick(String, running, "Getting the order ready…", "Reply below to go on.")
    case 1n:
      "Check the files below and send them for a quote."
    case 2n:
      "Getting your quote… usually 1-2 min"
    case _:
      Order.card.said(st, payable, tracking)

# (an open question comes first: the card says it needs the answer)
def Order.card.line(asked: Bool, +i: Nat, running: Bool, +st: String, payable: Bool, +tracking: String) -> String:
  match asked:
    case True{}:
      "Backplane needs your answer below."
    case False{}:
      Order.card.line.k(i, running, st, payable, tracking)

# Buy now only while the quote can be paid and is not paid yet
def Order.card.buys(+i: Nat, +st: String, payable: Bool) -> Bool:
  Bool.and(Bool.and(Nat.is_eq(i, 3n), payable), Bool.or(String.eq(st, "ready"), String.eq(st, "quoted")))

def Order.card.of(asked: Bool, +i: Nat, running: Bool, +q: J.Json) -> OrderCard:
  +st = J.Json.get_str(q, "state", "")
  +payable = J.Json.get_bool(q, "payable", False{})
  +shown = Nat.is_ge(i, 3n)
  +buys = Order.card.buys(i, st, payable)
  OrderCard{i, Order.card.line(asked, i, running, st, payable, J.Json.get_str(q, "tracking", "")),
    Bool.pick(String, shown, J.Json.get_str(q, "total", ""), ""),
    Bool.pick(String, Bool.and(shown, Bool.not(String.is_empty(J.Json.get_str(q, "total", "")))), Order.customs(J.Json.get_bool(q, "ddp", False{})), ""),
    Bool.pick(String, buys, J.Json.get_str(q, "pay", ""), ""), Bool.pick(String, buys, J.Json.get_str(q, "label", "Buy now"), "")}

# among a thread's open asks (client.bend's Asks.open): the files'
# approval, an order question
def Order.asks.files(as: List<&2, M.Ask>) -> Bool:
  match as:
    case Nil{}:
      False{}
    case M.Ask{_, _, _, +tool, _, _, _, _} <> t:
      Bool.or(String.eq(tool, "order_quote"), Order.asks.files(t))

def Order.asks.question(as: List<&2, M.Ask>) -> Bool:
  match as:
    case Nil{}:
      False{}
    case M.Ask{_, _, _, +tool, _, _, _, _} <> t:
      Bool.or(Order.ask.is(tool), Order.asks.question(t))

def Order.asks.keep(keep: Bool, a: M.Ask, rest: List<&2, M.Ask>) -> List<&2, M.Ask>:
  match keep:
    case True{}:
      a <> rest
    case False{}:
      rest

def Order.asks.open.go(as: List<&2, M.Ask>, +th: String) -> List<&2, M.Ask>:
  match as:
    case Nil{}:
      Nil{}
    case M.Ask{i, +t, k, tool, d, o, +op, an} <> rest:
      Order.asks.keep(Bool.and(op, String.eq(t, th)), M.Ask{i, t, k, tool, d, o, op, an}, Order.asks.open.go(rest, th))

# what order_status answers without asking the order server: the files not
# sent yet, or sent and the quote on its way ("" otherwise)
def Order.status.of(+s: M.State, +th: String) -> String:
  Order.status.now(Order.asks.files(Order.asks.open.go(M.Asks.all(s), th)), Bool.not(String.is_empty(M.Setting.get(s, Order.wait.key(th), ""))))

# asked: an order question open; files: the approval to send open;
# waiting: the quote on its way; sum: the setting Order.quote.key
def Order.card(asked: Bool, files: Bool, waiting: Bool, running: Bool, +sum: String) -> OrderCard:
  Order.card.of(asked, Order.card.step.k(files, waiting, Bool.not(String.is_empty(sum)), J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(sum), J.Null{}), "state", "")), running,
    Maybe.default(&2, J.Json, J.Json.parse(sum), J.Null{}))

# a step's mark on the card: "done" before the one at work, "on" for it
def Order.card.mark(+i: Nat, +k: Nat) -> String:
  Bool.pick(String, Nat.is_lt(k, i), "done", Bool.pick(String, Nat.is_eq(k, i), "on", ""))

def Order.card.step(c: OrderCard) -> Nat:
  match c:
    case OrderCard{i, _, _, _, _, _}:
      i

def Order.card.text(c: OrderCard) -> String:
  match c:
    case OrderCard{_, l, _, _, _, _}:
      l

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

# Preparing an order remains possible when the estimate service is down.
# The price gate above still applies; an unavailable estimate is unpriced.
def Card.action(+c: Card) -> String:
  match c:
    case Card{_, _, _, +n, +b, +err, _}:
      Bool.pick(String, Bool.and(Nat.is_gt(Nat.add(n, b), 0n), Bool.not(String.is_empty(err))), "Buy it", Card.buy(c))

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

# The boards the viewer's picker offers (test/boards_pick_test.bend). The
# hub lists every KiCad board and project file in the project (paths from
# its folder, server orderio.bend's Od.boards); offered are the boards that
# are a KiCad project's own (<stem>.kicad_pcb beside <stem>.kicad_pro)
# outside folders that hold copies (archive, build, backups, old, out...),
# so an agent's snapshots and staging copies stay out. With none such,
# every board outside those folders; with none, every board. The project's
# default board (the viewer's own) leads; at most Boards.most() are offered.
def Boards.most() -> Nat:
  24n

def Boards.copies.k(+l: String) -> Bool:
  Bool.or(String.starts_with(l, "archive"), Bool.or(String.starts_with(l, "build"), Bool.or(String.contains(l, "backup"),
    Bool.or(String.eq(l, "old"), Bool.or(String.eq(l, "out"), Bool.or(String.starts_with(l, "output"),
      Bool.or(String.eq(l, "tmp"), Bool.or(String.eq(l, "temp"), Bool.or(String.eq(l, "staging"),
        Bool.or(String.eq(l, "scratch"), Bool.or(String.eq(l, "renders"), String.starts_with(l, "export"))))))))))))

# a folder's name that says it holds copies
def Boards.copies(+d: String) -> Bool:
  Boards.copies.k(String.to_lower(d))

# all but the last (a path's folders, its file name left out)
def Boards.init(ss: List<&2, String>) -> List<&2, String>:
  match ss:
    case Nil{}:
      Nil{}
    case +s <> +t:
      Bool.pick(List<&2, String>, List.is_empty(&2, String, t), Nil{}, s <> Boards.init(t))

def Boards.any(ss: List<&2, String>) -> Bool:
  match ss:
    case Nil{}:
      False{}
    case +s <> t:
      Bool.or(Boards.copies(s), Boards.any(t))

# the path is inside a folder of copies
def Boards.copy(+p: String) -> Bool:
  Boards.any(Boards.init(String.split(p, '/')))

def Boards.pcb(+p: String) -> Bool:
  String.ends_with(p, ".kicad_pcb")

# the KiCad project file a board would have beside it
def Boards.pro(+p: String) -> String:
  String.take(p, Nat.sub(String.length(p), 10n)) ++ ".kicad_pro"

def Boards.has(xs: List<&2, String>, +x: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(String.eq(h, x), Boards.has(t, x))

def Boards.keep(ok: Bool, +p: String, rest: List<&2, String>) -> List<&2, String>:
  match ok:
    case True{}:
      p <> rest
    case False{}:
      rest

# a KiCad project's own boards, outside folders of copies
def Boards.real(ps: List<&2, String>, +all: List<&2, String>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Boards.keep(Bool.and(Boards.pcb(p), Bool.and(Bool.not(Boards.copy(p)), Boards.has(all, Boards.pro(p)))), p, Boards.real(t, all))

# every board outside folders of copies
def Boards.outside(ps: List<&2, String>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Boards.keep(Bool.and(Boards.pcb(p), Bool.not(Boards.copy(p))), p, Boards.outside(t))

def Boards.pcbs(ps: List<&2, String>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      Boards.keep(Boards.pcb(p), p, Boards.pcbs(t))

def Boards.or(+a: List<&2, String>, +b: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, List.is_empty(&2, String, a), b, a)

def Boards.without(xs: List<&2, String>, +x: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +h <> t:
      Boards.keep(Bool.not(String.eq(h, x)), h, Boards.without(t, x))

# the default board first (when there is one)
def Boards.lead(+bd: String, +xs: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(bd), xs, bd <> Boards.without(xs, bd))

def Boards.cap(xs: List<&2, String>, +n: Nat) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Bool.pick(List<&2, String>, Nat.is_eq(n, 0n), Nil{}, x <> Boards.cap(t, Nat.sub(n, 1n)))

# all: the hub's list (boards and project files); bd: the default board
def Boards.pick(+all: List<&2, String>, +bd: String) -> List<&2, String>:
  Boards.cap(Boards.lead(bd, Boards.or(Boards.real(all, all), Boards.or(Boards.outside(all), Boards.pcbs(all)))), Boards.most())

# The picker reads the boards again each time it opens (boards.list, the
# hub's job "boards": a rescan of the thread's folder, none of the card's
# other work; client.bend's Bd.look). Its answer: {thread, board_list: the
# boards Boards.pick offers, board_default}, kept where the card keeps
# them ("fab.boards:<t>", "fab.board:<t>"), one path a line (laws
# boards_list_*)
def Boards.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      J.Str{x} <> Boards.strs(t)

def Boards.list.j(+thread: String, bs: List<&2, String>, +bd: String) -> J.Json:
  J.Json.obj([J.KV{"thread", J.Str{thread}}, J.KV{"board_list", J.Json.arr(Boards.strs(bs))}, J.KV{"board_default", J.Str{bd}}])

# (the hub found: its board and project files; bd: the folder's own board)
def Boards.list(+thread: String, +found: List<&2, String>, +bd: String) -> J.Json:
  Boards.list.j(thread, Boards.pick(found, bd), bd)

# an answer to boards.list (not the card's, whose list is "board_files")
def Boards.listed(+r: J.Json) -> Bool:
  Bool.not(String.eq(J.Json.show(Maybe.default(&2, J.Json, J.Json.get(r, "board_list"), J.Null{})), "null"))

def Boards.text(+r: J.Json) -> String:
  String.join(Card.strs(J.Json.elems(J.Json.get_obj(r, "board_list"))), "\n")

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
def Card.ready.estimate(+line: String, +note: String) -> String:
  Bool.pick(String, String.is_empty(note), line, note ++ ". Check the fab files before ordering.")

def Card.ready.error(+line: String, +err: String) -> String:
  Bool.pick(String, String.is_empty(err), line, "Estimate unavailable: " ++ err ++ Bool.pick(String, String.starts_with(line, "Ready to order · "), " · Parts: " ++ String.drop(line, 17n), Bool.pick(String, String.is_empty(line), "", " · " ++ line)))

def Card.ready.j(+j: J.Json) -> String:
  Card.ready.error(Card.ready.estimate(Card.ready.k(String.join(Card.names.go(J.Json.elems(J.Json.get_obj(j, "parts"))), " · "), J.Json.get_nat(j, "boards", 0n)), J.Json.get_str(j, "estimate_problems", "")), J.Json.get_str(j, "error", ""))

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
  Bool.or(RF.Refs.word(draft, "/intent"), Bool.or(String.starts_with(draft, "/intent "), String.starts_with(draft, "/intent\n")))

def Intent.body(+draft: String) -> String:
  String.trim(String.drop(String.trim(draft), 7n))

# What the intent shows: the file less the headings with nothing under
# them (Edit's template saved with only a goal shows no empty "Parts" or
# "Notes"; the file keeps them, for the next Edit). A heading is 1 to 6 #s
# then a space or nothing; it is empty when no line of text comes before
# the next heading of its level or above (test/intent_menu_test.bend).
def Intent.hashes(+t: String) -> Nat:
  Bool.pick(Nat, String.starts_with(t, "######"), 6n, Bool.pick(Nat, String.starts_with(t, "#####"), 5n, Bool.pick(Nat, String.starts_with(t, "####"), 4n,
    Bool.pick(Nat, String.starts_with(t, "###"), 3n, Bool.pick(Nat, String.starts_with(t, "##"), 2n, Bool.pick(Nat, String.starts_with(t, "#"), 1n, 0n))))))

def Intent.level.k(+n: Nat, +rest: String) -> Nat:
  Bool.pick(Nat, Bool.and(Nat.is_gt(n, 0n), Bool.or(String.is_empty(rest), String.starts_with(rest, " "))), n, 0n)

# a line's heading level ("## Parts" is 2), 0 for any other line
def Intent.level(+l: String) -> Nat:
  Intent.level.k(Intent.hashes(String.trim(l)), String.drop(String.trim(l), Intent.hashes(String.trim(l))))

# whether a line of text comes under a heading of level k
def Intent.filled(ls: List<&2, String>, +k: Nat) -> Bool:
  match ls:
    case Nil{}:
      False{}
    case +l <> t:
      +lv = Intent.level(l)
      Bool.pick(Bool, Bool.and(Nat.is_gt(lv, 0n), Nat.is_le(lv, k)), False{},
        Bool.or(Bool.and(Nat.is_eq(lv, 0n), Bool.not(String.is_empty(String.trim(l)))), Intent.filled(t, k)))

def Intent.shown.go(ls: List<&2, String>, +acc: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      List.reverse(&2, String, acc)
    case +l <> +t:
      Intent.shown.go(t, Bool.pick(List<&2, String>, Bool.and(Nat.is_gt(Intent.level(l), 0n), Bool.not(Intent.filled(t, Intent.level(l)))), acc, l <> acc))

def Intent.shown(+text: String) -> String:
  String.trim(String.join(Intent.shown.go(String.lines(text), Nil{}), "\n"))

def Order.card.total(c: OrderCard) -> String:
  match c:
    case OrderCard{_, _, t, _, _, _}:
      t

def Order.card.customs(c: OrderCard) -> String:
  match c:
    case OrderCard{_, _, _, x, _, _}:
      x

def Order.card.pay(c: OrderCard) -> String:
  match c:
    case OrderCard{_, _, _, _, p, _}:
      p

def Order.card.label(c: OrderCard) -> String:
  match c:
    case OrderCard{_, _, _, _, _, l}:
      l
