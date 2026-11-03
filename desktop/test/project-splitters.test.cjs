const {test} = require('node:test'), assert = require('node:assert/strict');

function target(extra = {}) {
  const listeners = new Map(), attrs = new Map(), classes = new Set();
  return Object.assign({
    getAttribute: name => attrs.get(name), setAttribute: (name, value) => attrs.set(name, value), removeAttribute: name => attrs.delete(name),
    classList: {add: name => classes.add(name), remove: name => classes.delete(name), contains: name => classes.has(name)},
    addEventListener(type, fn) { if (!listeners.has(type)) listeners.set(type, new Set());listeners.get(type).add(fn); },
    removeEventListener(type, fn) { listeners.get(type)?.delete(fn); },
    emit(type, fields = {}) {
      const event = {target:this, button:0, pointerId:7, clientX:100, clientY:100, preventDefault() {this.prevented = true;}, ...fields};
      for (const fn of [...listeners.get(type) || []]) fn(event);
      return event;
    },
  }, extra);
}

async function fixture(storage = new Map()) {
  const {installProjectSplitters} = await import('../../src/web/project-splitters.js');
  const owner = {dataset:{splitProject:'A'}}, observers = [];
  let hub = 'hub-A';
  const doc = target({body:target(), edges:[], querySelectorAll() { return this.edges; }});
  const win = target({location:{origin:'fixture'}, getComputedStyle: group => ({flexDirection:group.direction}),
    ResizeObserver:class { constructor(fn) {this.fn=fn;this.groups=new Set();observers.push(this);} observe(g) {this.groups.add(g);} unobserve(g) {this.groups.delete(g);} disconnect() {this.groups.clear();} }});
  const values = new Map();
  const group = target({direction:'row', width:1000, height:600,
    style:{setProperty:(name,value)=>values.set(name,value),getPropertyValue:name=>values.get(name),removeProperty:name=>values.delete(name)},
    getBoundingClientRect() {return {width:this.width,height:this.height};}});
  const edge = target({dataset:{projectSplit:'primary',firstLabel:'Schematic',secondLabel:'Board and 3D'}, parentElement:group,isConnected:true,
    closest: selector => selector === '[data-split-project]' ? owner : edge,
    getBoundingClientRect:()=>({width:12,height:12}), focus() {}, setPointerCapture() {}, releasePointerCapture() {}});
  doc.edges=[edge];
  const api = installProjectSplitters({document:doc,window:win,scope:()=>hub,
    storage:{getItem:k=>storage.get(k)??null,setItem:(k,v)=>storage.set(k,v),removeItem:k=>storage.delete(k)}});
  api.sync();
  return {api,storage,owner,doc,win,edge,group,observers,ratio:()=>Number(values.get('--split-ratio')),
    key:(key,fields={})=>doc.emit('keydown',{target:edge,key,...fields}), hub:value=>hub=value,
    down:fields=>doc.emit('pointerdown',{target:edge,...fields})};
}

test('pane proportions survive reload and remain separate for each project and hub',async()=>{
  const f=await fixture(), initial=f.ratio();f.key('ArrowRight');const saved=f.ratio();assert(saved>initial);
  f.owner.dataset.splitProject='B';f.api.sync();assert.equal(f.ratio(),initial);f.key('ArrowLeft',{shiftKey:true});
  f.owner.dataset.splitProject='A';f.api.sync();assert.equal(f.ratio(),saved);
  f.hub('hub-B');f.api.sync();assert.equal(f.ratio(),initial);
  f.api.dispose();const reloaded=await fixture(f.storage);assert.equal(reloaded.ratio(),saved);reloaded.api.dispose();
});

test('separators expose their current orientation and keyboard-adjustable bounds',async()=>{
  const f=await fixture();
  assert.equal(f.edge.getAttribute('aria-orientation'),'vertical');
  assert.equal(f.key('ArrowLeft').prevented,true);assert.match(f.edge.getAttribute('aria-valuetext'),/Schematic.*Board and 3D/);
  f.key('Home');assert.equal(f.edge.getAttribute('aria-valuenow'),f.edge.getAttribute('aria-valuemin'));
  f.key('End');assert.equal(f.edge.getAttribute('aria-valuenow'),f.edge.getAttribute('aria-valuemax'));
  f.group.direction='column';f.observers[0].fn();assert.equal(f.edge.getAttribute('aria-orientation'),'horizontal');
  assert.equal(f.key('ArrowLeft').prevented,undefined);assert.equal(f.key('ArrowUp').prevented,true);
  f.key('Enter');assert.equal(f.storage.size,0);
  for(const fields of [{ctrlKey:true},{metaKey:true},{altKey:true},{isComposing:true},{keyCode:229}]) assert.equal(f.key('ArrowDown',fields).prevented,undefined);
  f.api.dispose();
});

