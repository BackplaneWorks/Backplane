# Build times and peak memory

Only measured numbers are here. "not measured" means no run has recorded it. Cold = a fresh target or derived-data folder; warm = touch one source file and rebuild in the same folder. Peak RSS is the Maximum resident set size from `/usr/bin/time -v` (KiB) unless the row says otherwise. Boat and h@backplane are different machines; the Machine column says which.

Commit: the h@backplane rows were measured on the tree of mech/build-md (base gpui 75bd853db5; the remote summary says "unknown" because the upload had no .git). Boat and screens' rows are from the jobs named.

| Target | Machine | Cold wall | Cold peak RSS | Warm wall | Warm peak RSS | Source |
|---|---|---|---|---|---|---|
| Rust hub, release (`cargo build --release --bin backplane-hub`) | h@backplane, 16 CPU, 30 GiB, rustc 1.99.1 | 0:42.78 | 1,882,032 KiB | 0:35.64 | 1,889,848 KiB | `scripts/gpui/measure-linux.sh` via bp-build (lane measure) |
| Whole test suite (`cargo test`, native/host) | h@backplane, same | 1:07.35 (build of the tests and the run) | 2,577,404 KiB | 0:39.74 | 1,382,416 KiB | same |
| GPUI desktop, release (`cargo build --release` native/gpui) | h@backplane, same | 2:10.87 | 2,139,044 KiB | 1:04.31 | 2,138,000 KiB | same |
| GPUI desktop, release | Boat, large (8 CPU, 15 GiB) | 6:50.82 wall, 99% CPU | 1,989,672 KiB | not measured | not measured | screens' build.time (job j20261105114629-2728427); the cold state is not recorded in that artifact |
| Rust hub, release | Boat, large | not measured | not measured | 50.15 s (job j20261105113017-2614253, `Finished release`; not verified warm) | not measured | that job's bins101 log |
| iOS app (simulator, Debug) | Mac (hs-mac-mini), under `mac-heavy` | not measured yet (`scripts/gpui/measure-mac.sh` written, not run) | not measured | not measured | not measured | none |
| Android APK | Boat or h@backplane | not measured (needs the Android SDK and JDK setup; the only one is in the Boat job) | not measured | not measured | not measured | none |
| Bend policy build (split, clang) | Boat, xlarge sandbox | killed at 8.0 GiB peak (8,406,084 KiB), wall 83 s, during build-split | 8,406,084 KiB at the kill: did not fit 8 GiB | not measured | not measured | slim's gate-policy job j20261105162351-994994 |
| Bend bridge native build | h@backplane | not measured (no bun or bend on h@backplane) | not measured | not measured | not measured | none |
| Whole-branch test suite (crate, Boat) | Boat, large | not measured per run; the gate job's whole run was 667 s wall, 4,307,288 KiB peak (includes the replay and the wide scenarios) | not measured per suite | not measured | not measured | gate2 job j20261105160055-804575 |

Notes:
- The Boat job lines (`peak RSS … wall …`) cover a whole job, not one build; they are not used above as a build's numbers unless the row says so.
- The policy: no build has fit 8 GiB on a large sandbox yet; slim's cap check is pending. No policy number is given beyond the kill.
- The iOS, Android and bridge rows are not measured yet; they wait on the Mac, the Android setup and bun respectively.
