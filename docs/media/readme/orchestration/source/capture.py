"""Capture the supplied current build in one disposable isolated session.
No builds, external network, real agents, or live homes. Requires Python/Pillow
and the Playwright/Chromium paths in current-capture.mjs.
"""
import os,subprocess,sys,time
from pathlib import Path
SRC=Path(__file__).resolve().parent
env={**os.environ,'README_B_RUN':'B-recapture-'+str(time.time_ns())}
subprocess.run([sys.executable,str(SRC/'current-hub.py'),'start'],env=env,check=True)
try:
    subprocess.run([sys.executable,str(SRC/'current-hub.py'),'run','node',str(SRC/'current-capture.mjs'),'all'],env=env,check=True,stdout=(Path('<capture-root>/run')/env['README_B_RUN']/'browser.log').open('w'),stderr=subprocess.STDOUT)
    subprocess.run([sys.executable,str(SRC/'current-finish.py')],env=env,check=True)
    subprocess.run([sys.executable,str(SRC/'current-audit.py')],env=env,check=True)
finally:
    subprocess.run([sys.executable,str(SRC/'current-hub.py'),'stop'],env=env,check=True)
