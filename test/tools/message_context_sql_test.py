"""Stored context must survive a hot state containing only recent tools."""
import json
import sqlite3
import unittest
from lazy_tools_sql_test import sql


class MessageContextSQL(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(':memory:')
        self.addCleanup(self.db.close)
        self.db.execute('CREATE TABLE entry(seq INTEGER PRIMARY KEY, ord INTEGER UNIQUE, id TEXT, thread TEXT, src TEXT, kind TEXT, role TEXT, akind TEXT, text TEXT, at INTEGER)')
        self.db.execute('CREATE INDEX owned ON entry(thread,seq)')
        self.put(1, 'human', 'msg', 'user', 'PRIVATE HUMAN BODY')
        self.put(2, 'agent', 'msg', 'assistant', 'PRIVATE ASSISTANT BODY')
        for n in range(3, 1231):
            self.put(n, 'tool-' + str(n), 'act', 'tool', 'PRIVATE TOOL BODY')
        self.put(1231, 'blank', 'msg', 'assistant', ' \t\r\n')
        self.put(1232, 'foreign', 'msg', 'user', 'FOREIGN', thread='other')

    def put(self, n, ident, kind, role, body, thread='chat', src=''):
        self.db.execute('INSERT INTO entry VALUES(?,?,?,?,?,?,?,?,?,?)', (n,n,ident,thread,src,kind,role,'shell',body,n))

    def test_two_text_identities_survive_a_long_tool_tail_without_bodies(self):
        rows = self.db.execute(sql('Q.sql.context.messages'), {'1':'chat'}).fetchall()
        self.assertEqual([row[2] for row in rows], ['agent', 'human'])
        self.assertEqual([row[8] for row in rows], ['context', 'context'])

    def test_fold_identity_is_stable_on_both_sides_of_the_hot_window(self):
        rows = self.db.execute(sql('Q.sql.context.runs'), {'1':'chat','2':json.dumps([3,17,450,1210,1230])}).fetchall()
        self.assertEqual(len(rows),5)
        self.assertEqual({row[1] for row in rows}, {'tool-3'})

    def test_real_text_and_errors_split_runs_and_unselected_ids_are_excluded(self):
        self.put(1233, 'explanation', 'msg', 'assistant', 'Explanation')
        self.put(1234, 'second-run', 'act', 'tool', 'Private second tool')
        self.put(1235, 'error', 'act', 'error', 'Failed')
        self.put(1236, 'third-run', 'act', 'step', '{}')
        self.put(1237, 'foreign-tool', 'act', 'tool', 'Foreign', thread='other')
        rows = self.db.execute(sql('Q.sql.context.runs'), {'1':'chat','2':json.dumps([1234,1236,1237])}).fetchall()
        self.assertEqual(rows,[('third-run','third-run'),('second-run','second-run')])

    def test_outgoing_text_links_are_context_but_other_conversations_are_not(self):
        self.put(1233,'outgoing','msg','user','Sent to another conversation',thread='other',src='chat')
        rows = self.db.execute(sql('Q.sql.context.messages'), {'1':'chat'}).fetchall()
        self.assertEqual([row[2] for row in rows], ['outgoing','agent'])


if __name__ == '__main__':
    unittest.main()
