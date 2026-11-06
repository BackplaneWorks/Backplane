# GPUI Files tab: parity with the main-branch web / Electron client

Reference (the Bend sources, removed from this tree; tag `main-bend-final`): `src/core/client` (`Fm.*`, `Pdf.*`), `src/web/view` (`View.fm.*`), `src/core/fm`, `src/mobile/view` (`Files.json`, `File.json`),
main commits since `7c9b3c223c` (#960 thread-folder boundary, Files listings kept hub-wide, `String.drop`). GPUI: `native/gpui/src/screens/viewer.rs`
(+ `filesui.rs`), hub side `native/host/src/files/*`.

Status: **web** = what main ships; **gpui before** = this branch before the work; **gpui now** = after (`done`, `partial`, `no` with why).
The web Files tab has no upload, create, rename, delete or filter; those rows are the task's asks beyond main, marked "new".

| # | Feature | Web (main) | GPUI before | GPUI now |
|---|---------|-----------|-------------|----------|
| 1 | Folder rows, folders first, size detail | yes (`View.fm.rows`) | yes, capped at 1000 rows, no scroll state | done (5000-row cap, "… N more" says so) |
| 2 | Breadcrumbs (root / a / b) | yes (`fm-cd`) | yes (plain text) | done; plus Up button (`fm-up`), `Backspace` |
| 3 | Scroll a long list | page scroll | one scroll pane for the whole viewer | done; the list is its own scroll pane (5000 rows, filter beyond that); keyboard moves scroll the selection into view (`ScrollHandle::scroll_to_item`; not screenshot-verified) |
| 4 | File-type icons (folder, board, schematic, 3d, image, pdf, file) | Lucide icons by `Fm.icon(kind)` | `▸` / `·` | done; glyph + colour per `icon` the hub sends |
| 5 | Open board / schematic / STEP in the right viewer | `fm-open` | hub does it; row click sends `fm-open` | done (unchanged, verified in e2e) |
| 6 | Preview image | `<img>` via `/img` | "Opening…" | done; `viewer.file` drawn through the images cache |
| 7 | Preview PDF | iframe on `/img` | "Opening…" | done; page 1..n rendered by `pdf.page` (poppler), prev/next |
| 8 | Preview text / Markdown / csv / json / log | `text-preview.js`, `Md.render` | "Opening…" | done: text in mono (256 KB cap); a `.md`/`.markdown` note is rendered by the hub (`md::render`, nodes in `viewer.file.blocks`) and drawn with the chat's Markdown (`screens::md`, headings, lists, code, links) |
| 9 | Back from a preview to the folder | `fm` action (Back bar) | none | done (`fm` action, Esc) |
| 10 | Download a file | per-row link `/files/get` | hub answered 501 | done; `/files/get` served, client saves to `~/Downloads` (`files_download` step) |
| 11 | Download a folder (zip) | `/files/zip` | 501 | done; `/files/zip` writes a stored zip, same containment rules |
| 12 | Upload into the project (button / drag-drop) | no | no | new, done: file picker + window drop → `files.op write` (never overwrites, 24 MB cap) |
| 13 | New folder | no | no | new, done (`files.op mkdir`) |
| 14 | Rename | no | no | new, done (`files.op rename`) |
| 15 | Delete (asks first) | no | no | new, done (`files.op delete`, two clicks) |
| 16 | Filter / search the folder | no | no | new, done: filter field, case-insensitive substring over the listing, client side |
| 17 | Keyboard navigation | no | no | new, done: Up/Down, Home/End, Enter (open), Backspace (up), Delete, F2, Ctrl+F, Esc |
| 18 | Right-click context actions | no | no | new, done: Open, Download, Rename, Delete, Copy path |
| 19 | Hidden files / symlinks / escapes | hidden dotfiles and links left out, containment (#960) | same (`dir.rs`) | same; every write op uses the same containment (`ops.rs`), dotfiles never touched |
| 20 | Hub-side state the agent can see/drive | `viewer_show`, `fm-*` view actions | `viewer_show` only | done: MCP `files_list`, `files_mkdir`, `files_rename`, `files_delete`, `files_write`, `files_read`, `files_show`; all `fm-*` view actions work unchanged |
| 21 | Script steps for e2e | n/a | `click` yes; `open_viewer`, `files_download` answered `not_implemented` | done; `open_viewer`, `files_download` implemented, `files_*` steps added |
| 22 | Copy-of-a-log safety | n/a | n/a | writes refused when `BACKPLANE_COPY=1` |

Not covered (and why): remote-machine threads (`<link>~<id>`) have no local folder in this hub; split view; drag *out* of the window.

## Evidence (run `scripts/desktop-files-run.sh CLIENT HUB ASSETS NAME`, script `test/files/files-script.json`)
Real hub (this branch's `backplane-hub`, started with **no `--policy-bin`**: the script refuses a hub whose command line has one) + this branch's client on an isolated Xvfb, stand-in agents, a project of 40 mixed files. Screenshots land in
`build/gpui/shots/files/<name>-NN-*.png` (not committed). Run `ro3` (34 `files` steps, none failing; hub log has no refusal):
- 01 folder list with icons, breadcrumbs, New/Upload/Zip; 02 three `key:down` select the 3rd row; 03 filter `kicad` shows "3 of 39"; 04 context menu on a row;
  05 sub-folder via `fm-cd`; 06 `pic.png` drawn; 07 `doc.pdf` page 1 drawn (poppler via `pdf.page`); 08 `notes.md` as text; 09 rename; 10 delete asks first;
  11 `ecc83-pp.kicad_pcb` opens in the Board viewer.
- On disk after: `dropped.txt` uploaded into the project; `newdir` was created, renamed and deleted (absent); `readme.txt` (28 bytes, identical) and
  `files.zip` (40 entries, `unzip -l` reads it) in the downloads folder.
- Unit tests: `cargo test screens::filesui` (client rules), `cargo test files::` in native/host (`ops.rs` containment, zip, `boundary_tests::files_op_...` for
  the RPC, `/files/get`, `/files/zip`, refusals). One unrelated test, `kicad::kipc::tests::spawn_reports_lines_and_exit`, failed once under load and passed alone.

## Known gaps
- Real X input is covered by `FILES_XINPUT=1 scripts/desktop-files-run.sh …` (`test/files/files-xinput.sh`, XTest + an XDND source `xdnd_drop.py`): click-to-focus/select, Down/Up, Return, BackSpace, right-click menu and Rename, filter field, Escape, Delete key and its confirm button, a file dragged onto the list, and a file added on disk by another process. Ctrl+F (no modifier support in `xinput.py`) and Page keys are only covered by unit tests / script steps.
- The Rust presenter (`present/viewer.rs`) reads the folder in-process (`files::dir::list`) each time it draws the view; `files/watch_dir.rs` polls the folders an open page shows every 400 ms and, on a change, bumps the hub's view revision (`Hub::bump_views`) so every view gets `view.changed` and redraws: a file added by an agent shows with no press (verified, `x12b`). Polling, not inotify; entries expire 5 minutes after the last draw.
- Remote-machine threads and Split view are not done. Escape on a preview page (not the list) still falls through to the window's `skill-close` action, which the Rust hub refuses ("action.unported"): outside this tab's scope.


## Rust presenter
`present/viewer.rs` draws the Files page (`files_page`, `file_rows`, `files_trail`, `files_action` for `fm`, `fm-cd`, `fm-up`, `fm-open`), previews (`image`/`pdf`/`text` kinds with `viewer.file`), and a board/schematic/3D opened from a row (watched by its own path). Board/Schematic/3D/Split/Mech/Browser behave as before; Review stays unported (refused, retryable). Tests: `present::viewer::tests::the_files_page_lists_rows_trail_and_downloads_as_core_fm_does`, `fm_actions_walk_folders_open_files_and_refuse_escapes`, and the old assertion now says Files opens (and stops the watch) while Review is still `None`.

## Real input findings (fixed)
- The window's `ClosePopup` action (Escape) ran before the page's key listener and sent `skill-close`, which the Rust hub refuses; `filesui::escape` now takes Escape first while the page has the keys (clears the filter, cancels rename/delete, goes up).
- GPUI asks for dragged data at `XdndPosition`, not at the drop; the XDND test source answers the request throughout the drag.
