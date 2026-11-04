# Windows KiCad health cache: #927 follow-up

Native qualification of the original #930 fix found a remaining JSON transport
bug. This follow-up keeps #927 open until a corrected package passes native
acceptance and ships.

## Native reproduction

GitHub Windows run [41896427524](https://github.com/BackplaneWorks/Backplane/actions/runs/41896427524)
used the authentic test-only 0.14.2 ZIP produced by run 41886983230, as a newly
created standard local user, with the real packaged Electron application and
its own service. The service and UI were not patched or overlaid.

- Product commit `afa9679273206b3e9b1da4c966c9b63fb6f57ab4`, tree
  `ec6c85b4a8b62a0227f09151a9ec52a362b20ae2`.
- ZIP SHA256 `de613310949b72b0a70bebf9a07aff021cdea98f894856fabdc42134d678cd22`.
- Service SHA256 `11f53c9619d095e0ccd520c5616743155ee240b9637021d4d5a0bea4707aadd1`.
- Compiler `composite-settle-41262875`, producer
  `w-t1792701529-415584-1614985`; historical `releaseBuildApproved:false`
  remains unchanged. This is not stock-compiler output equivalence evidence.

Typing `C:\backplane_kicad\` and `C:\bp927\new tools\test\` character by
character preserved the exact input in the DOM, actual RPC-persisted SQLite
setting, and a real page reload. Resolver output selected and ran the correct
CLI. However, `kicad-health.json` sometimes became invalid JSON, and its saved
`setting` decoded backspace/newline/tab characters instead of literal path
characters. Program Files and PATH selection also ran the expected native CLI,
but strict cache parsing failed. These are partial results, not an overall pass.

The separate Node and exact Bun-runtime probes reproduced the loss at the
native Windows child-argument to Cygwin `env.exe`/`sh.exe` boundary. The old
`Rt.save` supplied `Json.show` output as `$2` to `printf '%s'`: paired JSON
backslashes were halved before printf received them. JSON parsing subsequently
interpreted `\b`, `\n` and `\t`. The JSON encoder and input field did not do
that conversion. A raw DOS path's trailing backslash could also be doubled.
Naive global argument quoting did not preserve both raw paths and JSON.

The real in-app installer downloaded and extracted the exact bp.6 CLI (SHA256
`23b81e721e0a38ec8c277837badf4507e5518c4f5ddc9f89ec3d83a7a9ae2d43`).
The follow-up resolver/version step stopped while clearing an artificial
missing-path override. Install-only continuation
[41898136634](https://github.com/BackplaneWorks/Backplane/actions/runs/41898136634)
used no runtime fixtures and no override: the real in-app download/extraction
completed in about 20 seconds, produced that exact pinned executable, and the
installed CLI successfully printed `10.99.0` as the standard user. The bp.6
identity is bound by the verified archive, manifest and executable hash.
The remaining strict health check failed on the same invalid cache JSON.
This establishes successful installation/CLI execution without claiming full
acceptance. Peak job memory was 1,108,713,472 bytes; pagefile usage and peak
usage both remained zero in that continuation. #946 passed there again.

The independent unchanged #946 canonical native check passed in 41896427524,
including real System32 `msvcrt.dll`, AMD64 identity and `_get_osfhandle`
export observation. That does not make the #927 acceptance pass or promote
historical test-only package metadata.

The kernel-enforced Job Object cap was 8 GiB, peak job usage 3,629,793,280 bytes.
The user authorized the runner's existing pagefile without modifying it;
observed use started at 0 MiB, ended at 267 MiB, and peaked at 379 MiB.
Timestamped host-wide paging counters were retained in the Actions artifact.

## Fix and regression checks

`Rt.save` now writes UTF-8 through the native filesystem. `FS.write_atomic`
creates a private random temporary directory beside the destination, opens
an exclusive mode-0600 file, writes and closes it, then renames it over the
cache. JSON and paths never become child arguments. A write/rename failure
keeps the previous complete cache and cleans its private staging directory.

The actual Bend `Rt.save` IO regression checks Windows drive paths, trailing
backslashes, `\b`, `\n`, `\t`, spaces, UNC paths, Unicode, quotes and actual
control characters. The native effect regression also checks replacement,
file permissions on POSIX, partial-write failure, rename failure and cleanup.
The effect controls and a source assertion that `Rt.save` uses the native
writer without `Proc.run` run with Bun in every `scripts/local-small.py` lane.
The actual Bend IO fixture and effect controls also run in the Linux
nightly/release CI workflow; that workflow is not the per-PR merge gate.
Scoped Boat execution and canonical proof have separate receipts; an
interrupted proof never counts as passing.

Boat job `j20261104193708-1087045` completed successfully under kernel
`memory.max=3699998720`, swap 0. Atomic-write controls, the compiled actual
`Rt.save` IO test, all seven discovery tests, 21 installer tests (three actual
PowerShell cases skipped on Linux), and unchanged LAWS/PROOF shape checks
passed. The unchanged canonical `bend PROOF.bend` attempt was separately
**incomplete**: kernel-confirmed OOM kill after 13.529 seconds, before the
explicit 120-second timeout, with no proof output. No law/proof was changed.
Receipts are under `extra/kicad-927-native-fix/evidence/`.

After review, Boat `j20261104195200-1195105` reran those scoped checks with
Bun effect controls and the caller assertion: all passed. It then attempted
`scripts/local-small.py --focused-only --guards-only`. That broader gate
**failed**, in the existing sidebar-draft guard, because the pinned hashes
for `src/core/kexport.bend`, `src/core/refview.bend` and `src/mobile/hubs.bend`
were stale. All reviewed sidebar inputs, its manifest, scanner, guard and
test are byte-identical to branch base `caf71af247d87f9d89c0af7ced8a2721d152d541`;
current main `123676a2ff041a712c112010f7a04e8a85290a6f` retains those same
three mismatches. No guard or pin was bypassed/refreshed here. The scoped
pass does not qualify that full small gate. Its failure is routed separately.

This source fix still needs a real corrected packaged-build acceptance run;
the already-produced frozen package cannot contain it. Package production and
publication remain owned by the installer/root, with publication held.
