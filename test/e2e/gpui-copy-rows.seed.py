# Seeds for gpui-copy-rows (run inside gpui-f-rows.py, before the hub starts): what each chat surface says, written as an agent would.
INLINE = 'sudo apt-get install -y rootlesskit slirp4netns'
def reply(marker): return ('One thing needs you: please run `%s` first. Commit `3552c7cfd` has the fix.\n\n```bash\necho zq > %s\n```\n\nThen tell me.' % (INLINE, marker))
seed += [{'$': 'ThreadCreated', 'id': 't5', 'project': 'p1', 'title': 'Needs a package', 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now + 6},
         {'$': 'ThreadPinned', 'id': 't5', 'pinned': True, 'at': now + 6},
         {'$': 'MessagePosted', 'thread': 't5', 'msg': 'cp0', 'role': 'user', 'text': 'zqshort can you speed up the builds?', 'at': now + 6},
         {'$': 'MessagePosted', 'thread': 't5', 'msg': 'cp1', 'role': 'assistant', 'text': reply('zq-thread.marker'), 'at': now + 6}]
# the uberbot Bub: a bot with its own chat thread (no project: its terminal works in the home folder)
seed += [{'$': 'ThreadCreated', 'id': 'tb', 'project': '', 'title': 'Bub', 'env': 'local', 'branch': '', 'worktree': '', 'provider': 'claude', 'model': '', 'at': now + 7},
         {'$': 'BotSet', 'id': 'b1', 'name': 'Bub', 'thread': 'tb', 'look': 0, 'persona': '', 'at': now + 7},
         {'$': 'UberSet', 'bot': 'b1', 'uid': 'u-rows', 'at': now + 7},
         {'$': 'MessagePosted', 'thread': 'tb', 'msg': 'cp2', 'role': 'assistant', 'text': reply('zq-bub.marker'), 'at': now + 8}]
# a room between the person and Bub, with a post holding the same code
seed += [{'$': 'RoomSet', 'id': 'r1', 'name': 'bringup', 'members': 'b1', 'at': now + 9},
         {'$': 'RoomPosted', 'id': 'rp1', 'room': 'r1', 'from': 'Bub', 'text': reply('zq-room.marker'), 'hop': 0, 'at': now + 10}]
