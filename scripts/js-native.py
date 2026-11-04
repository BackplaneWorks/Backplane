#!/usr/bin/env python3
"""Base's string primitives as JavaScript's own, in a minified JS bundle.

Bend 2.0.36's JS target compares strings the way its Bend source reads
(`String.cmp`: a code point at a time, each step slicing off a fresh tail
and rebuilding both strings on the way out), so every `String.eq`, every
`Map.get`/`Map.set` on a String key (`Map.bit`: walks to the key's char by
slicing too) costs allocations quadratic in the strings' lengths. In the web
client that was a third of every frame. This replaces the bodies of those
defs with the same answers computed natively:

- `String.cmp(a, b)`: `((a, b), order)`, the order by code points (the
  strings it hands back are the ones it was given, as Bend's rebuild them)
- `Map.bit.go(key, ci, off)`: `(key, bit)`, the bit of the key's code point
  `ci` at offset `off` (0 is "there is such a char"; else bit 31-(off-1)),
  false past the key's end
- `String.split(s, sep)`: the pieces between seps, as a List (Base's is one
  stack frame per character, not a tail call: an agent's SKILL.md of some
  19,000 characters overflowed the hub's stack at a codex turn, and the
  server splits large text in hundreds of places)

- `List.length(xs)`: a loop (Base's is one stack frame per cell: the hub's
  Http.response.c measured a multi-MB body that way and overflowed the JS
  hub's stack the moment the desktop app loaded the web client)

- `String.starts_with(s, p)` and `String.contains(s, p)`: startsWith and
  indexOf, kept to code point boundaries as Base walks them (a match that
  starts or ends inside a surrogate pair of s is none). Base's walk a char at
  a time, slicing: the hub's request routing tests the raw text of every
  request with contains, and a 16 KB terminal paste held the hub 1.3 s, 64 KB
  for 12 s.

Server-only `Cbor.Enc.text(s, acc)` writes the same code-point UTF-8 bytes
straight onto its reversed byte accumulator. It counts UTF-8 width with a
native iteration first, avoiding the reference encoder's temporary byte list
and its reversal/copy. Lone surrogates remain their original three bytes;
TextEncoder would replace them and is deliberately not used. The original
Enc.head writes the length, so header limits and byte order stay unchanged.
No decoder, dictionary, wire format, cache, client or action state changes.

Server-only `String.drop(s, n)`: finds the offset by code points as Base
does and slices once. Base slices a fresh tail for every character dropped,
and the hub's control chunks drop 5461 characters off a 1.3 MB view text for
each packet: a third of every view reply on a large sidebar (2026-11-04).

Server-only `Json.parse(s)`: remembered for repeated texts of 1 KB or
more (the last 8). The hub's views parse the same models info (every
provider's models, opencode's hundreds) several times a render, every
render: measured 2026-10-24 on a long thread, most of a render's time. A
parse is pure and its value never changes, so the same text gives the same
value; smaller texts (short agent lines and requests) are parsed as before.
The 1 KB cutoff includes the measured 10.5 KB catalog and 3.2 KB shown-row
catalog that were each parsed repeatedly below the old 16 KB cutoff.
Texts of 64 bytes up to 1 KB are remembered too, the last 4096 of them: a
design step's record (core/hist.bend's Step.of, a few hundred bytes) is
parsed for every step of the thread, two or three times a render, while
the viewer is open (2026-11-02, a real 181-step thread: over half of the
viewer's render time, the viewer half of every view the hub sent).
The combined memo is charged at most16MiB including input and conservatively
counted expanded objects/strings; a single entry over2MiB or input over256Ki
UTF16 units parses uncached. Eligible input storage is owned before parsing,
so cached substring views cannot retain arbitrarily large parent strings.

Server-only `Json.lex(s)` scans token boundaries and unescaped string slices
without a new tail string and continuation for each character. It preserves
number lexemes, duplicate fields, the reference's whitespace/close tokens and
escaped surrogate replacement. Unsupported escapes/incomplete strings use the
retained pure lexer. Its entry shape and whole JSON module hash are fenced.
Server-only `String.reverse.go(s, acc)` reverses Unicode code points once and
appends acc; its exact complete Base body fences the substitution.

Server-only Utf8.decode and Lines.feed loop over immutable byte lists without
per-byte state rebuilding, preserving permissive overlong/invalid decoding and
reversed carry/queued-line ordering. Exact entries/callees and the pure module
hash fence both substitutions. No TextDecoder normalization is used.

Server-only String.to_lower/take, ASCII Fold.text and Utf8.encode remove
recursive text rebuilding in database search indexing. Base lowercase stays
ASCII-only; raw-surrogate errors and non-ASCII search folding retain their
pure paths. Prefixes count code points and encoding preserves raw surrogate
bytes. Complete Base hashes and pure module/wrapper/callee fences apply.

Server-only `Json.show(j)`: an iterative compact serializer preserving the
pure encoder's exact escapes, raw number text, field order and duplicate
keys. The original body remains the fallback for non-ASCII raw numbers.
No memo or protocol/client changes; scripts/js-json-show.js holds the helper.

Server-only Json.bytes.esc/str count code-point UTF-8 widths without slicing
a fresh string tail per character. Exact JSON escapes, raw lexemes and Nat
overflow remain the pure counter's; no whole-tree or cap predicate changes.
Server-only identity memos (js-memo.js) for pure defs asked again with the
same values: MEMO2 (Subs.aggs, the top bar's and the subagents panel's: the
very same first argument, weakly, and an equal String) and MEMO1
(SubHistory.own: a read's rows as a state, by the rows' identity, which
db.js's read cache keeps for an unchanged read). Pure and immutable, so
nothing is stale.

Server-only Tree.children(s, id) (core/subthreads.bend) works out every
thread's display parent once per state (a WeakMap: a state never changes)
with the very display_parent and Thread.id its walk calls, in Tree.all's
order, and sorts each id's list with the pure sort; the pure def scanned all
threads for each id the sidebar asked (2026-11-05: 88 → 24 ms a click
session, 318 of 318 real answers identical). Exact entry, walk and
Tree.roots.add bodies fence it.

Server-only Json.bytes(j) counts the same total over the tree and keeps each
object's and array's size by identity (a WeakMap: immutable values, so a
size never changes), so the several measurements of one reply on its way out
count it once (3-7 ms a view on a real home, 2026-11-05). Malformed chains
count as Json.bytes.go counts them; an unknown constructor goes to go.

Server-only ContentEq.same compares actual JSON constructors and fields, with
an identity fast path for immutable values. It does not compare dependency
keys or hashes. The pure wrapper, callee and source hash fence the rewrite.

Server-only `Sec.fine(vs, s)` (src/core/secret.bend, the secret box's
fine mask, run on every agent line, committed change and MCP answer that
holds a stored value): the same cover computed with indexOf at code point
boundaries, O(text + occurrences), where the pure walk re-slices the text
per character (a 100 KB line with a secret took 2.2 s). Fenced by the pure
module's hash; test/tools/secret.mjs checks it against the definition.
Sec.mask's laws hold whatever it answers (the pure check follows it).

Server-only Map.bit/Map.diff implement the same 33-bit code-point trie
positions without generic Nat.divmod, per-character slicing or iterators.
On the 5,750-thread catalog these primitives dominate synchronous sidebar
generation. Complete emitted entry/callee hashes fence Base's definitions;
the pure map, its callers, cache budgets and every protocol stay unchanged.
Map.get/has walk that immutable trie without reconstructing equal ancestors.
Map.set retains the root for an existing identical numeric value, avoiding
status projection no-op rebuilding; all other updates use the pure setter.
Their input root is returned unchanged alongside the original value/default.

Web-only `Refs.last.go(s, cur)` (src/core/refs.bend: the word being typed,
Refs.token, which the composer's slash, skill and reference popups ask on
every render): the part of s after its last space, tab, CR or LF, or, with
none, cur reversed and then s; found by scanning back from the end. Bend's
walk is from the front, a code point at a time, slicing: a 20 KB paste took
seconds of renders. Fenced by the exact emitted body and its three callees
(Refs.cur, Refs.sep, String.reverse); at most once per web entry.

Each is found by its shape and must be found exactly once, else the build
stops (a new Bend that emits them otherwise needs this looked at again).
With --server (scripts/build-js.sh: one bundle) that holds for every def;
for the web client's chunks (no flag) each separately compiled Bend entry
contributes its own Base helpers. String.cmp and Map.bit.go must have the
same positive copy count; optional helpers occur at most once per copy.
Every matched body must still be rewritten exactly once. Server fences
remain exactly-one.
`bun test/tools/js_native_test.ts` checks both against Base's definitions.

  scripts/js-native.py dist/web/chunk-*.js
"""
import re
import json
import hashlib
import sys
from pathlib import Path

