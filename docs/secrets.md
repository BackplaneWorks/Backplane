# Secrets: the secret box

The person gives an agent an API key, an auth key, a token or a password without pasting it into a chat. The value goes into a masked box, then straight into this machine's vault. Agents only ever get a handle: `{"secret":"tailscale-bub","set":true}`.

## Using it

- **An agent asks.** `secret_request {name, purpose}` puts a card in its thread on every client: "<thread> asks for `tailscale-bub`", the purpose, a masked field, then Save to vault or Not now. When the person saves, the thread gets a note with the handle, which wakes it.
- **The person offers one.** `/secret [name]` in any composer opens the box in that chat, and the thread is told the handle once it's saved. Settings > API keys & secrets > Add a secret opens it outside any chat.
- **Settings > Secrets** shows how the vault is sealed (the keychain, a key file, or Locked), the open requests (Set, Not now) and each secret with its purpose and last use (Replace, Delete, which asks twice). Values are never shown.
- **Phones.** A request shows as a queue row above the message box. Its Set opens the box in Settings, where the value field is a native secure field.

## The agent's tools

| tool | answers |
|---|---|
| `secret_request {name, purpose}` | `{secret, set}`. When it isn't set yet, end the turn: a message arrives once it is. |
| `secret_list {}` | `[{secret, set, purpose, usedAt}]` |
| `secret_run {command, env?, files?, cwd?, timeout?}` | `{code, timedOut, stdout, stderr}`, masked. `env {VAR: name}` puts the value in the command's environment. `files {VAR: name}` writes it to a 0600 file in a 0700 folder, removed after the run, and puts the file's path in VAR (`tailscale up --auth-key=file:$TS`). |

`secret_run` gets no new authority. It passes the same stale-generation, standing-rules and mail-charter gates as any tool, and the charter counts it among the shell tools. A hub on a copied log (`BACKPLANE_COPY=1`) refuses it.

The command runs in its own process group. Output is captured as it streams, keeping the last 64K characters of each stream. Once the command exits, its streams get 2 s, then the group is killed. At the timeout the group gets SIGTERM, then SIGKILL 2 s later, and the run is settled 3 s after that whatever still holds a pipe. Only the child's own group is signalled; an empty, 0 or 1 PID never is. The real exit code is kept, or 124 on a timeout. The answer is masked with the values this run injected, and their JSON-escaped and percent-encoded forms, as well as every current value. So a value replaced or deleted while the run works stays masked.

Every prompt carries a `<secrets>` block: the names set, how to use them, and "never ask for a secret in chat; use secret_request".

## Storage

