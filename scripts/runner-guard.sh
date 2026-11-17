#!/bin/sh
# A self-hosted runner's job-started hook (ACTIONS_RUNNER_HOOK_JOB_STARTED=<this file> in the runner's .env; bp-win runs scripts/runner-guard.ps1, the same
# rules): refuse every job but the nightly's and the release's. This repo is public and a fork's pull request runs its own copy of the workflows, which
# could name this runner's label; the org is on GitHub Free, so runner groups cannot be limited to workflows. A non-zero exit fails the job before any of
# its steps runs, and the hook lives on the runner, where no workflow can change it. (docs/release-runners.md)
#   nightly.yml on main, from its schedule or by hand (it calls release.yml: GITHUB_WORKFLOW_REF stays nightly.yml's)
#   release.yml on a v* tag, from the tag's push
#   release.yml on main, by hand (a dry run)
r=${GITHUB_WORKFLOW_REF:-}; e=${GITHUB_EVENT_NAME:-}; w=BackplaneWorks/Backplane/.github/workflows
[ "${GITHUB_REPOSITORY:-}" = BackplaneWorks/Backplane ] || { echo "refused: repository ${GITHUB_REPOSITORY:-?}" >&2; exit 1; }
ok=; ro=
# a runner-guard.release-only file beside this script takes no nightly (release.yml only), as on bp-winbuild (runner-guard.ps1)
[ ! -e "$(dirname "$0")/runner-guard.release-only" ] || ro=1
case "$r" in *[!A-Za-z0-9._/@-]*) ;;   # a ref with anything else in it (a space, a glob) is refused outright
  "$w/nightly.yml@refs/heads/main") [ -n "$ro" ] || case "$e" in schedule|workflow_dispatch) ok=1;; esac;;
  "$w/release.yml@refs/heads/main") [ "$e" = workflow_dispatch ] && ok=1;;
  "$w/release.yml@refs/tags/v"?*) [ "$e" = push ] && ok=1;;
esac
[ -z "$ok" ] || exit 0
echo "this runner takes only nightly.yml and release.yml from main or a v* tag: ${e:-?} ${r:-?} refused" >&2
exit 1
