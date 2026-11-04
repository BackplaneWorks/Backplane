# #927 native Windows acceptance: blocked before packaging

The current-main package prerequisite failed. This is NOT a native test pass
or a native product failure. No GitHub Windows acceptance job was launched.

Actual source: 2a87e6526abfedda9378567642a9675660293b60, tree
ccd53c6ab6b0cc323b0ebc4349794b054732bc97. The normal/default Boat runner's
provenance and individual production-source hashes bind the attempted source.
The raw command's requested-source label has a transcription error; it is
preserved without alteration in emission-result.raw.json. That label never
selected or fetched source: the command ran the uploaded current-main checkout.
BUILD-BLOCKED.json names the actual provenance revision and records this limit.

Boat j20261104164046-3848506 ran the unmodified canonical hub entry:
`bend src/server/main.bend -o build/kicad-927-native/raw-hub.js`.
A 180-second subprocess timeout did not fire. Bend was SIGKILLed (-9) after
11.267 seconds, at kernel memory.max and memory.peak = 3,699,998,720 bytes,
memory.swap.max = 0, with one oom and one oom_kill event. No usable hub
emission was produced. Web emission, Windows linking and packaging did not
run. The sandbox was deleted and deletion verified. Diagnostic job exit 0
means receipt collection completed, not that compilation passed.

The helper is emit-current-main.py. Raw outcome and Boat provenance are
retained beside this note; no full proof or product qualification is inferred.
No compiler, test or build ran on the user's machine. No resource cap was
raised, no xlarge was used, no proof changed, and no older package was relabeled
as current main. The latest published Windows v0.14.1 predates PR #930.

Still unverified: a real standard-user packaged current-main Windows run,
Program Files and PATH discovery, typing `C:\backplane_kicad\` in the app,
its submitted/saved bytes and selected launcher path, real in-app KiCad
installation/PowerShell extraction, and the reported backslash-b corruption.
Keep #927 open. The installer t1792701529-415584 receives this blocked handoff;
closure must wait for native evidence and shipment in a subsequent release.
