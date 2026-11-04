# Bounded binary asset effect

`Sock.send_file(Socket, String, U32)` keeps one file descriptor and at most
64 KiB of file bytes per transfer. It sends the opened file's initial size,
in order, yields between pieces and parks on backpressure. Thirty seconds
without write progress fails and releases the descriptor. Zero-byte writes
also park. Completion, read/write failure and disconnected handles close the
descriptor exactly once. No file payload enters a Bend byte list or cache.

The caller still owns authorization and the response header. The existing
64 MiB maximum, regular-file, no-follow and descriptor-identity checks remain.
The input is an immutable asset: atomic replacement cannot change the opened
descriptor, appends do not extend the transfer, and premature EOF fails without
injecting another response body.

`test/tools/asset_file_test.mjs` executes the production function with real
files and deterministic socket wakeups. It checks byte parity, partial writes,
bounded reads, yielding, repeated EAGAIN/zero writes, renewed idle deadlines,
disconnects, error cleanup, cap/path checks, short reads and truncation. It
passes under Node and Bun; backend cases cover POSIX and Bun handles, including
a Windows handle with no POSIX interface and Darwin's errno distinction.
The adjacent preview suite passes all 33 cases; indexed-wire tests pass all
three cases. Laws shape and diff checks also pass.

No pure Bend definition, law or proof changes. Full/root proof and full hub
compilation are intentionally omitted under the targeted-only qualification
instruction. The CI owner reports no current passing production shard receipt
under the 3.7 GB cap; the boundary results are not a pure proof of foreign code.

Current HTTP routes use `Sock.send_preview` and `Sock.send_path`, which already
stream files. This change hardens the binary effect before further callers use
it; it does not establish a production RSS, CPU or startup improvement. The
copied-store startup, 30-minute RSS, 20-agent CPU and p99 gates remain open.
