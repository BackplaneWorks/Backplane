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

Each is found by its shape and must be found exactly once, else the build
stops (a new Bend that emits them otherwise needs this looked at again).
With --server (scripts/build-js.sh: one bundle) that holds for every def;
for the web client's chunks (no flag) String.cmp and Map.bit.go must be
found exactly once and the others at most once (a chunk set need not use
them).
`bun test/tools/js_native_test.ts` checks both against Base's definitions.

  scripts/js-native.py dist/web/chunk-*.js
"""
import re
import sys

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
    "function __bpHi(c){return c>=55296&&c<56320}function __bpLo(c){return c>=56320&&c<57344}"
    "function __bpStarts(s,p){return s.startsWith(p)&&!(p.length>0&&__bpHi(p.charCodeAt(p.length-1))&&__bpLo(s.charCodeAt(p.length)))}"
    "function __bpContains(s,p){if(p===\"\")return true;for(let f=0;;){const i=s.indexOf(p,f);if(i<0)return false;"
    "if(!((i>0&&__bpLo(s.charCodeAt(i))&&__bpHi(s.charCodeAt(i-1)))||(__bpHi(p.charCodeAt(p.length-1))&&__bpLo(s.charCodeAt(i+p.length)))))return true;f=i+1}}\n"
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

# String.starts_with(s, p), a code point at a time (Base), and
# String.contains(s, p), which at every code point of s joins the rest back
# into a string and asks starts_with of it (quadratic: a bot turn's memory
# recall was 1.4 s of the JS hub's first minute tick): natively the same
# answers, JavaScript comparing UTF-16 units, so a match that ends or starts
# between the two halves of a surrogate pair is not one (as in Base, and as
# scripts/bridge-native.py does for the phones). Emitted as
#   function F($0,$1,$2){let $pc=0;for(;;)switch($pc){case 0:{let s=$0,p=$1;if(s==="")if(p==="")return!0;...
STARTS = re.compile(
    r'function (' + I + r')\((' + I + r'),(' + I + r'),(' + I + r')\)\{(?=let (' + I + r')=0;for\(;;\)switch\(\5\)\{case 0:\{let (' + I + r')=\2,(' + I + r')=\3;'
    r'if\(\6===""\)if\(\7===""\)return!0;else return!1;else\{)'
)
def contains_rx(starts):
    return re.compile(
        r'function (' + I + r')\((' + I + r'),(' + I + r'),(' + I + r')\)\{(?=let (' + I + r')=0;for\(;;\)switch\(\5\)\{case 0:\{let (' + I + r')=\2,(' + I + r')=\3;'
        r'if\(\6===""\)return ' + I + r'\(\7\);else\{[^{}]*' + re.escape(starts) + r'\()'
    )

args = sys.argv[1:]
server = "--server" in args
files = [a for a in args if a != "--server"]
srcs = {f: open(f, encoding="utf-8").read() for f in files}
found = {}
for name, rx, required in (("String.cmp", CMP, True), ("Map.bit.go", BIT, True),
                           ("String.split", SPLIT, server), ("List.length", LEN, server)):
    n = sum(len(rx.findall(s)) for s in srcs.values())
    if n != 1 and (required or n > 1):
        sys.exit(f"js-native: {name} found {n} times, not once")
    found[name] = n
# starts_with first (contains is found by its call to it)
n = sum(len(STARTS.findall(s)) for s in srcs.values())
if n != 1 and (server or n > 1):
    sys.exit(f"js-native: String.starts_with found {n} times, not once")
found["String.starts_with"] = n
starts = next((m.group(1) for s in srcs.values() for m in [STARTS.search(s)] if m), None)
CONTAINS = contains_rx(starts) if starts else None
n = sum(len(CONTAINS.findall(s)) for s in srcs.values()) if CONTAINS else 0
if n != 1 and (server or n > 1):
    sys.exit(f"js-native: String.contains found {n} times, not once")
found["String.contains"] = n
for f, s in srcs.items():
    hit = CMP.search(s) or BIT.search(s) or SPLIT.search(s) or LEN.search(s) or STARTS.search(s) or (CONTAINS and CONTAINS.search(s))
    if not hit:
        continue
    # one pass down the list; nat_chk once (the count only grows)
    s = LEN.sub(lambda m: 'function %s(%s){let n=0;for(let x=%s;x.$!=="Nil";x=x.tail)n++;return %s(n)}' % (m.group(1), m.group(2), m.group(2), m.group(4)), s)
    s = CMP.sub(lambda m: m.group(0) + 'return{$:"Tuple",fst:{$:"Tuple",fst:%s,snd:%s},snd:{$:__bpOrd(%s,%s)}};' % (m.group(2), m.group(3), m.group(2), m.group(3)), s)
    s = BIT.sub(lambda m: m.group(0) + 'return{$:"Tuple",fst:%s,snd:__bpBit(%s,%s,%s)};' % (m.group(2), m.group(2), m.group(3), m.group(4)), s)
    s = SPLIT.sub(lambda m: m.group(0) + 'return __bpSplit(%s,%s);' % (m.group(2), m.group(3)), s)
    s = STARTS.sub(lambda m: m.group(0) + 'return __bpStarts(%s,%s);' % (m.group(2), m.group(3)), s)
    if CONTAINS:
        s = CONTAINS.sub(lambda m: m.group(0) + 'return __bpContains(%s,%s);' % (m.group(2), m.group(3)), s)
    # (a module's imports may come after it: they are hoisted)
    s = HELPERS + s
    open(f, "w", encoding="utf-8").write(s)
# each swap must have landed as often as its shape was found (a shape that
# matched but was not rewritten, or rewritten twice, stops the build too)
outs = "".join(open(f, encoding="utf-8").read() for f in files)
for name, call in (("String.cmp", "snd:{$:__bpOrd("), ("Map.bit.go", "snd:__bpBit("),
                   ("String.split", "return __bpSplit("), ("List.length", 'x.$!=="Nil";x=x.tail)n++;return '),
                   ("String.starts_with", "return __bpStarts("), ("String.contains", "return __bpContains(")):
    n = outs.count(call)
    if n != found[name]:
        sys.exit(f"js-native: {name}'s native body is in the output {n} times, not {found[name]}")
print("js-native: " + ", ".join(k for k, v in found.items() if v) + " are native")
