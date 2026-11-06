# gpui landing exceptions

`gpui` moves only through `scripts/gpui/land.sh`. An exception is listed here, with who granted it, so an audit does not flag it.

| When | What | Granted by | Reviewer OK | Pushed by |
|---|---|---|---|---|
| pending | One merge of `main` at the port target (`docs/gpui/port-target.json`) into `gpui`. A single merge commit, pushed by the `gpui-merge` lock holder, read and OKed by merge tree hash, compiled and crate-tested in a warm sandbox first. Conflicts, build or test failures stop it. | coordinator, 2026-11-05 | | |
