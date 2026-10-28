# Multiple Claude/Codex accounts: verified behavior and Backplane design

Historical research snapshot, 2026-10-28, completed before implementation authorization. Its source observations and design alternatives are retained below. The subsequent authorized implementation, current base and qualification limits are documented in [Multiple provider accounts](../multiple-provider-accounts.md).

## Recommendation

Add **named, machine-local provider profiles**, each with its own provider-owned credential/config directory, and select a profile beside the model. Keep Backplane conversation history separate from each profile’s native session. Start with completely separate Claude and Codex homes. Switching profiles explicitly creates or returns to that profile’s own session, with a visible context handoff when necessary. Do not silently rotate accounts, change billing methods, or treat a second profile as extra quota.

T3 really supports multiple accounts, but there are three distinct mechanisms, not one universal account switch:

| Mechanism in current T3 | Simultaneously saved accounts | Continuing an existing native thread |
| --- | --- | --- |
| Claude instances with distinct `CLAUDE_CONFIG_DIR` | Yes; separate configuration, login and local history | Different directories are excluded from ordinary same-session selection |
| Codex CLI instances with independent `CODEX_HOME` | Yes; separate state and login | Different homes are excluded from ordinary same-session selection |
| Codex CLI shared home + private shadow home | Yes; auth and model cache private, conversation state shared | Compatible instances share a continuation key and can restart/resume |
| T3-managed Sign in with ChatGPT | Multiple saved registrations/connections, distinct from CLI login | Managed Codex runtime uses its own credential binding and compatible home layout; this is a different inference/auth route |

These are source-backed capabilities, not a live concurrency or account qualification result. Multiple **providers** means Claude versus Codex; multiple **instances/profiles** can mean two subscriptions, two API keys, or just two presets for the same principal. None proves independent billing pools. See [T3’s Codex guide][t-codex-doc], [Claude guide][t-claude-doc], [instance schema][t-instance], and [managed registration implementation][t-managed-auth].

## Source basis

Inspected actual upstream `https://github.com/pingdotgg/t3code`, `main` resolved by `git ls-remote` to **`9e09926d97824179d97311d1a810c6b5bb60d2d1`** (commit timestamp 2026-10-28 12:22:18 -07:00). All T3 source links below are immutable at that commit; search results and third-party wikis were used only for discovery, not conclusions. Checkout: `build/research-multi-account/t3code`.

Backplane upstream `main` resolved to **`ac3f2a519a9b282c07d8570bfb5c9b353c7bbdfe`**, also this isolated worktree’s HEAD. The newer allowance implementation was inspected separately from its recorded frozen source composition, **not represented as current main**. Selected source copies and their hashes are under `build/research-multi-account/backplane-allowance-snapshot`; provenance is in `build/research-multi-account/sources.json`.

Official Codex source for storage/concurrency was pinned separately to **`78a3185ecba7ca3422f14589aeac97059649f60f`**, the upstream `openai/codex` main observed during this investigation. Official documentation is mutable; links below were fetched on the research date. Backplane’s browser reported no attached desktop guest, so documentation was read through HTTP/web retrieval. No authenticated provider process was launched.

## What T3 actually does

**Identity and configuration.** `ProviderInstanceId` is separate from driver kind; each instance has its own name, enabled flag, driver config and environment. An instance is a configuration handle, not proof of a distinct person. The web picker selects `(instanceId, model)`, checks driver and continuation group, and displays the instance name. Mobile also resolves instance identity within the owning environment; the same default ID on two servers does not mean the same account. [Schema][t-instance], [web filtering][t-picker], [mobile identity][t-mobile].

**Claude isolation.** T3 resolves `homePath` to `CLAUDE_CONFIG_DIR`, preserves `HOME`, and keys continuation by the resolved config directory. Its capability cache also includes binary, directory and working directory. The adapter receives that explicit environment. Separate accounts therefore keep separate local sessions; two display names pointing at one directory remain the same login store. [Home/continuation helpers][t-claude-home], [adapter creation][t-claude-adapter].

