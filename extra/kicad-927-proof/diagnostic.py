"""Run exactly one canonical proof with a bounded wait and kernel diagnostics."""
import hashlib
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import time

out = Path('build/issue927-proof'); out.mkdir(parents=True, exist_ok=True)
group = Path('/sys/fs/cgroup') / Path('/proc/self/cgroup').read_text().split('0::', 1)[1].strip().lstrip('/')
def kernel():
    return {
        'cgroup': str(group),
        'memory_max': (group/'memory.max').read_text().strip(),
        'swap_max': (group/'memory.swap.max').read_text().strip(),
        'memory_peak': int((group/'memory.peak').read_text()),
        'events': dict((k, int(v)) for k,v in (line.split() for line in (group/'memory.events').read_text().splitlines())),
    }
before = kernel()
assert 0 < int(before['memory_max']) <= 3_700_000_000 and before['swap_max'] == '0', before
record = {
    'command': ['bend', 'PROOF.bend'], 'timeout_seconds': 120,
    'kernel_before': before,
    'inputs_sha256': {p: hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in
                     ['LAWS.bend','PROOF.bend','src/server/kicad.bend','src/core/kicad_install.bend']},
    'state': 'starting',
}
(out/'RESULT.json').write_text(json.dumps(record, indent=2)+'\n')
env = dict(os.environ, BEND_NO_TELEMETRY='1', BUN_JSC_maxPerThreadStackUsage='536870912', BUN_JSC_forceRAMSize='4294967296')
started = time.monotonic()
timed_out = False
with (out/'proof.txt').open('wb') as log:
    process = subprocess.Popen(['bend','PROOF.bend'], stdout=log, stderr=subprocess.STDOUT,
                               env=env, start_new_session=True)
    try:
        code = process.wait(timeout=120)
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(process.pid, signal.SIGTERM)
        try:
            code = process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            code = process.wait(timeout=5)
after = kernel()
delta = {k: after['events'].get(k, 0)-before['events'].get(k, 0) for k in after['events']}
text = (out/'proof.txt').read_text(errors='replace')
reason = ('cgroup_oom_kill' if delta.get('oom_kill', 0) else 'explicit_timeout' if timed_out
          else 'signal_'+signal.Signals(-code).name if code < 0 else 'exit_'+str(code))
record.update(state='incomplete', exit_code=code, exit_reason=reason, timed_out=timed_out,
              seconds=round(time.monotonic()-started, 3), kernel_after=after, events_delta=delta,
              peak_child_rss_kib=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
              output_sha256=hashlib.sha256((out/'proof.txt').read_bytes()).hexdigest(), output_bytes=len((out/'proof.txt').read_bytes()))
# Keep the repository's exact verdict rules; an interrupted proof never passes.
verdict = subprocess.run(['sh','-c','. scripts/bendok.sh; bend_verdict "$1" "$(cat "$2")"','v',str(code),str(out/'proof.txt')])
if verdict.returncode == 0:
    record['state'] = 'passed'
(out/'RESULT.json').write_text(json.dumps(record, indent=2)+'\n')
print(json.dumps(record, indent=2), flush=True)
print(text[-4000:], flush=True)
# Diagnostic collection completed; the RESULT state owns the proof verdict.
