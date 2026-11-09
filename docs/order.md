# Ordering: intent, Buy it, buy threads

**On unless turned off.** Settings → Ordering. Off, there is no Buy it on any client, the hub never contacts the order server (no estimates; it only keeps following quotes and orders made while it was on), `order.buy` and the order tools are refused (an agent that remembers them gets "Ordering is off"), and agents get no fab-file or review instructions and no order tools (laws `fab_buy_off_hidden`, `order_off_*`). The intent and the project layout stay on. With Ordering on, Settings → Price estimates turns the "from $X" off on its own.

Names in `Order.…`, `Od.…`, `Hub.order.…`, `View.…`, `Card.…` and "laws `order_…`" are 0.14's Bend symbols, kept as the way to find a rule in `v0.14.1`. Their homes now: the tools, review gate, upload, poll and watcher are `native/host/src/settings/order.rs`; the quote's words, the Order card and the Buy now link `settings/order_view.rs`; the project card and estimate `settings/fab.rs`; Buy it and the info keys `present/ready.rs`; the tests are `agents/tests/order.rs` and the `tests` modules of those files.

A project says what it is for and what can be made of it. Backplane shows a price for it
and orders it through a buy thread.

## Files in a project

- `.backplane/intent.md`: the person's goal, parts and notes, short.
  - At the start of a request that changes them, the agent has a subagent update it.
  - The person edits it from the project card: Edit puts it in the composer as `/intent ...`, and sending saves it.
- `*.fab.json`, beside each thing to order:
  - fields: `kind`, `name`, `source`, `qty`, `options`, measured `features`;
  - kinds: boards (`pcb` for rigid FR-4, `flex`, `hdi`, `aluminum`, `copper_core`, `rogers`, `ptfe`), `3dp`, `cnc` and `sheetmetal`. Every board kind is measured from its gerbers; only `pcb` boards are assembled. The options are the order server's (`order/fabsrv/catalog.py`, `GET /v1/catalog`), and the agents learn them from `ORDER_PROMPT_FAB` (`agents/prompt_text.rs`); a host test (`settings/fab.rs`) reads the catalog and fails when the prompt does not name every kind, option, choice and feature in it;
  - `"order": false` leaves a part out;
  - files under `extra/` and `mfg/` are never read.
  - a part made for SendCutSend (by the bundled `sendcutsend` skill, docs/bundled-skills.md) also has a `sendcutsend` block: its SKU, services, finish, hardware, flat pattern, what Backplane's catalog has no equivalent for (`unmapped`) and its DFM check with the sha256 of the files checked. The card names an unmapped material or finish, and a part or flat pattern changed since its check (`fab.rs` `scs_validation`), as estimate problems; the manufacturing review runs the skill's `scs.py dfm` on such a part (`Order.prompt.review`). Ordering it stays Buy it and the person's.
- `mfg/<YYYYMMDD-HHMM>/`: what a buy thread exported, with its `manifest.json`. It is exported from the saved files, as the review's fingerprint is taken: an edit made through `kicad_ipc` (docs/kicad-ipc.md) counts once saved with SaveDocument.

The agents' instructions are built in `native/host/src/agents/prompt.rs` (`order_project` and the buy prompt beside it).

## The card

- **Request:** clients ask `fab.look {thread}` when a thread is chosen and whenever its turn ends. A thread opened while its agent works is asked once too, so its card shows at once.
- **The hub's job `fab`**:
  1. reads the intent and the fab files in the thread's folder;
  2. with Ordering and Price estimates on, asks the order server's `POST /v1/estimate` for the bare parts' price. Only kinds, options and features are sent; never files.