**Codex CLI shadow isolation.** The process’s effective `CODEX_HOME` is the shadow directory. T3 materializes symlinks to shared sessions, SQLite state, skills/plugins, caches and runtime lock directories; it also links other shared entries except private/local exclusions. `auth.json`, `auth.lock`, and `models_cache.json` remain private. It rejects an auth-file symlink and conflicting non-link state instead of copying an entire home. Continuation identity uses the shared home, not the private auth home. The guide explicitly requires file-based auth for this overlay setup. This is T3 filesystem orchestration, not a documented native Codex “account profile” flag. Shared MCP/plugin/config state is a real coupling. [Layout and private entries][t-codex-layout], [materialization][t-codex-materialize], [CLI process environment][t-codex-adapter].

**Managed ChatGPT differs from CLI auth.** T3 stores registrations and token sets using provider/instance credential bindings, tracks an active client ID while retaining other sessions, validates OAuth identity, and retains stable connection labels. The credential store hashes its binding to a safe filename. Its secret backend writes bytes under a `0700` directory with `0600` files and atomic replacement; the inspected backend is permission-protected storage, not evidence of encryption at rest. A semaphore plus a filesystem lease serializes updates/rotating tokens. Managed launch uses the Responses API token-sharing provider, a local token broker and a revision check, rather than replacing the user’s CLI login. [Bindings][t-credential-store], [storage][t-secret-store], [registrations][t-managed-auth], [identity validation][t-managed-validation], [refresh][t-managed-refresh], [cross-process lease][t-managed-lock], [managed runtime][t-managed-runtime].

**Processes, reconnect and switching.** Codex sessions open an app-server child with the instance environment; Claude sessions use an SDK-backed query runner with that environment. The switch planner resolves current/target metadata and continuation identity, then chooses a transition, releasing the replaced session where appropriate. This is process/session ownership, not a global “log every thread into the new account” operation. Compatible Codex accounts may restart/resume the same native thread; isolated Claude homes do not gain that ability merely by sharing the Backplane/T3 conversation. The code supports independent live sessions, but establishes no provider-guaranteed numerical concurrency allowance. [Codex spawn][t-codex-adapter], [Claude adapter][t-claude-adapter], [switch planner][t-switch].

**Catalog and quota attribution.** Ordinary Codex discovery initializes app-server, reads its account, lists models and reads limits with separate deadlines. Claude initializes without yielding a user prompt, reads account metadata, and optionally invokes an explicitly experimental usage API. Its cache is per instance and a fallback removes usage rather than making an old reading fresh. Codex normalizes the returned Codex bucket; Claude maps reported windows and structurally checks newer model-scoped entries. Crucially, T3’s managed ChatGPT route deliberately returns **no numeric quota windows** and links to ChatGPT Usage. Do not promise the native CLI allowance experience for that route. [Codex discovery][t-codex-probe], [Claude probe][t-claude-probe], [Claude cache][t-claude-cache], [normalizers][t-claude-limits], [Codex normalizer][t-codex-limits], [managed usage limitation][t-managed-usage].

## What current official mechanisms establish

