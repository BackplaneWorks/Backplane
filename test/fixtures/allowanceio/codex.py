#!/usr/bin/env python3
"""Strict, credential-free app-server stand-in; used only by allowanceio_test."""
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import time

CASE = os.environ['ALLOWANCE_FIXTURE_CASE']
LOG = Path(os.environ['ALLOWANCE_FIXTURE_LOG'])
PRIVATE = 'private-canary-email@example.invalid/account-token-sk-never-output'


def log(value):
    with LOG.open('a') as f:
        f.write(json.dumps(value) + '\n')


def send(value):
    sys.stdout.write(json.dumps(value, separators=(',', ':')) + '\n')
    sys.stdout.flush()


def request(method):
    value = json.loads(sys.stdin.buffer.readline())
    log(value)
    assert value['method'] == method, 'Unexpected method'
    return value


def pause():
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    time.sleep(30)


def window(used=12.31, duration=300, reset=1900000000):
    return dict(usedPercent=used, windowDurationMins=duration, resetsAt=reset,
                email=PRIVATE, token=PRIVATE)


log({'pid': os.getpid(), 'argv': sys.argv[1:], 'cwd': os.getcwd()})
assert sys.argv[1:] == ['-c', 'features.daemon_auto_start=false', '-c',
                       'analytics.enabled=false', '-c', 'otel.trace_exporter="none"',
                       'app-server', '--listen', 'stdio://']
sys.stderr.write(PRIVATE + '\n')
sys.stderr.flush()
init = request('initialize')
assert init['id'] == 1 and init['params']['clientInfo']['name'] == 'backplane_allowance'
# The client must wait for initialize success before any other wire request.
assert not select.select([sys.stdin.buffer], [], [], 0.03)[0], 'Early request'
if CASE == 'timeout_init':
    pause()
if CASE == 'init_error':
    send({'id': 1, 'error': {'code': -32601, 'message': PRIVATE}})
    pause()
if CASE == 'malformed':
    sys.stdout.write(PRIVATE + '\n')
    sys.stdout.flush()
    pause()
if CASE == 'flood':
    sys.stdout.write('x' * 300000)
    sys.stdout.flush()
    pause()
if CASE == 'host_request':
    send({'id': 99, 'method': 'account/chatgptAuthTokens/refresh',
          'params': {'refreshToken': PRIVATE}})
    pause()
if CASE == 'notification_flood':
    for _ in range(260):
        send({'method': 'irrelevant', 'params': PRIVATE})
    pause()
send({'method': 'account/updated', 'params': {'email': PRIVATE}})
send({'id': 1, 'result': {'userAgent': PRIVATE}})
ready = request('initialized')
assert 'id' not in ready
acct = request('account/read')
assert acct == {'id': 2, 'method': 'account/read', 'params': {'refreshToken': False}}
limits = request('account/rateLimits/read')
assert limits == {'id': 3, 'method': 'account/rateLimits/read'}
mode = 'apiKey' if CASE == 'apikey' else 'chatgpt'
account = None if CASE == 'signed_out' else {'type': mode, 'email': PRIVATE,
                                           'id': PRIVATE, 'accessToken': PRIVATE,
                                           'planType': PRIVATE}
account_reply = {'id': 2, 'result': {'account': account, 'accountId': PRIVATE}}
if CASE == 'slow_multi':
    time.sleep(0.3)
if CASE in ('timeout_account', 'timeout_limits', 'timeout_total'):
    if CASE == 'timeout_account':
        send({'id': 3, 'result': {'rateLimits': None}})
    if CASE == 'timeout_limits':
        send(account_reply)
    if CASE == 'timeout_total':
        time.sleep(6)
        send(account_reply)
    pause()
if CASE == 'descendant_timeout':
    child = subprocess.Popen([sys.executable, '-I', '-c', 'import time;time.sleep(30)'])
    log({'child_pid': child.pid})
    pause()
if CASE in ('rpc_error', 'unsupported', 'apikey'):
    send(account_reply)
    send({'id': 3, 'error': {'code': -32601 if CASE == 'unsupported' else -32000,
                           'message': PRIVATE, 'data': PRIVATE}})
    pause()
