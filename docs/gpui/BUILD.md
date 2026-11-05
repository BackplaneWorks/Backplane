# Build times and peak memory

Only measured numbers are here. "not measured" means no run has recorded it. Cold means no target or derived-data cache; warm means a one-line touch and rebuild. Peak RSS comes from `/usr/bin/time -v` (Maximum resident set size) unless marked.

Commit: gpui 75bd853db5 (the branch mech/build-md is one commit on it); the GPUI row is from the build at 223b7c683 (screens' build job j20261105114629-2728427).

| Target | Machine | Cold wall | Cold peak RSS | Warm wall | Warm peak RSS | Source |
|---|---|---|---|---|---|---|
| Rust hub, release (`cargo build --release --bin backplane-hub`) | Boat, large (8 CPU, 15 GiB) | not measured (per build) | not measured | 50.15 s (job log `Finished release`, shared target, the bins rerun) | not measured | bins101 job log |
| GPUI desktop, release (`cargo build --release` native/gpui) | Boat, large | 6:50.82 wall, 99% CPU | 1,989,672 KB (1.9 GB) | not measured | not measured | screens' build.time (job j20261105114629-2728427); cold state not recorded in the artifact, assumed fresh but unconfirmed |
| iOS app | Mac (hs-mac-mini) under `mac-heavy` | not measured | not measured | not measured | not measured | none |
| Android APK | Boat | not measured | not measured | not measured | not measured | none in the job artifacts |
| Bend policy build (split, clang) | Boat, xlarge sandbox (not large) | killed at 8.4 GB peak, wall 83 s, during build-split | 8,406,084 KB at the kill (did not fit 8 GiB) | not measured | not measured | slim's gate-policy job j20261105162351-994994 |
| Bend bridge native build | not measured | not measured | not measured | not measured | not measured | none |
| Whole-branch test suite (crate, `cargo test` native/host) | Boat, large | not measured per run; the gate job's whole run was 667 s wall, peak 4.3 GB (includes replay and the wide scenarios) | not measured per suite | not measured | not measured | gate2 job j20261105160055-804575 |

Notes:
- The Boat job lines (`peak RSS … wall …`) cover the whole job, not one build; they're not used above as a build's numbers unless the row says so.
- Policy: the gating binary was not produced on a large sandbox; slim's cap check is pending. No policy number fits 8 GiB yet.
- Nothing here is a warm number except the hub's logged 50.15 s, whose warmth isn't verified.
