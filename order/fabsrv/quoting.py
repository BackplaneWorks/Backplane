"""Working out a quote: measure the files in a sandboxed process, then price
what they measure (pricing.py). Runs on a small thread pool, so at most
config.WORKERS analyses run at once.
"""
import concurrent.futures
import json
import os
import resource
import shutil
import subprocess
import sys
import time
import traceback

from . import config, pricing, store

_pool = None


def start():
    global _pool
    if _pool is None:
        _pool = concurrent.futures.ThreadPoolExecutor(max_workers=config.WORKERS, thread_name_prefix="quote")
        # quotes a restart interrupted go round again
        for r in store.q("select id from quotes where status in ('received','analysing')"):
            _pool.submit(run, r["id"])


def submit(qid):
    start()
    _pool.submit(run, qid)


def _limits():
    # the analysis process: 1.5 GB of memory, 90 s of CPU, 50 MB per written file, no core dumps
    resource.setrlimit(resource.RLIMIT_AS, (1536 * 1024 * 1024,) * 2)
    resource.setrlimit(resource.RLIMIT_CPU, (90, 90))
    resource.setrlimit(resource.RLIMIT_FSIZE, (50 * 1024 * 1024,) * 2)
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    os.setsid()


def measure(root):
    env = {"PATH": "/usr/bin:/bin", "PYTHONPATH": os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
           "HOME": str(root), "PYTHONDONTWRITEBYTECODE": "1"}
    try:
        subprocess.run([sys.executable, "-m", "fabsrv.analysis", str(root)], env=env, cwd=root,
                       preexec_fn=_limits, timeout=120, check=False, stdin=subprocess.DEVNULL,
                       stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    except subprocess.TimeoutExpired:
        return {"parts": {}, "problems": ["the files took too long to read"], "warnings": []}
    try:
        return json.loads((root / "features.json").read_text())
    except (OSError, ValueError):
        return {"parts": {}, "problems": ["the files could not be read"], "warnings": []}


def run(qid):
    row = store.quote(qid)
    if row is None:
        return
    root = config.DATA / "quotes" / qid
    try:
        store.set_quote(qid, status="analysing")
        feats = measure(root)
        manifest = json.loads((root / "manifest.json").read_text())
        result = pricing.price(manifest, feats)
        store.set_quote(qid, status="ready", result=result)
    except Exception:
        traceback.print_exc()
        store.set_quote(qid, status="failed", error="the quote could not be worked out; try again later")


def clean_old():
    """Files of quotes nobody paid for go after KEEP_DAYS; the rows stay (they hold no files)."""
    before = time.time() - config.KEEP_DAYS * 86400
    for r in store.stale_quotes(before):
        d = config.DATA / "quotes" / r["id"]
        if d.exists():
            shutil.rmtree(d, ignore_errors=True)
