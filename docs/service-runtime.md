# Service runtime preparation

The service is a Bun binary (`scripts/service-release.py js`, BUILD.json
`serviceRuntime: "bun"`), cross-built from one host. Run this on a host of the
target itself (Linux x64 or ARM64, a Mac of either kind, or Windows x64) after
the build. It performs no compile, emission, server startup, UI test, install
or publication. It takes the original BUILD and generates the missing
runtime/component inputs for `service-release.component`.

```sh
python3 /absolute/trusted-tools/scripts/service-runtime.py \
  --source-root "$GITHUB_WORKSPACE" \
  --build "$GITHUB_WORKSPACE/build/bootstrap-js" \
  --producer /absolute/trusted-tools/scripts/service-release.py \
  --producer-sha256 ACTUAL_APPROVED_HELPER_SHA256 \
  --out "$GITHUB_WORKSPACE/build/bootstrap-service"
```

The release coordinator pins the runtime script and the component producer
separately from the frozen product. The command imports only the hash-checked
producer. Both actual helper files are retained as component provenance. The
product checkout must still match the original BUILD commit/tree and all SOURCE
members; its Git status must be clean. Original BUILD bytes remain unchanged.

Outputs under the new absolute output directory are:

* `runtime/`: measured `RUNTIME.json`, `CLOSURE.json`, command/tool hashes and raw
  command logs, OS dependency inventory, actual measurement helper and notices;
* `RUNTIME-QUALIFICATION.json`: hash-bound measured dynamic ABI closure;
* `COMPONENT-REQUEST.json`: generated input to the existing component function;
* `component/SERVICE.json` and `component/payload/`: the actual service component
  for desktop staging;
* `RESULT.json`: component output references and both helper identities.

Give the desktop stage producer `component/SERVICE.json` as its service receipt
and `component/payload/` as its service payload. Include the entire output in
the job's artifact upload. Linux ARM64 can immediately stage and smoke
that component on the same ARM64 runner. There is no new request template to
fill and no private input is needed after the build.

Linux measurement runs the actual ELF interpreter's `--list`, reads the service
and every resolved system library's dynamic table, checks their architectures,
and records the complete transitive graph, actual library hashes/sizes and OS
identity. The approved policy is the host glibc ABI: libc, libm, libpthread,
libdl, librt and the matching ld-linux interpreter in standard system directories.
Unresolved libraries, foreign directories, unknown dependencies, RPATH/RUNPATH,
loader injection tags and imported runtime-loading APIs fail preparation.

The loader listing now captures a fixed `LD_DEBUG=files` diagnostic on stderr
from that same measurement invocation, separately from its stdout listing.
A leading address-only row is accepted only when the exact executable's unique
file-labelled link-map block binds that address to its base, dynamic segment,
entry point, program-header address and count. No address-only row is treated
as a library or ignored by shape alone; unknown, repeated, nonleading or
unbound rows fail. Both streams and the fixed diagnostic environment override
are retained. This is loader inspection, not service startup or UI acceptance.

macOS measurement records actual `otool -L/-l`, undefined symbols and `sw_vers`.
It requires the Apple libSystem umbrella ABI and `/usr/lib/dyld`, refuses foreign
libraries/search paths/secondary loading commands and runtime-loading imports,
then confirms libSystem through actual host dyld and checks its in-memory arm64
header. Apple shared-cache images may have no standalone file: no fictitious
file hash is assigned. The Apple-owned umbrella/shared-cache closure is an
explicit approved-system boundary, not a supplied third-party runtime bundle.

No OS library is redistributed. Nonempty runtime inventory contains the actual
measurement/provenance files, clearly named as evidence rather than libraries;
`systemDependencies` records measured host libraries separately. Notices retain
the product MIT license and explain the host-system/source disposition. Any
future supplied third-party library or newly imported runtime loading API needs
its own measured closure, notices and source; this script refuses that case.

Qualification here is limited to the measured native dynamic ABI. External CLI
tools, optional CAD/browser helpers and installed product workflows are not
qualified by this command. `runtimeAcceptance:PENDING` and `published:false`
remain explicit; the desktop owner performs actual stage/startup/auth/reconnect,
OS enforcement and UI acceptance afterward.

Bun opens libraries itself through `bun:ffi` (Bend's JS runtime makes its
syscalls that way). Those loads are declared, not measured: a service JS source that calls
`dlopen` declares its libraries as string literals in the file (one picked
per platform into a variable is fine), and those for the target must be the
C library on Linux, libSystem on macOS or kernel32.dll on Windows; a file
naming none, or any other library, fails.
TODO (fast-hub L1/J4): measure them instead, from the running service on each
target (the libraries it actually maps), and compare with the declared list.

Windows measurement reads the PE's normal and delay imports and requires each
to be a Windows system DLL or API set present in System32. No system DLL is
copied. On Windows the service's `--version` is report-only until Bend's JS
runtime has Win32 syscalls (fast-hub J4).

Offline parser controls:

```sh
uv run --offline --no-project python test/tools/service_runtime_test.py
```
