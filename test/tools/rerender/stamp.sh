#!/bin/sh
# What a build of this tree is: the commit, and a hash of the uncommitted
# changes under src/ and scripts/ (nothing else goes into the hub or the web
# client). build.sh writes it beside both; rerender.ts compares.
cd "$(dirname "$0")/../../.."
printf '%s+%s\n' "$(git rev-parse --short=12 HEAD)" "$( (git diff HEAD -- src scripts; git ls-files --others --exclude-standard -- src scripts | xargs -r cat) | sha256sum | cut -c1-12)"
