#!/usr/bin/python3
"""A stand-in `claude` for the Mac mini gate (the Mac has no bun for test/tools/claude_standin.ts): the stream-json agent protocol the
hub speaks, enough of it for the suites' "mcpcall:{json}" turns. Each such turn makes one Backplane MCP call ({"name", "arguments"},
the hub's MCP URL from --mcp-config) and logs `MCPCALL <name> <the JSON-RPC answer>` to $FAKE_LOG, as the bun stand-in does, so the
same `standin_ask` reads it. Any other turn is answered "ok". Stdlib only (the Mac's /usr/bin/python3).
"""
import json
import os
import sys
import urllib.request
import uuid

LOG = os.environ.get('FAKE_LOG', '')
MAX = int(os.environ.get('BP_STANDIN_MCPCALL_MAX', '600000'))
SESSION = 'mac-standin'


def log(line):
    if LOG:
        with open(LOG, 'a', encoding='utf-8') as f:
            f.write(line.replace('\n', ' ') + '\n')


def emit(o):
    sys.stdout.write(json.dumps(o) + '\n')
    sys.stdout.flush()


def mcp_url():
    a = sys.argv
    if '--mcp-config' not in a:
        return ''
    try:
        return json.loads((lambda v: v if v.lstrip().startswith('{') else open(v).read())(a[a.index('--mcp-config') + 1]))['mcpServers']['backplane']['url']
    except Exception:
        return ''


def call(url, name, arguments):
    body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call', 'params': {'name': name, 'arguments': arguments}}).encode()
    req = urllib.request.Request(url, body, {'content-type': 'application/json', 'accept': 'application/json'})
    try:
        return urllib.request.urlopen(req, timeout=900).read().decode('utf-8', 'replace')
    except Exception as e:
        return json.dumps({'error': str(e)})


def text_of(o):
    out = []
    for b in (o.get('message') or {}).get('content') or []:
        out.append(b if isinstance(b, str) else (b.get('text') or ''))
    return ' '.join(out)


def main():
    url = mcp_url()
    log(f'START {os.getpid()}')
    emit({'type': 'system', 'subtype': 'init', 'cwd': os.getcwd(), 'session_id': SESSION, 'tools': ['Bash'], 'mcp_servers': [],
          'model': 'claude-haiku-4-5-20251001'})
    for line in sys.stdin:
        log('IN ' + line.strip()[:2000])
        try:
            o = json.loads(line)
        except ValueError:
            continue
        if o.get('type') != 'user':
            continue
        text = text_of(o)
        reply = 'ok'
        if 'mcpcall:' in text and url:
            try:
                c = json.loads(text[text.index('mcpcall:') + 8:].strip())
            except ValueError as e:
                c = None
                log(f'MCPCALL bad json {e}')
            if c:
                tid = 'toolu_' + uuid.uuid4().hex[:12]
                emit({'type': 'assistant', 'message': {'id': 'msg_' + uuid.uuid4().hex[:12], 'type': 'message', 'role': 'assistant',
                      'content': [{'type': 'tool_use', 'id': tid, 'name': 'mcp__backplane__' + c['name'], 'input': c.get('arguments', {})}],
                      'usage': {'input_tokens': 10, 'output_tokens': 5}}, 'parent_tool_use_id': None, 'session_id': SESSION})
                got = call(url, c['name'], c.get('arguments', {}))
                log(f'MCPCALL {c["name"]} {got[:MAX]}')
                emit({'type': 'user', 'message': {'role': 'user', 'content': [{'type': 'tool_result', 'tool_use_id': tid, 'content': 'MCPRESULT ' + got[:1500]}]},
                      'parent_tool_use_id': None, 'session_id': SESSION})
                reply = f'MCPCALL {c["name"]} done'
        emit({'type': 'assistant', 'message': {'id': 'msg_' + uuid.uuid4().hex[:12], 'type': 'message', 'role': 'assistant',
              'content': [{'type': 'text', 'text': reply}], 'usage': {'input_tokens': 10, 'output_tokens': 5}}, 'parent_tool_use_id': None, 'session_id': SESSION})
        emit({'type': 'result', 'subtype': 'success', 'is_error': False, 'duration_ms': 10, 'num_turns': 1, 'result': reply, 'session_id': SESSION,
              'total_cost_usd': 0, 'usage': {}, 'uuid': uuid.uuid4().hex})


if __name__ == '__main__':
    main()
