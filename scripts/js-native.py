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

Server-only `Json.parse(s)`: remembered for repeated texts of 1 KB or
more (the last 8). The hub's views parse the same models info (every
provider's models, opencode's hundreds) several times a render, every
render: measured 2026-10-24 on a long thread, most of a render's time. A
parse is pure and its value never changes, so the same text gives the same
value; smaller texts (short agent lines and requests) are parsed as before.
The 1 KB cutoff includes the measured 10.5 KB catalog and 3.2 KB shown-row
catalog that were each parsed repeatedly below the old 16 KB cutoff.

Server-only `Json.show(j)`: an iterative compact serializer preserving the
pure encoder's exact escapes, raw number text, field order and duplicate
keys. The original body remains the fallback for non-ASCII raw numbers.
No memo or protocol/client changes; scripts/js-json-show.js holds the helper.

Server-only Json.bytes.esc/str count code-point UTF-8 widths without slicing
a fresh string tail per character. Exact JSON escapes, raw lexemes and Nat
overflow remain the pure counter's; no whole-tree or cap predicate changes.

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
    "const __bpParseMemo=new Map();function __bpParsed(s,f){if(typeof s!==\"string\"||s.length<1024)return f(s);"
    "let v=__bpParseMemo.get(s);if(v===undefined){v=f(s);if(__bpParseMemo.size>=8)__bpParseMemo.delete(__bpParseMemo.keys().next().value);"
    "__bpParseMemo.set(s,v)}return v}\n"
)

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
JSON_BYTES_SOURCE_SHA256 = "60ea726052fded72970f3be2784d09fbff0baa4a98a47ac82d00a4a311dc242d"

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

args = sys.argv[1:]
server = "--server" in args
files = [a for a in args if a != "--server"]
srcs = {f: open(f, encoding="utf-8").read() for f in files}
found = {}
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
found["Json.show"] = sum(len(SHOW.findall(s)) for s in srcs.values()) if server else 0
found["ContentEq.same"] = sum(len(CONTENT_EQ.findall(s)) for s in srcs.values()) if server else 0
for name, rx in (("Json.bytes.esc", BYTE_ESC), ("Json.bytes.str", BYTE_STR)):
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
    hit = CMP.search(s) or BIT.search(s) or SPLIT.search(s) or LEN.search(s) or PRE.search(s) or HAS.search(s) or SEC_FINE.search(s)
    if not hit:
        continue
    # one pass down the list; nat_chk once (the count only grows)
    s = LEN.sub(lambda m: 'function %s(%s){let n=0;for(let x=%s;x.$!=="Nil";x=x.tail)n++;return %s(n)}' % (m.group(1), m.group(2), m.group(2), m.group(4)), s)
    s = CMP.sub(lambda m: m.group(0) + 'return{$:"Tuple",fst:{$:"Tuple",fst:%s,snd:%s},snd:{$:__bpOrd(%s,%s)}};' % (m.group(2), m.group(3), m.group(2), m.group(3)), s)
    s = BIT.sub(lambda m: m.group(0) + 'return{$:"Tuple",fst:%s,snd:__bpBit(%s,%s,%s)};' % (m.group(2), m.group(2), m.group(3), m.group(4)), s)
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
        s = Path(__file__).with_name("js-json-bytes.js").read_text(encoding="utf-8") + "\n" + s
    if SEC_FINE.search(s):
        s = SEC_FINE.sub(lambda m: 'function %s(%s,%s){return __bpSecFine(%s,%s)}' % (m.group(1), m.group(2), m.group(3), m.group(2), m.group(3)), s)
        s = SEC_FINE_HELPER + s
    # (a module's imports may come after it: they are hoisted)
    s = HELPERS + s
    open(f, "w", encoding="utf-8").write(s)
# each swap must have landed as often as its shape was found (a shape that
# matched but was not rewritten, or rewritten twice, stops the build too)
outs = "".join(open(f, encoding="utf-8").read() for f in files)
for name, call in (("String.cmp", "snd:{$:__bpOrd("), ("Map.bit.go", "snd:__bpBit("),
                   ("String.split", "return __bpSplit("), ("List.length", 'x.$!=="Nil";x=x.tail)n++;return '),
                   ("String.starts_with", "return __bpPre("), ("String.contains", "return __bpHas("),
                   ("Cbor.Enc.text", "return __bpCborText("), ("Json.parse", "){return __bpParsed("),
                   ("Json.show", "){return __bpJsonShow("), ("ContentEq.same", "){return __bpContentEq("),
                   ("Json.bytes.esc", "){return __bpJsonBytesEsc("), ("Json.bytes.str", "){return __bpJsonBytesStr("),
                   ("Sec.fine", "){return __bpSecFine(")):
    n = outs.count(call)
    if n != found[name]:
        sys.exit(f"js-native: {name}'s native body is in the output {n} times, not {found[name]}")
print("js-native: " + ", ".join(k for k, v in found.items() if v) + " are native")
