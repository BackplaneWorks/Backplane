"""Read the actual current cgroup controls and record existing worker peak/wall."""
import argparse
import json
from pathlib import Path
import resource
import subprocess
import time

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--receipt', type=Path, required=True)
p.add_argument('command', nargs=argparse.REMAINDER)
a = p.parse_args()
command = a.command[1:] if a.command[:1] == ['--'] else a.command
group = next(x.split(':', 2)[2] for x in Path('/proc/self/cgroup').read_text().splitlines() if x.startswith('0::'))
cg = Path('/sys/fs/cgroup')/group.lstrip('/')
def value(n): return (cg/n).read_text().strip()
maximum, swap = int(value('memory.max')), int(value('memory.swap.max'))
assert 0 < maximum <= 3700000000 and swap == 0
a.receipt.parent.mkdir(parents=True, exist_ok=True)
d = {'schema': 1, 'state': 'RUNNING', 'actualKernelMemoryMaxBytes': maximum,
    'actualKernelSwapMaxBytes': swap, 'cgroupPath': str(cg), 'memoryEventsBefore': value('memory.events')}
a.receipt.write_text(json.dumps(d, indent=2)+'\n')
start = time.monotonic()
try:
    result = subprocess.run(command)
    d['exitCode'] = result.returncode
finally:
    d.update(state='TERMINAL', wallSeconds=time.monotonic()-start,
        actualKernelPeakBytes=int(value('memory.peak')), memoryEventsAfter=value('memory.events'),
        childPeakRSSKiB=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss)
    a.receipt.write_text(json.dumps(d, indent=2)+'\n')
    print(json.dumps(d), flush=True)
raise SystemExit(result.returncode)