I = r"[A-Za-z0-9_$]+"

# the helpers the new bodies call, put before everything else in the file
HELPERS = (
    "function __bpOrd(a,b){if(a===b)return\"EQ\";const n=Math.min(a.length,b.length);let i=0;"
    "while(i<n&&a.charCodeAt(i)===b.charCodeAt(i))i++;if(i===n)return a.length<b.length?\"LT\":\"GT\";"
    "return a.codePointAt(i)<b.codePointAt(i)?\"LT\":\"GT\"}"
    "function __bpBit(k,ci,off){let n=0;for(const c of k){if(n===ci){if(off===0)return true;"
    "return((c.codePointAt(0)>>>(31-(off-1)))&1)!==0}n++}return false}"
    "function __bpSplit(s,sep){let ps;const u=sep.charCodeAt(0);if(sep.length===1&&(u<0xd800||u>0xdfff))ps=s.split(sep);"
    "else{ps=[];let cur=\"\";for(const c of s){if(c===sep){ps.push(cur);cur=\"\"}else cur+=c}ps.push(cur)}"
    "let xs={$:\"Nil\"};for(let i=ps.length-1;i>=0;i--)xs={$:\"Con\",head:ps[i],tail:xs};return xs}"
    "function __bpCut(s,i){const a=s.charCodeAt(i-1),b=s.charCodeAt(i);return i>0&&i<s.length&&a>=0xd800&&a<=0xdbff&&b>=0xdc00&&b<=0xdfff}"
    "function __bpPre(s,p){return s.startsWith(p)&&!__bpCut(s,p.length)}"
    "function __bpHas(s,p){for(let i=s.indexOf(p);i>=0;i=s.indexOf(p,i+1))if(!__bpCut(s,i)&&!__bpCut(s,i+p.length))return true;return false}\n"
)

# String.cmp: function F(a,b){if(a==="")if(b==="")return{$:"Tuple",fst:{$:"Tuple",fst:"",snd:""},snd:{$:"EQ"}};...
CMP = re.compile(
    r'function (' + I + r')\((' + I + r'),(' + I + r')\)\{(?=if\(\2===""\)if\(\3===""\)return\{\$:"Tuple",fst:\{\$:"Tuple",fst:"",snd:""\},snd:\{\$:"EQ"\}\};)'
)
# Map.bit.go: function F(k,ci,off){if(k==="")return{$:"Tuple",fst:"",snd:!1};else{let c=k.codePointAt(0)>65535?k.slice(0,2):k[0],t=...;if(ci===0)
BIT = re.compile(
    r'function (' + I + r')\((' + I + r'),(' + I + r'),(' + I + r')\)\{(?=if\(\2===""\)return\{\$:"Tuple",fst:"",snd:!1\};else\{let ' + I + r'=\2\.codePointAt\(0\)>65535\?\2\.slice\(0,2\):\2\[0\],' + I + r'=\2\.codePointAt\(0\)>65535\?\2\.slice\(2\):\2\.slice\(1\);if\(\3===0\))'
)

# String.split: function F(s,sep){if(s==="")return{$:"Con",head:"",tail:{$:"Nil"}};else{let h=...
SPLIT = re.compile(
    r'function (' + I + r')\((' + I + r'),(' + I + r')\)\{(?=if\(\2===""\)return\{\$:"Con",head:"",tail:\{\$:"Nil"\}\};else\{let ' + I + r'=\2\.codePointAt\(0\)>65535\?\2\.slice\(0,2\):\2\[0\],)'
)

# List.length: function F(xs){if(xs.$==="Nil")return 0;else{let t=xs.tail;return nat_chk(F(t)+1)}}
LEN = re.compile(
    r'function (' + I + r')\((' + I + r')\)\{if\(\2\.\$==="Nil"\)return 0;else\{let (' + I + r')=\2\.tail;return (nat_chk)\(\1\(\3\)\+1\)\}\}'
)

# String.starts_with: function F(s,p,t){let pc=0;for(;;)switch(pc){case 0:{let a=s,b=p;if(a==="")if(b==="")return!0;else return!1;
PRE = re.compile(
    r'function (' + I + r')\((' + I + r'),(' + I + r'),' + I + r'\)\{(?=let (' + I + r')=0;for\(;;\)switch\(\4\)\{case 0:\{let (' + I + r')=\2,(' + I + r')=\3;if\(\5===""\)if\(\6===""\)return!0;else return!1;)'
)
# String.contains: function F(s,p,t){let pc=0;for(;;)switch(pc){case 0:{let a=s,b=p;if(a==="")return E(b);else{let h=a.codePointAt(0)>65535?
HAS = re.compile(
    r'function (' + I + r')\((' + I + r'),(' + I + r'),' + I + r'\)\{(?=let (' + I + r')=0;for\(;;\)switch\(\4\)\{case 0:\{let (' + I + r')=\2,(' + I + r')=\3;if\(\5===""\)return ' + I + r'\(\6\);else\{let ' + I + r'=\5\.codePointAt\(0\)>65535\?)'
)

# The unchanged pure Enc.text is one call to Enc.u8 then Enc.text.k. Keep
# both its namespaced identity and its exact emitted shape as build fences.
CBOR_TEXT = re.compile(
    r'function (' + I + r'\$058Enc\$text\$)\((' + I + r'),(' + I + r')\)\{'
    r'(?=return ' + I + r'\$058Enc\$text\$k\$\(' + I + r'\$058Enc\$u8\$\(\2,\{\$:"[^"\n]+\.U8",n:0,rev:\{\$:"Nil"\}\}\),\3\)\})'
)

