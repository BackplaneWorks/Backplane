#!/usr/bin/env python3
"""Native stage timestamps work with GNU and BSD date output."""
import os
from pathlib import Path
import subprocess
import tempfile
import time

s=Path('scripts/build-app.sh').read_text()
helper=s[s.index('now_ms() {'):s.index('\nrss_cmd()')]
with tempfile.TemporaryDirectory(prefix='bp-build-clock-') as d:
    root=Path(d)
    probe=root/'probe.sh';probe.write_text('#!/bin/sh\nset -eu\n'+helper+'\nnow_ms\n')
    for bsd in [False,True]:
        env=dict(os.environ)
        if bsd:
            date=root/'date';date.write_text('#!/bin/sh\nprintf "N\\n"\n');date.chmod(0o755)
            env['PATH']=str(root)+':'+env['PATH']
        before=int(time.time()*1000)
        value=int(subprocess.check_output(['sh',str(probe)],env=env,text=True).strip())
        after=int(time.time()*1000)
        assert before-20 <= value <= after+20,(value,before,after)
        print('ok native stage clock with '+('BSD date fallback' if bsd else 'GNU date'))
