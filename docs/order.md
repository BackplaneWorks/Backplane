# Ordering: intent, Buy it, buy threads

**On unless turned off.** Settings → Ordering (`order.enabled`, default on, for a new install and for one updated from a release where it was off by default: only the person's own Off, stored as `off`, keeps it off; one definition, order.bend's `Order.enabled.of`, law `order_on_by_default`). Off, there is no Buy it or Ready bar on any client, the hub never contacts the order server (no estimates; it only keeps following quotes and orders made while it was on), `order.buy` and the order tools are refused (an agent that remembers them gets "Ordering is off"), and agents get no fab-file or review instructions and no order tools (laws `fab_buy_off_hidden`, `order_off_*`). The intent and the project layout stay on. With Ordering on, Settings → Price estimates turns the "from $X" off on its own.

A project says what it is for and what can be made of it. Backplane shows a price for it
and orders it through a buy thread.

## Files in a project

- `.backplane/intent.md`: the person's goal, parts and notes, short.
  - At the start of a request that changes them, the agent has a subagent update it.
  - The person edits it from the project card: Edit puts it in the composer as `/intent ...`, and sending saves it.
- `*.fab.json`, beside each thing to order:
  - fields: `kind`, `name`, `source`, `qty`, `options`, measured `features`;
  - kinds: boards (`pcb` for rigid FR-4, `flex`, `hdi`, `aluminum`, `copper_core`, `rogers`, `ptfe`), `3dp`, `cnc` and `sheetmetal`. Every board kind is measured from its gerbers; only `pcb` boards are assembled. The options are the order server's (`order/fabsrv/catalog.py`, `GET /v1/catalog`), and the agents learn them from `Order.prompt.fab`;
  - `"order": false` leaves a part out;
  - files under `extra/` and `mfg/` are never read.
- `mfg/<YYYYMMDD-HHMM>/`: what a buy thread exported, with its `manifest.json`. It is exported from the saved files, as the review's fingerprint is taken: an edit made through `kicad_ipc` (docs/kicad-ipc.md) counts once saved with SaveDocument.

The agents' instructions are in `src/core/order.bend` (`Order.prompt.project`, `Order.prompt.buy`).

## The card

- **Request:** clients ask `fab.look {thread}` when a thread is chosen and whenever its turn ends (`Fab.sync` in client.bend). A thread opened while its agent works is asked once too, so its card shows at once.
- **The hub's job `fab`** (server/orderio.bend's `Od.look`):
  1. reads the intent and the fab files in the thread's folder;
  2. with Ordering and Price estimates on (`order.enabled`, `order.estimates`; hub.bend's `Buy.est`), asks the order server's `POST /v1/estimate` for the bare parts' price. Only kinds, options and features are sent; never files.
- **The answer** is kept as info keys `fab.line:`, `fab.intent:`, `fab.buy:`, `fab.ready:`, `fab.shown:` and `fab.open:` (plus the thread).
- **Boards:** the answer lists every KiCad board in the project (`board_files`, outside `extra/` and `mfg/`). With more than one, the viewer's Board, Schematic and 3D tabs offer each by its folder's name (client.bend's `Bd.*`, laws `bd_*`): chips on the web (`View.boards`) and in the window (`Side.boards`), the Sheet menu on phones (`View.board.list`, values `b:<path>`, so no new app). A pick shows that board, or the schematic beside it, and the other tabs stay on it (`Bd.keep`). Before any pick, the chip of the board the viewer opens by default is lit (the card's `board_default`, from kicad.bend's `Files.read`).
- **Two places:**
  - **The card** at the top of the thread: the goal (click for the whole intent) and Edit.
  - **Ready to order**, right above the message box: the parts by name and Buy it (`Fab.ready`, laws `order_ready_*`, `fab_ready_*`). It shows once the agent's turn is over, never while it works or in a buy thread; on the web it glows once as it comes. Once something of the project is paid for, it says so ("Ordered · paid, being placed", then in production, shipped, delivered: the newest order of the project's buy threads, `Od.order`) and the button reads Buy again (laws `order_ordered_*`, `fab_buy_again*`). Its × puts it away for the project (the setting `fab.min.<project>`, kept like any setting, so every device follows): the thread's header then has a Buy it (the box icon; Ordered once something is paid for) that brings it back. On phones the × is on the Ready row and Buy it heads the ⋮ menu (client.bend's `Fab.min.*`, `Fab.mini.*`; laws `fab_min_*`, test `test/fabmin_test.bend`).

  | Client | Card | Ready to order |
  |---|---|---|
  | Window | `Lay.fab` | `Lay.ready` |
  | Web | `View.fab` (sticky) | `View.ready` |
  | Phones | a queue row, Goal (`Mob.fab`) | a queue row, Ready (`Mob.fab.ready`) |

- **The Buy button's words** (`Card.buy`, laws `order_buy_*`):
  - "Buy it · from $X" with a price;
  - "Buy it" when estimates are off, or when there are boards but no fab files yet;
  - nothing when there is nothing to order or the server is out of reach.

## Buy threads

