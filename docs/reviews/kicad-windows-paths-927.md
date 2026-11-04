# Windows KiCad paths (#927)

Baseline: main c5a1b17f35b6fae97ceb3fcfe6c7bdc9c9288054.

The exact resolver failed six assertions: Windows runtime directories retained
backslashes/trailing separators and appended an unsuffixed CLI; native PATH
entries were split at drive colons. It searched bundle, managed, PATH, POSIX
system and macOS locations, but no standard Windows Program Files version
directories. Native Bun FS.stat does not supply Cygwin's implicit .exe lookup.

The exact embedded installer failed an offline backslash-path fixture at its
checksum comparison: GNU sha256sum adds an escape marker before its digest
when its filename contains backslashes. After hashing stdin, the same fixture
reached extraction and failed because GNU tar unquotes backslash sequences in
its -C argument. Windows ZIP extraction and native staging-root conversion
use explicit Cygwin/PowerShell stand-ins on Linux.

The reported JSON/backspace explanation was not reproduced: actual JSON
serialize/parse round trips preserve literal backslash-b, backslash-n and
backslash-t sequences. No JSON parser or global text rewriting was changed.
The manual path failure is confirmed at filesystem candidate construction.

Changes normalize only native filesystem candidates, preserve drive letters,
use the host's PATH separator and .exe suffix, resolve suffixes for older and
bundled candidates before cache stamping, and search Program Files version
directories. Explicit missing overrides remain explicit; stock KiCad retains
its stock health status. The installer maps its native staging root through
cygpath -u, hashes stdin, and extracts from within the staging directory.
Pinned packages, checksums, archive validation and promotion guards remain.

All execution used default Boat sandboxes through scripts/heavy-boat.sh,
which checks its own kernel memory.max <= 3,700,000,000 bytes and
memory.swap.max = 0 before running commands. No local compiler, proof, test,
benchmark or build ran. No deployment, installer build or real installation
was performed. HOME and platform recheck threads were not accessed.

Baseline job j20261104140524-2863021: six resolver failures, one installer
checksum failure; JSON round trips passed. Job j20261104140934-2891047
confirmed the separate tar -C unquoting failure after the checksum fix.

Job j20261104141042-2898346 passed the 16 path/JSON assertions, six filesystem
discovery cases, 20 installer cases (three native Cygwin cases skipped),
three launcher cases, 21 catalog assertions, and LAWS/PROOF shape. Job
j20261104141252-2913393 passed the expanded discovery suite (seven cases,
including the path/JSON assertions), installer and launcher suites and shape.
Its canonical bend PROOF.bend attempt ended with signal 15/exit 143 and no
verdict. This is incomplete, not a proof pass; the cause of termination is
unestablished. No law, proof, checker, guard or pass cache was changed.

Real Windows FS/process behavior, PowerShell extraction, GUI entry and a
full packaged application run remain unverified. A full proof and whole-hub
build are not qualified by the focused fixtures.

Job j20261104141439-2927658 passed the final 18 path/JSON assertions
(including POSIX semicolon and Cygwin UNC PATH preservation), seven discovery
cases, 20 installer cases (three native Cygwin cases skipped), three launcher
cases, 21 catalog assertions, LAWS/PROOF shape, and the production KiCad
module's type-check. Later test-only isolation closes the fixture PATH to
env/uname and invokes Bun by its captured absolute path.
