# bp-win's job-started hook (C:\actions-runner\.env: ACTIONS_RUNNER_HOOK_JOB_STARTED=C:\actions-runner\runner-guard.ps1): scripts/runner-guard.sh's rules,
# for a runner without sh. Only nightly.yml (main: schedule, by hand) and release.yml (a v* tag's push; main or a release/<name> branch by hand) run here. (docs/release-runners.md)
$r = "$env:GITHUB_WORKFLOW_REF"; $e = "$env:GITHUB_EVENT_NAME"; $w = 'BackplaneWorks/Backplane/.github/workflows'
$ok = $false
# a runner\runner-guard.release-only file beside this one (bp-winbuild: release-runner-setup.ps1 writes it) takes no nightly: release.yml only
$releaseOnly = Test-Path (Join-Path $PSScriptRoot 'runner-guard.release-only')
if ("$env:GITHUB_REPOSITORY" -ceq 'BackplaneWorks/Backplane') {
  if (-not $releaseOnly -and ($e -ceq 'schedule' -or $e -ceq 'workflow_dispatch') -and $r -ceq "$w/nightly.yml@refs/heads/main") { $ok = $true }
  if ($e -ceq 'push' -and $r.StartsWith("$w/release.yml@refs/tags/v", [StringComparison]::Ordinal) -and $r.Length -gt "$w/release.yml@refs/tags/v".Length) { $ok = $true }
  if ($e -ceq 'workflow_dispatch' -and ($r -ceq "$w/release.yml@refs/heads/main" -or ($r.StartsWith("$w/release.yml@refs/heads/release/", [StringComparison]::Ordinal) -and $r.Length -gt "$w/release.yml@refs/heads/release/".Length))) { $ok = $true }
}
if ($r -cmatch '[^A-Za-z0-9._/@-]') { $ok = $false }   # a ref with anything else in it is refused outright, as runner-guard.sh does
if (-not $ok) { Write-Error "this runner takes only nightly.yml and release.yml from main, a release/* branch (by hand) or a v* tag: $e $r ($env:GITHUB_REPOSITORY) refused"; exit 1 }
exit 0
