#!/bin/sh
# A self-hosted runner's job-started hook (ACTIONS_RUNNER_HOOK_JOB_STARTED=<this file> in the runner's .env): refuse every job but the nightly's.
# This repo is public and a fork's pull request runs its own copy of the workflows, which could name this runner's label; the org is on GitHub Free, so
# runner groups cannot be limited to workflows. A non-zero exit fails the job before any of its steps runs. (docs/nightly.md)
case "${GITHUB_EVENT_NAME:-}" in schedule|workflow_dispatch) ;; *) echo "this runner takes only the nightly: event ${GITHUB_EVENT_NAME:-?} refused" >&2; exit 1;; esac
[ "${GITHUB_WORKFLOW_REF:-}" = "BackplaneWorks/Backplane/.github/workflows/nightly.yml@refs/heads/main" ] || { echo "this runner takes only nightly.yml on main: ${GITHUB_WORKFLOW_REF:-?} refused" >&2; exit 1; }
[ "${GITHUB_REPOSITORY:-}" = BackplaneWorks/Backplane ] || { echo "refused: ${GITHUB_REPOSITORY:-?}" >&2; exit 1; }
exit 0