- **Starting one:** `order.buy {thread}` (hub.bend's `Buy.go`) makes a thread in the same folder, marked `buy.<thread>`, with the person's "Buy it" as its first message. A project's next one is "Buy: <project> · 2" (`Buy.title.of`).
- **What its agent gets:** the buy instructions and the tools `order_quote`, `order_status` and `order_ask`. Only buy threads have them (`Bots.buy.tools`). Their questions and the upload's approval alert the person in their own words, without the tool's name (notice.bend's `Alert.ask.say`, law `order_ask_alert_plain`).
- **The review gate:** a buy thread never reviews the design itself. It first asks `order_review {action: "check"}`; unless the review is current, the person gets two buttons (an `order_ask`): **Review for manufacturing** or **Order without a review**.
  - The review runs as a task of the buy thread (`delegate_task`): in the same folder, on the very files to be ordered, its result coming back as a message that wakes the buy thread.
  - Issues from a review stop the export until they are fixed (another task, in the same folder, then the review again) or the person says to order anyway.
  - The gate is the hub's, not the agent's (`order_review {action: "gate"}`; core/order.bend's `Order.gate*`, server.bend's `Od.gate.go`, `OrderGate`, `Hub.order.gate`; laws `order_gate_*`): a current, passed review answers "go on"; none or stale opens Review for manufacturing / Order without a review; current with issues posts them as bullets (the review's `issues`, else its summary cut at its semicolons) with the link `.backplane/review.md`, then opens Fix them first / Order anyway. Agents skipped listing the issues when told to.
- **The pilot:** while the order server's Stripe key is a test key (or `FAB_PILOT=1`; order/fabsrv's `config.PILOT`), Stripe's page, the quote and the page back from Stripe say that ordering through Backplane is a pilot, the card will not be charged (Stripe's test card 4242 4242 4242 4242 walks the checkout), and the exported files in `mfg/` can be ordered from any fabricator. The quote carries `pilot`, so the hub calls a paid test checkout "pilot" (`Order.state.pilot`): it says so in the thread, alerts no one, never shows as Ordered and is not watched further (laws `order_pilot_*`).
- **Nothing waits on the person:** `order_ask` and `order_quote` answer the agent at once and it ends its turn (an MCP call held open died after about five minutes, Node's fetch timeout, and the person's later tap went nowhere). An order question parks nothing: its answer is posted in the thread as the person's message, which starts (or queues) the agent's turn however long they took (hub.bend's `Hub.answer.order`), and a Stop or a crash leaves it open (`Retire.hit`; laws `order_ask_*`, test `test/orderask_test.bend`). The files' approval still parks, and once the person sends, the quote is posted and the agent gets a system message with its total (server.bend's `OrderWake`, `Od.wake`); Not now tells it too, and a retired run tells no one.
- **Manufacturing reviews** (`Order.prompt.review`, in every agent's project instructions): what makes a fab refuse a design or charge more, not the circuit. The findings go to `.backplane/review.md`; the verdict (ok or issues) and a line go to `order_review {action: "record"}`.
  - With Ordering on, `order_review` is every thread's tool (`Bots.order.tools`). The hub (orderio.bend's `Od.review`) hashes the design: the names and contents of every board, schematic, STEP, DXF and fab file outside `mfg/`, `extra/` and `.backplane/` (`Od.print`).
  - `record` keeps the hash with the verdict, the summary and the issues (one short line each, at most 8: `Order.review.rec.with`, laws `order_review_issues_*`) in `.backplane/review.json`; `check` answers none, stale (the design changed since) or current (`Order.review.state`, laws `order_review_*`).
- **`order_quote {dir}`** (server.bend's `Od.quote`):
  1. reads the manifest;
  2. refuses unsafe paths and folders holding links;
  3. opens an approval card listing every file ("Send these files for a quote?", Send / Not now);
  4. on Send, uploads them (curl, from the folder, by relative paths), polls the quote, and posts it as a message with its **Buy now · $X** link, which opens Stripe Checkout straight away (there is no quote page; the server only redirects).
- **The token** lives only in `<home>/orders/<quote>.json` and the Buy now link; the agent never sees either.
- **The watcher** (`Od.watch`, resumed at start) asks for the order's state every minute while unpaid and every ten minutes after. It says Paid, Ordered, Shipped (with tracking), Delivered, Refunded or Expired in the thread, once each. Each but Expired also alerts the person's devices, a note alert like any other (push, desktop, phones; `Hub.order.note`, laws `order_note_*`), unless they turned off alerts for settled work (`notify.settled`).

## The server

`order/` (Python): see `order/README.md` for the API, pricing, security and deployment.
- The app's default server is `Order.server()`; the `order.server` setting replaces it. `BACKPLANE_ORDER_SERVER` in the hub's environment replaces only the default (orderio.bend's `Od.at`): every test hub (skills/backplane-e2e/hub.sh, test/tools and desktop/test harnesses, scripts/test.sh, the smoke scripts) sets it to a dead local address, so with Ordering on by default no test reaches the real server; hub.sh also starts with Ordering off, and a story that orders sets `order.server` to its own stand-in.
- The fab's price models are a private pack (`FAB_VENDOR`), not in this repository.
