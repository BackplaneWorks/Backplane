# Backplane in Codex Cloud

Research and design, Nov 19 2026; trial run Nov 20 2026 (§9). Spike code on branch `spike/codex-cloud`, not for merging.

The question: can people install Backplane into a **Codex Cloud environment**, so a Codex cloud task gets Backplane's MCP tools
(kicad_edit, kicad_sim, fab_profile, the parts search and so on), headless KiCad and the skills?

**Short answer: yes, through a shell command and a repo skill. Tried in a real Codex Cloud task.** A Codex Cloud environment is a Linux VM
that we set up with an install script, and it can download from GitHub during setup. Backplane's hub runs headless there, Backplane KiCad
runs headless as `kicad-cli` and its IPC API server, and the board tools need no login. In the trial (§9), a cloud task on a current
published environment:
- **did not load our MCP server.** It listed only `codex_apps`, although setup wrote `mcp_servers.backplane` to `~/.codex/config.toml`
  and the repo has the same entry in `.codex/config.toml`;
- **did use the repo skill and `backplane-hub --tool`.** It moved a part with kicad_edit, got KiCad's DRC delta, and committed and
  pushed the board.

So the shell route is the product. The MCP server (`--mcp-attach`) is kept for when Codex Cloud loads configured servers.

## 1. What a Codex Cloud environment is

