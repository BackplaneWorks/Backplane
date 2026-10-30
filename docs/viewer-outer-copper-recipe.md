# Full outer-copper export recipe

Full original-model exports now include the saved outer tracks, zones and via body holes. Compact exports retain their existing pads/mask/silk recipe, cache identity and native plot overlay behavior. The existing `full` Boolean selects the recipe; no new RPC/CLI/schema parameter is introduced.

`Pv.glb.flags(bare, full, out, path)` retains the original base arguments. Only full=True adds `--include-tracks --include-zones --cut-vias-in-body`. Bare/parts still selects `--no-components`/`--subst-models`. No inner copper, material override, coordinate offset, via filling or shape fusion is added. The selected bp.4 CLI help confirms all three options. RenderAsset already requests full=True; compact kind2 remains False and kind4 remains True.

The existing `Pv.board.out(path,bare)` and its parts3/bare3 exact-revision filenames are unchanged. `Pv.board.out.recipe(path,bare,False)` calls that original function. Full=True uses:

```text
board-full-outer-v1-parts-<path-hash>.<exact-source-stamp>.glb
board-full-outer-v1-bare-<path-hash>.<exact-source-stamp>.glb
```

The recipe and parts/bare mode are explicit filename namespaces. Old pads-only GLBs cannot satisfy a full request. The existing `Pv.cache.revision` places the exact source stamp in the filename without hashing it; same-size writes within an mtime second remain distinct. Future changes to the full recipe must bump its version. `Pv.solid.out` passes the existing full flag only for board exports; raw GLB and STEP paths remain unchanged. Referenced model dependencies remain outside the main-board revision stamp, as before.

Existing success-only staging promotion is unchanged: nonzero export status never renames `.part.glb`; successful status must also complete the rename. Missing staging files or rename errors remain failures and preserve a previously published file. Private immutable-asset authorization, source/revision checks and content proof are unchanged by this delta.

The focused probe imports the actual server functions:

```sh
bend test/native/viewer_copper_probe.bend -o /tmp/viewer-copper-probe.js
bun test/tools/viewer_copper_recipe_test.mjs /tmp/viewer-copper-probe.js
```

It exercises compact/full parts/bare flags and cache paths, equal-size/same-mtime-second saves, GLB/STEP behavior, an existing compact cache that cannot satisfy full detail, failed-export staging, successful promotion, missing-stage failure and a nonempty-target rename error. Optional explicit `COPPER_BOARD`, `COPPER_KICAD_CLI` and `COPPER_RECEIPT` run the selected CLI using the probe's actual production flags and paths against read-only saved CAD; all outputs/cache are temporary.

The actual SMU paired export preserves PCB SHA256 `d2f84cd9ce0661d2835a64339d9b1b6ec992111ee778b2c4564bc30ee037733a` and schematic SHA256 `278c206f54faff1c95a6d5aa51b7fb01d777486a173b1b8138d4575302d07105`, with exact filesystem stamps unchanged after each export. Compact has76,939triangles/89,781POSITION and NORMAL vertices; full has311,551triangles/351,351POSITION and NORMAL vertices. Full adds `smu_card_copper` and `smu_card_via`; every one of94 existing component/pad/mask/silk mesh attribute/index SHA256 hashes is identical. The substrate changes because via bodies are cut. All245 source via centres are blocked by the compact substrate top and open in the full substrate. The test also checks their full-export bottom openings. No textures or physical manufacturing-joint geometry are synthesized.

The first production-probe pair took about1.44s compact/4.21s full in this environment, at3.707/18.133MB; serialized metadata/filenames cause small byte-size differences from earlier `/tmp` exports with identical geometry. These are one-run export measurements, not startup or frame-rate guarantees. Full detail contains22,357 primitive meshes before batching and exceeds the earlier illustrative-solder global geometry budgets. Renderer batching, source identity preservation and interactive performance are separate owner qualification. This exporter delta must not be described as making a dense original model instant.

Handoff uses a minimal patch against root plot SHA256 `1a519d0e8cd2a47356116998ccc9ab73e81430e443d4f9f659810283b0ec802b`. The private snapshot commit of that root blob is not an integration change. The feature diff changes only the board flags/cache-selection functions; unrelated authorized source/revision/promotion work is preserved. No installed app, CAD, common server/RPC/CLI files, broad app build or global proof was changed/run for this feature.
