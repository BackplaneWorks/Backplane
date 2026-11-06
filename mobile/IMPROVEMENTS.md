# Native Improvements

Settings → Improvements opens a device-local draft on iOS and Android. A new
draft selects **Public GitHub issue**. Existing records without a target remain
internal; explicit saved targets are kept. Existing browser/internal drafts are
not read, converted or overwritten by the native app.

The public form keeps the original description, an editable complete report,
evidence review, screenshot consent and opening receipt on the device. Filling
from the description calls the shared formatter with only the typed text,
OS family, the app's short version and typed behavior details. It does not supply
the native screen, origin context, thread, project, filesystem path or account.
The complete edited report goes through the shared body-only preview, so OS and
version are not appended twice. The native view checks that the returned body
equals the editor and that the encoded URL contains exactly the displayed body.
Overlong URLs retain the full preview and require shortening; nothing is truncated.

The person reviews the evidence and supplies a nonempty thorough behavior
description or selects a screenshot. A selected screenshot starts with an
unchecked consent control. The picker runs only after consent and a separate
Choose action. Images up to 10 MB are copied to private app storage, downsampled
for display and never uploaded or attached by the native app. The person must
attach the image themselves on GitHub after inspecting it. Revoking consent or
starting a new draft removes the local image. Native capture is not implemented;
the system image/file picker can select a screenshot taken by the person.

Opening the exact encoded issue form is the only public submission path. iOS
uses a Safari form; Android offers an explicit chooser so a browser can be used
for sign-in. The UI says the public report belongs to the person's own GitHub
account. No account is detected, inferred, switched or authenticated by Backplane.
Only the person presses Submit on GitHub. A receipt is persisted before opening;
reopening the native view or restarting the app does not repeat the open. The
explicit Retry action asks the person to check GitHub first. Editing the report
invalidates its preview, evidence review and opening receipt. Storage failures
block the handoff. Corrupt saved drafts are kept and fail closed.

**Internal agent thread** opens the existing paired hub browser workflow with
`#improvements-internal`, retaining the browser's existing draft and retry logic.
It does not reinterpret the native public draft as an internal submission.

**Write report with agent in hub browser** hands off only the typed description
using `#improvements-issue=<encodeURIComponent(JSON.stringify({description:text}))>`.
The browser consumes/removes the fragment, seeds only when there is no conflicting
saved draft, and requires a further explicit writer action. The parent-owned web
flow uses the normal project-thread agent transport and report retrieval, the
versioned template, ASD-STE100 guidance and supplied Unslop resource. The person
can review and finish there, or explicitly copy/paste the generated edited body
back to the native body editor. No automatic report retrieval or clipboard read
exists in native code. A large description must be shortened or pasted manually.

Shared integration contracts (owned outside native code):

- `Backplane.improvementsPreview(text, os, version, details, diagnostics)` returns
  a JSON string `{body,url}`; native always passes empty diagnostics.
- `Backplane.improvementsPreviewBody(body)` returns the exact complete body and
  encoded URL without another metadata footer.
- Both browser fragment routes above must be supported by the paired hub version.
- If the shared preview functions are missing or stale, the app fails closed and keeps the local draft.

Native diagnostics are deliberately unavailable in this first integration.
The existing iOS performance collector is debug/simulator-only and Android has
no matching safe exported production journal. Parent-owned local/web diagnostics
can be selected and reviewed in the hub browser. Native does not claim to attach
samples it cannot actually collect safely.

Offline checks, with no external issue or sign-in requests:

```sh
JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 uv run --no-project --offline mobile/tests/improvements/check.py
```

The check compiles/runs the actual Kotlin model using cached Gradle compiler
jars and runs Foundation Swift model tests if `swiftc` is available. Shared
format/normalization tests belong to the parent's `issue-report` checks. It
does not open a URL. Model checks cover target migration, screenshot default,
evidence gating, exact body/URL parity, destination restrictions and
duplicate-opening receipts.

On this Linux host, Kotlin model checks pass. Swift compiler/Xcode
are unavailable, so the iOS model executable and SwiftUI build were not run.
Android `:app:compileDebugKotlin` passed offline using Java 17, two workers and
not an APK or device qualification. No simulator/device UI, system picker,
Safari/Android chooser, browser fragment round trip, GitHub prefill/sign-in or
actual submission has been exercised. No app was installed and no release,
TestFlight upload or merge was performed.
