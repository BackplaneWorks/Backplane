#!/bin/sh
# on a host with bun: the policy_room check against staged-bins/bp-policy-page (synthetic events only)
export PATH="$HOME/.bun/bin:$PATH"
chmod +x staged-bins/bp-policy-page
exec bun test/gpui/slim/policy_room.mjs "$PWD/staged-bins/bp-policy-page" 3871
