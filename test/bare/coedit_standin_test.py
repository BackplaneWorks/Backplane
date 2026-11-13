#!/usr/bin/env python3
"""Exercise one persistent stand-in process with the full gate's late plan configuration. No hub, KiCad or model runs."""
import http.server
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import sys
import tempfile
import threading
import unittest


class StandinTest(unittest.TestCase):
    def test_plan_added_after_first_turn(self):
        calls = []

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args): pass

            def do_POST(self):
                request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                params = request['params']
                calls.append(params)
                body = {'message': {'document': {'path': '/board.kicad_pcb'}, 'items': []}}
                refused = params['name'] == 'plan_route' and not params['arguments'].get('ids')
                if refused:
                    body = 'nothing in the plan is approved to route (1 waiting: c-bare)'
                reply = {'jsonrpc': '2.0', 'id': request['id'], 'result': {
                    'isError': refused, 'content': [{'type': 'text', 'text': json.dumps(body)}]}}
                data = json.dumps(reply).encode()
                self.send_response(200)
                self.send_header('Content-Length', str(len(data)))
                self.end_headers()
                self.wfile.write(data)

        with tempfile.TemporaryDirectory(prefix='bp-standin-') as folder:
            root = Path(folder)
            script = root / 'claude'
            shutil.copyfile(Path(__file__).with_name('coedit_standin.py'), script)
            output = root / 'coedit.json'
            conf = {'out': str(output), 'pcb': '/board.kicad_pcb'}
            config = root / 'coedit.conf'
            config.write_text(json.dumps(conf))
            server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
            worker = threading.Thread(target=server.serve_forever, daemon=True)
            worker.start()
            mcp = {'mcpServers': {'backplane': {'url': f'http://127.0.0.1:{server.server_port}'}}}
            env = dict(os.environ)
            env.pop('BARE_COEDIT_OUT', None)
            env.pop('BARE_COEDIT_PCB', None)
            with (root / 'stderr').open('w+') as errors:
                process = subprocess.Popen([sys.executable, str(script), '--mcp-config', json.dumps(mcp),
                                            '--output-format', 'stream-json'],
                                           stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=errors,
                                           text=True, env=env)
                messages = queue.Queue()

                def read_output():
                    for line in process.stdout:
                        messages.put(json.loads(line))

                reader = threading.Thread(target=read_output, daemon=True)
                reader.start()
                controls = []

                def send(message):
                    process.stdin.write(json.dumps(message) + '\n')
                    process.stdin.flush()

                def turn(text):
                    send({'type': 'user', 'message': {'content': [{'type': 'text', 'text': text}]}})
                    while True:
                        try:
                            message = messages.get(timeout=10)
                        except queue.Empty:
                            errors.seek(0)
                            self.fail('stand-in stopped answering: ' + errors.read())
                        if message['type'] == 'control_response': controls.append(message['response'])
                        if message['type'] == 'result':
                            self.assertFalse(message['is_error'])
                            return

                try:
                    turn('add a track beside the first part, save, and comment')
                    self.assertIn('save', json.loads(output.read_text()))
                    turn('bare gate note\n\n<review_comments>context</review_comments>')
                    # Unlike the old local loop, the actual gate writes this only after earlier turns finish.
                    plan = {'corridors': [{'id': 'c-bare', 'status': 'proposed'}]}
                    conf['plan'] = plan
                    config.write_text(json.dumps(conf))
                    send({'type': 'control_request', 'request_id': 'check', 'request': {'subtype': 'interrupt'}})
                    turn('plan-propose\n\n<review_comments>context</review_comments>')
                    result = json.loads((root / 'coedit-plan1.json').read_text())
                    self.assertTrue(result['route_refused']['error'])
                    propose = next(c for c in calls if c['name'] == 'plan_propose')
                    self.assertEqual(propose['arguments']['plan'], plan)
                    self.assertEqual(calls[-1]['name'], 'plan_approve')
                    self.assertEqual(calls[-1]['arguments']['ids'], ['c-bare'])
                    self.assertEqual(controls, [{'subtype': 'success', 'request_id': 'check'}])
                    turn('plan-route')
                    self.assertFalse(json.loads((root / 'coedit-plan2.json').read_text())['route']['error'])
                    self.assertEqual(calls[-1]['arguments']['ids'], ['c-bare'])
                    process.stdin.close()
                    self.assertEqual(process.wait(timeout=5), 0)
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait(timeout=5)
                    reader.join(timeout=5)
                    process.stdout.close()
                    server.shutdown()
                    server.server_close()
                    worker.join(timeout=5)


if __name__ == '__main__':
    unittest.main()
