# Seeds for gpui-review-rows (run inside gpui-f-rows.py, before the hub starts): a pinned thread with full access, in the project with the ecc83 board.
seed += [{'$': 'ThreadCreated', 'id': 't6', 'project': 'p1', 'title': 'Board review', 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now + 6},
         {'$': 'ThreadPinned', 'id': 't6', 'pinned': True, 'at': now + 6},
         {'$': 'ModesSet', 'id': 't6', 'runtime': 'full-access', 'interaction': 'default', 'model': '', 'at': now + 6}]
