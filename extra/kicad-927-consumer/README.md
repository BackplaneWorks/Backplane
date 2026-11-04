# Prepared Windows artifact consumer — NOT EXECUTED

This folder is an artifact consumer only. It starts no emitter, package build
or release. The installer/root owns the 0.14.2 source freeze;
packaging 440155 supplies its exact Windows ZIP. The authoritative RC-CUT.json
records commit afa9679273206b3e9b1da4c966c9b63fb6f57ab4 and tree
ec6c85b4a8b62a0227f09151a9ec52a362b20ae2, version 0.14.2: main base 2a87,
excluding #932 and #935, with version metadata finalized by the build owner.
Do not substitute the earlier unmodified 2a87 identity. Lane C owns producer pins.

The user authorizes the TEST-ONLY Windows ZIP from run 1614985, using #934's
low-memory compiler mode, for #927 native acceptance. Record the actual
compiler mode and producer work identity with the result. This permission
does not approve release output or waive its separate equivalence review.

Before execution, obtain the authoritative ZIP, its SHA256, commit, tree and
version, and the vendor KiCad runtime/version/CLI SHA256. The runner bootstrap
must verify the ZIP and DESKTOP-BUILD.json identities, preserve package bytes,
and pass a JSON config to accept.cjs as a newly created standard user.
The consumer repeats package and identity checks itself. No source overlays,
service override, fake KiCad executable, fake downloader or direct RPC setter
is allowed. Run only on GitHub Windows; never on the person's machine.

The config requires these actual values (no defaults or invented receipts):
packagePath, packageSHA256, executable, sourceCommit, sourceTree, version,
out, standardUserSid, playwrightModule (absolute path to playwright-core),
programFilesRoot, pathRoot, manualRoots, runtimeVersion, runtimeCliSHA256,
buildProvenance (including actual compilerMode and producerWorkId).
Install real verified vendor runtime copies under the actual Program Files
KiCad version folder, a separate native PATH folder, C:\backplane_kicad\,
and a space-containing path with literal backslash-new/backslash-test.
Only the disposable runner bootstrap may create the Program Files fixture
and grant the new account access to its own test folders. All app launch,
assertions and the in-app download/extraction run as the standard user.

Resource prerequisite: place the PowerShell/Node/app subtree in an actual
Windows Job Object before the consumer starts, with job-wide committed
memory at most 8 GiB and no breakaway. The person explicitly authorizes the
hosted runner's existing pagefile for this #927/#946 attempt only. Query the
actual kernel limits/peak and membership, CIM pagefile state and host-wide raw
paging counters before/after and every five seconds during the consumer.
Write resources-start.json under out with jobMemoryLimitBytes, pageFiles
(the actual observed array), and the observed membership/limit flags. A caller-created
receipt alone is not resource attestation: the hosted bootstrap must enforce
and reread the job configuration. If job setup or paging observations cannot
be established, report BLOCKED before running acceptance. Do not alter host
paging or exceed the cap. Record the explicit pagefile exception, raw paging
samples, final job high-water and cleanup. Host-wide counters are not attributed
to the tested process; record their actual scope.
Job committed-memory semantics: Microsoft's JOBOBJECT_EXTENDED_LIMIT_INFORMATION
and JOBOBJECT_BASIC_LIMIT_INFORMATION documentation.

The existing scripts/windows-standard-user-smoke.ps1 is the reference for
creating a disposable non-Administrator account, loading its actual profile,
fixing HOME/USERPROFILE/APPDATA/TEMP and retiring only that account's processes.
Use those isolation/cleanup boundaries for this consumer, not a prestarted hub
or an elevated Electron instance. The exact hosted bootstrap and resource
probe must be reviewed against the packaging handoff before invocation.

accept.cjs consumes the unmodified package with Playwright's real Electron
launch. It checks discovery without overrides, types backslashes character
by character through the Settings field, reads actual SQLite settings and
health output, reloads the app page, and clicks the in-app install action.
It waits for real installed bytes and runs the installed CLI; no checksum,
network or PowerShell stand-ins are used. It writes result.json and screenshots
incrementally. A FAIL/timeout preserves partial evidence. The bootstrap must
save logs, kernel resources, package/provenance evidence and SID-owned cleanup.

This preparation is untested and is not a Windows PASS. Do not close #927 or
publish 0.14.2 based on it. #927 remains open until native acceptance passes
and the verified fix ships; 0.14.2 publication remains on user-confirmation HOLD.

The dedicated kicad-windows-927-native.yml workflow consumes authenticated
Actions artifact 13352107148 from producer run 41886983230 and verifies the
exact handed-off ZIP and receipt digests. bootstrap.ps1 enforces the actual
Windows Job Object cap and authorized paging policy before dependency setup, runtime
fixtures or application execution. It creates and retires one disposable
standard account. user.ps1 rereads inherited kernel limits under that account,
fixes its profile environment, and invokes the consumer. No native verdict
is implied by preparing these scripts; preserve the resulting evidence even
if resource setup blocks the application test. The workflow runs once when
its file is pushed on this consumer branch; any rerun needs a diagnosed reason.

The first hosted attempt 41888522683 was BLOCKED before app launch by the
runner's active pagefile; see evidence/run-41888522683. The person subsequently
selected option 1, permitting that existing pagefile while retaining the cap
and requiring paging observations. The original refusal is retained unchanged.
No native pass is implied by that policy exception.

canonical-946.py prepares a separate native ABI verdict using unchanged
merged verifier 2823fe4641bbadcb41a6cf49f91fe2c7fbc77978, script SHA256
b1f159e4304d82577a55465891c423e0e9c3fbce063696529ac15ef2a374b823.
It binds actual frozen product JS and the retained service EXE, invokes the
canonical ffi_libraries/windows_measure functions, and records native System32,
AMD64 PE, loaded CRT image path, _get_osfhandle export and dynamic edge evidence.
It never calls canonical prepare(), modifies the verifier or promotes TEST BUILD
status. The native measurement is unrun until a compliant/authorized host is
available; its verdict is independent of the application acceptance result.
