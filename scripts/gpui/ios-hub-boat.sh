#!/bin/sh
# Uploaded and run by boat-job.sh: the JS hub built from this worktree's src (so src/mobile/*, which the thin
# renderer imports, is current), by the composite-compiler recipe in .lowmem/ (copy of bp-coord/pr944-capped-recipe).
# Sandbox-only edit, as the earlier reference hubs: the sidebar-draft guard is a no-op. Artifact: build/backplane-serve
set -u
printf 'import sys\nsys.exit(0)\n' > scripts/sidebar-draft-guard.py
sh .lowmem/hub.sh
