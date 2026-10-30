# Bounded saved-component pages

Actual SMU metadata contains96 components. Compact UTF-8 response with no schematic is219406bytes; including the actual root schematic is860038bytes before both final revision fields. Symbol previews contribute641014bytes, footprint graphics121029, pads55854 and model references11526. U1 is122514bytes, of which114322 is its real instantiated symbol. Dropping symbols or increasing a limit would conceal this transport problem.

The unchanged server cap is220000UTF-8 bytes including the newline. Pure `Components.page` and server `Components.page.child(cwd, offset:Nat, expectedPcbRevision:String, expectedSchRevision:String)` send complete component rows within a180000byte row budget and at most32 rows. All original metadata fields are repeated, with `components` replaced by the current subset and this field added:

```json
{"page":{"offset":0,"next":20,"total":96,"complete":false}}
```

At the end, `next` is `null` and `complete` is `true`; an empty board has offset0,total0,nextnull,completetrue. Otherwise next equals offset plus the returned nonzero row count. An oversized individual row is an explicit error, never shortened. Offset must be within0..total and total at most4096. The final server wrapper is independently counted and bounded, so unusually large repeated headers also fail explicitly. The legacy single-response child retains its cap.

First page: offset0 and both expected revision strings empty are allowed. Continuations require the exact nonempty PCB revision and the exact schematic revision from the first page. An empty schematic revision matches only an actually missing schematic. Existing read-only canonical-file resolution,16MiB-per-input caps and before/after read stamps remain in force. Each page reparses the saved source; it does not retain an unbounded child cache.

The root owns CLI/RPC/UI integration: `--review components-page DIR OFFSET PCB_REV SCH_REV`, then sequential `kicad.components` calls with numeric `offset` and string `sourceRevision`/`schematicRevision`. An aggregate must remain unpublished until complete. Require identical schema, source, schematic, both revisions and fingerprint on every page; exact offset/next/total chain; unique UUIDs; bounded aggregate bytes/page count/deadline. A save or error discards the entire aggregate and restarts. Intended client bounds are4096 components,8MiB and256 pages. Those client bounds are an integration requirement, not implemented by this pure/server-only change.

Large-symbol testing found that generic `List.length` on an encoded UTF-8 byte list overflows the emitted JavaScript stack. The new tail-recursive `Components.utf8.bytes` counts scalar widths without constructing another byte list. Both page sizing and the existing server response cap use it; the cap is not raised.

Actual emitted parser/pager reconstructs all96 saved SMU rows exactly, including all symbol/pad/graphic fields. With both exact filesystem stamps, the full response is860123bytes. Five bounded pages, including envelope/newline, are176925,179487,155513,175037 and176396bytes, at offsets0,20,43,60,69; next values20,43,60,69,null. Full parse plus five pure page operations takes about2.78s in the isolated test; repeated child parsing/HTTP timing belongs to integration. Input PCB and schematic stamps remain unchanged. Receipt: `/tmp/backplane-solder-evidence/pages-receipt.json`.

`test/tools/components_pages_test.mjs` uses the actual emitted Bend module. It checks complete reconstruction, UTF-8 budgets, both revision stamps, schema/offset/count limits, empty boards, missing schematic and oversized indivisible rows. Optional `COMPONENT_SERVER_MODULE` also calls the actual emitted server response function for exact boundary and Unicode cap cases. Existing parser tests pass. The pure module reports `ALL PROOFS CHECK`; the server emits successfully and its check-only result lists expected foreign-dependent definitions. This is not a global proof or normal-app acceptance result. No common CLI/hub/web files, installed app or CAD files are changed by this paging module.
