"""Execute production page SQL: lazy bodies preserve the raw chronology contract."""
import json
from pathlib import Path
import re
import sqlite3
import unittest


SOURCE = (Path(__file__).resolve().parents[2] / "src/core/thinq.bend").read_text()


def sql(name, **values):
    """Read the actual constant/concatenation SQL definitions, without a twin query."""
    match = re.search(r"^def " + re.escape(name) + r"\([^\n]*\) -> String:\n(.*?)(?=\n\n|\Z)", SOURCE, re.M | re.S)
    if not match:
        raise AssertionError(name)
    expression = match[1].strip()
    token = re.compile(r'\s*("(?:[^"\\]|\\.)*"|Q\.[\w.]+\(\)|keep|\+\+)')
    parts, position = [], 0
    while position < len(expression):
        found = token.match(expression, position)
        if not found:
            raise AssertionError(expression[position:])
        value = found[1]
        position = found.end()
        if value.startswith('"'):
            parts.append(json.loads(value))
        elif value.startswith("Q."):
            parts.append(sql(value[:-2], **values))
        elif value != "++":
            parts.append(values[value])
    return "".join(parts)


class LazyToolsSQL(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.addCleanup(self.db.close)
        self.db.execute("CREATE TABLE entry(seq INTEGER PRIMARY KEY, ord INTEGER UNIQUE, id TEXT, thread TEXT, src TEXT, kind TEXT, role TEXT, akind TEXT, text TEXT, at INTEGER)")
        for n in range(1, 181):
            kind, role = ("msg", "assistant") if n % 11 == 0 else ("act", "tool")
            text = "message " + str(n) if kind == "msg" else "Summarized shell operation " + str(n) + "\n" + "private detail " * 600
            thread, src = ("elsewhere", "chat") if n % 7 == 0 else ("chat", "")
            self.db.execute("INSERT INTO entry VALUES(?,?,?,?,?,?,?,?,?,?)", (n, n, "row-" + str(n), thread, src, kind, role, "shell", text, n))
        self.db.execute("INSERT INTO entry VALUES(181,181,'foreign','other','','act','tool','shell','foreign secret',181)")
        self.db.execute("INSERT INTO entry VALUES(182,182,'baseline','chat','','act','step','design','{\"p\":\"\",\"w\":\"start\"}',182)")
        self.db.execute("INSERT INTO entry VALUES(183,183,'blank','chat','','msg','assistant','',' \n\t',183)")
        self.db.execute("INSERT INTO entry VALUES(184,184,'ride:receipt','chat','','act','info','ride','receipt',184)")

    def pages(self, name, keep=""):
        query = sql(name, keep=keep)
        for bound in [2**47, 165, 130, 35, 1]:
            yield self.db.execute(query, ("chat", bound, 49)).fetchall()

    def test_raw_ids_order_and_overlap_equal_at_both_page_edges(self):
        for keep in ["", sql("Q.sql.chat.keep")]:
            for original, lazy in zip(self.pages("Q.sql.page.body", keep), self.pages("Q.sql.page.lazy_tools", keep)):
                self.assertEqual([r[:8] + r[9:] for r in original], [r[:8] + r[9:] for r in lazy])
                self.assertEqual(len({r[1] for r in lazy}), len(lazy))
                self.assertTrue(all(r[3] == "chat" or r[4] == "chat" for r in lazy))

    def test_collapsed_tools_return_no_body_but_messages_and_step_flags_survive(self):
        rows = next(self.pages("Q.sql.page.lazy_tools"))
        self.assertTrue(any(r[3] == "elsewhere" for r in rows))
        for row in rows:
            if row[5:7] == ("act", "tool"):
                self.assertEqual(row[8], "Summarized shell operation " + str(row[1]))
            elif row[5] == "msg" and row[2] != "blank":
                self.assertTrue(row[8].startswith("message "))
        self.assertEqual(next(r[8] for r in rows if r[2] == "baseline"), '{"p":"","w":"start"}')

    def test_explicit_selected_ordinals_fetch_only_owned_tool_prefixes(self):
        rows = self.db.execute(sql("Q.sql.tools"), ("chat", json.dumps([178, 175, 176, 181]))).fetchall()
        self.assertEqual([r[1] for r in rows], [178, 175])
        self.assertTrue(all(0 < len(r[8]) <= 513 for r in rows))
        self.assertEqual(self.db.execute(sql("Q.sql.tools"), ("chat", "[]")).fetchall(), [])

    def test_visible_title_matches_160_scalar_multiline_boundary(self):
        title = " " + "測" * 200
        self.db.execute("UPDATE entry SET text=? WHERE ord=178", (title + "\nprivate body",))
        rows = next(self.pages("Q.sql.page.lazy_tools"))
        self.assertEqual(next(r[8] for r in rows if r[1] == 178), title[:160])
        self.db.execute("UPDATE entry SET text=? WHERE ord=178", ("  make drc\r\nprivate body",))
        self.assertEqual(next(r[8] for r in next(self.pages("Q.sql.page.lazy_tools")) if r[1] == 178), "  make drc\r")

    def test_existing_hidden_entry_predicate_applies_before_limits(self):
        rows = next(self.pages("Q.sql.page.lazy_tools", sql("Q.sql.chat.keep")))
        self.assertFalse({"blank", "ride:receipt"} & {r[2] for r in rows})
        self.assertEqual(len(rows), 49)


if __name__ == "__main__":
    unittest.main()
