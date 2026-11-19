# Seeds for gpui-unanswered-rows (run inside gpui-f-rows.py before the hub starts): a design thread with some history.
seed += [{'$': 'ThreadCreated', 'id': 't6', 'project': 'p1', 'title': 'Pogo connector research', 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now + 6},
         {'$': 'ThreadPinned', 'id': 't6', 'pinned': True, 'at': now + 6},
         {'$': 'MessagePosted', 'thread': 't6', 'msg': 'un0', 'role': 'user', 'text': 'zqshort which pogo pins are stocked at 1.27 mm?', 'at': now + 6},
         {'$': 'MessagePosted', 'thread': 't6', 'msg': 'un1', 'role': 'assistant', 'text': 'Three families are stocked at 1.27 mm: Harwin P13, Mill-Max 0906 and the INGUN GKS-050. I will compare travel and force next.', 'at': now + 6}]
