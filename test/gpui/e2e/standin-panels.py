#!/usr/bin/python3
# A stand-in `claude` that feeds the thread's panels: Claude's stream-json, keyed by words in the message. No model runs. Used with a throwaway hub
# (BACKPLANE_AGENTS) to see in a real window what each panel draws:
#   zqtodo   a TodoWrite list (the To-do panel)            zqsub   an Agent call with a background task (the Subagents panel)
#   zqtools  five tool calls in a row (the tool fold)      zqimg   a reply that names an image in the thread's folder (images, lightbox)
#   zqhold   works until its stdin closes (queue, steer)   zqask   an ApprovalRequest-style question is not faked: plain text only
#   anything else: "hello"
import json, os, select, sys, time
args = sys.argv[1:]
if '--version' in args: print('2.1.286 (Claude Code)'); sys.exit(0)
if '--input-format' not in args: print('ok'); sys.exit(0)
sid = args[args.index('--resume') + 1] if '--resume' in args else 'fake-%d' % os.getpid()
n = 0
def out(o): sys.stdout.write(json.dumps(o) + '\n'); sys.stdout.flush()
def mid(): global n; n += 1; return 'm%d-%d' % (os.getpid(), n)
def say(text): out({'type': 'assistant', 'message': {'id': mid(), 'role': 'assistant', 'content': [{'type': 'text', 'text': text}], 'usage': {'input_tokens': 1, 'output_tokens': 1}}, 'session_id': sid})
def tool(name, inp, result='ok'):
    i = 'tu%s' % mid()
    out({'type': 'assistant', 'message': {'id': mid(), 'role': 'assistant', 'content': [{'type': 'tool_use', 'id': i, 'name': name, 'input': inp}], 'usage': {'input_tokens': 1}}, 'session_id': sid})
    out({'type': 'user', 'message': {'role': 'user', 'content': [{'tool_use_id': i, 'type': 'tool_result', 'content': result, 'is_error': False}]}, 'session_id': sid})
    return i
def result(): out({'type': 'result', 'subtype': 'success', 'is_error': False, 'result': 'done', 'session_id': sid, 'modelUsage': {'fake': {'contextWindow': 200000}}})
for line in sys.stdin:
    if '"type":"user"' not in line.replace(' ', ''): continue
    out({'type': 'system', 'subtype': 'init', 'session_id': sid, 'cwd': os.getcwd(), 'tools': ['Bash', 'TodoWrite', 'Agent'], 'model': 'fake', 'permissionMode': 'default'})
    if 'zqtodo' in line:
        tool('TodoWrite', {'todos': [{'content': 'Survey the footprints', 'status': 'completed', 'activeForm': 'Surveying'}, {'content': 'Route the power rail', 'status': 'in_progress', 'activeForm': 'Routing the power rail'},
            {'content': 'Run DRC', 'status': 'pending', 'activeForm': 'Running DRC'}, {'content': 'Write the notes', 'status': 'pending', 'activeForm': 'Writing'}]}, 'Todos have been modified')
        say('Made a plan.')
    elif 'zqsub' in line:
        a = tool('Agent', {'description': 'Survey the footprints', 'prompt': 'Look through the footprints and report', 'subagent_type': 'general-purpose'}, 'The survey is done.')
        out({'type': 'system', 'subtype': 'task_started', 'task_id': 'a%d' % os.getpid(), 'tool_use_id': a, 'description': 'Survey the footprints', 'task_type': 'local_agent', 'is_backgrounded': True, 'session_id': sid})
        out({'type': 'system', 'subtype': 'task_started', 'task_id': 'b%d' % os.getpid(), 'tool_use_id': 'tb1', 'description': 'npm run watch', 'task_type': 'local_bash', 'is_backgrounded': True, 'session_id': sid})
        out({'type': 'system', 'subtype': 'task_progress', 'task_id': 'a%d' % os.getpid(), 'description': 'Reading footprint 3', 'session_id': sid})
        say('Started a subagent and a watcher.')
        # the work stays out until the next message: the panel shows it at work
        time.sleep(0.2)
    elif 'zqtools' in line:
        for i, (nm, inp) in enumerate([('Read', {'file_path': '/tmp/a.kicad_pcb'}), ('Bash', {'command': 'ls -la', 'description': 'List'}), ('Read', {'file_path': '/tmp/b.kicad_sch'}), ('Bash', {'command': 'git status'}), ('Grep', {'pattern': 'U3'})]):
            tool(nm, inp, 'line %d of output\nand a second line' % i)
        say('Looked at five things.')
    elif 'zqimg' in line:
        say('Here is the render: %s/pic.png\n\nIt shows the board outline.' % os.getcwd())
    elif 'zqhold' in line:
        say('working')
        while True:
            r, _, _ = select.select([sys.stdin], [], [], 1.0)
            if r and not sys.stdin.readline(): sys.exit(0)
    else:
        say('hello')
    result()
