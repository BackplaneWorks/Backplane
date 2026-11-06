#!/bin/sh
# The helper-free thread runtime check uses a small generated log, never real store data.
set -eu
mkdir -p build/gpui/root
bun -e '
const rows=[{$:"ProjectCreated",id:"p-root",title:"Thread smoke",root:"/tmp/root-thread-smoke",at:1700000000},{$:"ThreadCreated",id:"t-root",project:"p-root",title:"Plain thread",provider:"claude",model:"",env:"local",branch:"",worktree:"",at:1700000001}];
for(let i=0;i<120;i++)rows.push({$:"MessagePosted",thread:"t-root",msg:`m-root-${i}`,role:i%2?"assistant":"user",text:`Plain message ${i}`,at:1700000010+i});
Bun.write("build/gpui/root/synthetic-thread-smoke.jsonl",rows.map(x=>JSON.stringify(x)).join("\n")+"\n");
'
env -u BACKPLANE_POLICY_BIN -u BACKPLANE_WINDOW_BIN ROOT_THREAD_ID=t-root ROOT_RESULT_NAME=thread-wire bun test/root/recent.mjs "${1:?hub binary}" build/gpui/root/synthetic-thread-smoke.jsonl "${2:-3896}"
