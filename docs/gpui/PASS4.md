# Bub parity pass 4 (2026-11-06): what was tried in a real window

Real GPUI window on a private Xvfb, real XTest clicks and typing, a throwaway hub with stand-in agents (synthetic history).

| Area | Result |
|---|---|
| Settings dialog | Hub pages ported in `present/settings.rs`: Appearance (theme), Threads (grace, restart, attachment size, webhook, notify), Agents (provider, model, thinking), Ordering, Network and updates (reports what the hub says), Time zone, About, Privacy. Chips send `setting.set`; fields send on Save or Enter. Not yet: Writing, CAD, Devices, Voice, API keys, Usage, Antigravity, provider setup and fallback, Your machines, QR code. Accounts left to `gpui-accounts-allowance`. |
| Sidebar | Row menu (pin, settle, archive, rename, delete with ask), add and remove project, new thread: worked. Search did not filter and Enter did nothing: fixed. Quiet projects had no way back: an Older row now opens them. Active view (`sidebar.view=active`) is still not drawn: the setting alone shows the projects list. Reorder (drag) not tried. |
| Thread | Steer while busy and Stop worked. Model, effort and provider chips did nothing (and the model list held only "default"): fixed. Fork, Back to the original and a view after any thread move failed (the hub's `fab.look` answer sent the view to the missing helper): fixed. Fork does not yet open the fork in every path. `&model` inserts text, not a chip, and does not change the model (as asked). No provider logos, favourites or search in the model menu. Commands (rail) is disabled. |
