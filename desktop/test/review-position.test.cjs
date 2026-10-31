const { test } = require('node:test'), assert = require('node:assert/strict');
const memory = () => { const m = new Map();return { getItem:k=>m.get(k) ?? null, setItem:(k,v)=>m.set(k,v) }; };
const pane = (thread='parent', place='beta-bom', ready='1') => ({ dataset:{reviewThread:thread,reviewPlace:place,reviewReady:ready}, isConnected:true,scrollTop:0,scrollLeft:0,scrollHeight:3000,clientHeight:600,scrollWidth:1000,clientWidth:600 });
test('drawing detours and reloads keep each report position', async () => {
  const { ReviewPosition } = await import('../../src/web/review-position.js');
  const storage=memory(), r=new ReviewPosition(storage), first=pane();
  r.mount(first);first.scrollTop=900;first.scrollLeft=120;r.capture();r.mount(null);
  const loading=pane('parent','beta-bom','0');r.mount(loading);r.capture();
  const returned=pane();r.mount(returned);assert.equal(returned.scrollTop,900);assert.equal(returned.scrollLeft,120);
  const alpha=pane('parent','alpha-bom');r.capture();r.mount(alpha);assert.equal(alpha.scrollTop,0);
  const reload=new ReviewPosition(storage), restored=pane();reload.mount(restored);assert.equal(restored.scrollTop,900);
});
test('a fork snapshots reading positions and then keeps them independently', async () => {
  const { ReviewPosition } = await import('../../src/web/review-position.js');
  const r=new ReviewPosition(memory()), parent=pane();r.mount(parent);parent.scrollTop=900;r.fork('parent');
  parent.scrollTop=1200;r.capture();r.accepted({sel:'child',drafts:{'@viewer.fork:child':'parent'}});
  const child=pane('child');r.mount(child);assert.equal(child.scrollTop,900);child.scrollTop=600;r.capture();
  r.mount(parent);assert.equal(parent.scrollTop,1200);
  r.accepted({sel:'child',drafts:{'@viewer.fork:child':'parent'}});r.mount(child);assert.equal(child.scrollTop,600);
});
test('a shorter refreshed report clamps coordinates to its content', async () => {
  const { ReviewPosition } = await import('../../src/web/review-position.js');
  const r=new ReviewPosition(memory()), first=pane();r.mount(first);first.scrollTop=900;r.capture();r.mount(null);
  const shorter=pane();shorter.scrollHeight=700;r.mount(shorter);assert.equal(shorter.scrollTop,100);
});
