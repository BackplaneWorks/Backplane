# Linux compatibility

The x64 CI and release builds use Ubuntu 22.04 (Jammy, glibc 2.35), so the
same tarball, AppImage and `.deb` can also run on newer Ubuntu releases.
CI runs the resulting x64 binaries on both Ubuntu 22.04 and 24.04. Clang 19
comes from LLVM's Jammy repository; this does not upgrade the build's libc.
The arm64 build continues to use Ubuntu 24.04 (glibc 2.39).

CI checks the imported glibc symbols in **every** shipped ELF, including the
compiled Bun helpers, against that architecture's floor. The Debian package's
`libc6` dependency is computed from the same complete set. AppImage does not
supply a replacement libc: changing its container alone cannot make a binary
built for a newer libc work on an older distribution.

KiCad is a separate runtime. A working locally built KiCad 10 can be selected
with an absolute executable path in Settings → KiCad, or with
`BACKPLANE_KICAD_CLI=/absolute/path/to/kicad-cli`. The CLI fallback handles
exports and checks; the Backplane fork is required for its headless IPC
server. Installing Backplane does not make a newer KiCad runtime compatible
with an older host. Keep a working local KiCad selected on such a host.

Download CI artifacts from the pull request's **Build linux-x64** job to try
a change before a release. Publishing release assets still requires the
maintainer's normal tag/release workflow.