if CASE == 'account_error':
    send({'id': 2, 'error': {'code': -32000, 'message': PRIVATE}})
    send({'id': 3, 'result': {'rateLimits': None}})
    pause()
legacy = {'limitId': 'codex', 'limitName': None, 'primary': window(77.7),
          'secondary': None, 'normalModelSlug': PRIVATE}
buckets = {
    'codex': {'limitId': 'codex', 'limitName': None, 'primary': window(),
              'secondary': window(100, None, None)},
    'other-key': {'limitId': 'other-id', 'limitName': 'Other pool',
                  'primary': window(0, 0, 0), 'secondary': None,
                  'normalModelSlug': PRIVATE},
    'fallback-id': {'limitId': None, 'limitName': None,
                    'primary': window(99.9, 10080, None), 'secondary': None}}
body = {'rateLimits': legacy, 'rateLimitsByLimitId': buckets,
        'accountId': 'private-workspace-alpha', 'normalModelSlug': PRIVATE,
        'credits': {'remaining': PRIVATE}, 'planType': PRIVATE}
if CASE in ('account_switch', 'other_account_invalid'):
    body['accountId'] = 'private-workspace-beta'
    buckets['codex']['primary']['usedPercent'] = 42
if CASE in ('same_account_invalid', 'other_account_invalid', 'no_identity_invalid'):
    buckets['codex']['primary']['usedPercent'] = -1
if CASE in ('no_identity_good', 'no_identity_invalid'):
    body.pop('accountId')
if CASE == 'legacy':
    del body['rateLimitsByLimitId']
if CASE == 'legacy_no_id':
    body['rateLimitsByLimitId'] = None
    body['rateLimits'].pop('limitId')
if CASE == 'null':
    body = {'rateLimits': None, 'rateLimitsByLimitId': None}
if CASE == 'null_windows':
    body['rateLimitsByLimitId'] = {'codex': {'primary': None, 'secondary': None}}
if CASE == 'empty_multi':
    body['rateLimitsByLimitId'] = {}
if CASE == 'unicode_max':
    body['rateLimitsByLimitId'] = {str(i): dict(limitName='😀' * 128, limitId='😀' * 128,
                                              primary=window(), secondary=window()) for i in range(16)}
if CASE == 'max_buckets':
    body['rateLimitsByLimitId'] = {str(i): dict(primary=window(), secondary=window())
                                  for i in range(16)}
if CASE == 'too_many':
    body['rateLimitsByLimitId'] = {str(i): dict(primary=window()) for i in range(17)}
if CASE in ('long_label', 'control_label', 'long_id', 'long_key'):
    if CASE == 'long_key':
        body['rateLimitsByLimitId'] = {'x' * 129: dict(primary=window())}
    else:
        field = 'limitId' if CASE == 'long_id' else 'limitName'
        buckets['codex'][field] = 'x\nsecret' if CASE == 'control_label' else 'x' * 129
if CASE == 'max_label':
    buckets['codex']['limitName'] = 'x' * 128
if CASE.startswith('invalid_used_'):
    values = {'negative': -0.1, 'large': 100.01, 'bool': True, 'null': None,
              'string': '25', 'nan': float('nan'), 'inf': float('inf')}
    buckets['codex']['primary']['usedPercent'] = values[CASE.removeprefix('invalid_used_')]
if CASE.startswith('invalid_reported_'):
    field = 'resetsAt' if CASE.endswith('reset') else 'windowDurationMins'
    buckets['codex']['primary'][field] = -1
if CASE == 'wrong_bucket':
    body['rateLimitsByLimitId'] = {'codex': None}
if CASE == 'wrong_map':
    body['rateLimitsByLimitId'] = []
if CASE == 'wrong_window':
    buckets['codex']['primary'] = []
if CASE == 'decimal_precision':
    # Do not round a valid decimal before ceil(percentage * 10).
    encoded = json.dumps({'id': 3, 'result': body}).replace('12.31', '1.00000000000000000000000000001')
    sys.stdout.write(encoded + '\n')
    sys.stdout.flush()
else:
    # Out-of-order read responses are supported, and legacy is not duplicated.
    send({'id': 3, 'result': body})
send(account_reply)
# Capture any forbidden trailing request. A correct collector kills its child.
for line in sys.stdin.buffer:
    log({'unexpected': json.loads(line)})