# Json.parse: function F(s){return ...$058Parse$fin$(...$058Parse$go$(...$058Json$lex$(s),...
PARSE = re.compile(
    r'function (' + I + r'\$058Json\$parse\$)\((' + I + r')\)\{(?=return ' + I + r'\$058Parse\$fin\$\()'
)
PARSE_HELPER = (
    "const __bpParseMemo=new Map(),__bpParseSmall=new Map(),__bpParseOrder=new Map();\n"
    "let __bpParseRetained=0;\n"
    "const __bpParseLimit=16777216,__bpParseEntryLimit=2097152,__bpParseInputLimit=262144;\n"
    "function __bpParseWeight(s,v){\n"
    " let n=512+s.length*2;const todo=[v],seen=new Set();\n"
    " while(todo.length){const x=todo.pop();\n"
    "  if(typeof x===\"string\")n+=32+x.length*2;\n"
    "  else if(x&&typeof x===\"object\"){\n"
    "   if(seen.has(x)){n+=16;continue}seen.add(x);n+=64;\n"
    "   if(Array.isArray(x)){n+=x.length*16;if(n>__bpParseEntryLimit)return n;for(const y of x)todo.push(y)}\n"
    "   else for(const k of Object.keys(x)){n+=16+k.length*2;todo.push(x[k])}\n"
    "  }else n+=16;\n"
    "  if(n>__bpParseEntryLimit)return n;\n"
    " }return n;\n"
    "}\n"
    "function __bpParseDrop(k){const e=__bpParseOrder.get(k);if(e){e.m.delete(k);__bpParseOrder.delete(k);__bpParseRetained-=e.n}}\n"
    "function __bpParsed(s,f){\n"
    " if(typeof s!==\"string\"||s.length<64||s.length>__bpParseInputLimit)return f(s);\n"
    " const m=s.length<1024?__bpParseSmall:__bpParseMemo,cap=s.length<1024?4096:8;\n"
    " const found=m.get(s);if(found!==undefined)return found;\n"
    " // Own the eligible text before parsing: substring keys and parsed slices\n"
    " // must not pin a much larger backing string. UTF16 preserves lone units.\n"
    " const key=Buffer.from(s,\"utf16le\").toString(\"utf16le\"),v=f(key),n=__bpParseWeight(key,v);\n"
    " if(n>__bpParseEntryLimit)return v;\n"
    " if(m.size>=cap)__bpParseDrop(m.keys().next().value);\n"
    " while(__bpParseRetained+n>__bpParseLimit&&__bpParseOrder.size)__bpParseDrop(__bpParseOrder.keys().next().value);\n"
    " m.set(key,v);__bpParseOrder.set(key,{m,n});__bpParseRetained+=n;return v;\n"
    "}\n"
    "globalThis.BACKPLANE_PARSE_MEMO_STATS=()=>({large:__bpParseMemo.size,small:__bpParseSmall.size,chargedBytes:__bpParseRetained,maxChargedBytes:__bpParseLimit,maxInputCodeUnits:__bpParseInputLimit,maxEntryChargedBytes:__bpParseEntryLimit});\n"
)

# Exact pure lexer entry and state constructors; the full JSON module hash
# below also fences escape, surrogate and token-boundary behavior.
JSON_LEX = re.compile(
    r'function (' + I + r'\$058Json\$lex\$)\((' + I + r')\)\{return (' + I +
    r'\$058Lex\$go\$)\(\2,\{\$:"Tuple",fst:\{\$:"([^"\n]+)\.LIdle"\},snd:\{\$:"Nil"\}\}\)\}'
)
REVERSE_GO = re.compile(
    r'function \$String\$reverse\$go\$\(\$0,\$1\)\{for\(;;\)\{let _s_0=\$0,_acc_0=\$1;if\(_s_0===""\)return _acc_0;else\{let _h_0=_s_0\.codePointAt\(0\)>65535\?_s_0\.slice\(0,2\):_s_0\[0\];\$0=_s_0\.codePointAt\(0\)>65535\?_s_0\.slice\(2\):_s_0\.slice\(1\),\$1=_h_0\+_acc_0;continue\}\}\}'
)

# Exact emitted entry wrappers plus the whole pure UTF-8/framing module.
# Framing keeps reversed carry bytes and newest-first queued lines unchanged.
UTF8_DECODE = re.compile(
    r'function (' + I + r'\$058Utf8\$decode\$)\((' + I + r')\)\{return (' + I +
    r'\$058Utf8\$fin\$)\((' + I + r'\$058Utf8\$go\$)\(\2,\{\$:"([^"\n]+)\.Dec",acc:"",cp:0,owe:0\}\)\)\}'
)
LINES_FEED = re.compile(
    r'function (' + I + r'\$058Lines\$feed\$)\((' + I + r'),(' + I + r')\)\{return (' + I +
    r'\$058Lines\$split\$)\((' + I + r'\$058Lines\$go\$)\(\3,\2\)\)\}'
)
UTF8_SOURCE_SHA256 = "0202c5087fd255df10413080cda57f2a4a00aa9b90b5b425a00a729f9563627b"

UTF8_ENCODE = re.compile(
    r'function (' + I + r'\$058Utf8\$encode\$)\((' + I + r')\)\{return (' + I +
    r'\$058Utf8\$encode_onto\$)\(\2,\{\$:"Nil"\}\)\}'
)
FOLD_TEXT = re.compile(
    r'function (' + I + r'\$058Fold\$text\$)\((' + I + r')\)\{return (' + I + r'\$058Fold\$go\$)\(\2,""\)\}'
)
FOLD_SOURCE_SHA256 = "9f5bd7f7249b7ece879f3fb991af03a8ca198a0de68a26d643701ace430742ad"
TEXT_DEFS = {
    "$String$to_lower$": "68a284ae5e968b09d4b5e3defa9949eca24fe1670f4c1409ebe8a53bfc4630ba",
    "$Char$to_lower$": "a882f9514c243fb49890e458e03f422632d970100a6e2834e74f851514d68c61",
    "$Char$is_upper$": "cd25201f239e1b822957470ab197fc16973d6e677ff2a3b05b8a30d5e2710fac",
    "$String$take$": "8ce2088bba133bf0a09fe1a4aa357a4e1235eac7bc5c268276ee68503ab64daa",
    "$String$drop$": "e1e25ef4c2edfdb17b6148e820cd4c2f5cc4d2d9f6e20ad2b917edfc2d8fc427",
}

# Match the whole unchanged Json.show body and its exact Json.emit callee.
# Unlike a prefix match, any body change must stop this optimization.
SHOW = re.compile(
    r'function (' + I + r'\$058Json\$show\$)\((' + I + r')\)\{'
    r'return (' + I + r'\$058Json\$emit\$)\(\2,!0,""\)\}'
)

