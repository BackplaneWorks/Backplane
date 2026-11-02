# Browser rework

The native connection can be ready while Browser stays at “Connecting Browser…”.
An isolated Electron reproduction found a hidden `.model-menu` in a retained
thin-view picture. The renderer treated its DOM presence as an open overlay,
never presenting the native guest. Only visible overlays with nonempty viewport
geometry now block presentation; every matching overlay is checked so an old
hidden menu cannot mask a current modal.

Startup failures before controller creation now produce queryable error state
and an explicit Reconnect action. Presentation replies cannot overwrite a newer
controller notification. A connected browser without tabs gives an address hint.
No guest operation is replayed during recovery.

The Browser chrome uses a compact ownership row, individually labelled icon
buttons, tabs, and an address bar. The takeover/handback actions and owner values
are unchanged. Coarse pointers receive larger targets.

## Sidebar draft dependency review

The only changed Bend module is `src/web/bots.bend`. Changes are restricted to
`WB.browser.drive.loc`, `WB.browser.surface.k`, `WB.browser.tabs`,
`WB.browser.controls` and the new presentation helper `WB.browser.button`.
No draft reader was added or changed. Browser controls still use the existing
`U.Browse.field/address/tabs`, drive state and `@desktop.browser` gate. The helper
accepts strings and renders a button without reading `Ui`. Sidebar dependencies,
input filtering, unknown scratch retention, cache keys and authority are
unchanged. The conservative source pin must be refreshed only after the existing
24 sidebar input/full-page equivalence assertions pass on these exact sources.

## Validation

`build/browser-review/check.log`: full `scripts/check.sh` completed, including
repository acceptance of the foreign-effect proof verdict, 292 seconds. This is
not an independent kernel verdict. No laws or proofs were edited.

`build/browser-review/browser-tests.log`: all 55 focused Browser Node tests pass.
The candidate renderer also presented an actual native page after being injected
into the installed web bundle in a private Electron fixture. That targeted result
is in `build/browser-review/renderer-fixed-result.json`; it does not qualify the
new chrome or a candidate full build. Status updates preserve the managed text
node used by the DOM patcher to avoid duplicating the placeholder.

`desktop/test/browser-panel.cjs` exercises the actual native guest against a
controlled local page in a temporary home. Its candidate runtime result and the
sidebar equivalence/build results are pending.

The broader desktop suite currently reports 721/733 passing, with failures in
archive, bot-tab, notifications, focused-cache, image, improvements and release
fixture checks. Those are outside the changed browser paths; no blanket suite
pass is claimed.
