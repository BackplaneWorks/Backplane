# Pairing theme correction

Pairing and Devices now inherit Backplane's existing light/dark background,
text, accent, border, button and focus styles. Removed the green rule and
separate branding. Field/primary/on controls use the app's shared classes;
QR modules share the existing QR contrast rule. Browser theme-colour metadata
also reads the background token.

Pairing link, QR code and Short code are equal tabs, with arrow/Home/End
keyboard navigation. The QR tab exposes an app-styled image chooser; the
native file input is hidden. Device names derive from the advertised platform
and browser (or Desktop), stay editable and require no permission/network call.
The ASCII separator preserves the existing credential-exchange header format.
No enrollment, invitation/session security or persistence policy changed.

Validation on base 12530359d:

- Production web compilation passed.
- Actual pairing/device UI and app CSS: before/after at 390x900 and 1280x900,
  light and dark. Sixteen comparison PNGs plus eight QR/code detail PNGs.
  Verified computed theme colours, equal visible choices, keyboard navigation,
  custom file chooser, no horizontal overflow and submitted edited names.
  Screenshot fixtures contain only a non-secret example invitation.
- 36 real private-hub/browser pairing checks passed, including link, local QR
  image and code controls, HttpOnly cookies, expiry/reuse rejection, revoke,
  automatic re-pair and credential-free logs. Transport regression used the
  cached server emit below with its fixture clock; no new server build is
  claimed. The tested web bundle is freshly built from this change.
- 15 focused unit cases and all 28 existing token filesystem contracts passed.
- Laws shape, sidebar source guard and whitespace checks passed. No Bend file,
  law, proof, sidebar input receipt or mobile native source is changed.
- Required `bend PROOF.bend` on h@backplane under its bend lock failed with
  `Error: the machine stack overflowed (a deep recursion, or a literal too large
  to expand)`. This run did not reach a named law failure and does not qualify
  the broad proof. Its Bend input bytes are unchanged from the source baseline.
  No proof was weakened or deleted.

Local heavy job (serial web/screenshots/browser):
`w-t1793665018-1293799-1324797`, exit 0. Its initial timeout included time in
the lock queue; the existing job completed, with no concurrent local heavy job.
Remote proof job: `w-t1793665018-1293799-1324799`, exit 1. No Boat credit used.
No live hub state, device pairing or credential was modified.

Generated evidence remains under `build/pairing-theme/`: screenshots/, the ZIP,
screenshot-results.json, source-inputs.json, evidence-sha256.json and logs.
These are scoped web/browser checks; no fresh Electron/native phone runtime,
physical-camera qualification, signed phone release or live install is claimed.

Cached server emit: `/home/h/.cache/backplane-bend/emit-js/b0b27424c0d8c76f897d9301d009ab2eaac60a19.js`
SHA256 `23accb368fd6e10358a745220dec980bf78b089071603545a3e878cd5300a43d`.

Evidence SHA256:

- `web-build.log`: `2029c019562f11119fe1073168d7c9e87ff1c185381c8808e534578c8edffd6a`
- `e2e.log`: `aca63412132d0903c85ef60e72f0ada57567de8395296cd2c2a0c40665f92c51`
- `screenshots.log`: `044e2697f7001ae89b686c93d18d8014c38d5650bc5cb4e38ad1e8571fb2a658`
- `proof.log`: `88b5ff28ca821db96c8882b805d31ee48fcb8e97d63b4c988cbfd4ee3d53677c`
- `screenshot-results.json`: `9fb0b6466dfe5d06067d7691541fd3bf80e04c20fe00a146800a5aec998c87ca`
- `source-inputs.json`: `05e854f2bacf41c5add7f43c555aee6db200fd02535f77f45943249f400f0130`
- `pairing-screenshots.zip`: `da91cdbc6458e308c218f1739d3c041bd998c42df27572d7c4c554e5edcd3419`