- **The answer** (`settings/fab.rs`) is `{thread, intent, goal, parts, boards, label, estimate_problems, error, estimates, board_files, board_default, order, lead_days, estimate_warnings}`: the first twelve as 0.14 gave them, key for key; `lead_days` (the longest of the estimate's per-part `lead_days`, 0 for none) and `estimate_warnings` (its per-part `warnings`, each once) come from the estimate and are empty without one. `fab.intent {thread, text}` writes `.backplane/intent.md` and answers the same card. It is kept as info keys `fab.line:`, `fab.intent:`, `fab.buy:`, `fab.ready:`, `fab.shown:`, `fab.boards:`, `fab.board:` and `fab.again:` (plus the thread; `present/ready.rs` `keep`). No client draws `lead_days` or `estimate_warnings` yet.
- **Boards:** the answer lists every KiCad board in the project (`board_files`, outside `extra/` and `mfg/`). With more than one, the viewer's Board, Schematic and 3D tabs offer each by its folder's name: chips in the window, the Sheet menu on phones (values `b:<path>`). A pick shows that board, or the schematic beside it, and the other tabs stay on it (`Bd.keep`). Before any pick, the chip of the board the viewer opens by default is lit.
- **What counts as something to order:** a `*.fab.json`, or a KiCad project's own board (a `.kicad_pro` beside it, outside folders of copies: `boards::own`). A repository that only holds board files, as test fixtures say, has nothing to order: no Buy it, and the order server is not asked. The viewer's picker still offers those boards.
- **Two places:**
  - **The card** at the top of the thread: the goal (click for the whole intent) and Edit.
  - **Buy it**, in the thread's header (`present/ready.rs` `header`): the first of the thread's tools, so on phones it heads the ⋮ menu. It is there whenever there is something to order, whatever the turn is doing, never in a buy thread. Its tooltip names the parts, or how the project's newest order stands ("Ordered · paid, being placed", then in production, shipped, delivered). Once something of the project is paid for, it reads Buy again.
  - There is no bar above the message box: the person asked for it gone, after it showed on projects that were no KiCad project and not ready to order (`d1df376576`).

- **The Buy button's words** (`Card.buy`, laws `order_buy_*`):
  - "Buy it · from $X" with a price;
  - "Buy it" when estimates are off, or when there are boards but no fab files yet;
  - nothing when there is nothing to order or the server is out of reach.

## Buy threads

- **Starting one:** `order.buy {thread}` makes a thread in the same folder, marked `buy.<thread>`, with the person's "Buy it" as its first message. A project's next one is "Buy: <project> · 2" (`Buy.title.of`).
- **What its agent gets:** the buy instructions and the tools `order_quote`, `order_status` and `order_ask`. Only buy threads have them (`Bots.buy.tools`). Their questions and the upload's approval alert the person in their own words, without the tool's name.
- **The order card:** a buy thread has an Order card pinned under its top bar: "Order · <project>", the steps Details → Files → Quote → Pay → Made → Shipped with the one at work lit, and a line saying what happens now or what it needs. Once quoted it has the total, what it covers about customs and Buy now; after payment the order's state. It reads the thread's open asks, the quote on its way and the quote the hub keeps. The thread's title is marked Order with the cart (`View.chrome.order`).
- **A quote that cannot be had:** the server's quote status `failed` (its `error`) posts nothing; the agent is told why, and the card reads "The quote could not be worked out". A busy server (429), one restarting (5xx) or away does not end the wait: the poll goes on. Only a 4xx other than 429 (say 404, no such quote) or a `failed` status does.
- **Waiting for the quote:** from the person's Send until the quote, or why there is none, is posted, the setting `order.wait.<thread>` holds when it was sent (`Order.wait.set` in `Od.go`, cleared by `Hub.order.wake`, `Order.unwait`). The thread waits on it like on its work: the sidebar's waiting dot and "Waiting since", never Finished, and the card says "Getting your quote… usually 1-2 min". A hub restarted while it was getting a quote (its record says `quoting`) waits on it again and goes on polling it, then posts it (`Od.resume.quoting`). `order_status` answers from the thread first (`Order.status.of`): the files not sent yet, or sent and the quote on its way; a quote the server is still working out is never called unpayable (`Order.said`).
- **Backplane needs:** in a buy thread every open question is a highlighted card above the composer (`View.needs`). An `order_ask`'s choices are buttons and a box takes a typed answer (Enter or Continue). Where it ships is typed: `order_ask` with `typed` (the tool `order_ask.typed`, `Order.ask.tool`) leads with the box, and `last` (the earlier order's country) becomes the quick choice "<Country> · last time", sent without the mark (`Order.ask.choices`, `Order.ask.value`; laws `order_ask_typed_*`, `order_ask_last_value`). When it ships to the US the thread asks the state the same way and the manifest carries it as `ship_region` (the quote's sales tax follows it; without one the server estimates at a high rate and says so); the files' approval shows it beside the country ("Ships to US (GA)"). The files' approval keeps Send / Not now.
- **Customs:** the card and the posted quote (`Order.quote.customs`, after the order server's text) say what the price covers: a duty-paid quote (`shipping.ddp`) "Includes import VAT and duties: nothing to pay on delivery."; otherwise "Excludes customs: the courier will contact you to pay any duties and taxes before delivery." (`Order.customs`, laws `order_customs_*`).
- **The review gate:** a buy thread never reviews the design itself. It first asks `order_review {action: "check"}`; unless the review is current, the person gets two buttons (an `order_ask`): **Review for manufacturing** or **Order without a review**.
  - The review runs as a task of the buy thread (`delegate_task`): in the same folder, on the very files to be ordered, its result coming back as a message that wakes the buy thread.
  - Issues from a review stop the export until they are fixed (another task, in the same folder, then the review again) or the person says to order anyway.
  - The gate is the hub's, not the agent's: a current, passed review answers "go on"; none or stale opens Review for manufacturing / Order without a review; current with issues posts them as bullets (the review's `issues`, else its summary cut at its semicolons) with the link `.backplane/review.md`, then opens Fix them first / Order anyway. Agents skipped listing the issues when told to.
- **The pilot:** while the order server's Stripe key is a test key (or `FAB_PILOT=1`; order/fabsrv's `config.PILOT`), Stripe's page, the quote and the page back from Stripe say that ordering through Backplane is a pilot, the card will not be charged (Stripe's test card 4242 4242 4242 4242 walks the checkout), and the exported files in `mfg/` can be ordered from any fabricator. The quote carries `pilot`, so the hub calls a paid test checkout "pilot" (`Order.state.pilot`): it says so in the thread, alerts no one, never shows as Ordered and is not watched further (laws `order_pilot_*`).
- **Nothing waits on the person:** `order_ask` and `order_quote` answer the agent at once and it ends its turn (an MCP call held open died after about five minutes, Node's fetch timeout, and the person's later tap went nowhere). An order question parks nothing: its answer is posted in the thread as the person's message, which starts (or queues) the agent's turn however long they took, and a Stop or a crash leaves it open. The files' approval still parks, and once the person sends, the quote is posted and the agent gets a system message with its total; Not now tells it too, and a retired run tells no one. The approval stays open across a hub restart too (an update, say): what it would send is kept in `<home>/orders/ask-<id>.json`, so its Send still sends exactly the files the person saw; an approval whose list was not kept (made by an older hub) tells the agent to call `order_quote` again. Once answered, the thread keeps one line: "Sent for a quote: 13 files from mfg/…" or "Not sent: …".
- **Manufacturing reviews** (`Order.prompt.review`, in every agent's project instructions): what makes a fab refuse a design or charge more, not the circuit. The findings go to `.backplane/review.md`; the verdict (ok or issues) and a line go to `order_review {action: "record"}`.
  - With Ordering on, `order_review` is every thread's tool (`Bots.order.tools`). The hub hashes the design: the names and contents of every board, schematic, STEP, DXF and fab file outside `mfg/`, `extra/` and `.backplane/` (`Od.print`).
  - `record` keeps the hash with the verdict, the summary and the issues (one short line each, at most 8: `Order.review.rec.with`, laws `order_review_issues_*`) in `.backplane/review.json`; `check` answers none, stale (the design changed since) or current (`Order.review.state`, laws `order_review_*`).
- **`order_quote {dir}`**:
  1. reads the manifest;
  2. refuses unsafe paths and folders holding links;
  3. opens an approval card listing every file ("Send these files for a quote?", Send / Not now);
  4. on Send, uploads them (curl, from the folder, by relative paths), polls the quote, and posts it as a message with its **Buy now · $X** link, which opens Stripe Checkout straight away (there is no quote page; the server only redirects).
- **The token** lives only in `<home>/orders/<quote>.json` and the Buy now link; the agent never sees either.
- **The watcher** (`Od.watch`, resumed at start) asks for the order's state every minute while unpaid and every ten minutes after. It says Paid, Ordered, Shipped (with tracking), Delivered, Refunded or Expired in the thread, once each. A step taken while the hub was down is said at its start, not after the watch's next pause. Each but Expired also alerts the person's devices, a note alert like any other (push, desktop, phones; `Hub.order.note`, laws `order_note_*`), unless they turned off alerts for settled work (`notify.settled`).

## The server

`order/` (Python): see `order/README.md` for the API, pricing, security and deployment.
- The app's default server is `Order.server()`; the `order.server` setting replaces it. `BACKPLANE_ORDER_SERVER` in the hub's environment replaces only the default: every test hub (skills/backplane-e2e/hub.sh, test/tools and desktop/test harnesses, scripts/test.sh, the smoke scripts) sets it to a dead local address, so with Ordering on by default no test reaches the real server; hub.sh also starts with Ordering off, and a story that orders sets `order.server` to its own stand-in.
- The fab's price models are a private pack (`FAB_VENDOR`), not in this repository.

## Checking it end to end

`test/order/buy_e2e.py` walks the whole flow in the GPUI window with real X input, as a person does, against `order/fabsrv` on loopback
(`test/order/local_server.py`: the stand-in fab pack and a Stripe double) and a scripted stand-in agent (`claude_standin.ts` mode `buyflow`,
which exports with kicad-cli as a buy thread's agent does, `test/order/export_kicad.py`): Buy it with its estimate → the review gate →
Review for manufacturing as a task that records its verdict → the country and the US state typed in their boxes → the files' approval
("Ships to US (GA)"), outliving a hub restart → Send → waiting for the quote → the quote's four lines (Manufacturing, Shipping, Duties &
taxes, Sales tax (estimated)), the card's total, customs line and Buy now → the order server's page, which pays the double → Paid →
placed and shipped by the operator, said by a restarted hub. `--paused` checks that Buy now shows the paused page instead.

    XV=<Xvfb tree> TWM=<twm> BUN=<bun> VK_ICD_FILENAMES=<lavapipe icd> BACKPLANE_GPUI_ASSETS=assets \
      python3 test/order/buy_e2e.py --client <backplane-gpui> --hub <backplane-hub> --python <venv with order/requirements.txt> \
      [--project <a KiCad project folder, copied>] [--display :N] [--port N]