| Provider/mode | Supported mechanism and boundary |
| --- | --- |
| Claude subscription / saved API key | Official docs explicitly support side-by-side accounts using separate `CLAUDE_CONFIG_DIR`. It includes settings/history/plugins. Keep real `HOME` intact. macOS uses Keychain; Linux uses protected credential files. Directory isolation must be qualified on each supported platform/version. [Authentication][o-claude-auth], [environment reference][o-claude-env]. |
| Claude Console OAuth without an API key | **Important exception:** official docs say separate Claude config directories do not isolate these sign-ins; credentials live in Anthropic profiles outside them. `ANTHROPIC_PROFILE` is a separate credential selector. Do not advertise this auth kind as isolated by `CLAUDE_CONFIG_DIR` alone. [Authentication][o-claude-auth]. |
| Claude CLI lifecycle | `claude auth login`, `auth logout`, `auth status`; status reports auth method and, from 2.1.268, config directory. `--email` is a hint, not identity verification. `CLAUDE_CODE_OAUTH_TOKEN` is an externally supplied token with different renewal behavior. `ANTHROPIC_API_KEY` overrides subscription billing in print mode. [CLI reference][o-claude-cli], [environment reference][o-claude-env]. |
| Claude SDK/product-managed auth | SDK overview says offering claude.ai login/rate limits in third-party products requires prior approval. T3 source does not establish Backplane has it. Preserve CLI-owned authentication for the initial design; separately confirm the applicable product integration route before offering a managed subscription flow or adopting T3’s experimental usage probe. [SDK overview][o-claude-sdk]. |
| Codex CLI auth and state | `CODEX_HOME` controls config, auth, sessions, logs and related state. Credential storage supports file, keyring, auto and ephemeral modes. A Codex `--profile` selects configuration layers; it is not by itself a distinct subscription login. [Environment][o-codex-env], [authentication][o-codex-auth], [config profiles][o-codex-profiles]. |
| Codex app-server | Account read/login/cancel/logout, managed browser/device-code auth, external-token mode, model discovery and rate-limit reads are documented. Use the owning profile’s process. Its account API describes one current account, not per-thread account multiplexing. Official docs permit existing local/open-source app-server integrations to continue, recommend Sign in with ChatGPT, and exclude commercial/hosted services from app-server authentication. [Account RPCs][o-codex-server]. |
| App-owned Sign in with ChatGPT | Official OSS flow supports multiple registrations, each retaining its issued client ID, identity and credentials. Serialize refreshes for one renewable session. The documented `env_key` app-server setup needs process restart and native thread resume after token replacement. It has route/tool limitations and usage management in ChatGPT. This is a later, separate adapter, not an API key or reused CLI OAuth token. [Accounts/sessions][o-chatgpt-accounts], [app-server integration][o-chatgpt-server], [preview limitations][o-chatgpt-limits]. |

**Concurrency conclusion:** separate homes permit multiple saved accounts and independent processes. They do not promise unlimited requests or an independent pool for each login/key. Current Codex source hashes the canonical home for its direct keyring entry and locks `auth.lock` for storage read/modify/write. Refresh also has a per-manager semaphore, guarded reload and account-mismatch checks; the storage lock explicitly does not cover network requests. Do not claim all cross-process refresh races are eliminated. Backplane should avoid making its own concurrent refresh authority, and qualify the chosen CLI versions. For app-owned OAuth, serialize the full read/refresh/save lifecycle, including cross-process coordination. [Codex keyring key][o-codex-storage-key], [storage lock][o-codex-storage-lock], [refresh/reload][o-codex-refresh], [official refresh requirement][o-chatgpt-accounts].

No official numerical “maximum concurrent accounts/threads per subscription” was established. No live account tests were performed. Claude history resume requires the right local session; use an explicit ID rather than “most recent” in concurrent operation. [Claude session guidance][o-claude-sessions].

## Backplane gap and compatibility

Current main has provider/model/session on `Thread`; `Switch.session.key` and `Switch.seen.key` use only thread and provider. Reusing those keys for two Claude accounts would resume the wrong account’s session. Claude is a persistent per-thread process; Codex uses `exec` or `exec resume` per turn. Their launch path inherits the hub environment, with no first-class Claude/Codex account selection. Reuse identity also omits account. Model discovery reads the user’s default Codex model cache and a default-environment Claude initialization, and publishes one catalog per provider. The keyring gate currently classifies all Codex launches together; future gating must follow the selected profile’s actual store/dependencies. [Thread/switch state][b-state], [session keys][b-session], [spawn path][b-spawn], [Codex resume][b-codex], [Claude process/reuse][b-claude], [model discovery][b-models], [keyring rule][b-keyring].

