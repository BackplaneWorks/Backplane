# Account fallback and sidebar draft inputs

The account module enters the conservative sidebar import closure through
`src/core/client.bend`. Its only client call sites are
`AFB.Hold.profile.active` in the account settings rows. That helper reads
settings with the account-limit prefix and their reported resets. Its body is
unchanged by this patch.

The changed planner reads public provider account metadata, allowance windows,
thread provider/model/account bindings and account availability holds. The hub
uses it to choose a replacement and to recover a legacy Default-account reset.
The module imports Base, accounts, fallback, JSON and the event model; it imports
no client, sidebar draft, private-image, transport or authority implementation.

The changes introduce no sidebar draft/body reader. They do not change ordinary
draft filtering, unknown `@` retention, the private image key or cache slot, the
closure scanner, transport inputs, recipient ownership or authority. The new
hold-normalization changes are executed by the hub and do not read UI scratch
fields. Existing draft/key/authority laws remain unchanged.

Qualification passed on source `3f43f3440697dec014a971783f627135feb9e2bd`
in Boat job `j20261103030141-4140898` (work `1330605`). Direct execution of
`test/sidebar_draft_image_test.bend` printed all 24 full-output assertions.
The full output SHA-256 is
`cd83cd83cb66e6c0ea17d490d0baf8502c2b965f711f5e731bbd391a9cd5612c`.
The exact account module SHA-256 is
`05ba18dd4ee4f3c1787f69711e376e9934f6782e12454f7fe25bb1d18f2f82fc`.
Only that dependency pin is updated here.

The same serial job checked the new policy proof and passed the synthetic
account-failover, fallback and accounts suites, including cached weekly reset
migration. Peak RSS was 9,247,152 KiB; command wall time was 88.38 seconds.
The sandbox was deleted and deletion verified. No real provider was called.

This is an exact-source receipt, not a full repository proof, application build
or installed-binary acceptance. Later main changes to JSON, messages and view
code were not part of this frozen run; they retain separate review and combined
qualification obligations. The installer owns production compilation and the
private fake-provider installed-binary harness.