# The exact, one-call pure structural-comparison wrapper. Its source hash
# also fences the recursive comparator whose answer this helper implements.
CONTENT_EQ = re.compile(
    r'function (' + I + r'\$058ContentEq\$same\$)\((' + I + r'),(' + I + r')\)\{'
    r'return (' + I + r'\$058ContentEq\$go\$)\(\2,\3\)\}'
)
CONTENT_EQ_SOURCE_SHA256 = "054cdf42cc59907d5c860754a1566b90831384cd1d9cd97918f455aa47ee653a"

# Both complete emitted loop bodies, including their exact character callee,
# are build fences. The pure module hash also guards the width/escape rules.
def byte_loop(kind):
    before = "($0,$1){for(;;){let _s_0=$0,_n_0=$1;if(_s_0===\"\")return _n_0;else{let _t_0=_s_0.codePointAt(0)>65535?_s_0.slice(0,2):_s_0[0],_t_1=_s_0.codePointAt(0)>65535?_s_0.slice(2):_s_0.slice(1),_x_0="
    after = "(_t_0.codePointAt(0));$0=_t_1,$1=nat_chk(_n_0+_x_0);continue}}}"
    return re.compile(r"function (" + I + r"\$058Json\$bytes\$" + kind + r"\$)" +
                      re.escape(before) + "(" + I + r"\$058Json\$bytes\$" +
                      ("chr" if kind == "esc" else "raw") + r"\$)" + re.escape(after))

BYTE_ESC, BYTE_STR = byte_loop("esc"), byte_loop("str")
# pure core defs of an object and a String kept by identity (js-memo.js);
# each must be a def of exactly these two parameters, found once
MEMO2 = (("Subs.aggs", r"\$\$\$\$047core\$047subs\$058Subs\$aggs\$", "_st_0", "_th_0"),)
# pure defs of one object kept by identity; each a def of exactly this parameter, found once
MEMO1 = (("SubHistory.own", r"\$subagent\$045history\$058SubHistory\$own\$", "_rows_0"),
         # the page's sidebar region, found by a walk of the whole page tree,
         # asked ~3 times a render about the very same rendered page
         ("Slot.sidebar", r"\$\$\$\$047web\$047sidebar\$045slot\$058Slot\$sidebar\$", "_o_0"),
         ("Slot.count", r"\$\$\$\$047web\$047sidebar\$045slot\$058Slot\$count\$", "_o_0"))
# pure defs of two objects (by identity) and two Strings/numbers; found once
MEMO4 = (("Subs.rows", r"\$\$\$\$047core\$047subs\$058Subs\$rows\$", "_st_0", "_info_0", "_th_0", "_now_0"),)
def memo4_rx(entry, a, b, c, d):
    return re.compile(r"function (" + entry + r")\(" + a + "," + b + "," + c + "," + d + r"\)\{")
def memo1_rx(entry, a):
    return re.compile(r"function (" + entry + r")\(" + a + r"\)\{")
def memo2_rx(entry, a, b):
    return re.compile(r"function (" + entry + r")\(" + a + "," + b + r"\)\{")
# Tree.children(s, id) = sort(Tree.children.go(Tree.all(s), s, id)), and that
# walk exactly an in-order filter on display_parent through Tree.roots.add
TREE_KIDS = re.compile(r"function (" + I + r"\$058Tree\$children\$)\(_s_0,_id_0\)\{return (" + I + r")\((" + I + r"\$058Tree\$children\$go\$)\((" + I + r"\$058Tree\$all\$)\(_s_0\),_s_0,_id_0\)\)\}")
TREE_GO = (r"function {go}\(_ts_0,_s_0,_id_0\)\{{if\(_ts_0\.\$===\"Nil\"\)return\{{\$:\"Nil\"\}};else\{{let\{{head:_t_0,tail:_rest_0\}}=_ts_0;"
           r"return ({add})\(\$Bool\$not\$\(\$String\$eq\$\(({parent})\(_s_0,({tid})\(_t_0\)\),_id_0\)\),_t_0,{go}\(_rest_0,_s_0,_id_0\)\)\}}\}}")
TREE_ADD = r"function {add}\(_skip_0,_t_0,_rest_0\)\{{if\(_skip_0\)return _rest_0;else return\{{\$:\"Con\",head:_t_0,tail:_rest_0\}}\}}"
# Json.bytes(j), exactly the pure entry over its own Json.bytes.go
BYTES = re.compile(r"function (" + I + r"\$058Json\$bytes\$)\(_j_0\)\{return (" + I + r"\$058Json\$bytes\$go\$)\(_j_0,!0,0\)\}")
# Lexer-only surrogate repair reviewed; byte counters/callees remain unchanged.
JSON_BYTES_SOURCE_SHA256 = "f500b30686fc4eea0f6d7ac00bda03e524676f59dd00c6244696e678536560af"

SEC_FINE = re.compile(
    r'function (' + I + r'\$058Sec\$fine\$)\((' + I + r'),(' + I + r')\)\{return (' + I + r'\$058Sec\$fine\$go\$)\(\3,0,\2,""\)\}'
)
SEC_FINE_SOURCE_SHA256 = "2aa67d789bcaf5d4de369af089b533d2a3fdfc423d90e545f5d8a84423159bd6"
SEC_FINE_HELPER = (
    "function __bpSecFine(vs,s){const v=[];for(let l=vs;l.$!==\"Nil\";l=l.tail){const t=l.head.text;if(t!==\"\")v.push(t)}"
    "if(!v.length||s===\"\")return s;const at=new Int32Array(s.length+1).fill(-1),cp=[];let n=0;"
    "for(let i=0;i<s.length;){const c=s.codePointAt(i)>65535?s.slice(i,i+2):s[i];at[i]=n;cp.push(c);i+=c.length;n++}at[s.length]=n;"
    "const d=new Int32Array(n+1);let any=!1;for(const t of v){let len=0;for(const _ of t)len++;"
    "for(let i=s.indexOf(t);i>=0;i=s.indexOf(t,i+1)){const j=at[i];if(j<0||at[i+t.length]<0)continue;d[j]++;d[Math.min(n,j+len)]--;any=!0}}"
    "if(!any)return s;let out=\"\",run=0;for(let j=0;j<n;j++){run+=d[j];out+=run>0?\"\\u2022\":cp[j]}return out}\n"
)

# Refs.last.go, minified: function F(s,c){for(;;){let a=s,b=c;if(a==="")return REV(b);else{let h=...;s=...,c=CUR(SEP(h.codePointAt(0)),h.codePointAt(0),b);continue}}}
REFS_LAST = re.compile(
    r'function (' + I + r')\((' + I + r'),(' + I + r')\)\{for\(;;\)\{let (' + I + r')=\2,(' + I + r')=\3;if\(\4===""\)return (' + I + r')\(\5\);'
    r'else\{let (' + I + r')=\4\.codePointAt\(0\)>65535\?\4\.slice\(0,2\):\4\[0\];\2=\4\.codePointAt\(0\)>65535\?\4\.slice\(2\):\4\.slice\(1\),'
    r'\3=(' + I + r')\((' + I + r')\(\7\.codePointAt\(0\)\),\7\.codePointAt\(0\),\5\);continue\}\}\}'
)
REFS_HELPER = (
    "function __bpRefsLast(s,c){let i=s.length;while(i>0){const u=s.charCodeAt(i-1);if(u===32||u===10||u===9||u===13)break;i--}"
    "return i>0?s.slice(i):[...c].reverse().join(\"\")+s}\n"
)
def refs_fenced(src, m):
    """the callees are exactly Refs.cur, Refs.sep and String.reverse"""
    cur, sep, rev = re.escape(m.group(8)), re.escape(m.group(9)), re.escape(m.group(6))
    return (len(re.findall(r'function ' + cur + r'\((' + I + r'),(' + I + r'),(' + I + r')\)\{if\(\1\)return"";else return ' + I + r'\(\2\)\+\3\}', src)) == 1 and
            len(re.findall(r'function ' + sep + r'\((' + I + r')\)\{return \1===32\|\|\1===10\|\|\(\1===9\|\|\1===13\)\}', src)) == 1 and
            len(re.findall(r'function ' + rev + r'\((' + I + r')\)\{return ' + I + r'\(\1,""\)\}', src)) == 1)

