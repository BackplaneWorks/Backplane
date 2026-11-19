#!/bin/sh
# scripts/runner-guard.sh: which jobs a self-hosted runner takes (docs/release-runners.md)
here=$(cd "$(dirname "$0")/.." && pwd); g="$here/scripts/runner-guard.sh"; w=BackplaneWorks/Backplane/.github/workflows; fail=0
t() { # want event ref [repo]
  env -i GITHUB_REPOSITORY="${4:-BackplaneWorks/Backplane}" GITHUB_EVENT_NAME="$2" GITHUB_WORKFLOW_REF="$3" sh "$g" >/dev/null 2>&1; got=$?
  [ "$got" = "$1" ] || { echo "FAIL want $1 got $got: $2 $3 $4"; fail=1; }
}
t 0 workflow_dispatch "$w/release.yml@refs/heads/main"
t 0 workflow_dispatch "$w/release.yml@refs/heads/release/0.16.3"
t 0 push "$w/release.yml@refs/tags/v0.16.3"
t 0 schedule "$w/nightly.yml@refs/heads/main"
t 1 push "$w/release.yml@refs/heads/release/0.16.3"
t 1 pull_request "$w/release.yml@refs/heads/release/0.16.3"
t 1 workflow_dispatch "$w/release.yml@refs/heads/release/"
t 1 workflow_dispatch "$w/release.yml@refs/heads/release/a b"
t 1 workflow_dispatch "$w/release.yml@refs/heads/release/*"
t 1 workflow_dispatch "$w/nightly.yml@refs/heads/release/0.16.3"
t 1 workflow_dispatch "$w/ci.yml@refs/heads/release/0.16.3"
t 1 workflow_dispatch "$w/release.yml@refs/heads/other"
t 1 workflow_dispatch "$w/release.yml@refs/heads/release/0.16.3" Other/Repo
[ "$fail" = 0 ] && echo "runner-guard: ok"; exit $fail
