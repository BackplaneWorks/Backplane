#!/usr/bin/env python3
"""One target's release freeze and desktop stage request, written directly
(no root policy): the freeze compose-target-freeze would write for that policy.
  desktop/freeze-target.py TOOLING_CHECKOUT COMPONENT_DIR SERVICE_JSON OUT_DIR
COMPONENT_DIR is prepare-components.cjs's <out>/<target>; SERVICE_JSON the
release's service component (configured-service-<target>/bootstrap-service/component/SERVICE.json)."""
import hashlib, json, subprocess, sys
from pathlib import Path
tool, comp, service, out = (Path(a).resolve() for a in sys.argv[1:5])
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
ref = lambda p: {"path": str(p), "sha256": sha(p), "bytes": Path(p).stat().st_size}
inputs = json.loads((comp/"COMPONENT-INPUTS.json").read_text())
target, release = inputs["target"], inputs["release"]
svc = json.loads(service.read_text()); payload = service.parent/"payload"
build = json.loads((payload/"BUILD.json").read_text())
assert svc["target"] == target and svc["sourceCommit"] == release["commit"] and svc["version"] == release["version"], "service is another release/target"
epoch = int(subprocess.check_output(["git", "-C", str(tool), "log", "-1", "--format=%ct", release["commit"]], text=True))
out.mkdir(parents=True)
freeze = {"schema": 1, "kind": "backplane-release-freeze", "status": "FROZEN", **release,
  "sourceDateEpoch": epoch, "electronVersion": "44.9.2", "compileInputs": build["compileInputs"],
  "components": {target: {"shellReceiptSha256": sha(comp/"SHELL.json"), "electronReceiptSha256": sha(comp/"ELECTRON.json"),
                          "serviceReceiptSha256": sha(service), "webManifestSha256": sha(payload/"WEB.json")}},
  "signingPolicy": {target: "unsigned-portable-allowed"},
  "lifecyclePolicy": {target: {"mode": "resident-service", "managedStop": False, "managedUpdate": False}},
  "executionTooling": {"stageAdapterSha256": sha(tool/"desktop/prepare-native-stage.cjs"), "nativeSmokeSha256": sha(tool/"desktop/test/release-smoke.cjs"),
                       "serviceComponentProducerSha256": sha(payload/"EXECUTION-HELPER.py"),
                       "serviceRuntimeFactorySha256": sha(payload/"runtime-evidence/SERVICE-RUNTIME-PRODUCER.py")},
  "scope": "Staging freeze for this target only, written directly (no root policy). Desktop/sandbox/runtime acceptance pending.",
  "published": False}
(out/"FREEZE.json").write_text(json.dumps(freeze, indent=2) + "\n")
request = {"schema": 1, "kind": "backplane-electron-release-inputs", "target": target,
  "release": {**release, "freezeReceipt": ref(out/"FREEZE.json")},
  "shell": {"receipt": ref(comp/"SHELL.json"), "payload": {"directory": str(comp/"shell-payload")}},
  "electron": {"receipt": ref(comp/"ELECTRON.json"), "payload": {"directory": str(comp/"payload")}},
  "service": {"receipt": ref(service), "payload": {"directory": str(payload)}}}
(out/"stage.request.json").write_text(json.dumps(request, indent=2) + "\n")
print(out/"stage.request.json")
