# KiCad fork CI cache measurements

Measured November 17, 2026 on GitHub-hosted runners in
BackplaneWorks/Backplane_KiCad. Package comparisons ran on the desktop.
All six builds used source commit `20abbdd28898c00ccc86fa79fc9153464b57287b`
(10.99.0-bp.13.2). These were measurement builds, not published releases.
The Windows VM measurements belong to the separate winbuild lane.

## Build times

Minutes below exclude queue time. Cold means the compiler cache was empty;
Windows used ccache with `/Z7` in both rows. The build step also includes
installation, and on macOS and Windows, relocation, tests and packaging.

| Platform | Cold run | Warm run | Job minutes, cold → warm | Build-step minutes, cold → warm | Cold hits / misses | Warm hits / misses |
|---|---|---|---|---|---|---|
| macOS ARM64 | [44148808071, attempt 2](https://github.com/BackplaneWorks/Backplane_KiCad/actions/runs/44148808071/attempts/2) | [44169357783](https://github.com/BackplaneWorks/Backplane_KiCad/actions/runs/44169357783) | 81.2 → 12.4 | 77.5 → 8.4 | 40 / 2444 | 2483 / 1 |
| Linux x86_64 | [44148815024](https://github.com/BackplaneWorks/Backplane_KiCad/actions/runs/44148815024) | [44172451264](https://github.com/BackplaneWorks/Backplane_KiCad/actions/runs/44172451264) | 127.3 → 28.6 | 101.7 → 0.6 | 40 / 2452 | 2491 / 1 |
| Windows x86_64 | [44148821554](https://github.com/BackplaneWorks/Backplane_KiCad/actions/runs/44148821554) | [44190126170](https://github.com/BackplaneWorks/Backplane_KiCad/actions/runs/44190126170) | 182.8 → 26.0 | 179.7 → 21.6 | 45 / 2463 | 2507 / 1 |

Job and step durations come from the Actions jobs API. Cache hit counts are
per build, subtracting restored cumulative counters where needed. Each warm
build hit on about 99.96% of cacheable calls; the remaining miss was
`common/build_version.cpp`, which contains `__TIME__`.

Windows compile time fell from 2:34:33 to 0:05:41; configure, including
vcpkg and CMake, fell from 13:19 to 4:31. Windows warm ran on the temporary
`cache-test/bp13.2-20abbdd` branch because the release line had moved.
It restored the cold run's main cache after promote
[44181258471](https://github.com/BackplaneWorks/Backplane_KiCad/actions/runs/44181258471)
succeeded. `BACKPLANE_WIN_CCACHE` was unset once the warm job had started
and read it, and remains unset. These Windows gains require enabling it.

## Package checks

Every warm build passed the same in-job gates as its cold build, including
the applicable IPC, relocation and embedded GUI checks. After unpacking all
inner archives, paths and file types matched exactly:

| Platform | Files on each side | Byte-identical files | Differences |
|---|---|---|---|
| Linux | 115,323 | 115,321 | `libkicommon` build time and its GNU build ID; image archive member dates |
| macOS | 68,441 | 68,434 | `libkicommon` build time, Mach-O UUID and dependent signatures; archive dates; Homebrew install time and tap revision (same packages and versions) |
| Windows | 68,070 | 68,024 | 22 KiCad executable/DLL PE timestamps, checksums and PDB identifiers; 22 PDBs; image archive dates and `SHA256SUMS`; `kicommon.dll` also contains the build time |

All Windows third-party/vcpkg DLLs were byte-identical. Archive checksums
changed with archive dates and the binary differences above. The packages
are not byte-for-byte reproducible: the comparison found no differences
beyond build/link/install metadata and the signatures that cover it.

## Toolchain invalidation

Temporary commit `0afa29aca6` changed only the toolchain-ID input on
`cache-test/toolchain-miss`. All compiler-cache restores missed:

| Platform | Run | Missing compiler-cache prefix |
|---|---|---|
| Linux | [44196553466](https://github.com/BackplaneWorks/Backplane_KiCad/actions/runs/44196553466) | `kicad-ccache-Linux-2eee2fb2844e-` |
| Windows | [44196559201](https://github.com/BackplaneWorks/Backplane_KiCad/actions/runs/44196559201) | `kicad-windows-ccache-6226bdfed907-` |
| macOS | [44196549313](https://github.com/BackplaneWorks/Backplane_KiCad/actions/runs/44196549313) | `kicad-macos-ARM64-8d3a2329e531-` |

Windows independently restored its unchanged vcpkg cache. The runs were
cancelled after restore and all reached completed/cancelled. This proves
key separation when the toolchain-ID input changes; it does not simulate
installing a new compiler or runner image. Both test branches were deleted.

## Cache ownership and size

Branches upload one-day cache artifacts; the restricted promote workflow
on main saves and prunes the rolling entries. It accepts successful push
or dispatch runs from this repository on eligible candidate or release
branches, not PRs, forks or feature branches. Keys include a toolchain
fingerprint and a run ID for immutable cache rotation, not the source SHA.
Compiler content checks also remain enabled. The compiler-cache size caps
are 500M each for Linux/macOS and 3.5G for Windows.

The integrator landed the Linux trigger-name/manual-promote fix, the
Windows cache-upload guard (`9eaab8d427`), and the rejection of compiler
artifacts containing fewer than 1,000 files (`516c414667`). A rejected
artifact leaves the existing main cache in place.

Final cache API snapshot: **5,592,863,248 bytes (5.59 GB), 17 entries**,
under the 6 GB target. Main holds four entries totaling
**3,093,110,563 bytes (3.09 GB)**: Windows compiler cache 2,510,227,669;
Windows vcpkg 142,828,665; macOS 234,780,591; Linux 205,273,638.
The remaining entries are the explicitly protected legacy release and
candidate caches. Two approved stale feature entries totaling 427,153,350
bytes were removed after confirming their branch had no active run.

The manual candidate guard and batching rule are in
[the release playbook](release-playbook.md#candidates). Wait for cache
promotion before dispatching a subsequent cycle or pushing a release tag.
