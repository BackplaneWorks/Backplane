# Pairing end to end

Every client pairs with every hub, on LAN and on tailnet, through its real pairing UI. The hub decides each check: its device list, and the sockets it holds.
A cell is one hub, one client, one route (`docs/e2e/MATRIX.md`, rows PA..).

## A hub for a cell

A temp-home hub per cell, at the address under test, nothing else of the person's touched: `backplane-hub --home <tmp> --port <free> --host <LAN or tailnet IP> --no-tailscale --foreground`
(BACKPLANE_AGENTS pointed at a temp folder too). Its token is `<tmp>/token`. Pick a free port each time: fixed ports collided with other runs.
Wait until it answers (`rig.py BASE TOKEN wait`) before a client starts: the token is written before the hub listens.
Most drivers also take `--restart CMD` and `--expire CMD` (restart the hub; move its open invitations' `expiresAt` into the past, then restart). Without them those checks are `not run`.

## Drivers

| Driver | Client | Runs where | Needs |
|---|---|---|---|
| `gpui.sh` | the GPUI desktop client, a private instance (own config, own display) | the client's machine (LAUNCH runs it; the hub may be elsewhere) | `BASE TOKEN GPUI WORK LAUNCH [RESTART EXPIRE]`, see its header |
| `android.py` | the Android app (debug build, `dev.backplane.mobile.gpui`) | any machine with adb to the device | `--base --token-file --adb --sockets [--restart --expire --device --fresh]` |
| `ios_sim.py` | the iOS app on a simulator (no taps: debug hooks) | any machine with ssh to the Mac | `--base --token-file --sim --app --sockets [--restart --expire --mac]` |
| `proto.py`, `settings_revoke.py` | none: the protocol and Settings revoke, against the hub | anywhere that reaches the hub | a hub result: write `pass, hub only` |
| `gpui_hubs.sh`, `link.py`, `unified.py`, `bots.py` | several hubs in one client; linked hubs | see each header | |

A LAUNCH for `gpui.sh` (how a client starts on its machine) must: name the client's config folder by `RIG`, so a later step with `RIG=link` reuses that client's
settings; and after a pairing wait for the copy of itself the client starts (`pairing.rs` relaunches) before returning, or the checks read the log before
"Connected" is in it (`test/windows/rig/gpui-start.sh` does both). RESTART and EXPIRE run where the driver runs: a driver on another machine needs them over ssh,
else those two checks say not run. "Saved privately" (the first hub token and `hubs.json`) is mode 600 on Linux and macOS; on Windows `icacls` must list the user and no Users,
Everyone, Authenticated Users or inherited (I) entry.

`--sockets` is a command run where the hub is that prints how many connections are established to its port from the client (Linux `ss`, macOS `netstat`).
On an emulator pass `--fresh "scripts/android-emu.sh fresh"`: it resets the app in its own process (a `pm clear` per check leaks the emulator's memory).
A real phone (`--device "Pixel 9, a real phone"`) keeps `pm clear`. One lane at a time on a shared phone: `flock ~/.cache/pixel-<serial>.lock <command>`.

## Results

Every driver ends with `summary pass=N fail=N not_run=N`; a `not run` line says why. A matrix row is
`| ID | pass (N/N) | <commit> | <hub> ↔ <client>, <route> | <driver>; <log path>; <what was not run and why> |`.
A cell counts only on the commit named; a simulator or emulator result says so; a hub-only result never fills a client cell.
