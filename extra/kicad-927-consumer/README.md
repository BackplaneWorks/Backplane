# Prepared Windows artifact consumer — NOT EXECUTED

This folder is test preparation only. It starts no emitter, package build,
workflow/campaign or release. The installer/root owns the 0.14.2 source freeze;
packaging 440155 supplies its exact Windows ZIP. Do not reuse the earlier
2a87 main identity as the authoritative freeze: held #932 is to be excluded
and version metadata finalized by the build owner. Lane C owns producer pins.

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
programFilesRoot, pathRoot, manualRoots, runtimeVersion, runtimeCliSHA256.
Install real verified vendor runtime copies under the actual Program Files
KiCad version folder, a separate native PATH folder, C:\backplane_kicad\,
and a space-containing path with literal backslash-new/backslash-test.
Only the disposable runner bootstrap may create the Program Files fixture
and grant the new account access to its own test folders. All app launch,
assertions and the in-app download/extraction run as the standard user.

Resource prerequisite: place the PowerShell/Node/app subtree in an actual
Windows Job Object before the consumer starts, with job-wide committed
memory at most 8 GiB, no breakaway, and no active page files. Query the actual
kernel limits/peak and membership, and CIM pagefile state before and after.
Write resources-start.json under out with jobMemoryLimitBytes, pageFiles
(an empty array), and the observed membership/limit flags. A caller-created
receipt alone is not resource attestation: the hosted bootstrap must enforce
and reread the job configuration. If no-pagefile or job setup cannot be
established, report BLOCKED before running acceptance. Do not silently alter
host paging or exceed the cap. Record the final job high-water and cleanup.
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
