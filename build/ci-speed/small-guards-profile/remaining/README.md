# Remaining five non-Bend guard timings

Normal Boat job `j20261104065843-288456`, owned work1553208, completed the five
commands that the earlier checkpoint timeout prevented from starting. It did
not rerun the first seven guards. Frozen source is
`22d5e2202204371d3ad58f052f87cd4f89501b1f`, tree
`2544558e0e87f8c7b7f35ded93e87e471e80e9aa`; this is not a current-main/full-CI
qualification. The exact literal AST inventory and explicit argument lists
select indices7–11 of `scripts/local-small.py`.

| Original command | Exit | Wall | Peak command RSS |
|---|---:|---:|---:|
| bun test/tools/store_promote_test.cjs | 0 | 13.04s | 73,824KiB |
| bun test/tools/hotboot_wire_test.cjs | 0 | 0.02s | 48,384KiB |
| python3 test/tools/kicad_library_header_test.py | 0 | 0.14s | 24,320KiB |
| python3 scripts/kistack.py verify | 0 | 0.05s | 20,992KiB |
| python3 test/tools/kistack_test.py | 0 | 0.50s | 24,832KiB |

Sum13.75s; measured worker loop14.162s. Each command ran once, sequentially,
in one cold VM. No variance, before/after speedup or equivalence to the complete
production controller environment is established. Promotion's11 original
cases pass; no optional native probe/hub is supplied. KiStack tests exercise
resource copies in disposable folders, not an app compile or package build.

Bun1.3.10 and Node24.13.1 executable SHA256s are asserted and versions retained
in STARTED.json. Entire Git-export source hash/mode inventory, harness and tool
identities match at exit. No Bend is installed. Native `/tmp/bp-bend.lock` is
held and independently probed. The actual worker cap is3,699,998,720B,
controller536,870,912B; both swap0, combined caps4,236,870,912B <4GiB.
Worker peak473,722,880B includes tool provisioning; controller peak375,795,712B.
All max/OOM counters zero. VM-wide network counters are recorded per command;
background transport can contribute, so nonzero counters do not attribute
network use to a test. No local tests run.

Exact frozen `runner.py` SHA256:
`5cc2be64671fde3b4e15f9037709300b14909379c14f07fa46b9499ab63ad2f5`.
Source-only review identified two unexercised timeout limitations: the520s
controller budget is tight against five worst-case90s commands plus teardown
and tool setup; and an uninterruptible process can exceed the post-SIGKILL
five-second wait, aborting the remaining loop. This run reached neither path.
The archived harness is not silently repaired or promoted to a scheduler.

Whole remote command31.81s,38credit-s/$0.00038; sandbox `bx_797azggu` deletion
verified. This profile changes the next investigation: promotion's fixture
setup is the dominant remaining cost; the other four total0.71s.
