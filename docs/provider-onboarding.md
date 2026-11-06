# Connecting an agent during onboarding

The welcome card offers **Connect an agent**. On a new install without a verified, installed subscription account, Next opens guided setup once program discovery has arrived. A missing program and a discovered program with unknown sign-in status both get guidance. A completed tour stays completed; it is never reopened to replace an existing CLI account. The empty view and Settings → Agents also offer setup.

Setup pauses the tour at its current step. Choose a provider, follow its installation guide on the hub machine, and sign in. Program discovery updates every minute. A CLI version or model catalog alone never means authenticated. Codex and Claude subscription setup reuse the existing account registry, provider login ceremonies and sealed Claude response form. Labels create metadata only. The selected account shows sign-in progress, the provider's device code/link, cancellation, failure/expiry and retry, and verified success. An explicit **Use for new threads** chooses the verified account and provider for future threads. Existing threads, sessions and drafts retain their current account.

Antigravity opens its existing Settings page, with its existing methods and warnings. Grok, Muse, OpenCode, Pi and Oh My Pi use their existing CLI sign-in workflows; setup explains that their CLI authentication is not verified here. Existing CLI accounts remain usable as Default accounts. No credential copying, paid inference probe or automatic login is added.

**Keep my current setup / Set up later** and **Return to tour** leave without cancelling a pending sign-in or marking the tour done. Next then continues the tour. Closing Settings normally also reveals the paused tour. Setup can be reopened from Settings → Agents. Provider links open separately; Backplane's live account snapshot updates the current step when sign-in completes, including after returning from the provider page.

Desktop and responsive web use the same shared rows and the existing sealed forms. Native phone Settings and tour choices expose guidance and a direct browser link when the hub supplies its pairing address; subscription forms hand off to this hub in the browser because released native clients do not advertise account-profile support. No new native wire fields are required.

Validation entry points:

- `test/provider_setup_test`: real client actions and native phone projection, readiness, missing programs, return/skip, failed/expired retry, default selection, sealed routing, session/binding/draft preservation.
- Existing `test/onboard_test`, `test/tour_preview_test`, `test/accounts_test`, `test/initial_provider_test` retain tour and session coverage.
- `test/tools/provider_setup_e2e.ts BUILT_SERVICE WEB WIRE`: isolated built hub/web and fake provider-owned stores, desktop/390px browser flow, failure/retry/cancel/success, Codex device sign-in, Claude sealed response, returning users and no inference. Set `BACKPLANE_PLAYWRIGHT_MODULE` and `BACKPLANE_CHROMIUM` when dependencies live outside this checkout.

Provider instructions follow the supported CLI behavior and the official [Codex installation guide](https://developers.openai.com/codex/cli), [Claude installation guide](https://code.claude.com/docs/en/setup), and [Muse authentication guide](https://dev.meta.ai/docs/muse-code/auth). Browser tests block non-loopback requests and never perform a real login. Build products and screenshots are review artifacts, not an installed or released app.