The frozen allowance composition has useful honesty rules to retain: `100-used` is a percentage of a reported window; freshness expires; passed resets do not imply a full refill; different pools are not combined into one number. Codex uses account/limits reads without starting a model turn, a five-minute throttle, bounded execution, private cache locking and a salted identity digest before retaining old windows after failures. However it currently has one `allowance-codex.json` and `allowance.codex` per hub; Claude stream events write one `allowance.claude` and report identity unknown. These keys must become profile/generation scoped before adding accounts. A workspace/account ID alone is not necessarily a distinct human identity or independent allowance pool. See the [copied collector](../../build/research-multi-account/backplane-allowance-snapshot/src/server/allowanceio.bend), [projection](../../build/research-multi-account/backplane-allowance-snapshot/src/core/allowance.bend), and [stream adapter](../../build/research-multi-account/backplane-allowance-snapshot/src/core/allowance_stream.bend); exact lines/hashes are in the source manifest.

## Concrete implementation plan

All items below are proposals for later implementation, not changes made by this investigation.

### 1. Profile identity, secrets and launch resolution

| Record | Required fields / ownership |
| --- | --- |
| `ProviderProfile` | Opaque `profileId`; owning hub/environment ID; provider kind; user label; enabled; auth kind; canonical config/home path; binary and optional CLI configuration profile; credential binding reference; endpoint/routing preset; config revision; identity epoch. Public projections contain no secrets. |
| `ObservedIdentity` | Provider-reported opaque principal/workspace identifiers when available, safe display metadata, verification source/time and confidence. Email is a label, never a unique key. Missing identity stays unknown. Keep private equality fingerprints hub-local. |
| `SessionBinding` | `(hub, thread, profileId, identityEpoch, sessionNamespace)` → native session ID, seen cursor and generation. Historical turn attribution retains the profile/epoch used at dispatch. |
| `ModelCatalog` | `(hub, profileId, identityEpoch, configRevision, CLI version)` → reported models/efforts/capabilities, source and freshness; distinguish configured model IDs from verified discovery. |
| `AllowanceSnapshot` | Same owner plus collector generation, observed/attempted/expiry times, source/status, provider pool ID and each window’s actual unit/used/remaining/reset/model applicability. Unknown optional values remain absent. |

Ownership diagram: [multiple-provider-accounts.mmd](multiple-provider-accounts.mmd). A conversation may have several profile bindings; a native session and its asynchronous replies always belong to one binding/epoch.

Create new owned directories under `<backplane-home>/providers/<provider>/<profileId>/` with `0700` permissions. Let the CLI retain its native credentials/keychain binding; Backplane holds references, not copied auth JSON. Use the existing vault for API-key references and inject only into the intended child through a structured environment/file channel, never argv, source, event logs or frontend storage. Preserve provider-managed policies and existing redaction. Existing [vault IO][b-vault] is relevant; the current generic spawn wrapper needs an explicit per-process environment/unset facility.

One resolver must supply the **same** profile environment to chat, initialization, catalogs, allowance reads, login/status/logout and auxiliary generation (titles, summaries, commits, bot turns). Do not change the hub’s global environment. For owned subscription profiles, remove conflicting inherited API-key/token/base-URL/cloud/federation/profile overrides; explicitly configured API/cloud/gateway presets retain only their intended selectors. Claude settings can override inherited environment too, so check the effective auth/routing method and fail closed on mismatch. Do not rewrite external settings to fix it. Keep keyless Console OAuth unsupported until its separate Anthropic-profile store is modeled and qualified.

Bind identity changes to a new epoch; invalidate catalogs/allowances, stop reusing the old process and require an explicit session transition. Do not call a token refresh a new identity. For unknown identity, never merge two profiles’ readings or retain windows across a reauthentication boundary. Profile paths are not an OS sandbox: agent tools running as the same user may read other user-accessible files. Stronger hostile-agent isolation requires separate OS/container credentials, a separate product decision.

### 2. Session switching, defaults and concurrency

For old logs synthesize `legacy-default` per provider/hub without moving credentials. Preserve the exact inherited launch behavior and old session keys through a compatibility adapter. New keys use collision-safe tuples, not concatenated labels. Unattributed historical usage stays legacy/unknown; never reassign it to the currently signed-in person. Defaults resolve once at thread creation: explicit choice → project default → hub/provider default → legacy default. Changing a default affects new threads only.

