# Commerce audit

What the hub, the clients and the docs say about ordering, compared with what the order server in this repo (`order/fabsrv`) does now, and with the 0.14 tags (`v0.14.1`, `main-bend-final`).

Method: read `order/fabsrv/{app,catalog,package,pricing,estimate,store,pay,limits,config}.py` against `native/host/src/settings/{order,order_view,fab}.rs`, `present/ready.rs`, `agents/prompt_text.rs`, `docs/order.md`, `test/order/*` and the host tests in `agents/tests/order.rs`; ran `order/tests` (20 pass, in a venv from `order/requirements.txt`) and the host tests below. Only the repo's own server on loopback was used (no production endpoint, no payment).

The server in this tree is ahead of the 0.14 tags: `1f85acb19a` (US sales tax, the duty-paid parcel fee), `26e891f84d` and `3ad928c44d` (the pause switch `FAB_PAUSED`, `FAB_PAUSED_NOTE`). Whether the deployed server has them is not knowable from here (see "Needs upstream").

## Contract, endpoint by endpoint

| Server | Hub | Verdict |
|---|---|---|
| `GET /v1/health` `{ok, version, pricing}` | `fab.rs` `look_in`: only for a card with a KiCad board and no fab file | **Fixed**: `pricing: false` was read as healthy; the card now says "pricing is not available right now" (`health_got`) |
| `GET /v1/catalog` `{kinds, fee, fee_cnc, min_assembled}` | never called; agents get a hand-written list (`ORDER_PROMPT_FAB`) | **Fixed by a test**: the list had drifted (sheet metal `countersinks`, HDI thicknesses 1.4 and 1.8). Prompt corrected, and `fab.rs` test `the_agents_fab_json_contract_names_everything_the_order_servers_catalog_has` fails on any kind, option, choice or feature the catalog has and the prompt lacks, and on the order minimums (5 boards, assembly from 2) |
| `POST /v1/estimate` `{currency, from, parts[{name,kind,qty,from,lead_days,warnings}], label, note}` | `fab.rs` sends `{parts:[{name,kind,qty,options,features}]}`, reads `label`, per-part `from`/`problems`/`name`/`kind` | `lead_days` and `warnings` were dropped. **Fixed**: `fab.look` carries `lead_days` and `estimate_warnings`. Not drawn by any client yet (open) |
| `POST /v1/quotes` (multipart `manifest` + `file`; 202 `{id,token,status,poll_after}`; 400, 413, 429 with `{error}`) | `order.rs` `upload`: `X-Backplane-Install`, manifest and files by relative path, 300 s; error text shown to the agent | matches. The hub ignores `poll_after` (polls at 3 s, then 30 s; the server's read budget is 1200 an hour per address and per install, so one quote is far inside it) |
| `GET /v1/quotes/{id}` `{id,status,expires,country,pilot,quote,pay_path,order,error}` | `order.rs` `get`, `order_view.rs` `state`/`payable`/`pending`/`sum` | see the two bugs below; `pilot`, `shipping.ddp`, `pay_label`, `markdown`, `problems`, `warnings`, `order.{status,tracking}` all read as the server writes them |
| `/q/{id}?t=` Buy now (redirect to Stripe; paused page when `FAB_PAUSED`) | `order.rs` `pay_of`, `pay_link` | matches. The hub cannot know the server is paused before the person opens the page (open, needs a server field) |
| statuses: quote `received analysing ready failed expired`; order `checkout paid placed shipped delivered refunded cancelled` | `order_view.rs` `state`, `done`, `every`, `status_text` | all handled (`cancelled` falls back to the quote's status; pilot turns `paid` into `pilot`) |
| manifest `spec 1`, `ship_to` ISO, `ship_region` (US only), `assemble`, file roles | buy prompt step 3 and 4; approval text "Ships to US (GA)" | matches, and the review hash is unchanged from 0.14 (`fingerprint`, tested against the shell one) |

## Findings

1. **A transient server answer ended the wait for a quote (hub bug, fixed).** `order.rs` `poll` stopped on any error other than "unreachable": a 503 from nginx while the server restarts, or a 429 from the read budget, posted "The quote could not be worked out" to the agent although the quote was still being made. `est_got` now keeps the HTTP code (`http`), `transient` names 000, 429 and 5xx, and `poll` goes on for those. Test `a_busy_or_restarting_server_does_not_end_the_wait_for_a_quote`.
2. **A quote with status `failed` told the agent it was posted (hub bug, fixed).** The server answers `{status:"failed", error}`. `quoted` posted nothing (no markdown) but woke the agent with "The quote is posted in this thread… NO Buy now link: it cannot be paid until its problems are fixed", with no problems and not the server's error. Now the note is "The quote could not be worked out: <error>…"; `expired` has its own note. Test `a_quote_the_server_could_not_work_out_says_why_and_posts_no_link` (also checks the card state `failed`).
3. **`fab.look` was missing what the estimate says** (fixed, data only): per-part `lead_days` and `warnings`; see the table.
4. **The health answer's `pricing` flag was ignored** (fixed).
5. **Agent instructions had drifted from the catalog** (fixed, with the contract test): sheet metal's `countersinks` feature, HDI thicknesses 1.4 and 1.8. Fixture `test/agents/fixtures/argv-default.json` carries the prompt and is updated.
6. **`docs/order.md` was stale** (fixed): the info keys listed `fab.open` (nothing sets it; the code keeps `fab.boards`, `fab.board`, `fab.again`); it described web chips (`View.boards`) and a web "Sheet"; it did not list the card's keys; its Bend symbol names (`Od.go`, `Order.card`, laws `order_*`) had no pointer to where those rules live now. Corrected, with a short map at the top. Still: the prose keeps the Bend names as search keys into `v0.14.1`.
7. **The Ready to order bar is gone on purpose** (the person's decision, row in `docs/PARITY.md`). `fab.look`'s answer still matches 0.14's twelve keys one for one (`fab.rs` test asserts the key list), so what remains of the PARITY row "Card and Ready to order data" is the open items below, not a missing key.

## What was checked and is fine

- Order tokens: only `<home>/orders/q_*.json` (0600) and the drawn link; the log holds `order:<id>`; tested against the real server.
- Pilot: `pilot` on every quote answer; `paid` becomes `pilot`, says "your card was not charged", alerts no one, is not watched, never shows as Ordered.
- Customs line from `shipping.ddp`; the four-line quote with "Sales tax (estimated)" (rendered by the server's markdown; checked in `test/order/buy_e2e.py`).
- Review gate and hash: `order_review` check, record and gate, 8-issue cap, fingerprint equals the 0.14 shell pipeline.
- Rate limits: estimate budget 1500 an hour: the card asks on thread select and turn end only.
- Copy hubs send nothing (`copy_probe`).
- Server tests (`order/tests`, 20) pass.

## Open, and why

- **No client draws `lead_days` or `estimate_warnings`.** Adding a line to the Buy it tooltip or the card is a UI decision on three clients (GPUI, iOS, Android); left for the person to choose.
- **The hub cannot see a paused server** until Buy now is opened. A `paused` flag on `GET /v1/quotes/{id}` (and `/v1/health`) would let the quote and the card say it. That is a server change (`order/fabsrv/app.py`), not a client one; not made here because it changes the deployed contract.
- **`poll_after` is not honoured.** Harmless today; if the server ever slows it, the hub should follow it.
- **No live run of the phones** for the changed paths (the changes are in the hub's data, which all clients draw unchanged).

## Needs upstream (not guessed)

- The deployed server's version and environment: whether it runs the post-0.14 commits above (sales tax, `ship_region`, pause switch), whether `FAB_PILOT` is on or `STRIPE_KEY` is a live key, and `FAB_PAUSED`. `GET /v1/health` reports a `version`, but asking the production address is out of bounds here.
- `DEFAULT_SERVER` is `https://2.29.106.88` (`fab.rs`): a bare IP, which the server's README makes possible with a short-lived Let's Encrypt IP certificate (`lego … --profile shortlived`). Whether that address and certificate are still what production uses is for Bub to confirm.
- The real fab pack (`FAB_VENDOR`, private): its `min_qty`, option labels and price models decide what `/v1/catalog` returns in production. The contract test and the host's prompt use the stand-in pack's catalog only where `catalog.py` itself defines it; a pack that raises `MIN_QTY` ("a pack can raise these") would need the prompt's "at least 5" changed, and only the real pack shows that.
