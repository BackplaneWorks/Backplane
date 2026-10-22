#!/usr/bin/env python3
"""Print cbor.bend's balanced search trees (Cbor.keys.tree, Cbor.words.tree)
from its dictionaries. Run after appending to Cbor.keys()/Cbor.words():

  scripts/cbor-tree.py src/core/cbor.bend

and paste the output over the two tree defs. Law cbor_trees_exact fails
until they match the lists.
"""
import re, sys
src = open(sys.argv[1], encoding="utf-8").read()
def lst(name):
    body = re.search(r"def Cbor\." + name + r"\(\) -> List<&2, String>:\n(.*?)\n\n", src, re.S).group(1)
    return re.findall(r'"((?:[^"\\]|\\.)*)"', body)
def key(s):  # Bend's String.order: by code point
    return [ord(c) for c in s]
def tree(items):
    if not items: return "CLeaf{}"
    m = len(items) // 2
    w, i = items[m]
    return 'CNode{%s, "%s", %d, %s}' % (tree(items[:m]), w, i, tree(items[m + 1:]))
for name in ("keys", "words"):
    ws = lst(name)
    assert len(set(ws)) == len(ws), name + " has a duplicate"
    items = sorted(((w, i) for i, w in enumerate(ws)), key=lambda p: key(p[0]))
    print("def Cbor.%s.tree() -> CTree:\n  %s\n" % (name, tree(items)))
