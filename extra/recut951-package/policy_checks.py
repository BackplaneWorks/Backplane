"""Pure fail-closed recut policy checks, shared by admission and tiny controls."""
CAP = 8589934592


def need(ok, message):
    if not ok:
        raise ValueError(message)


def source_identity(value, commit, tree):
    need(value.get('sourceCommit', value.get('commit')) == commit
         and value.get('sourceTree', value.get('tree')) == tree, 'Wrong/stale source identity')


def selection(value, commit, tree):
    source_identity(value, commit, tree)
    merges = value.get('normalMergedCommits', {})
    need(all(isinstance(merges.get(str(n)), str) and len(merges[str(n)]) == 40
             for n in (948, 950, 951, 954, 928, 923, 922)), 'Required normal merges missing')
    need({932, 947} <= set(value.get('excludedPRs', [])), 'Both932 and947 must be excluded')
    consent = value.get('lawRemoval', {})
    need(consent.get('state') == 'EXPLICIT_USER_APPROVED'
         and set(consent.get('laws', [])) == {'cad_install_visible_action', 'cad_install_monotonic'}
         and consent.get('receipt') and consent.get('sha256'), 'Exact two-law explicit user consent required')
    q = value.get('fresh24') or {}
    source_identity(q, commit, tree)
    need(q.get('verdict') == 'PASS' and q.get('passed') == q.get('total') == 24
         and q.get('inherited') is False and q.get('receipt') and q.get('sha256'),
         'Fresh24/24 exact-source evidence required')
    review = value.get('pinManifestReview') or {}
    source_identity(review, commit, tree)
    need(review.get('verdict') == 'PASS' and review.get('receipt') and review.get('sha256')
         and review.get('manifest') and review.get('manifestSha256'), 'Actual pin/manifest review required')


def phase(phase, end, commit, tree, archive_sha):
    source_identity(phase, commit, tree)
    need(phase.get('exitCode') == 0 and phase.get('hardMemoryCapBytes') == CAP
         and phase.get('swapMaxBytes') == 0 and phase.get('compilerArchiveSHA256') == archive_sha,
         'Wrong compiler/cap/swap/verdict')
    need(isinstance(phase.get('seconds'), (int, float)) and phase['seconds'] > 0
         and isinstance(phase.get('peakRSSKiB'), int) and 0 < phase['peakRSSKiB'] * 1024 <= CAP,
         'Actual phase wall/RSS metrics missing or over cap')
    measured = phase.get('memoryPeakBytes')
    need(isinstance(measured, int) and 0 < measured <= CAP, 'Positive capped kernel peak required')
    need(end.get('exitCode') == 0 and end.get('compilerPostIdentity') is True
         and isinstance(end.get('memoryPeakBytes'), int) and end['memoryPeakBytes'] == measured,
         'Raw EXIT kernel peak/identity inconsistent')
    events = dict(line.split() for line in end.get('memoryEvents', '').splitlines())
    need(all(int(events.get(k, -1)) == 0 for k in ('oom', 'oom_kill', 'oom_group_kill')),
         'Actual OOM event evidence failed/missing')
