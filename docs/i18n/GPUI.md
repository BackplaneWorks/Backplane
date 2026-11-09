# The GPUI client's language

`native/gpui/src/i18n.rs` reads `i18n/en.msg` and `i18n/ja.msg` (compiled in). The device's choice (Settings → This device → Language:
Automatic, English, 日本語; `local::Local::lang`, never sent to the hub) is applied with `i18n::set_choice`; Automatic follows the system
(LC_ALL, LC_MESSAGES, LANGUAGE, LANG, else macOS AppleLanguages, else the Windows user locale). Nothing in the window needs a restart.

How a string reaches the screen:

- **English written in the code, or sent by the hub for display** goes through `tr(text)` / `trs(text)` at the place it is drawn. The catalog is
  searched by its English words: an exact message, or a message with `{args}`, `plural` and `select` matched as a template and rendered in
  Japanese with what it captured (`tr("12 threads")` finds `thread.count`). Text the catalog does not know (agent words, titles, file names)
  comes back unchanged, so chrome and content can share a widget as long as only the chrome is passed through `tr`.
- **A message by key**: `t("settings.lang")`, `tf("gpui.keys.fixed", &[("key", k.into())])`. Use it where one English text has two meanings
  (the reverse lookup picks the first key in order) or where the arguments are not worth a template match.
- Never translate: ids, action names, registered control labels (`ctl` entries and `ui:` ids are what the e2e drivers press), strings compared with hub data,
  log lines, file names, thread titles, agent text, URLs, and what is sent to the hub (a thread's title made by the client stays English).
- Plurals: write the ICU `plural` in both catalogs; Japanese uses `other` only.

New entries go in the section of your lane in both `i18n/en.msg` and `i18n/ja.msg` (`# ---- gpui-x: ... ----`), through
`scripts/i18n-add.py <section> gpui < entries.tsv` (key, English, 日本語 per line; key `-` makes one from the words; an English text already in the
catalog is not added twice). `python3 scripts/i18n-check.py` is the catalog gate (every key in ja, same arguments); the client's tests run it too
(`i18n::tests::every_key_has_a_japanese_entry_and_the_same_arguments`).

Tests that keep it so (`native/gpui/src/i18n_scan.rs`):

- every `tr("..")`/`trs("..")` literal in the source is a catalog message (or template) with a Japanese entry; every `t("key")`/`tf("key", ..)` is a key in both;
- no file outside `PENDING` has an English screen string that bypasses the catalog; a literal that is not for a person says so with a
  `/* i18n-ok: why */` at the start of its line. `PENDING` only shrinks: a lane removes its files when it has moved them.

Rig: `BACKPLANE_GPUI_I18N_MISSES=<file>` appends each English text `tr` could not find (in Japanese) once; walk the screens and read it.
The font is Noto Sans Mono CJK JP by default (`local::FONTS`), with the system's CJK fallback behind JetBrains Mono, so Japanese needs no bundled font on
Linux (fontconfig), macOS (Hiragino) or Windows (Yu Gothic / Meiryo); a Linux machine with no CJK font shows tofu: the packages recommend fonts-noto-cjk.
