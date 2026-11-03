# Heavy local work

The guarded check, test, and build scripts share `/tmp/bp-bend.lock`. For a
command without its own guarded entry point, start it through the guard:

```sh
sh scripts/heavy-command.sh proof -- bend PROOF.bend
sh scripts/heavy-command.sh check -- bend src/server/server.bend --check-only
sh scripts/heavy-command.sh test -- bun test path/to/test.ts
sh scripts/heavy-command.sh browser -- bun test/tools/example_e2e.ts
sh scripts/heavy-command.sh build -- bun build path/to/entry.ts
```

Profiles reserve measured peak plus execution margin; locally the host must
also have eight GiB available for other work. The guard refuses immediately
when memory or the native slot is unavailable, or offloads an eligible clean
command to Boat. Boat's runner marks its own sandbox so its admission uses the
sandbox's memory instead of the local host reserve. This marker is a runner
contract, not a security boundary; do not set it on the host. Boat jobs have
independent slots. Guarded descendants inherit the native lock descriptor.

This does not intercept arbitrary direct executable calls. Hub-managed named
locks other than `bend` need the separate launcher handoff before they share
this slot; until then, explicitly wrap heavy commands as shown above. Keep
`BUN_JSC_maxPerThreadStackUsage` at the existing measured setting where needed;
do not copy a large stack setting into ad-hoc commands.
