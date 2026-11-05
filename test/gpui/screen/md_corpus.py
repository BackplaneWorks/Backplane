#!/usr/bin/env python3
"""A deterministic markdown corpus: hand-written cases plus seeded random mixes of tokens that exercise
blocks, emphasis, code, labels, links, paths, refs and bare ids. usage: md_corpus.py N SEED > corpus.json"""
import json, random, sys
n = int(sys.argv[1]) if len(sys.argv) > 1 else 2000
seed = int(sys.argv[2]) if len(sys.argv) > 2 else 1
HAND = [
 "", " ", "\n", "plain text", "a **b** *c* `d`", "**unclosed", "*open", "`open", "***x***", "** **", "* x", "a * b", "[x](https://a.b)", "[x](", "[x]", "![img](a.png) text",
 "# H1\n## H2\n### H3\n#### H4\n####### no", "#nospace", "- a\n- b\n\n- c", "* a\n+ b", "1. a\n2. b\n10. c", "1) x\n2) y", "> quote\n> more", "| a | b |\n|---|---|\n| 1 | 2 |",
 "| a |\n| b |", "|---|", "---", "***", "___", "- - -", "```\ncode\n```", "```\nunclosed", "```py\nx = 1\n```\ntext", "  ```\n  indented\n  ```",
 "see src/main.rs and ./x/y.txt:12 and ~/a/b#L3", "and/or 1/2 origin/main", "https://example.com/a(b)c). next", "(https://a.b/c)", "http://", "xhttp://a.b", "see backplane://thread/t1-2#m3 now",
 "t1700000000-1 and b1-2-3 and p1-2.", "t1700000000-1.", "t1-2~x", "a.b.c t1-2.t3-4", "`src/main.rs` and `plain`", "**`code` in bold** and *[x](https://a.b)*",
 "[**bold**](backplane://thread/t1-2)", "[a [b](https://c.d)](https://e.f)", "[x](https://a.b \"t\")", "![alt](https://a.b/i.png)", "\ttab\ttext", "é 日本 😀 `é`", "line one\nline two",
 "a\r\nb", "- item with [link](https://a.b) and `code`", "> **q** `c`", "| **b** | `c` |\n|:-:|--:|\n| x | y |", "1. a\n   - b",
]
TOK = ["word", "foo", "bar baz", " ", "  ", "\t", "\n", "\n\n", "**", "*", "`", "```", "[", "]", "(", ")", "![", "!", "[x](https://a.b/c)", "[](https://a.b)", "[x](src/a.rs)", "[x](file:///tmp/a.rs)",
 "[**b**](backplane://thread/t1-2)", "src/main.rs", "~/a/b.txt:12", "./x.md", "../y/z.rs#L3", "and/or", "origin/main", "/etc/hosts", "notes.md", "https://a.b/c?d=1&e=2", "http://x.y",
 "https://a.b/c).", "(https://a.b/c)", "https://a.b/c,", "t1700000000-1", "b1-2-3", "p1-2", "r1-2", "o1-2", "t1-2~x", "x~t1-2", "backplane://thread/t1-2#m1", "backplane://person", "backplane://file/p/a/b",
 "backplane://x", "http://h/#/thread/t1-2", "- ", "1. ", "10. ", "# ", "## ", "> ", "|", "| a | b |", "|---|", "---", "- - -", "é", "日本", "😀", ".", ",", ":", "-", "_", "<", ">", "\"", "'"]
rnd = random.Random(seed)
out = list(HAND)
while len(out) < n:
    k = rnd.randint(1, 14)
    out.append("".join(rnd.choice(TOK) for _ in range(k)))
print(json.dumps(out, ensure_ascii=False))
