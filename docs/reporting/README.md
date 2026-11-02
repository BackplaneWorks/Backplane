# Issue reports

Improvements offers two destinations. Public GitHub issue is the default for a
new draft. Choose Internal implementation thread to keep the in-app development
workflow. The destination choice stays on this device for this authenticated hub.
Existing internal drafts, stable retry keys, account bindings and images retain
their original behavior. Backplane never guesses the owner from project paths,
GitHub credentials or account names.

A public report includes user-authored text, OS family and the app's numeric
release version. Unknown versions remain unknown. Private build suffixes and
host names do not enter automatic metadata. The versioned template is
[template.md](template.md). It requests a title, summary, installation and OS,
reproduction steps, expected and actual behavior, and evidence. Unknown facts
remain "Not provided." A report needs a thorough behavior description or an
image that the person will attach. The person confirms evidence before opening
GitHub. A checkbox is an attestation, not automated proof of evidence quality.

Write report with agent starts a visible normal Scratch thread, using the
usual configured provider, account binding, permissions and stable request
identities. It sends only the explicit description and selected evidence,
optional typed details and selected sanitized diagnostics. A selected image is
uploaded through the authenticated hub attachment endpoint for this agent.
The prompt treats those inputs as data. It forbids project, conversation,
account and filesystem inspection, except viewing the explicitly attached image.
It embeds the template, ASD-STE100 writing guidance and the exact supplied
[Unslop skill](unslop/SKILL.md). It does not claim standards compliance or a
verified dictionary. Normal agent permissions still apply; these writing
instructions are not a new sandbox or a changed tool-permission mode.

Load agent report reads the completed assistant response from that report's
Scratch thread. The result is editable. Open its thread to answer any questions;
no stopped thread is automatically resumed. The local template remains available
without an agent. Manual edits use the same body-only URL builder as the native
apps and do not append another environment footer. Changed sharing choices
invalidate the current report and keep an earlier report/writer reference locally.
Review the regenerated report after changing those choices.

Screenshots, diagnostic inclusion and extra details start unselected. The image
preview and replacement picker let the person inspect an image and replace it
with a redacted copy. There is no claim of perfect anonymization: typed text,
images and generated reports must be inspected for private information. Internal
origin context, project paths, conversation identifiers and account data are
never copied into a public report automatically. Diagnostics are reconstructed
through a closed allowlist again at the publication boundary. Raw logs and
traces are never included.

Review issue shows the exact editable body. Open GitHub draft sends that exact
body in an encoded HTTPS link to BackplaneWorks/Backplane. The person signs in
with their own GitHub account and presses Submit on GitHub. GitHub attributes
the issue to that account. Backplane does not inspect authentication, create an
issue through an API, change credentials, or automatically upload an image to
GitHub. Save selected image and attach it manually on GitHub if desired.

For reports whose encoded URL exceeds 8000 bytes, the complete text remains in
the preview. Copy issue text or Save report, open the plain GitHub form and
paste it. Nothing is truncated. Opening has a durable local receipt and a busy
guard. Failed local storage blocks the handoff. Preparing, writing or loading a
report never opens GitHub. Native behavior and build limitations are in
[mobile/IMPROVEMENTS.md](../../mobile/IMPROVEMENTS.md).

## Local diagnostics

The hub retains fixed structured timing, error-category and connection counts
in at most eight 64 KiB segments for seven days. The one MiB total cap includes
the control byte and measured file allocation. Flushes use the existing tick.
Measurement updates aggregate in bounded memory and make no filesystem calls.
Details and fault tests are in [diagnostics-backend.md](../diagnostics-backend.md).
This is local storage, separate from telemetry and its opt-out policy.

The browser connection ring retains at most 64 records for one day, has a
64 KiB encoded payload cap, at most 12 records a second, and samples decode and
ready latency at most once per metric per ten seconds. Its storage is managed
by the browser; this bounds the diagnostic payload, not the browser profile's
filesystem allocation. The desktop journal keeps one current/replacement pair
with a combined 128 KiB hard cap over measured file sizes and allocated blocks,
including the replacement and off marker. Each journal payload is at most
32 KiB. Writes that exceed the measured cap are dropped. Closing or crashing the desktop adds only closed metadata categories.

Local diagnostics controls turn device/hub collection off or on and clear prior
records. Disabled collection keeps existing records until cleared or expired.
Only elected exports enter a reviewed issue. No prompt text, message text,
credentials, token, IP, account identity, private path or exception message is
part of the automatic export schema. Disk/quota failures drop bounded diagnostic
work without disturbing conversation persistence, authentication or history.
Existing explicit developer raw trace/log environment options are separate and
are never part of this optional export.

## Qualification

`test/tools/reporting-gates.sh` checks the shared policy and prompt, report
reader, existing implementation workflow, storage failures, browser consent and
writer/editable-preview flow at desktop and phone widths, native model/adapters,
production bundles and a real isolated hub with a stand-in provider. No test
creates an external issue or uses a real provider credential. Native SwiftUI
execution requires Xcode; absence is reported as unqualified, not a pass.
