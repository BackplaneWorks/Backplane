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

Qualification is pending on this exact source. The dependency hash must only be
updated after direct execution of all 24 full-output assertions in
`test/sidebar_draft_image_test.bend`. The installer must retain that source and
log receipt; earlier frozen-source verdicts are not inherited here.
