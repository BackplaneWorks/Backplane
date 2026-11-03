# Pairing validation

Source baseline: 61752742f. All application fixtures use separate homes,
stand-in providers and owned processes. No live token file or client pairing is
read, replaced, revoked or unpaired. No deployment or release is claimed here.

Passed:

- Server, web and shared mobile Bend type checks; production service emit/build,
  web bundle and shared mobile bridge build.
- Android debug APK and iOS simulator compilation.
- Packaging checks: three pairing registration tests, five local workflow tests,
  three macOS bundle tests (one additional test skipped on Linux) and six Linux
  launcher tests.
- Thirteen browser/desktop/store unit cases (including rate limiting and atomic
  registry persistence/write failures) and the existing 28 pairing-token
  filesystem contracts. These preserve #754's original initialization behavior.
- All 24 full-output sidebar-image cases, with source review and an updated
  dependency receipt; no scanner/filter/authority changes.
- Real hub HTTP enrollment by link, decoded QR and short code, wrong/expired/used
  codes, revoke of an active websocket, another session staying valid, re-pairing and no credentials in hub
  logs. Expiry uses an isolated fixture-only clock preload, never a production
  bypass or a change to the system clock.
- Real browser link, manual code and local QR-image controls; HttpOnly session
  cookie; fragment/history cleanup; first-run and automatic re-pair on live
  revocation without reload; mobile-width layout.
- Actual Linux Electron app: link pairing, selecting a remote hub, preserving its
  local hub credential, distinct device session and automatic full-screen
  re-pair after revocation. Node Playwright and X11 are required; initial Bun and
  headless driver attempts did not qualify this check.
- Native iOS on a new dedicated simulator: first-run, invitation exchange,
  malformed invitation preserving saved pairing, automatic re-pair prompt on
  revoke and successful re-pair. The existing private simulator launch hook
  delivers the URL; the OS's external "Open in Backplane?" confirmation was
  observed but not automated. No existing simulator was used or erased.
- Native Android on a dedicated API-35 emulator: first-run, invitation exchange,
  malformed invitation preserving its durably saved pairing, automatic
  full-screen re-pair after revocation and short-code re-pairing. The final
  combined HTTP/browser/Android run passed 41 checks. The emulator is explicitly
  woken and unlocked before UI assertions; the prior sleeping-screen timeout
  is not counted as a pass.

Full proof: **failed at Laws.mail_file_one_conv**. The identical failure was
reproduced on unmodified baseline 61752742f with the same verified Bend 2.0.36
compiler on h@backplane. LAWS.bend and PROOF.bend are unchanged. This is not a
claim that the full proof or complete repository default suite passed.

Runtime evidence is in this folder's web-final.log, android-runtime-final.log
and ios-final.log; source tests live under test/tools/pairing* and
 desktop/test/pairing*. The initial driver/SDK/lock failures are retained in the
other logs and are not counted as product passes. The full 32-story release
suite, physical camera hardware, Windows/macOS desktop runtime, signed mobile
artifacts and live installation were not tested in this scoped change.

Integration: rebased onto e2fe649ec (concurrent manual parent precedence,
subthread wording and deployment review receipts). No pairing implementation
conflicts. All 24 sidebar cases passed again on the combined source. Platform
runtime/build receipts above precede this unrelated-main rebase; not claimed as
fresh platform builds of the combined commit. All upstream laws/proofs and
review receipts are retained.