- The vault is `<home>/secrets/vault.json` (0600, folder 0700). Its metadata (name, purpose, who asked, when set, last use and by which thread) is plaintext. Each value is sealed with AES-256-GCM, with the name as associated data.
- The master key lives in the OS keychain: Secret Service (`secret-tool`) on Linux, the login keychain (`security`, written through its stdin) on macOS, DPAPI on Windows. Otherwise it's in `<home>/secrets/vault.key` (0600).
- The backend is fixed at the first save. If it later stops answering, the vault reports locked. It is never re-keyed.
- **Fail closed.** These all lock the vault: a `vault.json` that can't be read, isn't JSON, has another version, names an unknown key store, isn't shaped as `{v:1, key, items:[{name, iv, ct, …}]}`, names a secret twice, or holds a value that doesn't open with the key. Only a missing file counts as a new vault. A locked vault keeps what it read: every name for Settings, and every value that did open for masking. It refuses saves, deletes and runs, so nothing is written over the evidence.
- **Staged writes.** A save or delete writes a temporary file, flushes it and renames it over `vault.json`. Only then does the change replace what's in memory, and with it the masking set. A failure before the rename leaves memory, disk and masking as they were (tested with injected write, fsync and rename faults).
  - After the rename the folder is flushed. If that fails with a real I/O error, the change stands (it's on disk and in use), but the answer is `committed …`, not ok, the hub logs a warning, and the vault's status reports `flush: unflushed <code>`.
  - A file system that can't flush a folder (EINVAL, ENOTSUP) is accepted and recorded as `unsupported <code>`.
- `BACKPLANE_VAULT_KEYRING=file` forces the key file (tests).
- The hub opens the vault before any turn resumes. Values stay in the effect's memory (`src/server/effects/vault.js`), and Bend reads them only to mask (`Vx.vault`).

## Where a value travels

1. The box's field. The field has no binding. At Save, the host (`host.js`'s `secretSave`; on phones, `Backplane.secret`, fed by the app's vault field) reads it once and sends the `secret.set` request that `Sbox.req` builds. It travels only in that request: never as a view action, a draft, the outbox or the client's state. A `bfield` named `sval` (an older phone) is dropped.
2. The hub's `Hub.rpc.parse` hands it to `VaultFx.put`. It goes into no change, no out, no reply and no log line.
3. The sealed file and the effect's memory.
4. A `secret_run` child's environment, or a temporary file.

## Masking (defence in depth)

`Sec.mask` turns every character inside an occurrence of a stored value into `•`, keeping the length. The API keys' values (`api.env`, 8 characters or more) are masked too. Masking applies at:

- `Hub.tagged`: every agent output line as it's read;
- `Hub.step.go`: every change before it is written and applied (`Secret.changes`: each change's JSON through `Sec.json`), so the log, the store, FTS, mail, the state and every client are covered;
- `Hub.step.go`: a step's outs too (`Secret.outs`): a turn's end carries its error and reply to the server for the push webhook, and both are masked as the changes they were read from are;
- `Mcp.ask.raw`: every MCP answer;
- `Spawn.agent`: every agent's launch arguments (Claude, Codex, the ACP, MSP and RPC runners, the writer's one-shot), wherever the step or out that carried the text came from;
- `Agent.write` and the one-shot's stdin: every line to an agent;
- `secret_run`'s output.

**Cost.** In the hub's bundle the clean check is `String.contains`, native already. `Sec.fine`'s body is native too (`scripts/js-native.py`): the same cover computed by `indexOf` at code point boundaries, fenced by `secret.bend`'s hash. `test/tools/secret.mjs` builds the module the way `build-js.sh` does and checks the native body against the pure definition on 300+ inputs (overlaps, emoji, lone surrogates). Measured there, a 4 MB line with a secret masks in about 130 ms and a clean one in about 0 ms. The pure walk takes about 140 ms for 10 KB. Sec.mask's laws hold whatever `Sec.fine` answers, because the pure check follows it.

## Laws

LAWS.bend's "secret box" block, with proofs in `test/secret_proof.bend`:

- `sec_mask_total`: no stored value occurs in masked text.
- `sec_vault_valid` and `sec_vault_total`: the vault the hub builds is always valid, so masking with it is total, with no premise.
- `sec_mask_idem`: masking twice is masking once.
- `sec_mask_clean_id`: clean text is left alone.
- `sec_empty_id`: an empty vault masks nothing.
- `sec_json_total`: every string and key of a masked JSON tree is clean.
- `sec_handle_shape`: a handle holds only the name and the set flag.
- `sec_changes_empty`: with nothing stored, a step's changes pass through unencoded.

Stored values live only in `SecVal`, gathered in a `Vault`. `test/tools/secret.mjs` gates the source: `SecVal{` and `Vault{` appear only in `secret.bend`, `vault.bend` and the proofs, and no other type names either in a field. So no Change, mail, prompt, render or tool-result constructor can hold one.

## Limits

Agents run as the same OS user as the hub. The box stops accidental exposure: transcripts, logs, the model's context, other clients, backups. A determined agent with a shell could still read the keychain or the key file. What it prints reaches the hub masked, but its own tool output reaches its model directly. Lines logged before a secret was stored aren't rewritten.

Tests:
- `bend test/secret_test.bend -o X.mjs && bun test/tools/secret.mjs X.mjs`: masks, pure and native; the source gate; and the vault's fail-closed, fault-injection, run-lifecycle and race negatives.
- `bun test/tools/secret_mcp_e2e.ts BINARY WIRE`: the tools through a real hub's gates. A copied log refuses `secret_run` and runs nothing; an ordinary hub runs it with masked output; an old process's call is refused as stale.
- `bun test/tools/secret_e2e.ts BINARY WIRE`: stand-in claude and codex capture their argv and stdin; the event log, the store and every frame are checked for the value.
