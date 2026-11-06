# Thread view: known missing (degraded keys, not refused screens)

Rule (lead, 2026-11-05): a gap degrades the key, not the screen, unless showing the screen would be wrong or unsafe. Each degraded key is listed here until it is ported.
State at gpui 9e0c089fad. Everything below is checked against the helper where a dump exists (`test/slim/dump_variants.mjs` -> `fixtures/thread-variants.json`).

## Drawn, but a key is degraded

| Key | Shows now | Missing | Source in |
|---|---|---|---|
| `skills`, composer popup rows for a draft whose last word starts with `$ @ > % #` or `&` | `[]` (no popup) | the Refs rows: ranked threads, projects, rooms, bots, skills, model mentions | `U.Refs.offer`, `Skills.of`, `Refs.refresh` |
| `picker.models` | the thread's own model only | the hub's models list and labels (`models` info) | `U.Models.phone` |
| `boat` | `""` | the worktree chip | `UZ.Chip.of` |
| `parent`, `subs` | `null` | "launched by" row; delegated tasks and Claude's background shells | `Mob.parent.loc`, `SB.Subs.rows` |
| `queue`, `queued` | `[]`, `""` | context handed over above the composer; the order card of a buy thread | `Mob.queue`, `U.Fab.*` |
| `todos` | `null` | the agent's to-do list | `U.Todos.shown` |
| `order` | `null` | a buy thread's Order card | `Mob.order` |
| reply `images` | paths a reply names (newest 30 replies) | none known | `Mob.imgs.seen` |
| a reminder, terminal or review row | a plain activity row | their own rows (`kind` reminder / emb / review) | `Mob.remind`, `Mob.act.k`, `RVW.Review.*` |
| an older stopped/failed turn's reply stamp | by the marks the lean state keeps | none known | `Stamp.stops` |
| `viewer`, `voice`, `diff`, `term`, `embs`, `btw` | the template's idle values | any open viewer panel (the dispatcher already refuses an open panel), dictation, terminals | `VW.View.json`, `Mob.voice` |

## Refused (retryable "not ported", fixed labels)
`thread.missing`, `thread.deleted`, `thread.title-empty`, `setting.effort|wait|stop|fail|files.max.mb|permissions.default`, `state.gate-key-not-retained`, `thread.ask-buy`, `thread.ask-browser`, `state.buy-not-retained`, `thread.stalled`, `page.*`, `row.malformed-or-foreign`, `composer.language` (a language other than English).

## Not mine
`answer` and the other ask buttons (parity-session), the sidebar (root), Files/Mech replies and the viewer panel (open: next).
