# Agent CLIs: which versions Backplane supports

Backplane runs each agent through its vendor's own CLI. The hub keeps one table of supported versions,
`native/host/src/settings/agent_versions.rs` (`TABLE`), and this page says the same thing in words. When you change one, change the other.

| Agent | CLI | Minimum | Tested | Update button runs |
|---|---|---|---|---|
| Claude Code | `claude` | 2.1.268 | 2.1.322 | `claude update` |
| Codex | `codex` | 0.169.0 | 0.169.0 | `codex update` (Codex installed by Backplane's setup gets its checked release again) |
| Grok | `grok` | 1.0.88 | 1.0.88 | `grok update` |
| Muse Code | `muse` | 1.4.23 | 1.4.23 | no updater: opens its page |
| Antigravity | `agy` | 1.5.10 | 1.5.10 | `agy update` |
| OpenCode | `opencode` | 2.3.0 | 2.3.0 | `opencode upgrade` |
| Pi | `pi` | 1.4.0 | 1.4.0 | `pi update --self` |
| Oh My Pi | `omp` | 18.18.8 | 18.18.8 | `omp update` |

- **Minimum:** the oldest version Backplane supports. Claude Code 2.1.268 is a hard floor: it is the first version that reports its
  isolated account directory, which Backplane's accounts need. For the others, the minimum is the oldest version Backplane has been run against.
  An older one may still work, but it is not supported.
- **Tested:** the newest version a release was checked with.

## What you see

The hub reads each CLI's `--version` in the same check that decides whether it is installed. The picker, Settings and setup all use that one result.

- **Not found:** "not installed". Setup and Settings offer Install where Backplane has an installer for it.
- **Older than the minimum:** "update needed: vX, needs ≥ vY", with an **Update** button that runs the CLI's own updater (the column above).
  The agent is still offered, because it is installed. Backplane never calls an old CLI "not installed".
- **Newer than tested:** allowed, with a quiet note ("v0.200.0, newer than tested v0.169.0").
- **Version unreadable** (its `--version` fails): treated as installed. Settings says why it did not answer.

The check runs again as soon as an install or update finishes, after setup's check, and after a sign-in or sign-out. It also runs every 15 minutes,
so a CLI installed by hand in a terminal shows up without restarting Backplane.

## Which agent a new thread gets

The default agent (Settings › Agents › Provider) when it is installed. Otherwise, the first installed agent with a connected account (the same
"connected" that Settings › Accounts shows), else the first installed one. A thread no agent has answered yet that is on an agent that is not
installed moves to that agent once the check finds it. A thread with a conversation keeps its agent: its picker says the agent is missing.
