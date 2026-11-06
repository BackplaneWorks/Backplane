#!/usr/bin/python3
# A stand-in `claude` for the uberbot end to end: Claude's stream-json, keyed by words in the message. No model runs.
# Each line it is given goes to <run>/turns.jsonl (0600, private: the harness reads it and prints only counts and yes/no).
#   zqsave:KEY   memory_save KEY over the MCP url it was started with
#   zqsend:THREAD  thread_send to THREAD      zqlaunch:PROJECT  thread_launch a thread there that says hello
#   zqhold       works until its stdin closes or it is killed (no result)
#   anything else: "hello"; a run's brief (<charter>) says "triaged"
import json, os, re, select, sys, time, urllib.request
RUN = os.environ.get('E2E_RUN') or os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
args = sys.argv[1:]
if '--version' in args: print('2.1.286 (Claude Code)'); sys.exit(0)
def log(**k):
    k.update(t=time.time(), pid=os.getpid(), cwd=os.getcwd())
    fd = os.open(RUN + '/turns.jsonl', os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    os.write(fd, (json.dumps(k) + '\n').encode()); os.close(fd)
def arg(n):
    return args[args.index(n) + 1] if n in args else ''
log(ev='start', argv=args)
if '--input-format' not in args:  # a one-shot text job (a title, a summary)
    print('ok'); sys.exit(0)
url = ''
try: url = json.loads(arg('--mcp-config'))['mcpServers']['backplane']['url']
except Exception: pass
sid = arg('--resume') or 'fake-%d' % os.getpid()
n = 0
def out(o): sys.stdout.write(json.dumps(o) + '\n'); sys.stdout.flush()
def tool(name, a):
    body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call', 'params': {'name': name, 'arguments': a}}).encode()
    try:
        r = urllib.request.urlopen(urllib.request.Request(url, body, {'content-type': 'application/json'}), timeout=60).read().decode()
    except Exception as e: r = json.dumps({'harness_error': str(e)})
    log(ev='tool', name=name, answer=r); return r
def say(text):
    global n; n += 1
    out({'type': 'assistant', 'message': {'id': 'm%d-%d' % (os.getpid(), n), 'role': 'assistant', 'content': [{'type': 'text', 'text': text}], 'usage': {'input_tokens': 1, 'output_tokens': 1}}, 'session_id': sid})
def result(): out({'type': 'result', 'subtype': 'success', 'is_error': False, 'result': 'done', 'session_id': sid, 'modelUsage': {'fake': {'contextWindow': 200000}}})
for line in sys.stdin:
    if '"type":"user"' not in line.replace(' ', ''): continue
    log(ev='turn', line=line, resume=arg('--resume'))
    out({'type': 'system', 'subtype': 'init', 'session_id': sid, 'cwd': os.getcwd(), 'tools': ['Bash'], 'model': 'fake', 'permissionMode': 'default'})
    m = re.search(r'zqsave:([a-z0-9-]+)', line)
    s = re.search(r'zqsend:([A-Za-z0-9_~-]+)', line)
    nf = re.search(r'zqnotify:([a-z_]+)', line)
    l = re.search(r'zqlaunch:([A-Za-z0-9_-]+)', line)
    if '<charter>' in line: say('triaged')  # a run's brief quotes the recent chat: its words are not orders
    elif 'zqhold' in line and 'continue' not in line.split('zqhold')[0][-400:]:
        say('working')
        while True:
            r, _, _ = select.select([sys.stdin], [], [], 1.0)
            if r and not sys.stdin.readline(): sys.exit(0)
    elif m: tool('memory_save', {'key': m.group(1), 'kind': 'fact', 'text': 'stand-in note ' + m.group(1)}); say('saved it')
    elif s: tool('thread_send', {'threadId': s.group(1), 'text': 'zqinbound from a stand-in'}); say('sent it')
    elif nf: tool('notify_person', {'text': 'stand-in alert', 'reason': nf.group(1)}); say('notified')
    elif l: tool('thread_launch', {'title': 'zq child', 'prompt': 'say hello', 'project': l.group(1), 'subthread': False}); say('launched')
    else: say('hello')
    result()
