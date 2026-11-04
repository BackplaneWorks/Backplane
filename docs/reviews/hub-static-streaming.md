# Stream the static HTTP route

The active static route read each complete plain/gzip file into a Bend byte
list, measured and framed it there, then sent it. `Conn.static.go` now delegates
to `static-http.bend`; the typed `Sock.send_static` effect opens one candidate
and streams the HTTP response in at most 64 KiB pieces. There is no file-payload
cache and no Bend byte list for this route. Transfers yield between pieces,
park on backpressure, and release their descriptor on completion, error,
disconnect or thirty seconds without write progress.

Path checks, landing aliases, MIME types, cache policy and frame protection
remain in Bend. Four existing policy bodies move unchanged, apart from their
namespace, into the small static leaf. The server keeps argument-preserving
delegates. Gzip preference falls back to plain when the preferred candidate
cannot be opened, is empty or is not regular. Unusable files retain the exact
404 response. Existing static symlink behavior is preserved. A transfer pins
its opened descriptor and initial size; truncation or later read errors close
it without injecting another response body.

Seven new canonical laws fix encoding selection, piece size, safe paths,
landing aliases, immutable chunks, fresh page checks and frame protection.
Existing laws/proofs are preserved. The boundary suite exercises actual
production effect code against real temporary files, with controlled socket
wakeups: exact headers/bodies, gzip fallback, symlinks, partial writes, EAGAIN,
zero writes, idle expiry, disconnection, short reads, replacement and errors.

Boat qualification passed on frozen source `80ec029e9`, job
`j20261104025128-135771`. All validation ran on Boat. The scoped runner
checks the exact seven canonical law/proof blocks and the complete unchanged
static/host/HTTP module closure with the original stock-Bend shard executor.
It rejected the false piece-size theorem in a separate copy. Node passed
62 static/preview cases plus the binary regression; Bun passed 29 static cases. The controller and
Node/Bun boundary suites have a hard 512 MiB/no-swap scope; proof workers have
a hard 3.7 GB/no-swap scope. Original verdicts, source/block/head hashes, OOM,
swap and peak counters are retained in
[the receipts](../../extra/hub-static-qualification/SUMMARY.json). Compiler RSS
peaked at 146,024 KiB; the controller peaked at 473,956,352 bytes. OOM, swap
activity and memory-limit event counters stayed zero. The policy relocation and five server
delegates receive a separate source-only audit. Fresh merged law shape and
exact preservation of all scoped inputs/delegates passed in Boat job
`j20261104025751-182681`, under 512 MiB/no swap, with no new proof execution.
The pre-merge scope peaked at 22,355,968 bytes, with no limit/OOM events.

This does not qualify the full server/application closure, execute a packaged
hub or establish startup, memory, CPU or p99 improvements. Those still require
the qualified packaged artifact and copied-store measurements. No new RC build
or installation is requested by this change.