# Bend 2.0.39 Base map definitions, after the standard Bun minification.
# Unlike the existing prefix rewrites, these fences cover whole definitions
# and the complete Map.diff helper chain. Compiler/Base drift needs review.
MAP_DEFS = {
    "$Map$set$": "cd028def3d1ecccffb11be69c9d5bae9c6ecde4275a5b79e2944f167f6f31a23",
    "$Map$bit$": "dd4233a5037a0d20842d148e7b38363f363fe1d52a811a39b9328667948dc4d9",
    "$Map$bit$at$": "95c255398d9ee74d17c933422bfeb2405c1cdb4be0113dc02aa3b4da2cc28309",
    "$Map$diff$": "7ce09b0adfd68677f26d35b394c01ec5f15eb05a0a4b7bbf5a25978c4597f0fe",
    "$Map$diff$fin$": "9d48784a9cc56f4c6e9bc1673c6470df648587a4f2980d9a512143ce5bfa4b2a",
    "$Map$diff$step$": "a39747002ab175e20d0809aac6432a8bdd6172e37fd08f6fbabfa8442be6d40c",
    "$Map$diff$chr$": "705898200e8a2e029ce9ca63bea5d54111e0883faab72ec8a0fe41d2e1a447f6",
    "$Map$msb$u$": "b74e19a892bcd2916ff947c10282d637cdce13fe7303a5f5e1acbf5f16701532",
    "$Map$msb$u$if$": "23e87ee5b37be52d7de67a0b056bd02aed3b7ed154b2ec0f6daa0cfd97e5123d",
    "$Map$get$": "3191b2a2af8d9ae21e6a53396f29db5058aafdb6b0e82cec062381db2be0516d",
    "$Map$get$go$": "088c116da48bd2e63846f0835f81ce4a81a886b59aca64842a396bd5b669928e",
    "$Map$get$leaf$": "9590a61bcc8fa5ce5c670189d6a3f09b25af41ec98959e0fb0a2a83f9b0d2b01",
    "$Map$has$": "229c41ccf6e51168d32fc75185822dc7cf846152bcaae64c70717fb78e64d7cb",
    "$Map$has$go$": "3bd24a89d7bce32fab5521e23238e44a668b3e98eb303efd4ae391f076acc8c8",
    "$Map$has$leaf$": "055e50dd78310908a5612208a5967185d018ed82d6b82815e4de5f1b3e469972",
    "$Map$lo$": "b0f937536eaf4daff4a5976af25e1e37bcdd74c95c13df77cc1a2436b459b21b",
    "$Map$hi$": "b886c4c910bc97eb48b1c1d41769fd84b61322d7e8b985f5f20fdeaf0b1c7341",
}

def map_def(name):
    return re.compile(r"function " + re.escape(name) + r"\([^\n]*?\}(?=function |\n|$)")

args = sys.argv[1:]
server = "--server" in args
files = [a for a in args if a != "--server"]
srcs = {f: open(f, encoding="utf-8").read() for f in files}
if server:
    for name, digest in {**MAP_DEFS, **TEXT_DEFS}.items():
        definitions = [m.group() for s in srcs.values() for m in map_def(name).finditer(s)]
        if len(definitions) != 1 or hashlib.sha256(definitions[0].encode()).hexdigest() != digest:
            sys.exit(f"js-native: {name} needs its exact Base definition once")
found = {}
found["Map.bit"] = found["Map.diff"] = int(server)
found["Map.get"] = found["Map.has"] = found["Map.set"] = int(server)
found["String.to_lower"] = found["String.take"] = found["String.drop"] = int(server)
for name, rx, required in (("String.cmp", CMP, True), ("Map.bit.go", BIT, True),
                           ("String.split", SPLIT, server), ("List.length", LEN, server),
                           ("String.starts_with", PRE, server), ("String.contains", HAS, server)):
    n = sum(len(rx.findall(s)) for s in srcs.values())
    if server and n != 1 or not server and required and n < 1:
        sys.exit(f"js-native: {name} found {n} times, expected " + ("once" if server else "at least once"))
    found[name] = n
if not server:
    copies = found["String.cmp"]
    if found["Map.bit.go"] != copies:
        sys.exit("js-native: each web Bend entry needs matching String.cmp and Map.bit.go copies")
    for name in ("String.split", "List.length", "String.starts_with", "String.contains"):
        if found[name] > copies:
            sys.exit(f"js-native: {name} exceeds the web Bend entry count")
found["Refs.last.go"] = 0
if not server:
    for s in srcs.values():
        # the loop's shape is generic (other word walks share it): its
        # callees, exactly Refs.cur, Refs.sep and String.reverse, name it
        ms = [m for m in REFS_LAST.finditer(s) if refs_fenced(s, m)]
        if len(ms) > 1:
            sys.exit("js-native: Refs.last.go needs its exact body and callees, once per entry")
        found["Refs.last.go"] += len(ms)
found["Sec.fine"] = sum(len(SEC_FINE.findall(s)) for s in srcs.values())
if (server and found["Sec.fine"] != 1) or (not server and found["Sec.fine"] > found["String.cmp"]):
    sys.exit(f"js-native: Sec.fine found {found['Sec.fine']} times, not once")
if found["Sec.fine"] and hashlib.sha256(Path(__file__).parent.parent.joinpath("src/core/secret.bend").read_bytes()).hexdigest() != SEC_FINE_SOURCE_SHA256:
    sys.exit("js-native: Sec.fine pure source changed")
for s in srcs.values():
    for match in SEC_FINE.finditer(s):
        if match.group(4) != match.group(1).replace("$058Sec$fine$", "$058Sec$fine$go$"):
            sys.exit("js-native: Sec.fine needs its exact Sec.fine.go")
found["Cbor.Enc.text"] = sum(len(CBOR_TEXT.findall(s)) for s in srcs.values()) if server else 0
found["Json.parse"] = sum(len(PARSE.findall(s)) for s in srcs.values()) if server else 0
found["Json.lex"] = sum(len(JSON_LEX.findall(s)) for s in srcs.values()) if server else 0
found["String.reverse.go"] = sum(len(REVERSE_GO.findall(s)) for s in srcs.values()) if server else 0
if server and (found["Json.lex"] != 1 or found["String.reverse.go"] != 1):
    sys.exit("js-native: Json.lex and String.reverse.go need their exact definitions once")
