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

## Independent review and bounded canonical-proof retry

Claude/Opus reviewed product head 8002582914008c664dc1449c28fe4920f327d423
read-only and posted APPROVE, no blocking findings:
https://github.com/BackplaneWorks/Backplane/pull/930#pullrequestreview-5769904896.
It ran no commands requiring execution qualification and changed no source.
The focused checks on this exact combined head passed in default Boat job
j20261104141655-2939768 (peak process RSS 453,656 KiB).

The one authorized canonical proof retry was executed in default Boat job
j20261104142230-2971209, using a 120-second subprocess timeout. An earlier
launcher attempt j20261104142135-2965122 rejected an unsupported systemd
MemoryOOMGroup property before starting the diagnostic or Bend; it was not
a proof execution. The corrected launcher used OOMPolicy=continue to retain
the small diagnostic parent after the kernel killed its compiler child.

The kernel reported memory.max = 3,699,998,720 bytes, memory.swap.max = 0,
peak = 3,699,998,720 bytes, and exactly one oom / oom_kill event. Bend exited
with SIGKILL (-9) after 11.267 seconds, with no output or verdict. The
explicit timeout did not fire. The diagnostic job's exit 0 means collection
completed; RESULT.json correctly says state=incomplete. Both VM deletions
were verified. Exact command, source hashes, resources and reason are in
extra/kicad-927-proof/diagnostic.py and extra/kicad-927-proof/RESULT.json.

This confirms the known whole-hub proof/load ceiling documented in
docs/lowmem/proof-slicing-negative-result.md, rather than a failed theorem.
LAWS.bend and PROOF.bend are unchanged from current main. No second baseline
proof execution or proof pass is claimed. The preceding signal-15 attempt
lacked kernel diagnostics, so its exact historical cause remains unproven;
the new run establishes the present cap failure directly. The user expressly
authorized scoped passing checks plus this independent approving review as
the merge bar in this situation, matching #913 and #929. The incomplete full
proof remains disclosed; no cap, proof or verifier was weakened.

## Manual-path field trace and remaining native check

Source trace of the current web/desktop path:

1. src/web/view.bend View.kn.path.loc renders the input with data-enter=setting
   and data-pre=kicad.path=. src/web/host.js's Enter handler calls
   dispatch(action, valueOf(el)); valueOf concatenates the prefix with the
   literal DOM el.value. It does not JSON-parse, unescape or evaluate the text.
2. The thin path queues that String as view.action's value (ThinHost.request /
   pump in thin-host.js). ThinViews snapshots it with JSON.stringify/parse.
   host.js builds a typed Json tree through Wire.json, then App.wire_out_json
   and CBOR. The compatibility path passes the same String to App.act.
3. server/thin-view.bend Action.run reads action/value with Json.get_str and
   Action.web calls the shared action dispatcher. core/client.bend Ui.setting
   splits only at '=' and rejoins the value; Rpc.text uses Json.show to build
   setting.set. Its mobile form path, Bots.bfield -> @f.kpath -> KN.save,
   likewise stores literal String text and uses the same Ui.setting.
4. core/hub.bend Hub.setting reads key/value into SettingSet and emits
   KiCadRetry. model.bend State.setting stores that value without decoding it.
   codec.bend serializes SettingSet via J.Str/Json.show and decodes it via
   Json.get_str. The JSON lexer interprets escapes in JSON syntax, but these
   producers escape each literal backslash first; the round-trip tests pass.
5. server.bend Hub.kcheck passes Setting.get(kicad.path) into KJob.run and
   Rt.refresh. kicad.bend Rt.list -> Rt.explicit -> Rt.candidate is the
   confirmed failing boundary before this patch: separators stayed mixed and
   a Windows root got bin/kicad-cli without .exe. The patch normalizes only
   native filesystem candidates, preserving the original saved setting.
6. Rt.version invokes Proc.run with Port.timeout's quoted positional "$@".
   KiCad.open uses its fixed script with the root and project files passed
   as argv, and quotes variable expansion. host.js bp_proc_spawn passes an
   argv array to child_process.spawn (no shell interpolation of typed text).
   Library preparation receives the root as a String/path argument too.

No point interpreting the field's literal backslash-b as backspace was found
in this trace. This is a source trace, not an executed web-to-hub or native
Windows GUI reproduction. GNU tar's confirmed -C unquoting belongs to archive
extraction and does not explain typing a runtime path into Settings. The PR
must not close #927: native Windows typing, transport/persisted bytes, runtime
selection and launcher behavior still need to be captured on the affected
application. The PR fixes the independently confirmed resolver/installer
failures; it does not establish the reported GUI corruption is resolved.
