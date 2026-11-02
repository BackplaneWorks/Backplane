'use strict';
const {test} = require('node:test'), assert = require('node:assert/strict');
const modulePromise = import('../../src/web/text-copy.js');

function legacy(accepted = true) {
  let field, copied, removed = false, focused = false, range;
  const active = {value:'Unsent draft 日本語',selectionStart:3,selectionEnd:8,selectionDirection:'backward',
    focus(options) { assert.deepEqual(options,{preventScroll:true});focused=true; },
    setSelectionRange(...args) { range=args; }};
  const doc = {activeElement:active,getSelection:()=>null,body:{append(value){field=value;}},
    createElement(tag) { assert.equal(tag,'textarea');return {style:{},setAttribute(){},focus(){},select(){},setSelectionRange(){},remove(){removed=true;}}; },
    execCommand(command) { assert.equal(command,'copy');copied=field.value;return accepted; }};
  return {win:{isSecureContext:false,navigator:{},document:doc},result:()=>({copied,removed,focused,range,draft:active.value})};
}

test('copy writes during the gesture and awaits browser acknowledgement', async () => {
  const {copyText} = await modulePromise;
  let invoked=false,done=false,release;
  const win={isSecureContext:true,navigator:{clipboard:{writeText(text){invoked=true;assert.equal(text,'# literal\n');return new Promise(resolve=>release=resolve);}}}};
  const pending=copyText('# literal\n',win).then(()=>done=true);
  assert(invoked,'writeText is invoked before any await');await Promise.resolve();assert(!done);
  release();await pending;assert(done);
});

test('missing and rejected clipboard APIs fall back without changing the draft or selection', async () => {
  const {copyText} = await modulePromise;
  for(const mode of ['missing','denied']) {
    const f=legacy();
    if(mode==='denied'){f.win.isSecureContext=true;f.win.navigator.clipboard={writeText:async()=>{throw Error('Permission denied');}};}
    const text='# shell comment\necho "<&日本語>"\n';await copyText(text,f.win);
    assert.deepEqual(f.result(),{copied:text,removed:true,focused:true,range:[3,8,'backward'],draft:'Unsent draft 日本語'});
  }
});

test('fallback refusal rejects and still restores the composer', async () => {
  const {copyText} = await modulePromise,f=legacy(false);
  await assert.rejects(copyText('text',f.win),/Clipboard unavailable/);
  assert(f.result().removed);assert(f.result().focused);
});

test('Electron bridge takes priority and must acknowledge a successful write', async () => {
  const {copyText}=await modulePromise;
  const win={backplaneDesktop:{clipboard:{writeText:async text=>({copied:text==='native'})}},navigator:{clipboard:{writeText(){assert.fail('must use native bridge');}}}};
  await copyText('native',win);await assert.rejects(copyText('refused',win),/did not accept/);
});

test('Copied feedback waits for success and cannot label a recycled message', async t => {
  t.mock.timers.enable({apis:['setTimeout']});
  const {copyButton}=await modulePromise;
  let release;
  const values=new Map([['data-value','code'],['title','Copy']]);
  const win={backplaneDesktop:{clipboard:{writeText:()=>new Promise(resolve=>release=resolve)}}};
  const button={ownerDocument:{defaultView:win},isConnected:true,textContent:'Copy',getAttribute:key=>values.get(key)??null,setAttribute:(key,value)=>values.set(key,value),removeAttribute:key=>values.delete(key)};
  const pending=copyButton(button,'code');assert.equal(button.textContent,'Copy');release({copied:true});await pending;
  assert.equal(button.textContent,'Copied');t.mock.timers.tick(2000);assert.equal(button.textContent,'Copy');
  const recycled=copyButton(button,'code');values.set('data-value','different code');release({copied:true});await recycled;
  assert.equal(button.textContent,'Copy');
});
