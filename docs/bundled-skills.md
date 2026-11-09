# Backplane's own skills

Besides KiStack's KiCad skills (`deploy/kistack`, pinned by `deploy/kistack.lock`), Backplane ships skills of its own,
kept in `deploy/skills/<name>/` and compiled into the hub (`native/host/src/agents/bundled.rs`, `FILES`).

- **Install:** at start the hub writes them into its home as `<home>/bundled`: a Claude plugin
  (`.claude-plugin/plugin.json` named `backplane`, `skills/<name>/...`). A file is rewritten only when it differs, and
  a file an older build shipped that this one does not is removed. Nothing is fetched at install.
- **Claude** gets `<home>/bundled` as `--plugin-dir` and `--add-dir`, like KiStack: the skill loads natively, `$name`
  in a message becomes `/name`.
- **Every provider's prompt** has a `<backplane_skills>` block naming each bundled skill and its SKILL.md
  (`prompt::bundled_skills`), so Codex, Grok and the rest read it before the work it covers. `$name` names the file to
  them as for any other skill.
- **The `$` popup** lists them under Backplane (`skills.rs` scans `<home>/bundled/skills`).
- **Adding one:** put it in `deploy/skills/<name>/`, list every file in `bundled.rs` `FILES`, test it (the hub's tests
  should run its own tests, as `agents::bundled` runs sendcutsend's).

## sendcutsend

Designs parts for SendCutSend and prepares them for ordering: laser, waterjet and CNC-router cutting of sheet metal,
wood (MDF, plywood), acrylic and composites; bending, tapping, countersinking, PEM hardware, deburring, finishes; file
rules (DXF/STEP, units, closed contours, no text as a font, kerf, minimum hole and web, bend relief, flange minimums,
K-factor, part-in-part). Its SKILL.md is the reference; an agent invokes it with `$sendcutsend` (Claude:
`/sendcutsend`), or reads `<home>/bundled/skills/sendcutsend/SKILL.md` when the work calls for it.

- **Live data only.** `scripts/scs.py` fetches SendCutSend's three files for agents at use time: the ordering guide
  (`cdn.sendcutsend.com/specs/sendcutsend-ordering-guide.md`), the catalog and the specs
  (`app.sendcutsend.com/sendcutsend-catalog.json`, `sendcutsend-specs.json`). Cache in `~/.cache/backplane/sendcutsend`
  (`SCS_CACHE`): younger than `SCS_TTL` (6 h) used as is, older revalidated by ETag / Last-Modified, a broken answer
  never replaces a good copy, offline (`SCS_OFFLINE=1` or no network) the old copy is used and every report says
  `OFFLINE` and its age. Requests are a second apart, as the guide asks. No price or limit is written in the skill.
- **DFM:** `scs.py dfm PART --sku SKU [...]` on a DXF (read by the helper itself) or a STEP (measured by FreeCAD,
  `scripts/scs_step.py`), with `--flat` for a bent STEP's flat pattern. Rules: material in the catalog and in stock;
  services and finish offered; thickness of the model; units, text, blocks, splines, closed contours, one part,
  part-in-part; min/max part size (instant vs custom quote) and each service's flat size; min hole, slot width, web,
  hole to edge; per bend: spans the part, length, angle, flat and formed flange, hole distance, inside radius; PEM
  hardware hole, sheet range and edge distance; tap range and edge; countersink minor/major/edge. Each row is
  PASS/FAIL/WARN with its source (`specs <SKU> <section.field>`, `catalog ...`, `ordering guide, <section>`, or
  `skill (<why>)`). Exit 0 none failed, 1 one failed, 2 could not check.
- **Fab file:** `scs.py fab PART --sku SKU ...` writes `<part>.fab.json`: `kind` sheetmetal (metals) or cnc (router
  and laser-cut plastics), options mapped to Backplane's catalog where an equivalent exists, measured features from the
  flat pattern, and a `sendcutsend` block (SKU, services, finish, hardware, taps, countersinks, the flat DXF, what has
  no equivalent in `unmapped`, the DFM result with the sha256 of the files it checked). See docs/order.md.
- **Tests:** `test/skills/sendcutsend/test_scs.py` (run by the hub test `agents::bundled`): the helper against trimmed
  recordings of the three files and a fake network (download, TTL, 304 revalidation, a changed file, offline age, no
  copy, a broken answer, offline mode, request spacing); DFM on the sample bent bracket and MDF panel and their broken
  variants; the limits following the specs; the fab files; FreeCAD's STEP of `examples/bracket.py` when freecadcmd
  is installed. `SCS_LIVE=1` adds one live fetch and revalidation of the real files.
  `test/skills/sendcutsend/make_fixtures.py [--record]` rewrites the fixtures.
