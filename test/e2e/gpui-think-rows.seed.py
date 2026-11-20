# Seeds for gpui-think-rows (run inside gpui-f-rows.py, before the hub starts): a thread whose agent thought before it answered, and Show thinking on.
THOUGHT = ('The person wants the vias checked before the pour, so I start with the drill table.\n\n'
           'The **0.3 mm** vias under U3 sit 0.15 mm from the GND pour: under the fab\'s 0.2 mm, so those move first. '
           'The rest of the board keeps its 0.4 mm vias, which every fab drills without a surcharge.\n\n'
           'Then I check the planes for slivers zqtail.')
seed += [{'$': 'SettingSet', 'key': 'agent.thinking', 'value': 'on', 'at': now + 6},
         {'$': 'ThreadCreated', 'id': 't7', 'project': 'p1', 'title': 'Thinks first', 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now + 6},
         {'$': 'ThreadPinned', 'id': 't7', 'pinned': True, 'at': now + 6},
         {'$': 'MessagePosted', 'thread': 't7', 'msg': 'tk0', 'role': 'user', 'text': 'zqshort check the vias before the pour', 'at': now + 6},
         {'$': 'ActivityLogged', 'thread': 't7', 'act': 'tk-th1', 'tone': 'thinking', 'kind': 'thinking', 'summary': THOUGHT, 'at': now + 7},
         {'$': 'MessagePosted', 'thread': 't7', 'msg': 'tk1', 'role': 'assistant', 'text': 'Three vias under U3 are too close to the pour; I moved them 0.1 mm out.', 'at': now + 8}]
