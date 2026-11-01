#!/usr/bin/env python3
"""Cheap gates from CI, executed once by the combined controller."""
from pathlib import Path
import subprocess
import re
import sys

root = Path(__file__).resolve().parent.parent
# Required in every local lane, including focused-only and guards-only.
subprocess.run([sys.executable, 'test/tools/sidebar_draft_guard_test.py'], cwd=root, check=True)
subprocess.run(['bun', 'test/tools/store_checkpoint_io_test.cjs'], cwd=root, check=True)
subprocess.run(['bun', 'test/tools/hotboot_wire_test.cjs'], cwd=root, check=True)
subprocess.run([sys.executable, 'test/tools/kicad_library_header_test.py'], cwd=root, check=True)
if '--focused-only' not in sys.argv:
    subprocess.run([sys.executable, 'scripts/local.py', 'deps'], cwd=root, check=True)
    for name in ['service_runtime_test.py', 'release_bootstrap_test.py', 'release_assets_test.py',
             'desktop_install_test.py', 'local_workflow_test.py', 'local_resource_test.py']:
        subprocess.run([sys.executable, 'test/tools/' + name], cwd=root, check=True)
    subprocess.run([sys.executable, '-m', 'unittest', 'discover', '-s', 'test/tools', '-p', 'test_runner_test.py'], cwd=root, check=True)
    # Preserve CI's existing thin/mail coverage and add the guards this batch touches.
    # Release-only legacy VM harnesses are not part of this local gate.
    files = set(Path('desktop/test').glob('thin-*.test.cjs'))
    for name in ['host-thin-startup','mail-draft-dispatch','mail-drafts','focused-cache',
                 'local-polish','prepare-components','web-assets','linux-launcher','startup']:
        files.add(Path('desktop/test') / (name + '.test.cjs'))
    subprocess.run(['node', '--test', *map(str, sorted(files))], cwd=root, check=True)
# Fail before another emit if the newly added full-replay reader is broken.
# This focused regression runs inside the same admitted combined controller.
if '--guards-only' not in sys.argv:
    focused = subprocess.run(['bend','test/mailtriage_test.bend'],cwd=root,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    print(focused.stdout,flush=True)
    if focused.returncode or not re.search(r'^ok ',focused.stdout,re.M) or re.search(r'^(FAIL|Error:)',focused.stdout,re.M):
        raise SystemExit(focused.returncode or 1)