Account selection applies between turns. While busy, offer an explicit stop-and-switch action; never replay a partially completed tool turn on another account. Revalidate profile, identity epoch and expected thread revision at dispatch. If the target profile already has a session for that conversation, resume that binding and hand over only unseen entries. Otherwise create a new native session and visibly hand over context. The selection UI must explain that context will be sent to the chosen account. Going back to A returns to A’s binding, not B’s native ID. Lost native sessions produce an honest new-session/handoff notice, with no “resume latest” fallback.

Allow independent threads on separate profiles concurrently. Serialize writes to one native session and login/logout/identity mutations for one credential binding; do not serialize all provider work globally. Use bounded collector jobs and existing worktree protections. A profile with expired auth, missing model, locked store or exhausted quota stops with a profile-specific action. It never silently falls back to another account, provider, API key, overage purchase or quota reset. Provider-internal model fallback must be reported as such, not confused with an account change.

Defer T3-style Codex shared-session overlays to an optional later feature, behind explicit compatibility grouping and CLI-version qualification. They require private auth/model state, shared writer/MCP locks, SQLite crash/concurrency checks, symlink protections and clear cross-account history sharing. Fully isolated homes plus Backplane handoff meet the basic request with much less filesystem coupling.

### 3. UI, login lifecycle and quota semantics

Desktop/web: Settings → Providers → Accounts, scoped to the selected execution machine. Add/import/rename/disable/default/remove profile; show safe identity, auth mode, connection state and owning machine. In the composer use `Claude · Work / model / effort` or `Codex · Personal / model / effort`. Same provider/model from two profiles remains two choices. Usage details always show the selected profile and source; two logins may share a provider pool.

iOS/Android: the same profile list and selection contract in a sheet, with machine label, compact composer summary, accessible full label, large-text support and no layout shift while discovery loads. Server-generated eligibility/reason/state is shared; clients do not invent quota percentages or choose accounts. Dismiss stale sheets on hub/thread/profile changes. Old clients may operate legacy-default threads; they must not silently send a named-profile thread through default auth. Use protocol capability gating/read-only treatment where necessary. Native allowance support remains a separate qualification, not established by the web delivery receipt.

Login/logout are user-initiated, machine-local operations through the selected CLI/store, with one active attempt per binding. Use Codex browser or device-code flow where supported; Claude uses its documented CLI login/terminal ceremony. Frontends receive only bounded URLs/codes/status; callbacks must be bound to hub/profile/attempt and validated, not accepted as arbitrary URLs. Completion requires status/identity verification in the same resolved environment. Cancel/expiry keeps the previous valid identity intact. Logout drains/stops only affected owned-profile sessions, clears its live catalog/allowance and preserves history. An imported default CLI store needs a warning about the other CLI/IDE clients that share it; disconnecting Backplane alone can just disable the profile without invoking provider logout. CLI-owned refresh stays CLI-owned; app-owned OAuth later needs a credential-broker lease, atomic rotation, selected-registration revocation and explicit reporting when remote revocation fails.

Quota: keep the existing honest projection, but scope collection and stale-data retention to profile + identity epoch. Codex `rateLimitsByLimitId` identifies metering buckets, not a model-to-bucket entitlement map; show multiple buckets separately unless the provider supplies mapping. Claude shows only actual emitted windows; its missing percent is unavailable. Treat the experimental T3 usage API as optional future research, not a guaranteed stable endpoint. API keys are not consumer subscriptions; token/cost totals are not remaining credit. If the provider supplies no remaining/reset/account data, show unavailable. Never calculate quota from locally counted tokens. Show shared pool semantics explicitly; multiple keys in one project/org and multiple registrations of one account do not imply additive allowances. Deduplicate only with provider-established scope/identity; otherwise label relationship unknown. Coalesce read jobs per credential binding, retain the five-minute read gate, and keep rendering/model changes free of collector/model calls.

### 4. Acceptance tests and delivery slices

Use fake CLIs/protocol fixtures and temporary protected homes first. Later real login/storage qualification must be separately authorized; no paid inference is needed to test account/catalog/limit plumbing.

