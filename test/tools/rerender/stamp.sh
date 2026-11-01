#!/bin/sh
# What a build of this tree is: a hash of what goes into the hub and the web
# client, src/ and scripts/ (their committed blobs, then any uncommitted
# change and untracked file there), so a commit elsewhere (a test, a doc)
# leaves it alone. build.sh writes it beside both; rerender.ts compares.
cd "$(dirname "$0")/../../.."
{ git ls-files -s -- src scripts; git diff HEAD -- src scripts; git ls-files --others --exclude-standard -- src scripts | xargs -r cat; } | sha256sum | cut -c1-16
