# Thread view: known missing (degraded keys, not refused screens)

Rule (lead, 2026-11-05): a small gap degrades the key, not the screen, unless showing the screen would be wrong or unsafe. Each degraded key is listed here until it is ported.

| Key | What it shows now | What is missing | Source in Bend |
|---|---|---|---|
| `skills` / composer popup rows for a draft whose last word starts with `$ @ > % #` or `&` | `[]` (no popup) | the Refs rows: ranked threads, projects, rooms, bots, skills, model mentions | `U.Refs.offer`, `Skills.of`, `Refs.refresh` |
| `skills` (the hub's skills list for `$`) | `[]` | the hub's skills info | `U.Ui.skills` |

Still refused (None, retryable "not ported"): running or failed turn, pinned/archived/snoozed/settled-marked thread, non-claude provider or model, task links, open asks, queued messages, effort/wait/stop/fail settings, a zone other than UTC, images/attachments/API-error/restart/reminder rows, a non-English language, a non-empty draft until `session::composer::fields` is bound.