Sources: [Cloud environments](https://learn.chatgpt.com/docs/environments/cloud-environments),
[Codex Cloud (Legacy)](https://learn.chatgpt.com/docs/environments/cloud-environment),
[Legacy internet access](https://learn.chatgpt.com/docs/cloud/internet-access), [MCP](https://learn.chatgpt.com/docs/extend/mcp),
[Build skills](https://learn.chatgpt.com/docs/build-skills), [Pricing](https://learn.chatgpt.com/docs/pricing), all read Nov 19 2026.

There are two generations, and both are live.

| | Current (published environments) | Legacy (Code Review, Linear and GitHub integrations, `codex cloud` CLI) |
|---|---|---|
| Machine | A VM per task. Plus and Edu Plus: 2 vCPU, 8 GiB RAM, **8 GiB disk**. Pro, Business, Enterprise and Edu: 4 vCPU, 16 GiB, 32 GiB. Larger on Enterprise | Container on the `universal` image ([openai/codex-universal](https://github.com/openai/codex-universal): `FROM ubuntu:24.04`, common languages preinstalled) |
| Setup | Codex infers it and records an **Install script** and a **Start skill** ("instructions to start services and check that they're ready"). Publishing captures the prepared filesystem | Setup script, plus an optional maintenance script on a resumed cache. Runs in a separate Bash session from the agent |
| Internet | Full public access, or restricted to package-manager domains plus "Additional allowed domains". No documented default | **Always on during setup**. Off during the agent phase unless enabled (limited or unrestricted) |
| Secrets | Environment variables are passed to programs. **Network secrets**: programs get a placeholder and the proxy fills in the real value for allowed HTTPS destinations, during setup and tasks. A Personal vault holds per-person values | Env vars for the whole task. **Secrets only during setup**, removed before the agent phase |
| State | A task's VM state is kept up to 7 days. New tasks start from the published filesystem | Container cache up to 12 h, invalidated when the script, variables or secrets change |
| Private network | **Tailscale VPN**: task VMs join the tailnet as ephemeral nodes (HTTP, HTTPS, TCP) | none |
| Unsupported | Computer use, GitHub Enterprise Server, some cloud OIDC services | |

Agent configuration in the cloud:
- **Skills:** "Skills stored in your repository are available in cloud tasks" (`.agents/skills/` in the repo). Personal local skills are not synced.
- **AGENTS.md:** read from the repo (documented for Legacy, standard for Codex).
- **MCP:** Codex reads `mcp_servers` from `~/.codex/config.toml` or a project's `.codex/config.toml` ("trusted projects only").
  The docs do not say whether a cloud task reads either one. The web section says only that "local MCP servers and transports may not be
  available in the cloud". Two more hints: an MCP server's `environment_id` "selects the environment for a stdio or Streamable HTTP server…
  to connect through a remote executor", and `env_vars` with `source = "remote"` "requires remote MCP stdio". So the agent loop may run
  outside the VM, with commands (and possibly stdio servers) executed in it. **This is the open question (§7).**
- A skill's `agents/openai.yaml` can declare an MCP dependency by URL, which "Codex can install and wire automatically". That gives a second
  way in: a skill that names the MCP server.

Auth and plans: cloud tasks need a **ChatGPT sign-in**. The pricing page lists the API-key plan with "No cloud-based features". Local
and cloud usage share the plan's allowance, and "cloud tasks may use more of your allowance than local messages". This matters for a
person's quota, not for this design: we install software into the person's own environment and never touch their OpenAI credentials.

The CLI side (codex-cli 0.170.0 on this machine, read only): `codex cloud exec --env <ENV_ID>`, `list`, `status`, `diff`, `apply`. There is
no command to list or create environments or to send a follow-up ([openai/codex#24777](https://github.com/openai/codex/issues/24777)), and
current published environments are not found by the CLI ([openai/codex#50182](https://github.com/openai/codex/issues/50182)). So
**the person creates the environment in ChatGPT's UI. We can't script it**, and we don't need to.

## 2. What runs inside the VM

```
Codex cloud agent ──shell──▶ backplane tool kicad_edit '{…}' ─┐
        │                                                     ├─▶ backplane hub (headless, 127.0.0.1, own home in ~/.backplane-cloud)
        └──MCP (if honoured)──▶ backplane-hub --mcp-attach ───┘            │
                                                                           ├─▶ Backplane KiCad: kicad-cli + api-server (headless)
                                                                           └─▶ pcbparts.dev (HTTPS, no auth)
```

- **The hub**: `backplane hub run --headless --home ~/.backplane-cloud --host 127.0.0.1 --no-tailscale`, from the Linux x64 release
  (Ubuntu 24.04 matches what we already support). It needs no login and makes its own token. It runs no provider: the cloud agent is the agent.
- **KiCad**: Backplane KiCad's Linux runtime from the Backplane_KiCad releases (`backplane-kicad-linux-x86_64-<v>.tar.gz`, 969 MB
  download, 4.6 GB unpacked). 3.2 GB of that is `share/kicad/3dmodels` and 341 MB is demos. Without them it is about 1 GB, which fits the
  Plus plan's 8 GiB disk with room for the repo. So we publish a **cloud cut** without 3D models and demos (a full download stays an option on
  larger VMs, for STEP exports with parts). The IPC API server is started by the hub the way it is today. Nothing needs a display.
- **Skills**: KiStack's pinned skills and Backplane's bundled ones go into `~/.agents/skills/`. A copy in the repo's `.agents/skills/` is
  the fallback that is documented to work.

### The hub change: an attached agent

Today the hub's MCP is **one URL per thread and process generation** on loopback (`/mcp/<thread>.<gen>?k=<HMAC>`,
`agents/mcp.rs`), handed to agent processes the hub starts. `--mcp-stdio` (`agents/mcp_stdio.rs`) bridges stdio to that URL
through `BACKPLANE_MCP_URL`. A Codex cloud task is an agent the hub didn't start, so it has no thread and no URL. New:

- **An attached thread**: a thread whose provider is `outside` (labelled "Codex Cloud"), for the project at the working directory,
  created on first use and kept in the hub's home. It has a URL that doesn't change with generations: there is no process to go stale. Locks,
  `work_run`, design history and review comments are kept under it as for any thread.
- **`backplane-hub --mcp-attach [--project <dir>]`**: the stdio MCP server. It finds or starts the local hub, takes the attached thread's
  URL and bridges stdio as `--mcp-stdio` does.
- **`backplane tool <name> '<json>' [--project <dir>]`**: one call, the result printed as JSON, exit code non-zero on `ok:false`. Also
  `backplane tool --list` to list the tools with their schemas. This is the path that works whatever Codex does with MCP config, and it is also
  useful locally and in CI.

## 3. Which tools work there

| Works offline once installed | Needs the internet, no login | Needs a link back to the person's hub (phase 2) | Not in the cloud |
|---|---|---|---|
| kicad_edit, kicad_ipc, kicad_ipc_status, kicad_sim, kicad_export10, design_review (deterministic), fab_profile (bundled baselines), review_comment(s), work_*, lock_*, wake_*, altium_write | pcbparts tools (`pcbparts.dev/mcp`), fab profile refresh from the fabs' pages, datasheets | thread_* and machine_list to the person's machines, secret_run with the person's vault, viewer_show on their screen, notify, order_review into their project | kicad_select and kicad_bring_back (no person in a live KiCad), preview_* (no browser helper or person watching), altium_status/script (Windows Altium), account_list, delegate_task and swarm (no provider logins), terminal_* to the person, the person's local files, Buy (ordering stays in the desktop app) |

`viewer_show` in the cloud answers with the rendered PNG paths (board, schematic, 3D) for the agent to read, and with
"there is no viewer here" for anything else. Tools not in this list answer what they would need, so the cloud agent doesn't retry them.

Auth summary: **nothing in the first column needs a token**. The parts search is a public MCP. Fab profiles ship with the build. A
person's own fab or distributor keys, if a tool needs them later, go in as **network secrets** limited to that service's domain, so the
agent only ever sees a placeholder. Codex Cloud's secrets are the right place for them; Backplane's vault is not reachable from the cloud
in phase 1.

## 4. What we ship

1. **`codex-cloud.sh`** on each release (`releases/latest/download/codex-cloud.sh`), pinned to a tag on request. It downloads the hub-only
   Linux tarball and the KiCad cloud cut, checks both against SHA256SUMS, unpacks them under `~/.backplane-cloud`, puts `backplane` and
   `backplane-hub` on PATH, installs the skills, writes the `mcp_servers.backplane` entry to `~/.codex/config.toml`, starts the hub once and
   runs `backplane tool kicad_ipc_status '{}'` as its check. It is idempotent so a maintenance run or resume can repeat it. Install line for the
   environment's Install script:
   ```sh
   curl -fsSL https://github.com/BackplaneWorks/Backplane/releases/latest/download/codex-cloud.sh | sh
   ```
   Domains needed during setup: `github.com`, `objects.githubusercontent.com` (release assets). During tasks: `pcbparts.dev` (and the
   fabs' pages only for a profile refresh).
2. **The Start skill text** (`hub start --cloud` is new: the guard has `hub run` and `hub stop` today): "Run `backplane hub start --cloud` and wait until `backplane tool kicad_ipc_status '{}'` answers ok." The hub
   is started per task, because processes do not survive publishing.
3. **Repo files**, added by Backplane with the person's OK as one commit or PR:
   - an `AGENTS.md` section:
     ```md
     ## Backplane
     This repo's KiCad and mechanical work goes through Backplane. Its tools: `backplane tool --list`, call one with
     `backplane tool <name> '<json>'` (or the `backplane` MCP server when it is listed). Edit boards and schematics only with
     kicad_edit / kicad_ipc, never by rewriting .kicad_* files. Check with `backplane tool design_review '{"scope":"order"}'`.
     The canonical project is the one in `.backplane.json`. Commit KiCad files together with their `.backplane.json` companions.
     ```
   - `.agents/skills/backplane/SKILL.md`: the cloud form of Backplane's agent rules (the tool list, the kicad_edit-first rule, what isn't
     available here), plus `agents/openai.yaml` declaring the MCP dependency;
   - `.codex/config.toml` with the `backplane` stdio server (`command = "backplane-hub"`, `args = ["--mcp-attach"]`), for when cloud
     tasks trust the project config.
4. **Onboarding and Settings: a "Codex Cloud" section** (Settings > Machines, beside Boat and linked machines, and a card in setup's agents
   step for people who use Codex). It shows:
   - the install line, the start text and the domains, each with a Copy button;
   - **Add to this repo**, which writes the files in item 3 on a branch and offers the PR;
   - a link to chatgpt.com/codex/settings/environments;
   - a plain note on what works there (§3).

   Nothing is sent to OpenAI by Backplane.
5. **docs/codex-cloud.md** (this file), turned into user-facing steps once it ships.

### Bringing the work back

The cloud task's result is a branch or PR on the project's GitHub repo, like any Codex cloud task. On the desktop, Backplane shows it the
way it shows any PR branch: the board and schematic at that commit in the viewers, a diff against main, and design review. Merging is the
person's call. KiCad files merge badly when the person has also edited the same board, so the doc and the skill say to keep a cloud task
to one board, or to a part of it nobody else is editing. The desktop's Agents panel history doesn't cover cloud edits: their history is the
git history.

## 5. Phase 2: the person's own hub over Tailscale

Codex Cloud task VMs can join the person's tailnet. Backplane already links machines over tailnets. Instead of a hub in the VM, a cloud
task could use **the person's own hub** as a remote MCP server: the canonical board, edited live while the person has it open in
KiCad, plus their projects and threads. That needs:
- a scoped token for the attached thread (one project, a tool allowlist, an expiry), minted in Settings and stored as a Codex **network
  secret** for the hub's tailnet hostname, so the VM sees only a placeholder. Tailscale TCP doesn't get secret substitution, so this
  works only over the hub's HTTPS address (it has one with `--tailscale-https`);
- the ephemeral cloud node allowed only to the hub's port (Tailscale ACL), which the doc would spell out;
- `kicad_select` and the person-aware rules applying to the attached thread as to any other.

This is more powerful and has more risk: a cloud VM reaches into the person's machine. It belongs after phase 1 and needs its own security
review.

## 6. Effort (one engineer)

| Piece | Days |
|---|---|
| Release CI: a hub-only Linux tarball and the KiCad cloud cut (no 3D models or demos) with SHA256SUMS | 2–3 |
| `codex-cloud.sh`, `backplane hub start --cloud`, the health check | 1–2 |
| The attached thread, `backplane tool`, `--mcp-attach` (reusing mcp_stdio), viewer_show's cloud answer, clear refusals | 4–5 |
| The skill, the AGENTS.md section, `.codex/config.toml`, tests on a local `codex-universal` container | 2 |
| Settings/onboarding section and "Add to this repo" | 2 |
| Live check in a real Codex Cloud environment (with the person's OK) and fixes | 1–2 |
| **MVP total** | **about 2.5–3 weeks** |

The spike (§9) has done the installer, `backplane tool` and the live check, so about 2 weeks remain: the release cuts, the attached
thread done properly (tests, the cloud tool list, refusals), the skill and AGENTS.md text, and Settings. Phase 2 (Tailscale) is about
another 1.5 weeks plus a security review.

## 7. Open questions and blockers

- **MCP in cloud tasks: not loaded (trial, §9).** On a current published environment, neither `~/.codex/config.toml` written by setup
  nor the repo's `.codex/config.toml` gave the task a `backplane` server. Not tried: a skill's `agents/openai.yaml` MCP dependency (it
  names a URL, and our server is stdio on loopback), and Legacy environments. The shell route doesn't depend on any of them.
- **The tool list.** `--tool --list` shows all 95 hub tools, including ones that can't work in the cloud (threads, browser, vault,
  accounts). The MVP lists only the cloud-capable ones (§3) and answers the rest with what they would need.
- **PRs.** The task pushed its branch, but GitHub's API answered Forbidden when it tried to open the PR. Codex Cloud's own "Create PR"
  button is the way. The skill should say so instead of retrying.
- **Big diffs for small edits.** A 1 mm move rewrote 1039/966 lines of the .kicad_pcb (KiCad's save plus the pour refill). Review on the
  desktop should use Backplane's board diff, not the text diff.
- **The CLI can't see current environments or their tasks.** `codex cloud list` showed no tasks after the trial ran
  ([openai/codex#50182](https://github.com/openai/codex/issues/50182)). The person starts tasks in ChatGPT. We can't launch or track
  them from Backplane until OpenAI ships that.
- **Disk on Plus (8 GiB).** The install is 1.2 GB with the KiCad cloud cut. The full KiCad (4.6 GB) plus a large repo may not fit. The
  installer checks free space and says so.
- **Two environment generations.** The scripts are the same, but where they go in the UI differs (Install script and Start skill vs. setup and
  maintenance scripts). The Legacy agent phase has no internet by default, and that switches off the parts search there.
- **Terms.** We install open-source software (the hub, GPL KiCad) into an environment the person owns, and use no OpenAI account or API
  ourselves. That is ordinary use of a setup script, not a ToS issue. KiCad's GPL source stays offered beside the binary, as today.
- **No scripting of environments.** People create the environment in ChatGPT. We give them the text to paste, which is enough.
- **Updates.** The cache keeps whatever was installed. `codex-cloud.sh` pins a tag, and the Settings section says when the pinned build is
  older than the desktop's, so the person can update the line.

## 8. Recommendation

Build the MVP (§4) on the shell route: `backplane tool` plus the repo skill and AGENTS.md section, with the tool list cut to what works in
the cloud. Keep `--mcp-attach` and the `.codex/config.toml` entry, since they cost little, and check again when Codex Cloud documents MCP for
cloud tasks. Hold phase 2 (the person's hub over Tailscale) until the MVP is in use and the security review is done.

## 9. Trial (Nov 20 2026)

Setup: spike hub from `spike/codex-cloud` (an attached thread through `GET /dev/attach?dir=`, `backplane-hub --tool`, `--tool --describe`,
`--mcp-attach`; about 120 lines in `agents/attach.rs`), built on Debian bookworm (glibc 2.36). A private test repo held the KiCad `ecc83`
demo, `setup/codex-cloud.sh`, the hub (20 MB gzipped), `AGENTS.md`, `.agents/skills/backplane/` and `.codex/config.toml`.

**Stand-in (clean `ubuntu:24.04` container on this machine):**
- **Install:** 52 s. Downloading KiCad (970 MB from GitHub) took 30 s and unpacking it without 3D models and demos about 20 s. The install is 1.2 GB on disk (KiCad 1.1 GB).
- **Startup:** the hub served in 70 ms. The first tool call started headless KiCad's IPC server: "KiCad server: ready for ecc83-pp".
- **kicad_edit** (move R1 by 1 mm, from a subfolder of the repo): saved, with KiCad's DRC delta, in 1.9 s (`served_by: headless`).
- **design_review** (scope "order"): 2.5 s. fab_profile answered from the bundled profiles with no network.
- **MCP:** `--mcp-attach` answered `initialize` and `tools/list` over stdio.

**Codex Cloud (current published environment, set up and started by the person in ChatGPT, cheapest model in the picker):**
- **MCP:** the task's MCP list had only `codex_apps`. It said `backplane` wasn't connected and used `backplane-hub --tool`, as the skill tells it to.
- **The edit:** it moved R1 +1.000 mm in x, from (136.271, 107.950) to (137.271, 107.950). kicad_edit's check reported the same five new
  DRC findings as the stand-in (R1 pad to the bottom GND zone clearance, R1/U1 courtyard overlap, R1's reference over its pad, two silk
  overlaps), 32 remaining.
- **Git:** one commit, board file only (KiCad's save and the refill), on branch `move-r1-1mm-x`, pushed. Opening the PR through GitHub's
  API was Forbidden. `git status` was clean.
- **Setup time in the cloud:** not captured. The environment's setup report has it, and the stand-in's 52 s is the estimate.
- **Quota:** the weekly allowance (Pro, 7-day window) went from 47% to 51% used over the trial's window. That covers the environment's
  setup run, the task, and any other Codex use on the account in those hours, so the trial cost at most 4 points. The credit balance was
  unchanged (6230.85). It was read from Codex's `account/rateLimits/read` before and after, with no turn.