if server:
    for s in srcs.values():
        for m in JSON_LEX.finditer(s):
            expected = m.group(1).replace("$058Json$lex$", "$058Lex$go$")
            if m.group(3) != expected or len(re.findall(r"function " + re.escape(expected) + r"\(", s)) != 1:
                sys.exit("js-native: Json.lex needs its exact Lex.go once")
            if not re.search(r'\$:"' + re.escape(m.group(4)) + r'\.TStr"', s):
                sys.exit("js-native: Json.lex token namespace changed")
for name, rx in (("Utf8.decode", UTF8_DECODE), ("Lines.feed", LINES_FEED), ("Utf8.encode", UTF8_ENCODE), ("Fold.text", FOLD_TEXT)):
    found[name] = sum(len(rx.findall(s)) for s in srcs.values()) if server else 0
    if server and found[name] != 1:
        sys.exit(f"js-native: {name} needs its exact definition once")
if server:
    if hashlib.sha256(Path(__file__).parent.parent.joinpath("src/core/utf8.bend").read_bytes()).hexdigest() != UTF8_SOURCE_SHA256:
        sys.exit("js-native: UTF-8/framing pure source changed")
    for s in srcs.values():
        for rx, pairs in ((UTF8_DECODE, ((3, "$058Utf8$fin$"), (4, "$058Utf8$go$"))),
                          (LINES_FEED, ((4, "$058Lines$split$"), (5, "$058Lines$go$")))):
            for m in rx.finditer(s):
                for group, suffix in pairs:
                    expected = m.group(1).rsplit("$058", 1)[0] + suffix
                    if m.group(group) != expected or len(re.findall(r"function " + re.escape(expected) + r"\(", s)) != 1:
                        sys.exit("js-native: UTF-8/framing callee changed")
        for m in UTF8_DECODE.finditer(s):
            if not re.search(r'\$:"' + re.escape(m.group(5)) + r'\.Lines",part:', s):
                sys.exit("js-native: UTF-8/framing constructor namespace changed")
if server:
    if hashlib.sha256(Path(__file__).parent.parent.joinpath("src/core/search-fold.bend").read_bytes()).hexdigest() != FOLD_SOURCE_SHA256:
        sys.exit("js-native: search folding pure source changed")
    for s in srcs.values():
        for rx, old, new in ((UTF8_ENCODE, "$058Utf8$encode$", "$058Utf8$encode_onto$"),
                             (FOLD_TEXT, "$058Fold$text$", "$058Fold$go$")):
            for m in rx.finditer(s):
                expected = m.group(1).replace(old, new)
                if m.group(3) != expected or len(re.findall(r"function " + re.escape(expected) + r"\(", s)) != 1:
                    sys.exit("js-native: encoding/folding callee changed")
found["Json.show"] = sum(len(SHOW.findall(s)) for s in srcs.values()) if server else 0
found["ContentEq.same"] = sum(len(CONTENT_EQ.findall(s)) for s in srcs.values()) if server else 0
def tree_kids(s, m):
    """the walk's display_parent and Thread.id, when Tree.children.go and Tree.roots.add are exactly the pure ones"""
    go, sub = m.group(3), m.group(1).rsplit("$058Tree$children$", 1)[0]
    if go != m.group(1) + "go$" or m.group(4) != sub + "$058Tree$all$":
        return None
    w = re.search(TREE_GO.format(go=re.escape(go), add=re.escape(sub + "$058Tree$roots$add$"),
                                 parent=re.escape(sub + "$058Tree$display_parent$"), tid=I + r"\$058Thread\$id\$"), s)
    if w is None or len(re.findall(r"function " + re.escape(go) + r"\(", s)) != 1:
        return None
    add = w.group(1)
    if not re.search(TREE_ADD.format(add=re.escape(add)), s) or len(re.findall(r"function " + re.escape(add) + r"\(", s)) != 1:
        return None
    for f in (w.group(2), w.group(3)):
        if len(re.findall(r"function " + re.escape(f) + r"\(", s)) != 1:
            return None
    return w.group(2), w.group(3)
for name, entry, a in MEMO1:
    found[name] = sum(len(memo1_rx(entry, a).findall(s)) for s in srcs.values()) if server else 0
    if server and found[name] != 1:
        sys.exit(f"js-native: {name} found {found[name]} times as a def of ({a}), not once")
for name, entry, a, b in MEMO2:
    found[name] = sum(len(memo2_rx(entry, a, b).findall(s)) for s in srcs.values()) if server else 0
    if server and found[name] != 1:
        sys.exit(f"js-native: {name} found {found[name]} times as a def of ({a}, {b}), not once")
for name, entry, a, b, c, d in MEMO4:
    found[name] = sum(len(memo4_rx(entry, a, b, c, d).findall(s)) for s in srcs.values()) if server else 0
    if server and found[name] != 1:
        sys.exit(f"js-native: {name} found {found[name]} times as a def of ({a}, {b}, {c}, {d}), not once")
found["Tree.children"] = sum(len(TREE_KIDS.findall(s)) for s in srcs.values()) if server else 0
if server and found["Tree.children"] != 1:
    sys.exit(f"js-native: Tree.children found {found['Tree.children']} times, not once")
if server:
    for s in srcs.values():
        for m in TREE_KIDS.finditer(s):
            if tree_kids(s, m) is None:
                sys.exit("js-native: Tree.children needs its exact Tree.children.go and Tree.roots.add once")
for name, rx in (("Json.bytes.esc", BYTE_ESC), ("Json.bytes.str", BYTE_STR), ("Json.bytes", BYTES)):
    found[name] = sum(len(rx.findall(s)) for s in srcs.values()) if server else 0
    if server and found[name] != 1:
        sys.exit(f"js-native: {name} found {found[name]} times, not once")
if server and hashlib.sha256(Path(__file__).parent.parent.joinpath("src/core/json.bend").read_bytes()).hexdigest() != JSON_BYTES_SOURCE_SHA256:
    sys.exit("js-native: Json.bytes pure source changed")
if server:
    for s in srcs.values():
        for rx, kind, callee in ((BYTE_ESC, "esc", "chr"), (BYTE_STR, "str", "raw")):
            for match in rx.finditer(s):
                expected = match.group(1).replace("$058Json$bytes$" + kind + "$", "$058Json$bytes$" + callee + "$")
                if match.group(2) != expected or len(re.findall(r"function " + re.escape(expected) + r"\(", s)) != 1:
                    sys.exit(f"js-native: Json.bytes.{kind} needs its exact character callee once")
        for match in BYTES.finditer(s):
            if match.group(2) != match.group(1) + "go$" or len(re.findall(r"function " + re.escape(match.group(2)) + r"\(", s)) != 1:
                sys.exit("js-native: Json.bytes needs its exact Json.bytes.go once")
        if len(re.findall(r"function nat_chk\(", s)) != 1:
            sys.exit("js-native: Json.bytes needs its exact Nat guard once")
