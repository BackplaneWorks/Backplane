import json
import re
import sqlite3
import unittest
from pathlib import Path


class ReviewHistoryQuery(unittest.TestCase):
    def test_selected_complete_snapshot(self):
        source = (Path(__file__).resolve().parents[2] / 'src/server/review-history.bend').read_text()
        sql = json.loads(re.search(r'def ReviewHistory.sql\(\) -> String:\s*("[^\n]+")', source)[1])
        db = sqlite3.connect(':memory:')
        db.execute('CREATE TABLE entry(seq INTEGER, ord INTEGER, id TEXT, thread TEXT, src TEXT, kind TEXT, role TEXT, akind TEXT, text TEXT, at INTEGER)')
        rows = [
            (1, 1, 'root', 'a', '', 'act', 'review', 'add', 'x' * 4000, 1),
            (2, 2, 'private', 'other', '', 'act', 'review', 'add', 'Private', 2),
            (3, 3, 'tool', 'a', '', 'act', 'tool', 'Bash', 'y' * 8000, 3),
            (4, 4, 'resolve', 'a', '', 'act', 'review', 'resolve', 'Root', 4),
            (5, 5, 'future', 'a', '', 'act', 'review', 'reopen', 'Future', 5),
        ]
        db.executemany('INSERT INTO entry VALUES(?,?,?,?,?,?,?,?,?,?)', rows)
        got = db.execute(sql, ('a', 4)).fetchall()
        self.assertEqual([r[2] for r in got], ['root', 'resolve'])
        self.assertEqual(got[0][8], 'x' * 4000)
        self.assertEqual(db.execute(sql, ('missing', 100)).fetchall(), [])
        self.assertEqual(db.execute(sql, ("a' OR 1=1 --", 100)).fetchall(), [])
        db.close()


if __name__ == '__main__':
    unittest.main()