| Test | Required observable result |
| --- | --- |
| A/B environment separation | Concurrent stand-ins record only their own resolved home and marker; restarting hub restores the same bindings; system `HOME` stays unchanged. |
| Auth precedence | Subscription profile cannot inherit a paid API key, cloud/profile selector or alternate endpoint; settings-induced mismatch is surfaced without rewriting settings. |
| Session ownership | A → B → A keeps separate native IDs/seen cursors; identity replacement prevents old-session reuse; missing session never resumes an unrelated latest session. |
| Busy switch and stale replies | Old-generation stream/model/quota replies cannot alter B; late Stop/permission results cannot reach the new process. No automatic retry of an executed tool turn. |
| Same-account aliases | Two labels/two keys do not double a known shared allowance; equal email/different workspace does not collapse profiles. Unknown identity stays unknown. |
| Quota truth | API-key/no-auth/missing methods/partial windows/multiple buckets/reset passed/stale clock/error all preserve unavailable/stale semantics; a new epoch cannot inherit old windows. |
| Collector behavior | One bounded read per binding per gate; no login, generation, reset redemption or purchase RPC; no calls from render/model/account-picker navigation. |
| Refresh/logout races | Duplicate refresh/status/login cancellation/crash recovery tests; selected binding logout leaves other accounts running; late callback cannot replace a newer login. |
| Secret exposure | Sentinel credentials absent from events, argv, stderr, RPC snapshots, UI caches, diagnostics and artifacts; files protected; malicious paths/symlinks rejected for owned stores. |
| Catalog ownership | Different profile entitlements/efforts survive reconnect; inaccessible model selection errors instead of falling back; missing discovery is labeled. |
| Migration | Old logs/default sessions replay unchanged; no copied auth; old client cannot execute a named-profile thread with default credentials. |
| All clients / multiple hubs | Same thread/profile shown and dispatched on desktop/web/iOS/Android at 390px and large text; identical labels on different hubs stay distinct; offline state cannot initiate auth or refresh quota. |

Implementation sequence: (1) pure Bend profile/session/epoch laws, persistence migration and stand-ins; (2) structured per-child launch environment plus scoped catalogs/collector; (3) shared selection/lifecycle contract and all-client UI; (4) optional managed ChatGPT adapter and Codex overlay only after separate mechanism/platform qualification. In future implementation, add laws without weakening existing ones, then run relevant tests and `bend PROOF.bend`; no such build was needed or run for this document.

## Remaining decisions

Choose CLI-owned profiles first. Keep app-owned ChatGPT plan sharing as a distinct future auth mode because its capabilities and quota display differ. Confirm Anthropic’s applicable third-party integration route before introducing managed subscription auth. Qualify selected CLI versions on Linux/macOS/Windows for native-store isolation and refresh behavior; current upstream source is not evidence about the installed runtime. Determine how much context an explicit cross-account handoff includes and show it to the user. Keep independent accounts separate unless the provider actually establishes a shared billing scope.