if server and found["ContentEq.same"] != 1:
    sys.exit(f"js-native: ContentEq.same found {found['ContentEq.same']} times, not once")
if server and hashlib.sha256(Path(__file__).parent.parent.joinpath("src/server/thin-content-eq.bend").read_bytes()).hexdigest() != CONTENT_EQ_SOURCE_SHA256:
    sys.exit("js-native: ContentEq.same pure source changed")
if server and found["Json.show"] != 1:
    sys.exit(f"js-native: Json.show found {found['Json.show']} times, not once")
if server and found["Json.parse"] != 1:
    sys.exit(f"js-native: Json.parse found {found['Json.parse']} times, not once")
if server and found["Cbor.Enc.text"] != 1:
    sys.exit(f"js-native: Cbor.Enc.text found {found['Cbor.Enc.text']} times, not once")
if server:
    for s in srcs.values():
        for match in CONTENT_EQ.finditer(s):
            go = match.group(1).replace("$058ContentEq$same$", "$058ContentEq$go$")
            if match.group(4) != go or len(re.findall(r"function " + re.escape(go) + r"\(", s)) != 1:
                sys.exit("js-native: ContentEq.same needs its exact ContentEq.go once")
        for match in SHOW.finditer(s):
            emit = match.group(1).replace("$058Json$show$", "$058Json$emit$")
            if match.group(3) != emit or len(re.findall(r"function " + re.escape(emit) + r"\(", s)) != 1:
                sys.exit("js-native: Json.show needs its exact Json.emit once")
        for match in CBOR_TEXT.finditer(s):
            head = match.group(1).replace("$058Enc$text$", "$058Enc$head$")
            if len(re.findall(r"function " + re.escape(head) + r"\(", s)) != 1:
                sys.exit("js-native: Cbor.Enc.text needs its exact Enc.head once")
for f, s in srcs.items():
    hit = CMP.search(s) or BIT.search(s) or SPLIT.search(s) or LEN.search(s) or PRE.search(s) or HAS.search(s) or SEC_FINE.search(s) or not server and any(refs_fenced(s, m) for m in REFS_LAST.finditer(s))
    if not hit:
        continue
    # one pass down the list; nat_chk once (the count only grows)
    s = LEN.sub(lambda m: 'function %s(%s){let n=0;for(let x=%s;x.$!=="Nil";x=x.tail)n++;return %s(n)}' % (m.group(1), m.group(2), m.group(2), m.group(4)), s)
    s = CMP.sub(lambda m: m.group(0) + 'return{$:"Tuple",fst:{$:"Tuple",fst:%s,snd:%s},snd:{$:__bpOrd(%s,%s)}};' % (m.group(2), m.group(3), m.group(2), m.group(3)), s)
    s = BIT.sub(lambda m: m.group(0) + 'return{$:"Tuple",fst:%s,snd:__bpBit(%s,%s,%s)};' % (m.group(2), m.group(2), m.group(3), m.group(4)), s)
    if server:
        s = map_def("$Map$bit$").sub('function $Map$bit$(_key_0,_pos_0){return __bpMapBit(_key_0,_pos_0)}', s)
        s = map_def("$Map$diff$").sub('function $Map$diff$(_a_0,_b_0){return __bpMapDiff(_a_0,_b_0,nat_chk)}', s)
        s = map_def("$Map$set$").sub(lambda m: 'function $Map$set$(_m_0,_key_0,_x_0){return __bpMapSet(_m_0,_key_0,_x_0,$bpMapSetRaw)}' + m.group().replace('function $Map$set$(', 'function $bpMapSetRaw('), s)
        s = map_def("$Map$get$").sub('function $Map$get$(_d_0,_m_0,_key_0){return __bpMapRead(_m_0,_key_0,_d_0,false)}', s)
        s = map_def("$Map$has$").sub('function $Map$has$(_m_0,_key_0){return __bpMapRead(_m_0,_key_0,false,true)}', s)
        s = Path(__file__).with_name("js-map.js").read_text(encoding="utf-8") + "\n" + s
    s = SPLIT.sub(lambda m: m.group(0) + 'return __bpSplit(%s,%s);' % (m.group(2), m.group(3)), s)
    s = PRE.sub(lambda m: m.group(0) + 'return __bpPre(%s,%s);' % (m.group(2), m.group(3)), s)
    s = HAS.sub(lambda m: m.group(0) + 'return __bpHas(%s,%s);' % (m.group(2), m.group(3)), s)
    if server and CBOR_TEXT.search(s):
        s = CBOR_TEXT.sub(lambda m: m.group(0) + 'return __bpCborText(%s,%s,%s);' %
            (m.group(2), m.group(3), m.group(1).replace("$058Enc$text$", "$058Enc$head$")), s)
        s = Path(__file__).with_name("js-cbor-text.js").read_text(encoding="utf-8") + "\n" + s
    if server and PARSE.search(s):
        s = PARSE.sub(lambda m: 'function %s(%s){return __bpParsed(%s,%s$raw)}function %s$raw(%s){' % (m.group(1), m.group(2), m.group(2), m.group(1), m.group(1), m.group(2)), s)
        s = PARSE_HELPER + s
    if server and JSON_LEX.search(s):
        s = JSON_LEX.sub(lambda m: 'function %s(%s){return __bpJsonLex(%s,%s$raw,%s)}function %s$raw(%s){return %s(%s,{$:"Tuple",fst:{$:%s},snd:{$:"Nil"}})}' %
            (m.group(1), m.group(2), m.group(2), m.group(1), json.dumps(m.group(4)), m.group(1), m.group(2), m.group(3), m.group(2), json.dumps(m.group(4) + ".LIdle")), s)
        s = Path(__file__).with_name("js-json-lex.js").read_text(encoding="utf-8") + "\n" + s
    if server:
        s = map_def("$String$to_lower$").sub(lambda m: 'function $String$to_lower$(_s_0){return __bpLower(_s_0,$bpLowerRaw)}' + m.group().replace('$String$to_lower$', '$bpLowerRaw'), s)
        s = map_def("$String$take$").sub('function $String$take$(_s_0,_n_0){return __bpTake(_s_0,_n_0)}', s)
        s = map_def("$String$drop$").sub('function $String$drop$(_s_0,_n_0){return __bpDrop(_s_0,_n_0)}', s)
        s = FOLD_TEXT.sub(lambda m: 'function %s(%s){return __bpFoldText(%s,%s$raw)}function %s$raw(%s){return %s(%s,"")}' %
            (m.group(1), m.group(2), m.group(2), m.group(1), m.group(1), m.group(2), m.group(3), m.group(2)), s)
        s = UTF8_ENCODE.sub(lambda m: 'function %s(%s){return __bpUtf8Encode(%s)}' % (m.group(1), m.group(2), m.group(2)), s)
        s = Path(__file__).with_name("js-text.js").read_text(encoding="utf-8") + "\n" + s
        s = REVERSE_GO.sub('function $String$reverse$go$($0,$1){return Array.from($0).reverse().join("")+$1}', s)
    if server:
        s = UTF8_DECODE.sub(lambda m: 'function %s(%s){return __bpUtf8Decode(%s)}' % (m.group(1), m.group(2), m.group(2)), s)
        # The decode constructor namespace is fenced above and shared by Lines.
        ns = UTF8_DECODE.search(srcs[f]).group(5)
        s = LINES_FEED.sub(lambda m: 'function %s(%s,%s){return __bpLinesFeed(%s,%s,%s)}' %
            (m.group(1), m.group(2), m.group(3), m.group(2), m.group(3), json.dumps(ns + ".Lines")), s)
        s = Path(__file__).with_name("js-utf8.js").read_text(encoding="utf-8") + "\n" + s
    if server and SHOW.search(s):
        s = SHOW.sub(lambda m: 'function %s(%s){return __bpJsonShow(%s,%s$raw)}function %s$raw(%s){return %s(%s,!0,"")}' %
            (m.group(1), m.group(2), m.group(2), m.group(1), m.group(1), m.group(2), m.group(3), m.group(2)), s)
        s = Path(__file__).with_name("js-json-show.js").read_text(encoding="utf-8") + "\n" + s
    if server and CONTENT_EQ.search(s):
        s = CONTENT_EQ.sub(lambda m: 'function %s(%s,%s){return __bpContentEq(%s,%s,%s)}' %
            (m.group(1), m.group(2), m.group(3), m.group(2), m.group(3), m.group(4)), s)
        s = Path(__file__).with_name("js-content-eq.js").read_text(encoding="utf-8") + "\n" + s
    if server:
        s = BYTE_ESC.sub(lambda m: 'function %s($0,$1){return __bpJsonBytesEsc($0,$1,nat_chk)}' % m.group(1), s)
        s = BYTE_STR.sub(lambda m: 'function %s($0,$1){return __bpJsonBytesStr($0,$1,nat_chk)}' % m.group(1), s)
        s = BYTES.sub(lambda m: 'function %s(_j_0){return __bpJsonBytes(_j_0,%s,nat_chk)}' % (m.group(1), m.group(2)), s)
        memos = 0
        for name, entry, a in MEMO1:
            rx = memo1_rx(entry, a)
            memos += len(rx.findall(s))
            s = rx.sub(lambda m: 'function %s(%s){return __bpMemo1(%s,%s,%s$raw)}function %s$raw(%s){' %
                       (m.group(1), a, json.dumps(name), a, m.group(1), m.group(1), a), s)
        for name, entry, a, b in MEMO2:
            rx = memo2_rx(entry, a, b)
            memos += len(rx.findall(s))
            s = rx.sub(lambda m: 'function %s(%s,%s){return __bpMemo2(%s,%s,%s,%s$raw)}function %s$raw(%s,%s){' %
                       (m.group(1), a, b, json.dumps(name), a, b, m.group(1), m.group(1), a, b), s)
        for name, entry, a, b, c, d in MEMO4:
            rx = memo4_rx(entry, a, b, c, d)
            memos += len(rx.findall(s))
            s = rx.sub(lambda m: 'function %s(%s,%s,%s,%s){return __bpMemo4(%s,%s,%s,%s,%s,%s$raw)}function %s$raw(%s,%s,%s,%s){' %
                       (m.group(1), a, b, c, d, json.dumps(name), a, b, c, d, m.group(1), m.group(1), a, b, c, d), s)
        if memos:
            s = Path(__file__).with_name("js-memo.js").read_text(encoding="utf-8") + "\n" + s
        kids = [(m, tree_kids(s, m)) for m in TREE_KIDS.finditer(s)]
        for m, (parent, tid) in kids:
            s = s.replace(m.group(0), 'function %s(_s_0,_id_0){return __bpTreeChildren(_s_0,_id_0,%s,%s,%s,%s)}' % (m.group(1), m.group(4), parent, tid, m.group(2)))
        if kids:
            s = Path(__file__).with_name("js-tree-children.js").read_text(encoding="utf-8") + "\n" + s
        s = Path(__file__).with_name("js-json-bytes.js").read_text(encoding="utf-8") + "\n" + s
    if SEC_FINE.search(s):
        s = SEC_FINE.sub(lambda m: 'function %s(%s,%s){return __bpSecFine(%s,%s)}' % (m.group(1), m.group(2), m.group(3), m.group(2), m.group(3)), s)
        s = SEC_FINE_HELPER + s
    if not server and any(refs_fenced(s, m) for m in REFS_LAST.finditer(s)):
        s = REFS_LAST.sub(lambda m: 'function %s(%s,%s){return __bpRefsLast(%s,%s)}' % (m.group(1), m.group(2), m.group(3), m.group(2), m.group(3)) if refs_fenced(s, m) else m.group(0), s)
        s = REFS_HELPER + s
    # (a module's imports may come after it: they are hoisted)
    s = HELPERS + s
    open(f, "w", encoding="utf-8").write(s)
