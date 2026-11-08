# Third-party notices

Backplane itself is MIT-licensed (`LICENSE`, here). This folder holds the
notices for everything else in this archive, one folder per component.
`scripts/licenses.sh` in Backplane's repository builds it for each release
(release.yml), and a release that lacks any file listed below is refused.

| Folder | What | Where it is in the archive | Licence |
|---|---|---|---|
| `rust/` | The Rust crates compiled into each program: `hub.txt` (backplane-hub, bp-sh), `client.txt` (backplane-gpui), `guard.txt` (backplane), `voice.txt` (backplane-voice). Made at release time by cargo-about for this platform | the programs | MIT, Apache-2.0, BSD and others, each text in full |
| `gpui/` | gpui 0.2.2 by Zed Industries, vendored with the changes in `PATCH.md` | backplane-gpui | Apache-2.0 |
| `bun/` | The Bun runtime, with JavaScriptCore/WebKit and tinycc, in the two programs built with `bun build --compile` | backplane-browser, backplane-step2glb | MIT; LGPL-2 and LGPL-2.1 (see `NOTICE.md`) |
| `playwright-core/` | Playwright's core library, bundled into the Browser tab's helper | backplane-browser | Apache-2.0 |
| `step2glb/` | OpenCascade and occt-import-js, compiled to WebAssembly and embedded in the STEP mesher | backplane-step2glb | LGPL-2.1 |
| `cef/` | Chromium Embedded Framework (`LICENSE.txt`) and Chromium's own credits for every library it carries (`CREDITS.html`). Linux only | libcef.so and its files | BSD-3-Clause; Chromium's are listed in `CREDITS.html` |
| `silero-vad/` | Silero VAD, the model dictation finds speech with | silero_vad.onnx | MIT |
| `voice-engine/` | Dictation's local engine (sherpa-onnx, ONNX Runtime, the Parakeet models): downloaded when you turn it on, not shipped | not in the archive | Apache-2.0, MIT, CC-BY-4.0 |
| `fonts/` | JetBrains Mono (`OFL.txt`), Spleen (`SPLEEN-LICENSE`) | assets/fonts, and compiled into backplane-gpui | OFL-1.1, BSD-2-Clause |
| `icons/` | Lucide's icons (`lucide-LICENSE`), drawn from assets/icons.json; simple-icons' provider marks (`simple-icons.md`) | assets/icons.json, assets/providers | ISC (and MIT for the Feather icons); CC0-1.0 |

Settings → About in the desktop client shows where this folder is on your machine.
