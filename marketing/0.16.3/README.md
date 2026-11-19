# Backplane 0.16.3: X graphics

Images for the 0.16.3 release post, after `docs/STYLE_GUIDE.md`. Nothing here is posted by an agent; the person posts.

| File | Size | Use |
|---|---|---|
| `backplane-0.16.3-1-hero-1080.png` | 1080×1080 | Main post, or first of the carousel |
| `backplane-0.16.3-2-altium-1080.png` | 1080×1080 | Thread 2/5 |
| `backplane-0.16.3-3-snapshots-1080.png` | 1080×1080 | Thread 3/5 |
| `backplane-0.16.3-4-copy-run-1080.png` | 1080×1080 | Thread 4/5 |
| `backplane-0.16.3-5-terminal-1080.png` | 1080×1080 | Thread 5/5 |
| `backplane-0.16.3-hero-1600x900.png` | 1600×900 | The hero as a single-image post |
| `backplane-0.16.3-banner-1500x500.png` | 1500×500 | Profile header |

## Post text

Main post (hero):

> Backplane 0.16.3 is out.
>
> KiCad now opens in the pane on a Mac, as it does on Linux and Windows. Altium Designer projects open next to KiCad ones. Every project gets a git snapshot before each agent turn.
>
> Press Update in the app, or: https://github.com/BackplaneWorks/Backplane/releases/tag/v0.16.3

Thread, one line per image:

> 2/ Altium projects are recognised. Boards and schematics open in Backplane's viewers without Altium, and agents can write Altium libraries and sheets on any computer.

> 3/ Every project is a git repository, and Backplane takes a snapshot before each turn. Don't like what the agent did? /restore.

> 4/ A copy button wherever there's text to copy, on the desktop and both phones. Commands also get Run in terminal.

> 5/ Every chat has its own terminal: threads, bot chats and rooms. Ctrl+J opens it in the project's folder.

> Full notes, with the fixes to the embedded editor and to agents: https://github.com/BackplaneWorks/Backplane/releases/tag/v0.16.3

## Where each claim comes from

`docs/releases/v0.16.3.md` (the release's notes), sections "Backplane KiCad bp.13.1", "Altium Designer", "Projects and git" and
"Copy and terminals". Nothing from 0.16.4 is shown or named: the pictures are the released 0.16.3 build itself.

## How the pictures were made

Every picture is the real 0.16.3 build (the linux-x64 archive the release was tagged from), captured headless. Nothing is drawn by
hand or retouched; crops only.

- `src/capture.sh`: a private Xvfb display with its own window manager, a temp hub home and a temp HOME, never the person's display,
  home or chats. Demo projects: a copy of the RoyalBlue54L Feather KiCad demo (by Lord's Boards, CERN-OHL-P; it ships with Backplane
  KiCad), and an empty folder that the demo turn fills with Altium files through Backplane's own `altium_write`.
- The agent is the repo's stand-in (`test/tools/claude_standin.ts`, no model runs), answering with the demo chat in `src/demo.json`.
  Its tool calls are real: the board edit, the `altium_write` call, the snapshot and the `/restore`.
- `src/steps.json` (squares) and `src/steps-wide.json` (16:9) are the window's script; each mark is a screenshot.
  Squares: `SIZE=1904x1500 DPI=192`; wide: `SIZE=2944x1240 DPI=192`.
- `src/make_assets.sh` crops the captures into `src/img/`. Every crop ends above the composer's agent row, which carries the agent's
  own mark: no third-party logo is in any image. KiCad and Altium appear by name only, with their trademark lines in the footnotes.
- `src/graphic.html` is the source of all seven images; `src/render.sh` renders it with a headless browser in Berkeley Mono (installed
  on the rendering machine, never committed).

```sh
src/capture.sh APP_DIR KICAD_RUNTIME_DIR REPO_AT_0.16.3 src/steps.json src/shots   # then steps-wide.json: its kicad.png is shots/kicad-wide.png
src/make_assets.sh && src/render.sh
```