# each swap must have landed as often as its shape was found (a shape that
# matched but was not rewritten, or rewritten twice, stops the build too)
outs = "".join(open(f, encoding="utf-8").read() for f in files)
for name, call in (("Map.bit", "{return __bpMapBit("), ("Map.diff", "{return __bpMapDiff("),
                   ("Map.get", "{return __bpMapRead(_m_0,_key_0,_d_0,false)"), ("Map.has", "{return __bpMapRead(_m_0,_key_0,false,true)"),
                   ("String.cmp", "snd:{$:__bpOrd("), ("Map.bit.go", "snd:__bpBit("),
                   ("String.split", "return __bpSplit("), ("List.length", 'x.$!=="Nil";x=x.tail)n++;return '),
                   ("String.starts_with", "return __bpPre("), ("String.contains", "return __bpHas("),
                   ("Cbor.Enc.text", "return __bpCborText("), ("Json.parse", "){return __bpParsed("),
                   ("Json.lex", "){return __bpJsonLex("), ("String.reverse.go", '{return Array.from($0).reverse().join("")+$1}'),
                   ("String.to_lower", "){return __bpLower("), ("String.take", "){return __bpTake("),
                   ("String.drop", "){return __bpDrop("),
                   ("Utf8.encode", "){return __bpUtf8Encode("), ("Fold.text", "){return __bpFoldText("),
                   ("Utf8.decode", "){return __bpUtf8Decode("), ("Lines.feed", "){return __bpLinesFeed("),
                   ("Json.show", "){return __bpJsonShow("), ("ContentEq.same", "){return __bpContentEq("),
                   ("Json.bytes.esc", "){return __bpJsonBytesEsc("), ("Json.bytes.str", "){return __bpJsonBytesStr("),
                   ("Json.bytes", "){return __bpJsonBytes(_j_0,"),
                   ("Tree.children", "){return __bpTreeChildren(_s_0,_id_0,"),
                   ("Subs.aggs", '){return __bpMemo2("Subs.aggs",'),
                   ("SubHistory.own", '){return __bpMemo1("SubHistory.own",'),
                   ("Sec.fine", "){return __bpSecFine("), ("Refs.last.go", "){return __bpRefsLast(")):
    n = outs.count(call)
    if n != found[name]:
        sys.exit(f"js-native: {name}'s native body is in the output {n} times, not {found[name]}")
print("js-native: " + ", ".join(k for k, v in found.items() if v) + " are native")
