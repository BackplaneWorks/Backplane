const { test } = require('node:test'), assert = require('node:assert/strict');
const fs = require('node:fs'), os = require('node:os'), path = require('node:path');
const { localImagePath, createImageActions, installImageMenu } = require('../image-actions.cjs');
const origin = 'http://127.0.0.1:3788';
test('desktop file reveal stays on the selected local service image endpoint', () => {
  assert.equal(localImagePath('/img?path=%2Ftmp%2Fa%20b.png', origin), '/tmp/a b.png');
  assert.equal(localImagePath('/img?path=~/a.png', origin, '/home/person'), '/home/person/a.png');
  for (const source of ['/tmp/a.png', 'file:///tmp/a.png', '/img?path=relative.png', '/img?path=/tmp/a.pdf',
    '/img?path=/tmp/a.png&token=secret', '/img?path=/tmp/a.png&path=/tmp/b.png',
    '/img?path=/tmp/a.png#token=secret', '/img?path=%00a.png',
    'https://other-hub/img?path=/tmp/a.png', 'http://user:secret@127.0.0.1:3788/img?path=/tmp/a.png']) assert.equal(localImagePath(source, origin), null, source);
  assert.equal(localImagePath('/img?path=/tmp/a.png', 'https://remote-hub'), null);
});
test('reveal requires a file and the exact encoded FileManager1 acknowledgement', async t => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bp-image-unit-'));t.after(() => fs.rmSync(dir, { recursive:true, force:true }));
  const file = path.join(dir, 'image 日本語 #?% ;.png');fs.writeFileSync(file, 'synthetic');
  const calls = [], actions = createImageActions({ electron: {}, origin: () => origin, platform: 'linux', runFile: async (...args) => { calls.push(args); } });
  const source = '/img?path=' + encodeURIComponent(file);
  assert.deepEqual(await actions.reveal(source), { accepted:true });
  assert.equal(calls[0][0], 'dbus-send');
  assert(calls[0][1].includes('array:string:' + require('node:url').pathToFileURL(file).href));
  assert.equal(calls[0][2].timeout, 4000);
  await assert.rejects(actions.reveal('/img?path=/missing.png'), /no longer exists/);
  await assert.rejects(actions.reveal('https://other-hub/img?path='+encodeURIComponent(file)), /not a file on this desktop/);
  assert.equal(calls.length, 1);
  const refused = createImageActions({ electron:{}, origin:()=>origin, platform:'linux', runFile: async () => { throw Error('service unknown'); } });
  await assert.rejects(refused.reveal(source), /did not accept/);
});
test('bitmap bridge rejects URLs and oversized decode dimensions before native decoding', async () => {
  let decoded = 0;
  const actions = createImageActions({ electron:{nativeImage:{createFromBuffer(){decoded++;}}}, origin:()=>origin });
  for (const bad of ['https://hub/img?path=/tmp/a.png&token=secret','data:text/html;base64,abcd','data:image/png;base64,abcd']) await assert.rejects(actions.copy(bad));
  const header=Buffer.alloc(24);Buffer.from('89504e470d0a1a0a','hex').copy(header);header.write('IHDR',12);header.writeUInt32BE(100000,16);header.writeUInt32BE(100000,20);
  await assert.rejects(actions.copy('data:image/png;base64,'+header.toString('base64')), /too large/);assert.equal(decoded,0);
});
test('Electron 44 bitmap write awaits MIME payload and verifies clipboard acknowledgement', async () => {
  const bytes=fs.readFileSync(path.join(__dirname,'../../assets/icon/backplane-64.png')),events=[];
  const image={isEmpty:()=>false,toPNG:()=>bytes,toBitmap:()=>Buffer.from('pixels')};
  class ClipboardItem { constructor(data){this.data=data;} }
  const electron={ClipboardItem,nativeImage:{createFromBuffer:()=>image},clipboard:{
    async write(items){assert(items[0] instanceof ClipboardItem);assert.deepEqual(Object.keys(items[0].data),['image/png']);assert(items[0].data['image/png'] instanceof Blob);events.push('write');},
    async read(){events.push('read');return[{types:['image/png'],getType:async()=>new Blob([bytes],{type:'image/png'})}];},
  }};
  const actions=createImageActions({electron,origin:()=>origin});
  assert.deepEqual(await actions.copy('data:image/png;base64,'+bytes.toString('base64')),{copied:true});assert.deepEqual(events,['write','read']);
  electron.clipboard.read=async()=>[];await assert.rejects(actions.copy('data:image/png;base64,'+bytes.toString('base64')),/did not accept/);
  electron.clipboard.write=async()=>{throw Error('permission denied');};await assert.rejects(actions.copy('data:image/png;base64,'+bytes.toString('base64')),/permission denied/);
});
test('native menu leaves non-image and untrusted-frame menus alone', () => {
  let listener, menus=0;
  const window={webContents:{on:(_name, fn)=>{listener=fn;}}};
  installImageMenu(window,{electron:{Menu:{buildFromTemplate(){menus++;return{popup(){}};}}},trusted:p=>p.frameURL===origin,actions:{local:()=>false}});
  listener({}, {frameURL:origin,mediaType:'none'});listener({}, {frameURL:'https://guest',mediaType:'image'});assert.equal(menus,0);
  listener({}, {frameURL:origin,mediaType:'image',hasImageContents:false});assert.equal(menus,1);
});
test('trusted editable fields have the native editing menu with Chromium edit availability',()=>{
  let listener,items,popups=0;
  const window={webContents:{on:(_name,fn)=>listener=fn}};
  installImageMenu(window,{electron:{Menu:{buildFromTemplate:value=>{items=value;return{popup:options=>{assert.equal(options.window,window);popups++;}};}}},trusted:p=>p.frameURL===origin,actions:{local:()=>false}});
  const params={frameURL:origin,mediaType:'none',isEditable:true,editFlags:{canUndo:true,canRedo:false,canCut:true,canCopy:true,canPaste:false,canSelectAll:true}};
  listener({},params);
  assert.deepEqual(items.filter(x=>x.role).map(x=>[x.role,x.enabled]),[['undo',true],['redo',false],['cut',true],['copy',true],['paste',false],['selectAll',true]]);
  listener({},{...params,frameURL:'https://other'});assert.equal(popups,1);
});