test('a narrow pane clamps the display without overwriting the remembered wide proportion',async()=>{
  const f=await fixture();f.key('End');const wide=f.ratio(), saved=[...f.storage.values()][0];
  f.group.width=200;f.api.sync();assert(f.ratio()<=.55);assert.equal([...f.storage.values()][0],saved);
  const at=Number(f.edge.getAttribute('aria-valuenow'));
  assert(at>=Number(f.edge.getAttribute('aria-valuemin')) && at<=Number(f.edge.getAttribute('aria-valuemax')));
  f.group.width=1000;f.api.sync();assert.equal(f.ratio(),wide);f.api.dispose();
});

test('pointer capture saves a completed drag and cancellation restores its starting ratio',async()=>{
  const f=await fixture(), initial=f.ratio();f.down();
  f.edge.emit('pointermove',{pointerId:99,clientX:300});assert.equal(f.ratio(),initial);
  f.edge.emit('pointermove',{clientX:300});assert(f.ratio()>initial);f.edge.emit('pointerup');
  const saved=f.ratio();assert.equal(f.storage.size,1);assert.equal(f.doc.body.classList.contains('resizing-project-split'),false);
  f.down();f.edge.emit('pointermove',{clientX:50});f.edge.emit('pointercancel');assert.equal(f.ratio(),saved);
  f.doc.emit('dblclick',{target:f.edge});assert.equal(f.ratio(),initial);assert.equal(f.storage.size,0);f.api.dispose();
});

test('changing projects or hiding a pane during a drag cannot write to another owner or leave half-width layout',async()=>{
  const f=await fixture();f.down();f.edge.emit('pointermove',{clientX:300});
  f.owner.dataset.splitProject='B';f.api.sync();f.edge.emit('pointerup');assert.equal(f.storage.size,0);
  f.down();f.edge.emit('pointermove',{clientX:300});f.doc.edges=[];f.edge.isConnected=false;f.api.sync();
  assert.equal(f.storage.size,0);assert.equal(f.doc.body.classList.contains('resizing-project-split'),false);
  assert.equal(f.group.getAttribute('data-split-active'),undefined);assert.equal(f.observers[0].groups.size,0);f.api.dispose();
});

test('orientation changes cancel an active drag; secondary panes remember a separate proportion',async()=>{
  const f=await fixture();f.key('ArrowRight');const primary=f.ratio();
  f.edge.dataset.projectSplit='secondary';f.api.sync();assert.equal(f.ratio(),.5);f.key('ArrowLeft');const secondary=f.ratio();
  f.down();f.edge.emit('pointermove',{clientX:250});f.group.direction='column';f.observers[0].fn();
  assert.equal(f.doc.body.classList.contains('resizing-project-split'),false);assert.equal(f.ratio(),secondary);
  f.edge.dataset.projectSplit='primary';f.api.sync();assert.equal(f.ratio(),primary);f.api.dispose();
});

test('invalid or inaccessible browser storage does not prevent resizing',async()=>{
  for(const value of ['garbage','Infinity','0','1','-1']) {
    const stored=new Map([['backplane-project-split:v1:'+JSON.stringify(['hub-A','A','primary']),value]]);
    const f=await fixture(stored);assert(f.ratio()>0&&f.ratio()<1);f.key('ArrowLeft');f.api.dispose();
  }
  const f=await fixture({get(){throw Error('blocked');},set(){throw Error('blocked');},delete(){throw Error('blocked');}});
  const before=f.ratio();f.key('ArrowRight');assert(f.ratio()>before);f.key('Enter');assert.equal(f.ratio(),before);f.api.dispose();
});

test('home viewer proportions stay independent of both CAD Split groups',async()=>{
  const f=await fixture();f.key('ArrowRight');const primary=f.ratio();
  f.edge.dataset.projectSplit='home';f.api.sync();assert.equal(f.ratio(),.68);
  f.key('ArrowLeft');const home=f.ratio();
  f.edge.dataset.projectSplit='secondary';f.api.sync();assert.equal(f.ratio(),.5);
  f.edge.dataset.projectSplit='home';f.api.sync();assert.equal(f.ratio(),home);
  f.key('Enter');assert.equal(f.ratio(),.68);
  f.edge.dataset.projectSplit='primary';f.api.sync();assert.equal(f.ratio(),primary);
  f.api.dispose();
});
