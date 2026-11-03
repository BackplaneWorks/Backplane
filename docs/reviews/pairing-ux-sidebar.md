# Pairing UX sidebar dependency review

The pairing change adds the Devices Settings page and replaces the shared-token
row with an invitation action. It does not add conversation-draft lookups,
scratch fields, sidebar inputs, transport fields or image-authority changes.
Pairing secrets remain outside the hub-drawn Ui and its render/cache keys.

The checked-in manifest was already behind this worktree's base. Reviewed those
additional differences before refreshing it: bounded actor-owned board catalog
inputs; NUL-delimited file paths; request-scoped file-list replies; ordered RPC
outbox values; mechanical model choice and render diagnostics; complete compact
chrome responses; and release-plan validation. The added board-catalog,
pathrecords and rpcout modules take explicit data and read no Ui/draft state.
New mechanical scratch reads are @-prefixed and remain included conservatively;
board catalog state is info-map data, also included. The compact response path
uses complete output checks and does not alter SidebarImage's draft filtering.

No change to roots, bridges, scanner, scratch filtering (including unknown @
keys), draft ownership or cache authority is authorized by this receipt. The
24-case full-output sidebar test must pass on the current dependency closure
before the manifest records new hashes. Earlier receipts are retained as history,
not claimed as validation of this changed closure.

Concurrent main review: preserved the explicit parent/detach precedence change
and approved subthread badge wording. These read relationship settings and
existing count projections, not conversation drafts. Re-ran all 24 full-output
cases successfully on the combined source before recording its hashes.
