# Named Claude and Codex accounts

This implementation is isolated from canonical main `7eccc1e6f29187929d24e0cbb4a8d85127f9d713` (tree `2e58fcc31644639f3c8828147486ca54434dc264`). It adds machine-local subscription profiles and login management to Backplane's web UI, including the desktop web shell. It does not import existing credentials or change existing default CLI homes. Native account-selection UI remains a separate delivery; released native clients receive a visible refusal for named-account threads and cannot dispatch provider work with default credentials.

The [pinned upstream research](research/multiple-provider-accounts.md) compares T3 Code commit `9e09926d97824179d97311d1a810c6b5bb60d2d1`, official Codex source commit `78a3185ecba7ca3422f14589aeac97059649f60f`, and Backplane's existing allowance work. T3 has several account mechanisms; this implementation uses independent CLI profiles and Backplane's conversation handoff, rather than shared native-session overlays or managed subscription tokens.

## Login and selection

Open Settings → Agents → Accounts on this machine. Add a Claude or Codex profile with a label, then choose Connect / Login. The label alone never establishes an authenticated account.

Codex uses the provider's app-server device ceremony. Backplane displays the provider URL and device code, tracks its own login ID, and accepts completion only from that profile's active app-server attempt. Claude uses its own `auth login --claudeai` ceremony. Backplane displays its authorization URL and provides a sealed input if the CLI asks for a sign-in response. That input goes directly to the matching attempt's stdin; it is not conversation text, a draft, command-line argument or persistent form value. Cancel, expiry and reconnect preserve any previously verified active store until a replacement verifies successfully.

Status / Refresh verifies the provider identity and retrieves supported metadata. The provider owns token refresh. Disconnect runs the supported provider logout and removes that profile's active private store. Login/disconnect refuse while an owned provider child still uses that profile. Failed verification never creates a successful account from CLI availability or a display name.

Use the Account chip beside the model to select between turns. Selection saves the old native session under its profile/generation, restores only the selected account's session, and hands over unseen recent conversation with an earlier-history pointer. The popup explains that context is handed to the selected account. An active turn must finish or be stopped first. There is no automatic account rotation or fallback for a named-account thread.

Each provider's new-thread default is frozen when a thread is created with that provider. Changing a default leaves existing threads alone. An old thread with no account binding continues to use its existing default CLI authentication and native session. Forks copy account bindings but receive their own thread/session ownership. Switching providers retains each provider's selected profile and catalog separately.

## Ownership and storage

| Item | Owner / representation |
| --- | --- |
| Public profile metadata | Machine, opaque profile UUID, provider, label, verified identity generation, safe identity/catalog/quota metadata |
| Thread binding | `account.thread.<thread>.<provider>` → `{id, epoch}`; missing old binding means legacy default |
| Native session and handoff cursor | Length-prefixed thread/provider/profile/epoch tuple; different accounts, threads or generations cannot alias |
| Private registry | Hub-owned `providers/accounts-v1/profiles.json`, mode 0600; profile/store UUIDs and identity fingerprint, no raw credentials |
| Provider stores | Private 0700 profile/attempt directories; fresh candidate for each login, verified promotion, obsolete/canceled-store cleanup |
| Login attempts | Transient hub/profile/attempt/CLI-login-ID binding, ten-minute expiry and bounded slots; excluded from durable event metadata and hub caches |

Claude children use `CLAUDE_CONFIG_DIR`; real `HOME` is preserved. Verification requires the exact `configDirectory` reported by Claude Code 2.1.268 or later and `authMethod: claude.ai`. Claude Console profiles use a different store and are not supported by this subscription-profile flow. Codex children use `CODEX_HOME`, file-backed CLI auth, and the OpenAI model provider. Inherited API keys, token selectors, cloud-routing overrides and alternate profile selectors are scrubbed per child. Codex diagnostic verbosity is restricted. Environment changes are never global. These directories separate stores under the same OS user; they are not an OS sandbox.

Captured thread mutations carry the account binding they were composed against. The server checks that binding before accepting work; a stale outbox/reconnect cannot send through a newly selected account. Global Settings/account-management requests have no conversation owner. View actions use the authoritative selected target. Restarted profiles are unchecked until provider verification; launch revalidates identity before creating the child. Identity replacement retires stale running ownership and requires explicit selection of the new generation. Concurrent independent threads may share one profile's provider store while retaining different native sessions; verification is serialized per profile.

## Catalogs and allowance

Both provider catalogs in the model picker are scoped to their saved profile and identity generation. A missing or stale named profile does not borrow the default account's catalog. Codex metadata uses `account/read`, `model/list` and `account/rateLimits/read`; the quota collector has a five-minute gate. Claude catalog discovery is an initialize-only control request, with no user/model prompt. Claude allowance is populated only from actual passive rate-limit windows received on the current account generation.

Used percentage, reset time and window duration are displayed only when the provider reports them. Missing values remain unknown; older readings become stale. Provider limit bucket IDs do not establish a per-model entitlement mapping. Profiles may share provider pools, and their allowances are never added together. Locally counted tokens/costs are not remaining subscription quota. No paid model probes, account upgrades or budget changes are part of qualification.

Primary contracts: [Codex app-server authentication](https://learn.chatgpt.com/docs/app-server#authentication), [Codex environment](https://learn.chatgpt.com/docs/environment-variables), [Claude authentication and multiple accounts](https://code.claude.com/docs/en/authentication#log-in-with-multiple-accounts), [Claude auth status CLI](https://code.claude.com/docs/en/cli-reference#auth-status). Exact pinned source/file-line links and the API-key versus subscription distinctions are retained in the research document.

## Qualification and integration

Evidence is retained under `build/multi-account/`: guard source/output clearances, exact source maps, normal server/web build receipts, executed Bend/IO/hub/browser logs and desktop/390px screenshots. The fake provider programs never contact a provider or run inference. Test entry points are `test/accounts*_test.bend`, `test/tools/accounts_io_test.cjs`, `accounts_hub_e2e.ts`, `accounts_ui_e2e.ts` and the explicit-input frozen-native replay `accounts_native_replay.mjs`. Browser qualification needs the repository browser helper's Playwright dependency (or `BACKPLANE_PLAYWRIGHT_MODULE`) and a local Chromium executable. Build/test work is serialized through Backplane's named locks.

The shared server/client contract offers `accountProfiles: 1` only from updated web views. Existing `ios-v1`/`nativeSidebar: 1` markers do not grant account support. Unsupported named-thread reads retain bounded valid Screen/Out data with a visible error and no commands; provider mutations are refused before enqueue/spawn. The frozen released native bridge was replayed against an actual built-service refusal. Foundation-only actual Screen/Out decoding is qualified separately by the existing native owner, without a new app/archive/TestFlight distribution. The actual captured service draft is empty; a nonempty decoder sentinel is not a new service-level draft guarantee.

The required global `bend PROOF.bend` attempt has an inherited late-import parse error in `LAWS.bend`; no global proof acceptance is claimed. Existing laws were not weakened or removed. The isolated account contracts and focused executed tests provide narrower evidence. Fake CLI success does not qualify real provider browser interaction, installed CLI versions on every OS, commercial managed-auth eligibility or all platform credential-store behavior. Real account login, release installation and deployment were not performed.

The next integration step is review and deliberate merge of this isolated feature against the then-current main, followed by runtime adoption and user-driven provider sign-in through Backplane. Native account-selection UI requires its own compatible client release. This work does not authorize any of those distribution actions.
