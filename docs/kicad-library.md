# Bundled KiCad library

Backplane carries 199 footprints and 42 STEP models from
[Backplane_KiCad_Library](https://github.com/BackplaneWorks/Backplane_KiCad_Library).
`deploy/kicad-library.lock.json` pins release `v2026.10.26`, source
`6e0ccc182c6307a6030142610316db7f45871026`, archive bytes and SHA-256.
A library update is an explicit pin change. Existing fork pins stay Linux bp.4
and Apple Silicon/Windows bp.6. Native footprint format 20260206 requires the
compatible fork; 10.0.6 is not a supported substitute.

The shared service producer, `scripts/build-js.sh`, embeds the verified archive
and digest inventory for dev, classic/headless and native desktop components
on Linux, macOS and Windows. Build verification rejects checksum/identity
mismatches, unsafe paths, links, duplicate destinations, inventory differences
and excessive expanded bytes/file counts. Original notices, docs and catalogue
remain included. This adds no release/installer/SDK gate exemption and no
runtime download.

Setup is lazy: background runtime selection, an actual CLI export, IPC/GUI
launch or agent turn preparation. Ordinary boot, join and design parsing do not
decode or extract the archive. Under the hub home it creates:

```text
kicad-library/<version>-<source commit>/Backplane_KiCad_Library/
kicad-library/profiles/<original config root hash>/10.99/
```

Setup rechecks the archive and manifest, accepts only regular files and bounded
local PAX path headers, and rejects traversal, links and unexpected types. A
nonblocking kernel lock (flock on Linux/macOS, exclusive write handle on Windows)
protects staging/validation/rename and releases on process death. Every plot and
render child sets up once, so a setup that finds the lock held tries again for up
to 5 s before it fails. Activation is
last. Other versions stay intact; a damaged extraction is retained under a
`.damaged-<uuid>` name before repair. An existing valid extraction is checked
against the compiled inventory without decoding the archive again. In-process
reuse checks inventory/file stamps and the managed footprint table.

The initial managed profile copies the original 10.99 profile, including custom
tables, path definitions and preferences. It uses explicit `KICAD_CONFIG_HOME`,
otherwise the platform default:

| Platform | Original configuration root |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/kicad`, otherwise `$HOME/.config/kicad` |
| macOS | `$HOME/Library/Preferences/kicad` |
| Windows | `%APPDATA%\kicad` |

Without an existing footprint table, the selected runtime's stock table seeds
it. Five nicknames are appended in the managed table, with URIs through
`${BACKPLANE_KICAD_LIBRARY}/libraries/<nickname>.pretty`. A conflicting custom
nickname fails without overwriting it or activating a new profile. Original
global and project tables are never written. Later selections retain managed
preferences/custom entries; this is a snapshot, not two-way preference sync.
Different explicit source profiles get separate overlays.

Relative URIs, `${KIPRJMOD}` and custom variable expressions retain their values
and context. In copied footprint/symbol/design-block tables,
`${KICAD_CONFIG_HOME}` binds to the original root. The environment marker
`BACKPLANE_KICAD_USER_CONFIG` carries that origin into export/review children,
preventing nested overlays.

Actual process environment variables `BACKPLANE_KICAD_LIBRARY` and
`KICAD_CONFIG_HOME` activate only after successful setup. CLI exports and new
agent processes receive them. Each new/resumed provider spawn also gets an
explicit environment: on setup failure it clears the bundled path and selects
the original profile, so a repair agent can start without silently inheriting
an unvalidated path. A hub has one managed profile origin and one pinned library;
successful concurrent preparations therefore activate identical paths. Every IPC helper spawn explicitly carries both
paths, even when the helper predates selection. Direct GUI launch inherits the
environment, using extensionless or `.exe` launchers and setsid or a Perl fork
fallback; it avoids macOS LaunchServices dropping process variables. Existing
provider/GUI processes retain their old environment until their next normal
launch. Errors appear in runtime health/logs; general agent access remains
available to repair profile conflicts.

Focused checks use disposable homes and no live service changes:

```sh
python3 scripts/kicad-library-header.py /tmp/library-header.js
python3 test/tools/kicad_library_header_test.py
bun test/tools/kicad_library_test.ts /tmp/library-header.js
bun test/tools/kicad_library_real.ts /tmp/library-header.js RUNTIME NEW_EVIDENCE_DIR
```

The header command accepts `--archive FILE` for offline use, still checked
against the immutable pin. Negatives cover archive/cache/profile failures,
concurrent/interrupted setup and child inheritance. GUI tests launch harmless
stubs through the exact shell, including Perl fallback and `.exe` selection.
Native acceptance enumerates 199 entries, loads a hole, all six Shoggoth
variants and three passive densities, checks usable custom and project-relative
libraries, and verifies the resistor product in actual CLI and IPC STEP output.
Its missing-model control shows why CLI exit zero alone is insufficient.

Native Linux bp.4 CLI/IPC was exercised. macOS/Windows paths share the source
but still require native execution gates; GUI stubs are not interactive GUI
acceptance. Full compiled service/package qualification and startup measurements
remain coordinator-owned. Compiler, KiStack, release and deployment policies
are unchanged.