[t-codex-doc]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/docs/user/providers-codex.md#L28-L77
[t-claude-doc]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/docs/user/providers-claude.md#L7-L38
[t-instance]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/packages/contracts/src/providerInstance.ts#L19-L75
[t-picker]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/web/src/components/chat/ModelPickerContent.tsx#L505-L518
[t-mobile]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/mobile/src/features/threads/thread-provider-instance.ts#L19-L55
[t-claude-home]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/Drivers/ClaudeHome.ts#L12-L77
[t-claude-adapter]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/orchestration-v2/Adapters/ClaudeAdapterV2.ts#L11154-L11188
[t-codex-layout]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/Drivers/CodexHomeLayout.ts#L11-L71
[t-codex-materialize]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/Drivers/CodexHomeLayout.ts#L325-L450
[t-codex-adapter]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L1492-L1515
[t-credential-store]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/ProviderCredentialStore.ts#L7-L23
[t-secret-store]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/auth/ServerSecretStore.ts#L153-L205
[t-managed-auth]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/CodexChatGptAuth.ts#L327-L359
[t-managed-validation]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/CodexChatGptAuth.ts#L619-L691
[t-managed-refresh]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/CodexChatGptAuth.ts#L730-L806
[t-managed-lock]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/CodexChatGptSessionLock.ts#L8-L44
[t-managed-runtime]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/CodexManagedRuntime.ts#L22-L138
[t-managed-usage]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/Drivers/CodexManagedProvider.ts#L109-L148
[t-switch]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/orchestration-v2/ProviderSwitchService.ts#L63-L188
[t-codex-probe]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/CodexProvider.ts#L515-L589
[t-claude-probe]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/ClaudeProvider.ts#L318-L392
[t-claude-cache]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/Drivers/ClaudeDriver.ts#L200-L245
[t-claude-limits]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/claudeUsageLimits.ts#L174-L220
[t-codex-limits]: https://github.com/pingdotgg/t3code/blob/9e09926d97824179d97311d1a810c6b5bb60d2d1/apps/server/src/provider/codexUsageLimits.ts#L116-L140
[o-claude-auth]: https://code.claude.com/docs/en/authentication#log-in-with-multiple-accounts
[o-claude-env]: https://code.claude.com/docs/en/env-vars
[o-claude-cli]: https://code.claude.com/docs/en/cli-reference#cli-commands
[o-claude-sdk]: https://code.claude.com/docs/en/agent-sdk/overview#get-started
[o-claude-sessions]: https://code.claude.com/docs/en/agent-sdk/sessions#continue-resume-and-fork
[o-codex-env]: https://learn.chatgpt.com/docs/config-file/environment-variables#core-locations
[o-codex-auth]: https://learn.chatgpt.com/docs/auth#credential-storage
[o-codex-profiles]: https://learn.chatgpt.com/docs/config-file/config-advanced#profiles
[o-codex-server]: https://learn.chatgpt.com/docs/app-server#auth-endpoints
[o-chatgpt-accounts]: https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions
[o-chatgpt-server]: https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server
[o-chatgpt-limits]: https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations
[o-codex-storage-key]: https://github.com/openai/codex/blob/78a3185ecba7ca3422f14589aeac97059649f60f/codex-rs/login/src/auth/storage.rs#L285-L300
[o-codex-storage-lock]: https://github.com/openai/codex/blob/78a3185ecba7ca3422f14589aeac97059649f60f/codex-rs/login/src/auth/storage.rs#L175-L208
[o-codex-refresh]: https://github.com/openai/codex/blob/78a3185ecba7ca3422f14589aeac97059649f60f/codex-rs/login/src/auth/manager.rs#L3131-L3194
[b-state]: https://github.com/i2cjak/Backplane/blob/ac3f2a519a9b282c07d8570bfb5c9b353c7bbdfe/src/core/model.bend#L56-L79
[b-session]: https://github.com/i2cjak/Backplane/blob/ac3f2a519a9b282c07d8570bfb5c9b353c7bbdfe/src/core/model.bend#L1377-L1390
[b-spawn]: https://github.com/i2cjak/Backplane/blob/ac3f2a519a9b282c07d8570bfb5c9b353c7bbdfe/src/server/server.bend#L1310-L1323
[b-codex]: https://github.com/i2cjak/Backplane/blob/ac3f2a519a9b282c07d8570bfb5c9b353c7bbdfe/src/server/server.bend#L2296-L2302
[b-claude]: https://github.com/i2cjak/Backplane/blob/ac3f2a519a9b282c07d8570bfb5c9b353c7bbdfe/src/server/server.bend#L9367-L9445
[b-models]: https://github.com/i2cjak/Backplane/blob/ac3f2a519a9b282c07d8570bfb5c9b353c7bbdfe/src/server/server.bend#L6381-L6408
[b-vault]: https://github.com/i2cjak/Backplane/blob/ac3f2a519a9b282c07d8570bfb5c9b353c7bbdfe/src/server/vault.bend#L5-L36
[b-keyring]: https://github.com/i2cjak/Backplane/blob/ac3f2a519a9b282c07d8570bfb5c9b353c7bbdfe/src/core/keyring.bend#L17-L36
