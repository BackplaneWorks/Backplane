# Desktop release packages

How one release commit becomes the four desktop packages (`backplane-v<ver>-<plat>.tar.gz`, Windows `.zip`). Everything runs from a tooling checkout of `fast/integration` or `main`. The product is the frozen release commit. Nothing here installs, signs or publishes.

1. **Service, per target.** Run `release-bootstrap.yml`, operation `js`, on the frozen commit. Each `configured-service-<target>` artifact has `bootstrap-service/component/{SERVICE.json,payload/}`. A downloaded artifact keeps no file modes. Put back the ones the receipt records before staging:
   `node desktop/restore-modes.cjs SERVICE.json payload`.
   Windows records its own modes (0666), so this step is needed even when the producer runs on Linux.
2. **Shell and Electron.** Use the frozen commit's checkout, clean, with `npm ci --ignore-scripts` in `desktop/` for ws:
   `node desktop/prepare-components.cjs --source CHECKOUT --vendor VENDOR --out NEW [--targets ...]`.
   The shell is packed from that commit's own `desktop/`. The Electron payload is the retained upstream 44.9.2 component for each target (`VENDOR/<target>/{payload,ELECTRON.json}`), copied unchanged. `RESULT.json` has the hashes a root policy needs. The in-app browser is on by default, and 0.12.1 ships with it. `--direct-browser off` packs a shell without it and is kept only as a fallback. With the browser on, the hub the smoke starts needs `BACKPLANE_DESKTOP_BROWSER=1`, unless the hub serves it by default.
3. **Freeze and stage request.**
   - With a root policy: `node desktop/compose-target-freeze.cjs --policy … --components …/COMPONENT-INPUTS.json --service …/SERVICE.json --out FREEZE.json`.
   - Without one: `python3 desktop/freeze-target.py TOOLING CHECKOUT_COMPONENT_DIR SERVICE.json OUT`. That writes the same freeze without a `rootStagingDisposition`, plus `stage.request.json`.
4. **Stage.** `node desktop/produce-release.cjs stage --request stage.request.json --output NEW_STAGE` gives `STAGED_UNQUALIFIED`. For a quick local look, `bp-coord/fast-hub/stage/check-stage.sh` takes the stage's service and app; its `BP_APP=runtime/backplane-desktop` passes `--no-sandbox`.
5. **Smoke, on the target itself,** with the OS sandbox on and the stage bytes unchanged: `node desktop/test/release-smoke.cjs --stage STAGE --out NEW_EVIDENCE`. It needs playwright-core 1.64.1 (`npm ci` in `desktop/`). Keep the whole evidence folder as it is.
6. **Archive, one producer.** `node desktop/archive-after-smoke.cjs --stage STAGE --evidence EVIDENCE --out NEW_ACCEPTANCE --dist NEW_DIST`. It checks:
   - that the evidence is a complete PASS of the smoke the freeze names (`executionTooling.nativeSmokeSha256`);
   - the eight essential checks;
   - an OS-enforced sandbox;
   - the binary it ran.

   It then writes the four proofs and `ACCEPTANCE.json` and runs `produce-release.cjs archive`. The archive and its `.sha256` are in `NEW_DIST`.

macOS: re-seal the `.app` between step 4 and step 5 with an ad-hoc `codesign --force --deep -s -`. Only `_CodeSignature` and the signable paths may change, and `archive` checks that.
